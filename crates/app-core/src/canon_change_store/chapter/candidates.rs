//! Bounded, non-Canon provenance for exactly one complete, persisted and visible
//! chapter candidate. No Provider argument can manufacture this displayed proof.
use super::*;
use crate::conversation::{ConversationRole, ConversationSession, ConversationTurnStatus};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub(in crate::canon_change_store) const MAX_CANDIDATES: usize = 64;
pub(in crate::canon_change_store) const MAX_CANDIDATE_RECORD_BYTES: u64 = 256 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCandidate {
    schema_version: u16,
    candidate: ChapterCandidate,
    assistant_source: SourceReference,
}

pub(in crate::canon_change_store) fn candidate_file_id(name: &str) -> Option<&str> {
    let id = name.strip_prefix(".candidate-")?.strip_suffix(".json")?;
    Uuid::parse_str(id)
        .ok()
        .filter(|uuid| !uuid.is_nil() && uuid.to_string() == id)?;
    Some(id)
}

/// This recognizes a data format, never user intent. Incomplete fences, multiple
/// chapter blocks and nonconforming documents cannot become a named candidate.
fn visible_chapter(text: &str) -> Option<&str> {
    let mut offset = 0usize;
    let mut opening = None;
    let mut found = None;
    for line in text.split_inclusive('\n') {
        let marker = line.trim_end_matches(['\r', '\n']);
        if opening.is_none() && matches!(marker, "```markdown" | "```md") {
            opening = Some(offset + line.len());
        } else if marker == "```" {
            if let Some(start) = opening.take() {
                let body = &text[start..offset];
                if ChapterDocument::parse("chapters/candidate.md", body.as_bytes()).is_ok() {
                    if found.is_some() {
                        return None;
                    }
                    found = Some(body);
                }
            }
        }
        offset += line.len();
    }
    if opening.is_some() {
        None
    } else {
        found
    }
}

impl StoredCandidate {
    fn validate(&self, session: &ConversationSession) -> Result<(), ChangeError> {
        let candidate = &self.candidate;
        if self.schema_version != 1
            || candidate.schema_version != 1
            || !candidate.complete
            || candidate.scope.project_id != session.project_id
            || candidate.scope.session_id != session.session_id
            || candidate.intent != ChapterIntent::TrialDraft
            || candidate.content_hash != sha256_hex(candidate.proposed_markdown.as_bytes())
            || Uuid::parse_str(&candidate.candidate_id).is_err()
        {
            return Err(ChangeError::CandidateInvalid);
        }
        let turn = session
            .turns
            .iter()
            .find(|turn| turn.turn_id == candidate.scope.turn_id)
            .ok_or(ChangeError::SourceInvalid)?;
        if turn.status != ConversationTurnStatus::Completed {
            return Err(ChangeError::SourceInvalid);
        }
        let user = turn
            .messages
            .iter()
            .find(|message| {
                message.message_id == candidate.source.message_id
                    && message.role == ConversationRole::User
            })
            .ok_or(ChangeError::SourceInvalid)?;
        let assistant = turn
            .messages
            .iter()
            .find(|message| {
                message.message_id == self.assistant_source.message_id
                    && message.role == ConversationRole::Assistant
            })
            .ok_or(ChangeError::SourceInvalid)?;
        if sha256_hex(user.content.as_bytes()) != candidate.source.content_hash
            || sha256_hex(assistant.content.as_bytes()) != self.assistant_source.content_hash
            || visible_chapter(&assistant.content) != Some(&candidate.proposed_markdown)
        {
            return Err(ChangeError::SourceInvalid);
        }
        let document = ChapterDocument::parse(
            "chapters/candidate.md",
            candidate.proposed_markdown.as_bytes(),
        )
        .map_err(|_| ChangeError::CandidateInvalid)?;
        let identity = match &candidate.target {
            ChapterTarget::Create { identity } | ChapterTarget::Replace { identity, .. } => {
                identity
            }
        };
        if &document.identity != identity {
            return Err(ChangeError::TargetInvalid);
        }
        Ok(())
    }
}

impl CanonChangeCoordinator {
    /// Call only after the terminal visible message has been persisted. Failure
    /// here must never convert that message to a failed or empty chat reply.
    pub fn capture_displayed_chapter(&self, evidence: &ChangeEvidence) -> Result<(), ChangeError> {
        if !self.nr5 {
            return Ok(());
        }
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        self.verify_lease()?;
        let session = read_conversation(&self.handle).map_err(|_| ChangeError::SourceInvalid)?;
        let scope = evidence.chapter_scope();
        if !evidence.matches_context(self.project_id(), &session.session_id, &scope.turn_id) {
            return Err(ChangeError::SourceInvalid);
        }
        let turn = session
            .turns
            .iter()
            .find(|turn| turn.turn_id == scope.turn_id)
            .ok_or(ChangeError::SourceInvalid)?;
        if turn.status != ConversationTurnStatus::Completed {
            return Ok(());
        }
        let Some(assistant) = turn
            .messages
            .iter()
            .find(|message| message.role == ConversationRole::Assistant)
        else {
            return Ok(());
        };
        let Some(markdown) = visible_chapter(&assistant.content) else {
            return Ok(());
        };
        let document = ChapterDocument::parse("chapters/candidate.md", markdown.as_bytes())
            .map_err(|_| ChangeError::CandidateInvalid)?;
        let records = self.records()?;
        ensure_recovered(&records)?;
        if records.iter().any(|record| {
            record.conversation_session_id == session.session_id
                && record.turn_id == scope.turn_id
                && record.chapter.is_some()
        }) {
            return Ok(());
        }
        let target = if document.identity.chapter_id == evidence.chapter_draft_id() {
            let inventory = self.chapter_inventory(&CancellationToken::default())?;
            if inventory.iter().any(|(_, existing)| {
                existing.identity.chapter_id == document.identity.chapter_id
                    || existing.identity.chapter_number == document.identity.chapter_number
            }) {
                return Err(ChangeError::DuplicateTarget);
            }
            ChapterTarget::Create {
                identity: document.identity,
            }
        } else {
            let read = evidence
                .chapter_read_for_identity(&document.identity)
                .ok_or(ChangeError::IncompleteRead)?;
            ChapterTarget::Replace {
                identity: document.identity,
                expected_hash: read.content_hash,
                read_receipt_id: read.receipt_id,
            }
        };
        let user = turn
            .messages
            .iter()
            .find(|message| message.role == ConversationRole::User)
            .ok_or(ChangeError::SourceInvalid)?;
        let stored = StoredCandidate {
            schema_version: 1,
            candidate: ChapterCandidate {
                schema_version: 1,
                candidate_id: evidence.chapter_draft_id().to_owned(),
                scope,
                source: SourceReference {
                    message_id: user.message_id.clone(),
                    content_hash: sha256_hex(user.content.as_bytes()),
                },
                intent: ChapterIntent::TrialDraft,
                target,
                content_hash: sha256_hex(markdown.as_bytes()),
                proposed_markdown: markdown.to_owned(),
                complete: true,
                discarded: false,
            },
            assistant_source: SourceReference {
                message_id: assistant.message_id.clone(),
                content_hash: sha256_hex(assistant.content.as_bytes()),
            },
        };
        stored.validate(&session)?;
        let target = self
            .directory()?
            .join(format!(".candidate-{}.json", stored.candidate.candidate_id));
        if fs::symlink_metadata(target).is_ok() {
            // Capturing the same terminal message again must not resurrect a
            // deliberately discarded candidate or replace a corrupt proof.
            self.load_candidate(&stored.candidate.candidate_id, &session)?;
            return Ok(());
        }
        self.save_candidate(&stored)
    }

    fn candidate_ids(&self) -> Result<Vec<String>, ChangeError> {
        let mut ids = Vec::new();
        for (index, entry) in fs::read_dir(self.directory()?)
            .map_err(|_| ChangeError::Io)?
            .enumerate()
        {
            if index >= self.record_limit() * 3 {
                return Err(ChangeError::StoreFull);
            }
            let entry = entry.map_err(|_| ChangeError::Io)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(ChangeError::CandidateInvalid)?;
            if let Some(id) = candidate_file_id(name) {
                ids.push(id.to_owned());
            }
            if ids.len() > MAX_CANDIDATES {
                return Err(ChangeError::StoreFull);
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn load_candidate(
        &self,
        id: &str,
        session: &ConversationSession,
    ) -> Result<StoredCandidate, ChangeError> {
        let name = format!(".candidate-{id}.json");
        if candidate_file_id(&name) != Some(id) {
            return Err(ChangeError::CandidateInvalid);
        }
        let target = self.directory()?.join(name);
        let metadata = fs::symlink_metadata(&target).map_err(|_| ChangeError::CandidateInvalid)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(ChangeError::CandidateInvalid);
        }
        let bytes = read_bounded(&target, MAX_CANDIDATE_RECORD_BYTES)
            .map_err(|_| ChangeError::CandidateInvalid)?;
        let stored: StoredCandidate =
            serde_json::from_slice(&bytes).map_err(|_| ChangeError::CandidateInvalid)?;
        if stored.candidate.candidate_id != id {
            return Err(ChangeError::CandidateInvalid);
        }
        stored.validate(session)?;
        Ok(stored)
    }

    pub fn displayed_chapter_candidate(
        &self,
        id: &str,
        session_id: &str,
    ) -> Result<ChapterCandidate, ChangeError> {
        if !self.nr5 {
            return Err(ChangeError::ChapterSourceUnavailable);
        }
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        let session = read_conversation(&self.handle).map_err(|_| ChangeError::SourceInvalid)?;
        if session.session_id != session_id {
            return Err(ChangeError::SourceInvalid);
        }
        let stored = self.load_candidate(id, &session)?;
        if stored.candidate.discarded {
            return Err(ChangeError::CandidateInvalid);
        }
        Ok(stored.candidate)
    }

    pub fn displayed_chapter_catalog(&self, session_id: &str) -> Result<Value, ChangeError> {
        if !self.nr5 {
            return Ok(json!([]));
        }
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        let session = read_conversation(&self.handle).map_err(|_| ChangeError::SourceInvalid)?;
        if session.session_id != session_id {
            return Err(ChangeError::SourceInvalid);
        }
        let records = self.records()?;
        let mut catalog = Vec::new();
        for id in self.candidate_ids()? {
            let Ok(stored) = self.load_candidate(&id, &session) else {
                continue;
            };
            if stored.candidate.discarded || candidate_consumed(&records, &id) {
                continue;
            }
            let position = session
                .turns
                .iter()
                .position(|turn| turn.turn_id == stored.candidate.scope.turn_id)
                .ok_or(ChangeError::SourceInvalid)?;
            catalog.push((position, json!({"candidateId":id,"turnId":stored.candidate.scope.turn_id,"contentHash":stored.candidate.content_hash,"target":stored.candidate.target,"assistantSource":stored.assistant_source,"saved":false})));
        }
        catalog.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
        Ok(Value::Array(
            catalog
                .into_iter()
                .take(8)
                .map(|(_, value)| value)
                .collect(),
        ))
    }

    pub fn discard_displayed_chapter(&self, id: &str, session_id: &str) -> Result<(), ChangeError> {
        if !self.nr5 {
            return Err(ChangeError::ChapterSourceUnavailable);
        }
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        self.verify_lease()?;
        let session = read_conversation(&self.handle).map_err(|_| ChangeError::SourceInvalid)?;
        if session.session_id != session_id {
            return Err(ChangeError::SourceInvalid);
        }
        if candidate_consumed(&self.records()?, id) {
            return Err(ChangeError::RevisionConflict);
        }
        let mut stored = self.load_candidate(id, &session)?;
        stored.candidate.discard();
        self.save_candidate(&stored)
    }

    pub(in crate::canon_change_store) fn chapter_candidate_activities(
        &self,
        session: &ConversationSession,
        records: &[ChangeRecord],
    ) -> Result<Vec<ChapterCandidateActivity>, ChangeError> {
        if !self.nr5 {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for id in self.candidate_ids()? {
            let Ok(stored) = self.load_candidate(&id, session) else {
                continue;
            };
            if stored.candidate.discarded || candidate_consumed(records, &id) {
                continue;
            }
            let position = session
                .turns
                .iter()
                .position(|turn| turn.turn_id == stored.candidate.scope.turn_id)
                .ok_or(ChangeError::SourceInvalid)?;
            entries.push((
                position,
                ChapterCandidateActivity {
                    candidate_id: id,
                    turn_id: stored.candidate.scope.turn_id,
                    content_hash: stored.candidate.content_hash,
                },
            ));
        }
        entries.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
        Ok(entries
            .into_iter()
            .take(8)
            .map(|(_, entry)| entry)
            .collect())
    }

    fn save_candidate(&self, stored: &StoredCandidate) -> Result<(), ChangeError> {
        let id = &stored.candidate.candidate_id;
        let ids = self.candidate_ids()?;
        if !ids.iter().any(|old| old == id) && ids.len() >= MAX_CANDIDATES {
            return Err(ChangeError::StoreFull);
        }
        let bytes = serde_json::to_vec(stored).map_err(|_| ChangeError::Io)?;
        if bytes.len() as u64 > MAX_CANDIDATE_RECORD_BYTES {
            return Err(ChangeError::StoreFull);
        }
        let directory = self.directory()?;
        let mut total = 0u64;
        for (index, entry) in fs::read_dir(&directory)
            .map_err(|_| ChangeError::Io)?
            .enumerate()
        {
            if index >= self.record_limit() * 3 - 3 {
                return Err(ChangeError::StoreFull);
            }
            let metadata = fs::symlink_metadata(entry.map_err(|_| ChangeError::Io)?.path())
                .map_err(|_| ChangeError::Io)?;
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(ChangeError::CandidateInvalid);
            }
            total = total
                .checked_add(metadata.len())
                .ok_or(ChangeError::StoreFull)?;
        }
        if total + 3 * self.record_byte_limit() + bytes.len() as u64 > self.store_limit() {
            return Err(ChangeError::StoreFull);
        }
        let target = directory.join(format!(".candidate-{id}.json"));
        if let Ok(metadata) = fs::symlink_metadata(&target) {
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(ChangeError::CandidateInvalid);
            }
        }
        let temporary = directory.join(format!(".record-{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| ChangeError::Io)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| ChangeError::Io)?;
            drop(file);
            if self.directory()? != directory {
                return Err(ChangeError::TargetInvalid);
            }
            atomic_replace(&temporary, &target).map_err(|_| ChangeError::Io)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn candidate_consumed(records: &[ChangeRecord], id: &str) -> bool {
    records.iter().any(|record| {
        record
            .chapter
            .as_ref()
            .is_some_and(|chapter| chapter.candidate_id == id)
            && !matches!(
                record.receipt.status,
                ChangeStatus::Rejected | ChangeStatus::Cancelled
            )
    })
}
