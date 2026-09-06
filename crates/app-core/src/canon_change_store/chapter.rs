//! NR-502 Host chapter transaction path. Production registration is a later step
//! of this Story, after the transaction and read-evidence integration is tested.
use super::*;
use crate::chapter_contract::{
    validate_chapter_inventory, ChapterCandidate, ChapterCommitReceipt, ChapterCommitStatus,
    ChapterDocument, ChapterIdentity, ChapterIntent, ChapterStatus, ChapterTarget,
    MAX_CHAPTER_BYTES,
};
pub(super) mod candidates;
mod manual;
pub(super) mod tool;

pub struct ChapterSubmission<'a> {
    pub candidate: &'a ChapterCandidate,
    pub method_receipt_id: &'a str,
    /// Explicit semantic decision from the current user source, not an old final flag.
    pub finalize: bool,
    pub confirmation_required: bool,
    /// Supplied only by the Host's delivered-candidate store, never tool arguments.
    pub displayed: Option<&'a ChapterCandidate>,
}

impl CanonChangeCoordinator {
    pub fn submit_chapter(
        &self,
        evidence: &ChangeEvidence,
        submission: ChapterSubmission<'_>,
        call_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<ChangeReceipt, ChangeError> {
        self.submit_chapter_with_hook(evidence, submission, call_id, cancellation, |_| Ok(()))
    }

    fn submit_chapter_with_hook(
        &self,
        evidence: &ChangeEvidence,
        submission: ChapterSubmission<'_>,
        call_id: &str,
        cancellation: &CancellationToken,
        mut hook: impl FnMut(CommitStage) -> Result<(), ChangeError>,
    ) -> Result<ChangeReceipt, ChangeError> {
        if !self.nr5 {
            return Err(ChangeError::ChapterSourceUnavailable);
        }
        check_cancelled(cancellation)?;
        let state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        self.verify_lease()?;
        let session = read_conversation(&self.handle).map_err(|_| ChangeError::SourceInvalid)?;
        let turn = session.turns.last().ok_or(ChangeError::SourceInvalid)?;
        if !evidence.matches_context(self.project_id(), &session.session_id, &turn.turn_id)
            || !evidence.has_chapter_method(submission.method_receipt_id)
        {
            return Err(ChangeError::SourceInvalid);
        }
        let mut candidate = submission.candidate.clone();
        if sha256_hex(candidate.proposed_markdown.as_bytes()) != candidate.content_hash {
            return Err(ChangeError::CandidateInvalid);
        }
        let request_hash = sha256_hex(
            &serde_json::to_vec(&(
                submission.candidate,
                submission.finalize,
                submission.confirmation_required,
            ))
            .map_err(|_| ChangeError::InvalidRequest)?,
        );
        let records = self.records()?;
        for record in &records {
            if record.conversation_session_id == session.session_id
                && record.turn_id == turn.turn_id
                && record.call_id == call_id
            {
                return if record.request_hash == request_hash {
                    Ok(record.receipt.clone())
                } else {
                    Err(ChangeError::RevisionConflict)
                };
            }
        }
        ensure_recovered(&records)?;
        if records.iter().any(|r| {
            r.chapter.is_some()
                && r.turn_id == turn.turn_id
                && r.conversation_session_id == session.session_id
                && !matches!(
                    r.receipt.status,
                    ChangeStatus::Rejected | ChangeStatus::Cancelled
                )
        }) {
            return Err(ChangeError::BudgetExceeded);
        }
        let inventory = self.chapter_inventory(cancellation)?;
        let identities = inventory
            .iter()
            .map(|(_, d)| d.identity.clone())
            .collect::<Vec<_>>();
        // New identity and path belong to the Host, not a Provider-selected filename.
        if candidate.intent == ChapterIntent::WriteAndSave {
            if let ChapterTarget::Create { identity } = &mut candidate.target {
                identity.chapter_id = Uuid::new_v4().to_string();
                candidate.proposed_markdown =
                    replace_field(&candidate.proposed_markdown, "id", &identity.chapter_id)?;
                candidate.content_hash = sha256_hex(candidate.proposed_markdown.as_bytes());
            }
        }
        let read = match &candidate.target {
            ChapterTarget::Replace {
                read_receipt_id, ..
            } => evidence.chapter_read(read_receipt_id),
            _ => None,
        };
        let document = candidate
            .validate(
                &session,
                &candidate.scope,
                &identities,
                read.as_ref(),
                submission.displayed,
            )
            .map_err(|_| ChangeError::CandidateInvalid)?;
        if candidate.scope.project_id != self.project_id()
            || candidate.scope.turn_id != turn.turn_id
        {
            return Err(ChangeError::SourceInvalid);
        }
        let (path, before) = match &candidate.target {
            ChapterTarget::Create { identity } => {
                if inventory.len() >= 500 {
                    return Err(ChangeError::StoreFull);
                }
                (format!("chapters/{}.md", identity.chapter_id), None)
            }
            ChapterTarget::Replace {
                identity,
                expected_hash,
                read_receipt_id,
            } => {
                let (path, current) = inventory
                    .iter()
                    .find(|(_, doc)| &doc.identity == identity)
                    .ok_or(ChangeError::TargetInvalid)?;
                if current.content_hash != *expected_hash
                    || evidence.target_path(read_receipt_id) != Some(path.as_str())
                {
                    return Err(ChangeError::SourceChanged);
                }
                let bytes = read_bounded(
                    &resolve_scoped_target(self.handle.root(), path)
                        .map_err(|_| ChangeError::TargetInvalid)?,
                    MAX_CHAPTER_BYTES as u64,
                )
                .map_err(|_| ChangeError::SourceChanged)?;
                if sha256_hex(&bytes) != *expected_hash {
                    return Err(ChangeError::SourceChanged);
                }
                (
                    path.clone(),
                    Some(String::from_utf8(bytes).map_err(|_| ChangeError::CandidateInvalid)?),
                )
            }
        };
        CanonEntityKind::Chapter.validate_path(&path)?;
        let mut after = candidate.proposed_markdown;
        if let Some(old) = &before {
            // A replacement preserves exactly the source encoding. Mixed endings are
            // readable but cannot be silently converted by AI.
            let old_body = old.strip_prefix('\u{feff}').unwrap_or(old);
            let crlf = old_body.contains("\r\n");
            if old_body.replace("\r\n", "").contains('\r')
                || crlf && old_body.replace("\r\n", "").contains('\n')
            {
                return Err(ChangeError::CandidateInvalid);
            }
            let normalized = after
                .strip_prefix('\u{feff}')
                .unwrap_or(&after)
                .replace("\r\n", "\n");
            let bytes = crate::canon_apply::encode_chapter_target_bytes(
                &normalized,
                if old.starts_with('\u{feff}') {
                    crate::workspace::Utf8Bom::Utf8
                } else {
                    crate::workspace::Utf8Bom::None
                },
                if crlf {
                    crate::workspace::LineEnding::CrLf
                } else {
                    crate::workspace::LineEnding::Lf
                },
            )
            .map_err(|_| ChangeError::CandidateInvalid)?;
            after = String::from_utf8(bytes).map_err(|_| ChangeError::CandidateInvalid)?;
        }
        if submission.finalize {
            after = replace_field(&after, "status", "final")?;
        } else if before.as_deref() != Some(after.as_str())
            && document.status == ChapterStatus::Final
        {
            after = replace_field(&after, "status", "draft")?;
        }
        let actual = ChapterDocument::parse(&path, after.as_bytes())
            .map_err(|_| ChangeError::CandidateInvalid)?;
        let unchanged = before.as_deref() == Some(after.as_str());
        let prior = records.iter().filter(|r| {
            r.chapter
                .as_ref()
                .is_some_and(|c| c.identity == actual.identity)
        });
        let generation = prior
            .clone()
            .filter_map(|r| r.chapter.as_ref().map(|c| c.generation))
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ChangeError::StoreFull)?;
        let existing_current = prior
            .filter(|r| {
                matches!(
                    r.receipt.status,
                    ChangeStatus::Committed | ChangeStatus::Undone
                )
            })
            .max_by_key(|r| r.chapter.as_ref().map(|c| c.generation).unwrap_or(0))
            .is_some_and(|r| {
                r.receipt.status == ChangeStatus::Committed
                    && r.receipt.files[0].after_hash == actual.content_hash
                    && r.chapter.as_ref().is_some_and(|c| !c.final_invalidated)
            });
        let dirty = state
            .buffers
            .get(&path.to_lowercase())
            .is_some_and(|(_, dirty)| *dirty);
        let mut record = ChangeRecord {
            schema_version: CHANGE_SCHEMA_VERSION,
            project_id: self.project_id().to_owned(),
            conversation_session_id: session.session_id,
            turn_id: turn.turn_id.clone(),
            call_id: call_id.to_owned(),
            request_hash,
            source_kind: ChangeSourceKind::UserDecision,
            sources: vec![candidate.source],
            receipt: ChangeReceipt {
                schema_version: CHANGE_SCHEMA_VERSION,
                change_id: Uuid::new_v4().to_string(),
                revision: 1,
                status: if dirty || submission.confirmation_required {
                    ChangeStatus::PendingConfirmation
                } else if unchanged && existing_current && !submission.finalize {
                    ChangeStatus::NoChange
                } else {
                    ChangeStatus::Prepared
                },
                files: vec![ChangeFileReceipt {
                    relative_path: path.clone(),
                    before_hash: before.as_ref().map(|s| sha256_hex(s.as_bytes())),
                    after_hash: actual.content_hash,
                }],
            },
            snapshots: vec![ChangeSnapshot {
                entity_kind: CanonEntityKind::Chapter,
                relative_path: path.clone(),
                before,
                after,
            }],
            editor_revisions: state
                .buffers
                .get(&path.to_lowercase())
                .map(|(revision, _)| BTreeMap::from([(path, *revision)]))
                .unwrap_or_default(),
            undo_revision: None,
            chapter: Some(ChapterAuthority {
                generation,
                candidate_id: candidate.candidate_id,
                identity: actual.identity,
                intent: candidate.intent,
                final_confirmed: submission.finalize,
                final_invalidated: false,
            }),
            chapter_dependencies: Vec::new(),
        };
        if record.receipt.status == ChangeStatus::NoChange {
            hook(CommitStage::BeforeCommit)?;
            check_cancelled(cancellation)?;
            self.chapter_preflight(&record, cancellation)?;
            if self
                .current_hash(&record.receipt.files[0].relative_path)?
                .as_deref()
                != Some(&record.receipt.files[0].after_hash)
            {
                return Err(ChangeError::SourceChanged);
            }
        }
        self.save_record(&record)?;
        if record.receipt.status == ChangeStatus::Prepared {
            hook(CommitStage::Prepared)?;
            self.apply_record(&mut record, cancellation, &mut hook)?;
        }
        Ok(record.receipt)
    }

    pub(super) fn chapter_preflight(
        &self,
        record: &ChangeRecord,
        cancellation: &CancellationToken,
    ) -> Result<(), ChangeError> {
        let Some(chapter) = &record.chapter else {
            return Ok(());
        };
        let inventory = self.chapter_inventory(cancellation)?;
        let path = &record.snapshots[0].relative_path;
        if inventory.iter().any(|(p, d)| {
            p != path
                && (d.identity.chapter_id == chapter.identity.chapter_id
                    || d.identity.chapter_number == chapter.identity.chapter_number)
        }) {
            return Err(ChangeError::DuplicateTarget);
        }
        Ok(())
    }

    pub(super) fn chapter_inventory(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<(String, ChapterDocument)>, ChangeError> {
        let index = list_project_files(&self.handle).map_err(|_| ChangeError::TargetInvalid)?;
        let mut paths = Vec::new();
        collect_paths(&index.nodes, &mut paths);
        let mut inventory = Vec::new();
        for path in paths.into_iter().filter(|p| p.starts_with("chapters/")) {
            check_cancelled(cancellation)?;
            if inventory.len() >= 500 {
                return Err(ChangeError::StoreFull);
            }
            CanonEntityKind::Chapter.validate_path(&path)?;
            let bytes = read_bounded(
                &resolve_scoped_target(self.handle.root(), &path)
                    .map_err(|_| ChangeError::TargetInvalid)?,
                MAX_CHAPTER_BYTES as u64,
            )
            .map_err(|_| ChangeError::CandidateInvalid)?;
            let document =
                ChapterDocument::parse(&path, &bytes).map_err(|_| ChangeError::CandidateInvalid)?;
            inventory.push((path, document));
        }
        validate_chapter_inventory(
            &inventory
                .iter()
                .map(|(_, d)| d.identity.clone())
                .collect::<Vec<ChapterIdentity>>(),
        )
        .map_err(|_| ChangeError::DuplicateTarget)?;
        Ok(inventory)
    }

    /// A read of an existing chapter never invents this proof. Only a stored
    /// committed transaction with a matching current file may produce it.
    pub fn chapter_commit_receipt(
        &self,
        change_id: &str,
    ) -> Result<ChapterCommitReceipt, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        self.verify_lease()?;
        let records = self.records()?;
        ensure_recovered(&records)?;
        self.chapter_commit_receipt_from_records(change_id, &records)
    }

    pub(super) fn chapter_commit_receipt_from_records(
        &self,
        change_id: &str,
        records: &[ChangeRecord],
    ) -> Result<ChapterCommitReceipt, ChangeError> {
        self.verify_lease()?;
        let mut record = self.load_record(change_id)?;
        let chapter = record.chapter.as_ref().ok_or(ChangeError::SourceInvalid)?;
        if record.receipt.status != ChangeStatus::Committed
            || chapter.final_invalidated
            || records.iter().any(|r| {
                matches!(
                    r.receipt.status,
                    ChangeStatus::Committed | ChangeStatus::Undone
                ) && r.chapter.as_ref().is_some_and(|c| {
                    c.identity == chapter.identity && c.generation > chapter.generation
                })
            })
        {
            return Err(ChangeError::SourceChanged);
        }
        // Failure to read a formerly committed source (including oversized or
        // replaced/reparse targets) invalidates observed authority persistently.
        let current = self.current_hash(&record.receipt.files[0].relative_path);
        if !current.is_ok_and(|hash| hash.as_deref() == Some(&record.receipt.files[0].after_hash)) {
            record
                .chapter
                .as_mut()
                .ok_or(ChangeError::SourceInvalid)?
                .final_invalidated = true;
            self.save_record(&record)?;
            return Err(ChangeError::SourceChanged);
        }
        let receipt = ChapterCommitReceipt {
            schema_version: 1,
            transaction_id: record.receipt.change_id,
            revision: record.receipt.revision,
            scope: crate::chapter_contract::ChapterScope {
                project_id: record.project_id,
                session_id: record.conversation_session_id,
                turn_id: record.turn_id,
            },
            identity: record
                .chapter
                .as_ref()
                .ok_or(ChangeError::SourceInvalid)?
                .identity
                .clone(),
            before_hash: record.receipt.files[0].before_hash.clone(),
            after_hash: record.receipt.files[0].after_hash.clone(),
            source: if record.source_kind == ChangeSourceKind::ManualSave {
                crate::chapter_contract::ChapterCommitSource::ManualSave {
                    event_id: record.sources[0].message_id.clone(),
                    content_hash: record.sources[0].content_hash.clone(),
                }
            } else {
                crate::chapter_contract::ChapterCommitSource::UserMessage {
                    reference: record.sources[0].clone(),
                }
            },
            status: ChapterCommitStatus::Committed,
        };
        receipt.validate().map_err(|_| ChangeError::SourceInvalid)?;
        Ok(receipt)
    }
}

fn replace_field(content: &str, field: &str, value: &str) -> Result<String, ChangeError> {
    let bom = content.starts_with('\u{feff}');
    let body = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut in_header = false;
    let mut replaced = false;
    let mut output = if bom {
        "\u{feff}".to_owned()
    } else {
        String::new()
    };
    for (index, line) in body.split_inclusive('\n').enumerate() {
        let text = line.trim_end_matches(['\r', '\n']);
        if index == 0 {
            in_header = text == "---";
        } else if text == "---" {
            in_header = false;
        }
        if in_header && text.starts_with(&format!("{field}:")) {
            if replaced {
                return Err(ChangeError::CandidateInvalid);
            }
            output.push_str(&format!(
                "{field}: {value}{}",
                if line.ends_with("\r\n") { "\r\n" } else { "\n" }
            ));
            replaced = true;
        } else {
            output.push_str(line);
        }
    }
    if !replaced {
        return Err(ChangeError::CandidateInvalid);
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
