use super::*;

impl CanonChangeCoordinator {
    pub(super) fn apply_undo(
        &self,
        record: &mut ChangeRecord,
        cancellation: &CancellationToken,
        hook: &mut impl FnMut(CommitStage) -> Result<(), ChangeError>,
    ) -> Result<(), ChangeError> {
        // All-file preflight must precede even the durable inverse intent.
        for file in &record.receipt.files {
            if self.current_hash(&file.relative_path)?.as_deref() != Some(&file.after_hash) {
                return Err(ChangeError::SourceChanged);
            }
        }
        record.undo_revision = Some(record.receipt.revision);
        if let Some(chapter) = &mut record.chapter {
            chapter.final_invalidated = true;
        }
        record.receipt.revision += 1;
        record.receipt.status = ChangeStatus::Applying;
        self.save_record(record)?;
        let outcome = (|| {
            hook(CommitStage::Prepared)?;
            for (index, (snapshot, file)) in record
                .snapshots
                .iter()
                .zip(&record.receipt.files)
                .enumerate()
                .rev()
            {
                check_cancelled(cancellation)?;
                self.verify_lease()?;
                hook(CommitStage::BeforeFile(index))?;
                if let Some(before) = &snapshot.before {
                    apply_change_file_recoverable(
                        &self.handle,
                        snapshot.entity_kind,
                        CanonFileOperationKind::Replace,
                        &snapshot.relative_path,
                        &file.after_hash,
                        before.as_bytes(),
                        &self.recovery,
                    )
                    .map_err(|_| ChangeError::SourceChanged)?;
                } else {
                    let target = resolve_scoped_target(self.handle.root(), &snapshot.relative_path)
                        .map_err(|_| ChangeError::TargetInvalid)?;
                    if self.current_hash(&snapshot.relative_path)?.as_deref()
                        != Some(&file.after_hash)
                    {
                        return Err(ChangeError::SourceChanged);
                    }
                    // Only the exact, unmodified file created by this transaction.
                    fs::remove_file(target).map_err(|_| ChangeError::Io)?;
                }
                hook(CommitStage::AfterFile(index))?;
            }
            check_cancelled(cancellation)?;
            hook(CommitStage::BeforeCommit)?;
            for file in &record.receipt.files {
                if self.current_hash(&file.relative_path)? != file.before_hash {
                    return Err(ChangeError::SourceChanged);
                }
            }
            record.receipt.status = ChangeStatus::Undone;
            record.receipt.revision += 1;
            self.save_record(record)
        })();
        if let Err(error) = outcome {
            if let Ok(stored) = self.load_record(&record.receipt.change_id) {
                if stored.receipt.status == ChangeStatus::Undone {
                    *record = stored;
                    return Ok(());
                }
            }
            self.rollback_undo(record)?;
            return Err(error);
        }
        let _ = hook(CommitStage::Committed);
        Ok(())
    }

    /// An inverse transaction without its commit marker restores the previously
    /// committed after-images. Never rerun Provider, finish an interrupted Undo,
    /// overwrite an external edit, or reorient the immutable snapshots.
    pub(super) fn rollback_undo(&self, record: &mut ChangeRecord) -> Result<(), ChangeError> {
        let result = (|| {
            if self.recovery.recover_project(&self.handle).status == RecoveryStatus::Required {
                return Err(ChangeError::RecoveryRequired);
            }
            for (snapshot, file) in record.snapshots.iter().zip(&record.receipt.files) {
                self.verify_lease()?;
                let current = self.current_hash(&snapshot.relative_path)?;
                if current.as_deref() == Some(&file.after_hash) {
                    continue;
                }
                if current != file.before_hash {
                    return Err(ChangeError::RecoveryRequired);
                }
                apply_change_file_recoverable(
                    &self.handle,
                    snapshot.entity_kind,
                    if snapshot.before.is_some() {
                        CanonFileOperationKind::Replace
                    } else {
                        CanonFileOperationKind::Create
                    },
                    &snapshot.relative_path,
                    file.before_hash.as_deref().unwrap_or("absent"),
                    snapshot.after.as_bytes(),
                    &self.recovery,
                )
                .map_err(|_| ChangeError::RecoveryRequired)?;
            }
            for file in &record.receipt.files {
                if self.current_hash(&file.relative_path)?.as_deref() != Some(&file.after_hash) {
                    return Err(ChangeError::RecoveryRequired);
                }
            }
            Ok(())
        })();
        record.receipt.status = if result.is_ok() {
            ChangeStatus::Committed
        } else {
            ChangeStatus::RecoveryRequired
        };
        if result.is_ok() {
            record.undo_revision = None;
        }
        record.receipt.revision += 1;
        self.save_record(record)
            .map_err(|_| ChangeError::RecoveryRequired)?;
        result
    }
}
