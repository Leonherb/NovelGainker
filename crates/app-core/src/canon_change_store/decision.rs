use super::*;

impl CanonChangeCoordinator {
    pub fn activities(
        &self,
        conversation_session_id: &str,
    ) -> Result<Vec<ChangeActivity>, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        Ok(self
            .records()?
            .into_iter()
            .filter(|record| record.conversation_session_id == conversation_session_id)
            .map(|record| ChangeActivity {
                conversation_session_id: record.conversation_session_id,
                turn_id: record.turn_id,
                receipt: record.receipt,
            })
            .collect())
    }

    pub fn detail(
        &self,
        conversation_session_id: &str,
        change_id: &str,
    ) -> Result<ChangeDetail, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        let record = self.load_record(change_id)?;
        if record.conversation_session_id != conversation_session_id {
            return Err(ChangeError::SourceInvalid);
        }
        let files = record
            .snapshots
            .iter()
            .map(|snapshot| ChangeFileDiff {
                relative_path: snapshot.relative_path.clone(),
                lines: diff_lines(snapshot.before.as_deref().unwrap_or(""), &snapshot.after),
            })
            .collect();
        Ok(ChangeDetail {
            activity: ChangeActivity {
                conversation_session_id: record.conversation_session_id,
                turn_id: record.turn_id,
                receipt: record.receipt,
            },
            files,
        })
    }

    /// Local confirmation consumes a stored proposal, never frontend-supplied bytes.
    /// Revisions are CAS tokens; changed buffers/disk require a newly prepared proposal.
    pub fn decide(
        &self,
        conversation_session_id: &str,
        change_id: &str,
        revision: u64,
        decision: ChangeDecision,
        cancellation: &CancellationToken,
    ) -> Result<ChangeReceipt, ChangeError> {
        self.decide_with_hook(
            conversation_session_id,
            change_id,
            revision,
            decision,
            cancellation,
            |_| Ok(()),
        )
    }

    pub(super) fn decide_with_hook(
        &self,
        conversation_session_id: &str,
        change_id: &str,
        revision: u64,
        decision: ChangeDecision,
        cancellation: &CancellationToken,
        mut hook: impl FnMut(CommitStage) -> Result<(), ChangeError>,
    ) -> Result<ChangeReceipt, ChangeError> {
        let state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        self.verify_lease()?;
        let mut record = self.load_record(change_id)?;
        if record.conversation_session_id != conversation_session_id {
            return Err(ChangeError::SourceInvalid);
        }
        // Only replay the exact prior local operation, never a different action.
        let replay = match decision {
            ChangeDecision::Confirm => {
                record.receipt.status == ChangeStatus::Committed
                    && (revision == record.receipt.revision
                        || revision.checked_add(2) == Some(record.receipt.revision))
            }
            ChangeDecision::Discard => {
                record.receipt.status == ChangeStatus::Rejected
                    && (revision == record.receipt.revision
                        || revision.checked_add(1) == Some(record.receipt.revision))
            }
            ChangeDecision::Undo => {
                record.receipt.status == ChangeStatus::Undone
                    && (record.undo_revision == Some(revision)
                        || record.receipt.revision == revision)
            }
            ChangeDecision::RetryRecovery => false,
        };
        if revision > 0 && replay {
            return Ok(record.receipt);
        }
        if revision == record.receipt.revision
            && revision > 0
            && matches!(
                decision,
                ChangeDecision::Undo | ChangeDecision::RetryRecovery
            )
        {
            check_cancelled(cancellation)?;
            for file in &record.receipt.files {
                if state
                    .buffers
                    .get(&file.relative_path.to_lowercase())
                    .is_some_and(|(_, dirty)| *dirty)
                {
                    return Err(ChangeError::RevisionConflict);
                }
            }
            if decision == ChangeDecision::Undo && record.receipt.status == ChangeStatus::Committed
            {
                ensure_recovered(&self.records()?)?;
                self.apply_undo(&mut record, cancellation, &mut hook)?;
                return Ok(record.receipt);
            }
            if decision == ChangeDecision::RetryRecovery
                && record.receipt.status == ChangeStatus::RecoveryRequired
            {
                if record.undo_revision.is_some() {
                    self.rollback_undo(&mut record)?;
                } else {
                    self.rollback(&mut record)?;
                }
                return Ok(record.receipt);
            }
        }
        if revision == 0
            || revision != record.receipt.revision
            || record.receipt.status != ChangeStatus::PendingConfirmation
        {
            return Err(ChangeError::RevisionConflict);
        }
        match decision {
            ChangeDecision::Discard => {
                check_cancelled(cancellation)?;
                record.receipt.status = ChangeStatus::Rejected;
                record.receipt.revision += 1;
                self.save_record(&record)?;
                Ok(record.receipt)
            }
            ChangeDecision::Confirm => {
                check_cancelled(cancellation)?;
                ensure_recovered(&self.records()?)?;
                let session =
                    read_conversation(&self.handle).map_err(|_| ChangeError::SourceInvalid)?;
                let fresh = ChangeEvidence::from_conversation(&session, &record.turn_id)?;
                if record
                    .sources
                    .iter()
                    .any(|source| !fresh.source_references().contains(source))
                {
                    return Err(ChangeError::SourceInvalid);
                }
                for (snapshot, file) in record.snapshots.iter().zip(&record.receipt.files) {
                    let buffer = state.buffers.get(&snapshot.relative_path.to_lowercase());
                    if buffer.is_some_and(|(_, dirty)| *dirty)
                        || buffer.map(|(revision, _)| revision)
                            != record.editor_revisions.get(&snapshot.relative_path)
                    {
                        return Err(ChangeError::RevisionConflict);
                    }
                    if self.current_hash(&snapshot.relative_path)? != file.before_hash {
                        return Err(ChangeError::SourceChanged);
                    }
                    if snapshot.before.is_none() {
                        // Recheck semantic-name collisions that arose while pending.
                        self.check_new_identity(
                            &CanonMutation {
                                kind: MutationKind::Create,
                                entity_kind: snapshot.entity_kind,
                                capability_id: snapshot.entity_kind.capability_id().to_owned(),
                                method_receipt_id: String::new(),
                                target_id: None,
                                read_receipt_id: None,
                                expected_source_hash: None,
                                proposed_markdown: snapshot.after.clone(),
                            },
                            cancellation,
                        )?;
                    }
                }
                self.apply_record(&mut record, cancellation, &mut |_| Ok(()))?;
                Ok(record.receipt)
            }
            ChangeDecision::Undo | ChangeDecision::RetryRecovery => {
                Err(ChangeError::InvalidRequest)
            }
        }
    }
}

/// Exact normalized-byte difference, with common prefix/suffix retained. This
/// bounded linear algorithm avoids quadratic work on newline-heavy 8 KiB files.
fn diff_lines(before: &str, after: &str) -> Vec<ChangeDiffLine> {
    let left: Vec<_> = before.split_inclusive('\n').collect();
    let right: Vec<_> = after.split_inclusive('\n').collect();
    let prefix = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
    let suffix = left[prefix..]
        .iter()
        .rev()
        .zip(right[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut lines = Vec::new();
    for text in &left[..prefix] {
        lines.push(ChangeDiffLine {
            kind: DiffLineKind::Context,
            text: (*text).to_owned(),
        });
    }
    for text in &left[prefix..left.len() - suffix] {
        lines.push(ChangeDiffLine {
            kind: DiffLineKind::Removed,
            text: (*text).to_owned(),
        });
    }
    for text in &right[prefix..right.len() - suffix] {
        lines.push(ChangeDiffLine {
            kind: DiffLineKind::Added,
            text: (*text).to_owned(),
        });
    }
    for text in &right[right.len() - suffix..] {
        lines.push(ChangeDiffLine {
            kind: DiffLineKind::Context,
            text: (*text).to_owned(),
        });
    }
    lines
}
