use super::*;

impl CanonChangeCoordinator {
    /// The caller already holds the common editor/AI transaction mutex.
    pub(in crate::canon_change_store) fn save_manual_chapter_locked(
        &self,
        path: &str,
        content: &str,
        source_hash: &str,
    ) -> Result<ProjectFileSaveResult, ChangeError> {
        CanonEntityKind::Chapter.validate_path(path)?;
        let current =
            read_project_file(&self.handle, path).map_err(|_| ChangeError::SourceChanged)?;
        if current.content_hash != source_hash {
            return Err(ChangeError::SourceChanged);
        }
        let target = resolve_scoped_target(self.handle.root(), path)
            .map_err(|_| ChangeError::TargetInvalid)?;
        let before_bytes = read_bounded(&target, crate::workspace::MAX_EDITABLE_FILE_BYTES)
            .map_err(|_| ChangeError::SourceChanged)?;
        if sha256_hex(&before_bytes) != source_hash {
            return Err(ChangeError::SourceChanged);
        }
        let normalized = content
            .strip_prefix('\u{feff}')
            .unwrap_or(content)
            .replace("\r\n", "\n");
        let encoded = crate::canon_apply::encode_chapter_target_bytes(
            &normalized,
            current.bom,
            current.line_ending,
        );
        let eligible = encoded.as_ref().ok().and_then(|bytes| {
            let old = ChapterDocument::parse(path, &before_bytes).ok()?;
            let new = ChapterDocument::parse(path, bytes).ok()?;
            (old.identity == new.identity).then_some((old, new))
        });
        let mut records = self.records()?;
        let Some((old, new)) = eligible else {
            // Ordinary editing remains available for malformed/oversized legacy
            // text, but it cannot mint a formal chapter source. Invalidate before
            // attempting the save, so even an interrupted save is conservative.
            for record in &mut records {
                if record.chapter.is_some() && record.receipt.files[0].relative_path == path {
                    record
                        .chapter
                        .as_mut()
                        .ok_or(ChangeError::SourceInvalid)?
                        .final_invalidated = true;
                    self.save_record(record)?;
                }
            }
            return save_project_file_recoverable(
                &self.handle,
                path,
                content,
                source_hash,
                &self.recovery,
            )
            .map_err(|_| ChangeError::Io);
        };
        let mut after = String::from_utf8(encoded.map_err(|_| ChangeError::CandidateInvalid)?)
            .map_err(|_| ChangeError::CandidateInvalid)?;
        let before = String::from_utf8(before_bytes).map_err(|_| ChangeError::CandidateInvalid)?;
        if before != after && new.status == ChapterStatus::Final {
            after = replace_field(&after, "status", "draft")?;
        }
        let actual = ChapterDocument::parse(path, after.as_bytes())
            .map_err(|_| ChangeError::CandidateInvalid)?;
        let prior = records.iter().filter(|r| {
            r.chapter
                .as_ref()
                .is_some_and(|c| c.identity == old.identity)
        });
        let latest = prior
            .clone()
            .filter(|r| {
                matches!(
                    r.receipt.status,
                    ChangeStatus::Committed | ChangeStatus::Undone
                )
            })
            .max_by_key(|r| r.chapter.as_ref().map(|c| c.generation).unwrap_or(0));
        if before == after
            && latest.is_some_and(|r| {
                r.receipt.status == ChangeStatus::Committed
                    && r.receipt.files[0].after_hash == actual.content_hash
                    && r.chapter.as_ref().is_some_and(|c| !c.final_invalidated)
            })
        {
            return self.chapter_save_result(path, &actual.content_hash);
        }
        let generation = prior
            .filter_map(|r| r.chapter.as_ref().map(|c| c.generation))
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ChangeError::StoreFull)?;
        let session = crate::conversation::ensure_conversation_session(&self.handle)
            .map_err(|_| ChangeError::SourceInvalid)?;
        let event = Uuid::new_v4().to_string();
        let mut record = ChangeRecord {
            schema_version: CHANGE_SCHEMA_VERSION,
            project_id: self.project_id().to_owned(),
            conversation_session_id: session.session_id,
            turn_id: event.clone(),
            call_id: format!("manual_{event}"),
            request_hash: sha256_hex(
                &serde_json::to_vec(&("manual_save", path, source_hash, &actual.content_hash))
                    .map_err(|_| ChangeError::InvalidRequest)?,
            ),
            source_kind: ChangeSourceKind::ManualSave,
            sources: vec![SourceReference {
                message_id: event.clone(),
                content_hash: actual.content_hash.clone(),
            }],
            receipt: ChangeReceipt {
                schema_version: CHANGE_SCHEMA_VERSION,
                change_id: Uuid::new_v4().to_string(),
                revision: 1,
                // Adoption of identical bytes still traverses the durable
                // transaction and its before/after CAS checks.
                status: ChangeStatus::Prepared,
                files: vec![ChangeFileReceipt {
                    relative_path: path.to_owned(),
                    before_hash: Some(source_hash.to_owned()),
                    after_hash: actual.content_hash.clone(),
                }],
            },
            snapshots: vec![ChangeSnapshot {
                entity_kind: CanonEntityKind::Chapter,
                relative_path: path.to_owned(),
                before: Some(before),
                after,
            }],
            editor_revisions: BTreeMap::new(),
            undo_revision: None,
            chapter: Some(ChapterAuthority {
                generation,
                candidate_id: event,
                identity: actual.identity,
                intent: ChapterIntent::WriteAndSave,
                final_confirmed: false,
                final_invalidated: false,
            }),
            chapter_dependencies: Vec::new(),
        };
        self.chapter_preflight(&record, &CancellationToken::default())?;
        self.save_record(&record)?;
        if record.receipt.status == ChangeStatus::Prepared {
            self.apply_record(&mut record, &CancellationToken::default(), &mut |_| Ok(()))?;
        }
        self.chapter_save_result(path, &actual.content_hash)
    }

    fn chapter_save_result(
        &self,
        path: &str,
        expected_hash: &str,
    ) -> Result<ProjectFileSaveResult, ChangeError> {
        let current = read_project_file(&self.handle, path).map_err(|_| ChangeError::Io)?;
        if current.content_hash != expected_hash {
            return Err(ChangeError::SourceChanged);
        }
        let bytes = read_bounded(
            &resolve_scoped_target(self.handle.root(), path)
                .map_err(|_| ChangeError::TargetInvalid)?,
            MAX_CHAPTER_BYTES as u64,
        )
        .map_err(|_| ChangeError::SourceChanged)?;
        if sha256_hex(&bytes) != expected_hash {
            return Err(ChangeError::SourceChanged);
        }
        let records = self.records()?;
        let confirmed = records
            .iter()
            .filter(|record| {
                record.chapter.is_some()
                    && record.receipt.files[0].relative_path == path
                    && matches!(
                        record.receipt.status,
                        ChangeStatus::Committed | ChangeStatus::Undone
                    )
            })
            .max_by_key(|record| {
                record
                    .chapter
                    .as_ref()
                    .map(|chapter| chapter.generation)
                    .unwrap_or(0)
            })
            .is_some_and(|record| {
                record.receipt.status == ChangeStatus::Committed
                    && record.receipt.files[0].after_hash == expected_hash
                    && record.chapter.as_ref().is_some_and(|chapter| {
                        chapter.final_confirmed && !chapter.final_invalidated
                    })
            });
        Ok(ProjectFileSaveResult {
            relative_path: path.to_owned(),
            content_hash: current.content_hash,
            modified_time_ms: current.modified_time_ms,
            size_bytes: current.size_bytes,
            bom: current.bom,
            line_ending: current.line_ending,
            diagnostics_triggered: true,
            diagnostic_state: crate::workspace::DiagnosticState::Ready,
            validation: novelgainker_validation::validate_source_bytes(path, &bytes),
            saved_content: Some(current.content),
            chapter_finalization_confirmed: Some(confirmed),
        })
    }
}
