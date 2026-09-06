//! NR-501 pure chapter contracts. No filesystem or Provider side effects.
//! Only NR-502 may turn a validated candidate into a durable transaction.
use crate::canon_change::SourceReference;
use crate::conversation::{ConversationRole, ConversationSession, ConversationTurnStatus};
use crate::workspace::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const MAX_CHAPTER_BYTES: usize = 32 * 1024;
pub const NR5_AGENT_ATTEMPTS: usize = 8;
pub const NR5_REQUESTED_OUTPUT_TOKENS: u32 = 33_792;
pub const NR5_TOOL_ARGUMENT_BYTES: usize = 64 * 1024;
pub const NR5_TOOL_RESULT_BYTES: usize = 64 * 1024;
pub const NR5_CONTINUATION_BYTES: usize = 512 * 1024;
pub const NR5_REQUEST_BYTES: usize = 768 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterStatus {
    Draft,
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChapterContractError {
    InvalidDocument,
    InvalidIdentity,
    DuplicateTarget,
    InvalidSource,
    ReadRequired,
    Stale,
    TooLarge,
    NotAuthorized,
    Discarded,
}

fn id(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|v| !v.is_nil() && v.to_string() == value)
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChapterIdentity {
    pub chapter_id: String,
    pub chapter_number: u32,
}

impl ChapterIdentity {
    pub fn validate(&self) -> Result<(), ChapterContractError> {
        if !id(&self.chapter_id) || self.chapter_number == 0 {
            return Err(ChapterContractError::InvalidIdentity);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterDocument {
    pub identity: ChapterIdentity,
    pub title: String,
    pub status: ChapterStatus,
    /// Exact source bytes, including BOM and line endings. Never normalized for CAS.
    pub content_hash: String,
}

impl ChapterDocument {
    pub fn parse(relative_path: &str, bytes: &[u8]) -> Result<Self, ChapterContractError> {
        let name = relative_path
            .strip_prefix("chapters/")
            .ok_or(ChapterContractError::InvalidDocument)?;
        if name.is_empty()
            || !name.ends_with(".md")
            || name.contains(['/', '\\', ':', '\0'])
            || name.starts_with('.')
            || name.trim() != name
        {
            return Err(ChapterContractError::InvalidDocument);
        }
        if bytes.len() > MAX_CHAPTER_BYTES {
            return Err(ChapterContractError::TooLarge);
        }
        let text = std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
            .map_err(|_| ChapterContractError::InvalidDocument)?;
        let fields = novelgainker_validation::validated_frontmatter(relative_path, text)
            .ok_or(ChapterContractError::InvalidDocument)?;
        if text.contains('\0') {
            return Err(ChapterContractError::InvalidDocument);
        }
        let allowed = [
            "id",
            "title",
            "kind",
            "schemaVersion",
            "writingLanguage",
            "chapterNumber",
            "status",
        ];
        if fields.len() != allowed.len() || !allowed.iter().all(|key| fields.contains_key(*key)) {
            return Err(ChapterContractError::InvalidDocument);
        }
        let get = |key: &str| fields.get(key).map(String::as_str).unwrap_or("");
        if get("kind") != "chapter"
            || get("schemaVersion") != "1"
            || get("writingLanguage") != "zh-CN"
            || get("title").trim().is_empty()
            || get("title").len() > 512
            || get("title").chars().any(char::is_control)
            || !get("chapterNumber").bytes().all(|b| b.is_ascii_digit())
        {
            return Err(ChapterContractError::InvalidDocument);
        }
        let identity = ChapterIdentity {
            chapter_id: get("id").to_owned(),
            chapter_number: get("chapterNumber")
                .parse()
                .map_err(|_| ChapterContractError::InvalidDocument)?,
        };
        identity.validate()?;
        let status = match get("status") {
            "draft" => ChapterStatus::Draft,
            "final" => ChapterStatus::Final,
            _ => return Err(ChapterContractError::InvalidDocument),
        };
        Ok(Self {
            identity,
            title: get("title").to_owned(),
            status,
            content_hash: sha256_hex(bytes),
        })
    }
}

/// The index is supplied from the Host's current, complete chapter inventory.
pub fn validate_chapter_inventory(
    chapters: &[ChapterIdentity],
) -> Result<(), ChapterContractError> {
    let mut ids = BTreeSet::new();
    let mut numbers = BTreeSet::new();
    for chapter in chapters {
        chapter.validate()?;
        if !ids.insert(&chapter.chapter_id) || !numbers.insert(chapter.chapter_number) {
            return Err(ChapterContractError::DuplicateTarget);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChapterScope {
    pub project_id: String,
    pub session_id: String,
    pub turn_id: String,
}

impl ChapterScope {
    fn validate(&self) -> Result<(), ChapterContractError> {
        if [&self.project_id, &self.session_id, &self.turn_id]
            .into_iter()
            .all(|v| id(v))
        {
            Ok(())
        } else {
            Err(ChapterContractError::InvalidSource)
        }
    }

    /// Verifies source provenance, not the semantic meaning of a user's text.
    pub fn verify_user_source(
        &self,
        session: &ConversationSession,
        source: &SourceReference,
    ) -> Result<(), ChapterContractError> {
        self.validate()?;
        if self.project_id != session.project_id
            || self.session_id != session.session_id
            || !id(&source.message_id)
            || !hash(&source.content_hash)
        {
            return Err(ChapterContractError::InvalidSource);
        }
        let turn = session
            .turns
            .iter()
            .find(|t| t.turn_id == self.turn_id)
            .ok_or(ChapterContractError::InvalidSource)?;
        if turn.status != ConversationTurnStatus::Running {
            return Err(ChapterContractError::InvalidSource);
        }
        let matching = turn
            .messages
            .iter()
            .filter(|m| m.message_id == source.message_id)
            .collect::<Vec<_>>();
        if matching.len() != 1
            || matching[0].role != ConversationRole::User
            || sha256_hex(matching[0].content.as_bytes()) != source.content_hash
        {
            return Err(ChapterContractError::InvalidSource);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterIntent {
    Discussion,
    TrialDraft,
    WriteAndSave,
    AcceptCandidate,
    Ambiguous,
}

/// Closed target variants make missing/null preconditions a schema error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChapterTarget {
    Create {
        identity: ChapterIdentity,
    },
    Replace {
        identity: ChapterIdentity,
        #[serde(rename = "expectedHash")]
        expected_hash: String,
        #[serde(rename = "readReceiptId")]
        read_receipt_id: String,
    },
}

impl ChapterTarget {
    /// Accepting a displayed replacement keeps its identity and old bytes, but
    /// must use a newly delivered full-read receipt in the accepting Turn.
    pub(crate) fn same_version(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Create { identity: a }, Self::Create { identity: b }) => a == b,
            (
                Self::Replace {
                    identity: a,
                    expected_hash: ah,
                    ..
                },
                Self::Replace {
                    identity: b,
                    expected_hash: bh,
                    ..
                },
            ) => a == b && ah == bh,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterReadReceipt {
    pub receipt_id: String,
    pub scope: ChapterScope,
    pub identity: ChapterIdentity,
    pub content_hash: String,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChapterCandidate {
    pub schema_version: u16,
    pub candidate_id: String,
    pub scope: ChapterScope,
    pub source: SourceReference,
    pub intent: ChapterIntent,
    pub target: ChapterTarget,
    pub content_hash: String,
    pub proposed_markdown: String,
    pub complete: bool,
    pub discarded: bool,
}

impl ChapterCandidate {
    pub fn discard(&mut self) {
        self.discarded = true;
    }

    /// The Host supplies authoritative inventory, delivered reads, and current conversation.
    /// An accepted candidate also needs the exact prior displayed candidate; a model cannot
    /// create its own "displayed" proof in its request.
    pub fn validate(
        &self,
        session: &ConversationSession,
        current_scope: &ChapterScope,
        inventory: &[ChapterIdentity],
        read: Option<&ChapterReadReceipt>,
        displayed: Option<&ChapterCandidate>,
    ) -> Result<ChapterDocument, ChapterContractError> {
        if self.discarded {
            return Err(ChapterContractError::Discarded);
        }
        if self.schema_version != 1
            || !id(&self.candidate_id)
            || &self.scope != current_scope
            || !self.complete
            || !hash(&self.content_hash)
            || sha256_hex(self.proposed_markdown.as_bytes()) != self.content_hash
        {
            return Err(ChapterContractError::Stale);
        }
        self.scope.verify_user_source(session, &self.source)?;
        match self.intent {
            ChapterIntent::WriteAndSave => {}
            ChapterIntent::AcceptCandidate => {
                let old = displayed.ok_or(ChapterContractError::NotAuthorized)?;
                if old.discarded
                    || !old.complete
                    || old.scope.project_id != self.scope.project_id
                    || old.scope.session_id != self.scope.session_id
                    || old.candidate_id != self.candidate_id
                    || old.content_hash != self.content_hash
                    || old.proposed_markdown != self.proposed_markdown
                    || !old.target.same_version(&self.target)
                {
                    return Err(ChapterContractError::NotAuthorized);
                }
            }
            _ => return Err(ChapterContractError::NotAuthorized),
        }
        validate_chapter_inventory(inventory)?;
        let document =
            ChapterDocument::parse("chapters/candidate.md", self.proposed_markdown.as_bytes())?;
        match &self.target {
            ChapterTarget::Create { identity } => {
                if inventory.iter().any(|c| {
                    c.chapter_id == identity.chapter_id
                        || c.chapter_number == identity.chapter_number
                }) {
                    return Err(ChapterContractError::DuplicateTarget);
                }
                if &document.identity != identity {
                    return Err(ChapterContractError::InvalidIdentity);
                }
            }
            ChapterTarget::Replace {
                identity,
                expected_hash,
                read_receipt_id,
            } => {
                if !inventory.contains(identity) || &document.identity != identity {
                    return Err(ChapterContractError::InvalidIdentity);
                }
                let read = read.ok_or(ChapterContractError::ReadRequired)?;
                if !hash(expected_hash)
                    || !id(read_receipt_id)
                    || !read.complete
                    || read.receipt_id != *read_receipt_id
                    || read.scope != self.scope
                    || read.identity != *identity
                    || read.content_hash != *expected_hash
                {
                    return Err(ChapterContractError::ReadRequired);
                }
            }
        }
        Ok(document)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterCommitStatus {
    Committed,
    Undone,
    RecoveryRequired,
}

/// Only the durable Host transaction may issue this receipt, never Provider JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChapterCommitSource {
    UserMessage {
        reference: SourceReference,
    },
    ManualSave {
        #[serde(rename = "eventId")]
        event_id: String,
        #[serde(rename = "contentHash")]
        content_hash: String,
    },
}

impl ChapterCommitSource {
    fn validate(&self) -> bool {
        match self {
            Self::UserMessage { reference } => {
                id(&reference.message_id) && hash(&reference.content_hash)
            }
            Self::ManualSave {
                event_id,
                content_hash,
            } => id(event_id) && hash(content_hash),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChapterCommitReceipt {
    pub schema_version: u16,
    pub transaction_id: String,
    pub scope: ChapterScope,
    pub identity: ChapterIdentity,
    pub revision: u64,
    #[serde(deserialize_with = "required_nullable_hash")]
    pub before_hash: Option<String>,
    pub after_hash: String,
    pub source: ChapterCommitSource,
    pub status: ChapterCommitStatus,
}

fn required_nullable_hash<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

impl ChapterCommitReceipt {
    pub fn validate(&self) -> Result<(), ChapterContractError> {
        self.scope.validate()?;
        self.identity.validate()?;
        if self.schema_version != 1
            || !id(&self.transaction_id)
            || self.revision == 0
            || !hash(&self.after_hash)
            || self.before_hash.as_ref().is_some_and(|h| !hash(h))
            || !self.source.validate()
        {
            return Err(ChapterContractError::InvalidSource);
        }
        Ok(())
    }
    pub fn is_current(&self, project_id: &str, document: &ChapterDocument) -> bool {
        self.validate().is_ok()
            && self.status == ChapterCommitStatus::Committed
            && self.scope.project_id == project_id
            && self.identity == document.identity
            && self.after_hash == document.content_hash
    }
}

/// Stored outside Canon; afterHash includes the final status bytes, never this receipt itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChapterFinalization {
    pub receipt: ChapterCommitReceipt,
    pub confirmation_source: SourceReference,
    pub confirmation_scope: ChapterScope,
    pub invalidated: bool,
}

impl ChapterFinalization {
    pub fn confirm(
        receipt: ChapterCommitReceipt,
        document: &ChapterDocument,
        scope: ChapterScope,
        source: SourceReference,
        conversation: &ConversationSession,
    ) -> Result<Self, ChapterContractError> {
        scope.verify_user_source(conversation, &source)?;
        if document.status != ChapterStatus::Final
            || !receipt.is_current(&scope.project_id, document)
        {
            return Err(ChapterContractError::NotAuthorized);
        }
        Ok(Self {
            receipt,
            confirmation_source: source,
            confirmation_scope: scope,
            invalidated: false,
        })
    }
    pub fn observe(&mut self, project_id: &str, current: &ChapterDocument) {
        if !self.receipt.is_current(project_id, current) || current.status != ChapterStatus::Final {
            self.invalidated = true;
        }
    }
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }
    pub fn permits_publication(
        &self,
        receipt: &ChapterCommitReceipt,
        current: &ChapterDocument,
    ) -> bool {
        !self.invalidated
            && self.confirmation_scope.validate().is_ok()
            && id(&self.confirmation_source.message_id)
            && hash(&self.confirmation_source.content_hash)
            && self.receipt == *receipt
            && current.status == ChapterStatus::Final
            && self
                .receipt
                .is_current(&self.confirmation_scope.project_id, current)
    }
}

#[cfg(test)]
#[path = "chapter_contract_tests.rs"]
mod tests;
