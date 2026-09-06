use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PageCursor {
    version: u16,
    project_id: String,
    session_id: String,
    turn_id: Option<String>,
    limit: u16,
    snapshot: String,
    offset: usize,
}

impl CanonChangeCoordinator {
    pub fn activities_page(
        &self,
        conversation_session_id: &str,
        request: &ChangeListRequest,
    ) -> Result<ChangePage, ChangeError> {
        request.validate()?;
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        let session = read_conversation(&self.handle).map_err(|_| ChangeError::SourceInvalid)?;
        if session.session_id != conversation_session_id {
            return Err(ChangeError::SourceInvalid);
        }
        let all_records = self.records()?;
        let candidates = self
            .chapter_candidate_activities(&session, &all_records)?
            .into_iter()
            .filter(|candidate| {
                request
                    .turn_id
                    .as_ref()
                    .is_none_or(|turn| turn == &candidate.turn_id)
            })
            .collect();
        let mut records = all_records
            .iter()
            .filter(|record| {
                record.conversation_session_id == conversation_session_id
                    && request
                        .turn_id
                        .as_ref()
                        .is_none_or(|turn| turn == &record.turn_id)
            })
            .collect::<Vec<_>>();
        let order = session
            .turns
            .iter()
            .enumerate()
            .map(|(index, turn)| (turn.turn_id.as_str(), index))
            .collect::<BTreeMap<_, _>>();
        // Manual save events have no fabricated chat Turn. They are included at
        // the front and rendered separately from the chat's actual messages.
        records.sort_by(|a, b| {
            let a_order = order.get(a.turn_id.as_str()).copied().unwrap_or(usize::MAX);
            let b_order = order.get(b.turn_id.as_str()).copied().unwrap_or(usize::MAX);
            b_order
                .cmp(&a_order)
                .then_with(|| b.receipt.change_id.cmp(&a.receipt.change_id))
        });
        let snapshot = sha256_hex(
            &serde_json::to_vec(
                &records
                    .iter()
                    .map(|record| {
                        (
                            &record.receipt.change_id,
                            record.receipt.revision,
                            record.receipt.status,
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| ChangeError::Io)?,
        );
        let offset = if let Some(cursor) = &request.cursor {
            let bytes = URL_SAFE_NO_PAD
                .decode(cursor)
                .map_err(|_| ChangeError::InvalidRequest)?;
            let cursor: PageCursor =
                serde_json::from_slice(&bytes).map_err(|_| ChangeError::InvalidRequest)?;
            if cursor.version != 1
                || cursor.project_id != self.project_id()
                || cursor.session_id != conversation_session_id
                || cursor.turn_id != request.turn_id
                || cursor.limit != request.limit
                || cursor.snapshot != snapshot
                || cursor.offset == 0
                || cursor.offset >= records.len()
                || !cursor.offset.is_multiple_of(usize::from(request.limit))
            {
                return Err(ChangeError::RevisionConflict);
            }
            cursor.offset
        } else {
            0
        };
        let end = offset
            .saturating_add(usize::from(request.limit))
            .min(records.len());
        let next_cursor = if end < records.len() {
            let cursor = PageCursor {
                version: 1,
                project_id: self.project_id().to_owned(),
                session_id: conversation_session_id.to_owned(),
                turn_id: request.turn_id.clone(),
                limit: request.limit,
                snapshot,
                offset: end,
            };
            Some(URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor).map_err(|_| ChangeError::Io)?))
        } else {
            None
        };
        let items = records
            .into_iter()
            .skip(offset)
            .take(usize::from(request.limit))
            .map(|record| self.activity_with_source_state(record, &all_records))
            .collect();
        Ok(ChangePage {
            schema_version: 1,
            items,
            next_cursor,
            candidates,
        })
    }
    pub fn activities(
        &self,
        conversation_session_id: &str,
    ) -> Result<Vec<ChangeActivity>, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        let records = self.records()?;
        Ok(records
            .iter()
            .filter(|record| record.conversation_session_id == conversation_session_id)
            .map(|record| self.activity_with_source_state(record, &records))
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
            activity: self.activity_with_source_state(&record, &self.records()?),
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
        // Reuse the closed local discard action for an opaque draft ID. It
        // cannot confirm/save a draft or touch Canon; existing changes win in
        // the unlikely event of an ID collision.
        if self.nr5
            && decision == ChangeDecision::Discard
            && Uuid::parse_str(change_id)
                .is_ok_and(|id| !id.is_nil() && id.to_string() == change_id)
            && fs::symlink_metadata(self.directory()?.join(format!("{change_id}.json"))).is_err()
            && fs::symlink_metadata(
                self.directory()?
                    .join(format!(".candidate-{change_id}.json")),
            )
            .is_ok()
        {
            check_cancelled(cancellation)?;
            if revision != 1 {
                return Err(ChangeError::RevisionConflict);
            }
            self.discard_displayed_chapter(change_id, conversation_session_id)?;
            return Ok(ChangeReceipt {
                schema_version: CHANGE_SCHEMA_VERSION,
                change_id: change_id.to_owned(),
                revision: 2,
                status: ChangeStatus::Rejected,
                files: Vec::new(),
            });
        }
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
