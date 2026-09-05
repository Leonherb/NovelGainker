//! NR-401: bounded, side-effect-free Canon mutation contracts.
//!
//! Passing preflight is not a write authorization: the executor must resolve
//! identity, acquire the project lease, recheck disk/buffer revisions and create
//! a durable recovery record before applying anything.
use crate::capability_pack::ValidatedCapabilityPack;
use crate::conversation::{ConversationRole, ConversationSession};
use crate::tool_runtime::{ToolDefinition, MAX_TOOL_ARGUMENTS_BYTES};
use crate::workspace::{sha256_hex, validate_writable_relative_path};
use novelgainker_validation::validate_source;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const CHANGE_SCHEMA_VERSION: u16 = 1;
pub const PROPOSE_CHANGE_TOOL_NAME: &str = "novelist.propose_change";
pub const PROPOSE_CHANGE_WIRE_NAME: &str = "novelist_propose_change";
pub const MAX_CHANGE_TARGETS: usize = 4;
pub const MAX_CHANGE_FILE_BYTES: usize = 8 * 1024;
pub const MAX_CHANGE_BODY_BYTES: usize = 8 * 1024;
pub const MAX_CHANGE_REFERENCES: usize = 8;
pub const MAX_CHANGE_STORE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_CHANGE_RECORDS: usize = 100;
pub const MAX_PENDING_CHANGES: usize = 10;
pub const MAX_NR4_READ_ROUNDS: usize = 2;
pub const MAX_NR4_MUTATION_ROUNDS: usize = 1;
pub const MAX_NR4_AGENT_ATTEMPTS: usize = 4;
pub const NR4_CUMULATIVE_OUTPUT_TOKENS: u32 = 17_408;
pub const NR4_CONTINUATION_BYTES: usize = 160 * 1024;
pub const NR4_LOOP_RESERVE_BYTES: usize = 192 * 1024;
pub const NR4_REQUEST_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonEntityKind {
    Character,
    World,
    Material,
    Plot,
    Outline,
}

impl CanonEntityKind {
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Character => "characters",
            Self::World => "world",
            Self::Material => "materials",
            Self::Plot => "plot",
            Self::Outline => "outlines",
        }
    }

    pub const fn capability_id(self) -> &'static str {
        match self {
            Self::Character => "create-character",
            Self::World => "create-setting",
            Self::Material => "create-material",
            Self::Plot => "create-plot",
            Self::Outline => "novel-story-telling",
        }
    }

    pub const fn metadata_kind(self) -> &'static str {
        match self {
            Self::World => "setting",
            Self::Character => "character",
            Self::Material => "material",
            Self::Plot => "plot",
            Self::Outline => "outline",
        }
    }

    pub const fn metadata_version(self) -> &'static str {
        match self {
            Self::Character => "2",
            _ => "1",
        }
    }

    pub fn validate_path(self, path: &str) -> Result<(), ChangeError> {
        validate_writable_relative_path(path).map_err(|_| ChangeError::TargetInvalid)?;
        let prefix = format!("{}/", self.directory());
        if !path.starts_with(&prefix) || !path.ends_with(".md") {
            return Err(ChangeError::TargetInvalid);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    Create,
    Update,
    Upsert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSourceKind {
    UserDecision,
    AcceptedSuggestion,
    AiSuggestion,
    ChapterCommit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeRisk {
    Low,
    SemanticConflict,
    AmbiguousTarget,
    BroadReplacement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceReference {
    pub message_id: String,
    pub content_hash: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonMutation {
    pub kind: MutationKind,
    pub entity_kind: CanonEntityKind,
    pub capability_id: String,
    pub method_receipt_id: String,
    pub target_id: Option<String>,
    pub read_receipt_id: Option<String>,
    pub expected_source_hash: Option<String>,
    pub proposed_markdown: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposeChangeRequest {
    pub schema_version: u16,
    pub source_kind: ChangeSourceKind,
    pub sources: Vec<SourceReference>,
    pub risk: ChangeRisk,
    pub operations: Vec<CanonMutation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    Prepared,
    PendingConfirmation,
    Applying,
    Committed,
    NoChange,
    Rejected,
    Cancelled,
    RecoveryRequired,
    Undone,
}

impl ChangeStatus {
    pub const fn is_saved(self) -> bool {
        matches!(self, Self::Committed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeDecision {
    Confirm,
    Discard,
    Undo,
    RetryRecovery,
}

/// Local IPC carries references and a CAS revision, never replacement content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeDecisionRequest {
    pub project_session_id: String,
    pub change_id: String,
    pub revision: u64,
    pub decision: ChangeDecision,
}

impl ChangeDecisionRequest {
    pub fn validate(&self) -> Result<(), ChangeError> {
        if !valid_id(&self.project_session_id) || !valid_id(&self.change_id) || self.revision == 0 {
            return Err(ChangeError::RevisionConflict);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeFileReceipt {
    pub relative_path: String,
    pub before_hash: Option<String>,
    pub after_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeReceipt {
    pub schema_version: u16,
    pub change_id: String,
    pub revision: u64,
    pub status: ChangeStatus,
    pub files: Vec<ChangeFileReceipt>,
}

/// Normalized local snapshots, not Provider arguments or a continuation.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeSnapshot {
    pub entity_kind: CanonEntityKind,
    pub relative_path: String,
    pub before: Option<String>,
    pub after: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeRecord {
    pub schema_version: u16,
    pub project_id: String,
    pub conversation_session_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub request_hash: String,
    pub source_kind: ChangeSourceKind,
    pub sources: Vec<SourceReference>,
    pub receipt: ChangeReceipt,
    pub snapshots: Vec<ChangeSnapshot>,
    pub editor_revisions: BTreeMap<String, u64>,
    /// Original committed revision at the start of a local inverse transaction.
    /// Immutable snapshots stay in their forward orientation.
    pub undo_revision: Option<u64>,
}

impl ChangeRecord {
    pub fn validate(&self) -> Result<(), ChangeError> {
        if self.schema_version != CHANGE_SCHEMA_VERSION
            || self.receipt.schema_version != CHANGE_SCHEMA_VERSION
            || !valid_id(&self.project_id)
            || !valid_id(&self.conversation_session_id)
            || !valid_id(&self.turn_id)
            || !valid_id(&self.receipt.change_id)
            || self.receipt.revision == 0
            || self.undo_revision.is_some_and(|revision| {
                revision == 0
                    || revision >= self.receipt.revision
                    || !matches!(
                        self.receipt.status,
                        ChangeStatus::Applying
                            | ChangeStatus::RecoveryRequired
                            | ChangeStatus::Undone
                    )
            })
            || (self.receipt.status == ChangeStatus::Undone && self.undo_revision.is_none())
            || self.call_id.is_empty()
            || self.call_id.len() > 160
            || !self
                .call_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            || !valid_hash(&self.request_hash)
            || self.sources.is_empty()
            || self.sources.len() > MAX_CHANGE_REFERENCES
            || self.snapshots.is_empty()
            || self.snapshots.len() > MAX_CHANGE_TARGETS
            || self.receipt.files.len() != self.snapshots.len()
            || self.editor_revisions.len() > MAX_CHANGE_TARGETS
            || self.editor_revisions.iter().any(|(path, revision)| {
                *revision == 0
                    || !self
                        .snapshots
                        .iter()
                        .any(|snapshot| snapshot.relative_path == *path)
            })
        {
            return Err(ChangeError::RecoveryRequired);
        }
        let mut sources = BTreeSet::new();
        for source in &self.sources {
            if !valid_id(&source.message_id)
                || !valid_hash(&source.content_hash)
                || !sources.insert(&source.message_id)
            {
                return Err(ChangeError::RecoveryRequired);
            }
        }
        let mut paths = BTreeSet::new();
        let mut after_bytes = 0usize;
        for (snapshot, receipt) in self.snapshots.iter().zip(&self.receipt.files) {
            snapshot
                .entity_kind
                .validate_path(&snapshot.relative_path)
                .map_err(|_| ChangeError::RecoveryRequired)?;
            validate_candidate(snapshot.entity_kind, &snapshot.after)
                .map_err(|_| ChangeError::RecoveryRequired)?;
            after_bytes += snapshot.after.len();
            if after_bytes > MAX_CHANGE_BODY_BYTES
                || !paths.insert(snapshot.relative_path.to_lowercase())
                || snapshot.relative_path != receipt.relative_path
                || sha256_hex(snapshot.after.as_bytes()) != receipt.after_hash
                || snapshot
                    .before
                    .as_ref()
                    .map(|body| sha256_hex(body.as_bytes()))
                    != receipt.before_hash
                || metadata_field(&snapshot.after, "id").as_deref() == Some("new")
            {
                return Err(ChangeError::RecoveryRequired);
            }
            if let Some(before) = &snapshot.before {
                validate_candidate(snapshot.entity_kind, before)
                    .map_err(|_| ChangeError::RecoveryRequired)?;
                if metadata_field(before, "id") != metadata_field(&snapshot.after, "id") {
                    return Err(ChangeError::RecoveryRequired);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeActivity {
    pub conversation_session_id: String,
    pub turn_id: String,
    pub receipt: ChangeReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Removed,
    Added,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeDiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeFileDiff {
    pub relative_path: String,
    pub lines: Vec<ChangeDiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeDetail {
    pub activity: ChangeActivity,
    pub files: Vec<ChangeFileDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorBufferRevision {
    pub project_session_id: String,
    pub file_id: String,
    pub revision: u64,
    pub dirty: bool,
}

/// Accounting contract. Consumed before HTTP or executor side effects.
#[derive(Default)]
pub struct ChangeTurnBudget {
    agent_attempts: usize,
    read_rounds: usize,
    mutation_rounds: usize,
    compactor_used: bool,
    requested_tokens: u32,
}

impl ChangeTurnBudget {
    pub fn consume_agent(&mut self, requested_tokens: u32) -> Result<(), ChangeError> {
        if self.agent_attempts >= MAX_NR4_AGENT_ATTEMPTS
            || requested_tokens == 0
            || requested_tokens > 4096
            || self.requested_tokens + requested_tokens > NR4_CUMULATIVE_OUTPUT_TOKENS
        {
            return Err(ChangeError::BudgetExceeded);
        }
        self.agent_attempts += 1;
        self.requested_tokens += requested_tokens;
        Ok(())
    }

    pub fn consume_compactor(&mut self) -> Result<(), ChangeError> {
        if self.compactor_used || self.requested_tokens + 1024 > NR4_CUMULATIVE_OUTPUT_TOKENS {
            return Err(ChangeError::BudgetExceeded);
        }
        self.compactor_used = true;
        self.requested_tokens += 1024;
        Ok(())
    }

    pub fn consume_read(&mut self) -> Result<(), ChangeError> {
        if self.mutation_rounds != 0 || self.read_rounds >= MAX_NR4_READ_ROUNDS {
            return Err(ChangeError::BudgetExceeded);
        }
        self.read_rounds += 1;
        Ok(())
    }

    /// final_request_bytes is the fully serialized, Provider-specific next
    /// request including the bounded result envelope, not content bytes alone.
    pub fn consume_mutation(
        &mut self,
        final_request_bytes: usize,
        continuation_bytes: usize,
    ) -> Result<(), ChangeError> {
        if self.mutation_rounds >= MAX_NR4_MUTATION_ROUNDS
            || self.agent_attempts >= MAX_NR4_AGENT_ATTEMPTS
            || self.requested_tokens + 4096 > NR4_CUMULATIVE_OUTPUT_TOKENS
            || final_request_bytes > NR4_REQUEST_BYTES
            || continuation_bytes > NR4_CONTINUATION_BYTES
        {
            return Err(ChangeError::BudgetExceeded);
        }
        self.mutation_rounds += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeError {
    InvalidRequest,
    BudgetExceeded,
    SourceInvalid,
    ChapterSourceUnavailable,
    MethodNotRead,
    IncompleteRead,
    TargetInvalid,
    SourceChanged,
    CandidateInvalid,
    DuplicateTarget,
    RevisionConflict,
    RecoveryRequired,
    StoreFull,
    Io,
    WriterBusy,
    Cancelled,
}

impl ChangeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "canon.change_invalid",
            Self::BudgetExceeded => "canon.change_budget_exceeded",
            Self::SourceInvalid => "canon.change_source_invalid",
            Self::ChapterSourceUnavailable => "canon.chapter_source_unavailable",
            Self::MethodNotRead => "canon.change_method_not_read",
            Self::IncompleteRead => "canon.change_incomplete_read",
            Self::TargetInvalid => "canon.change_target_invalid",
            Self::SourceChanged => "canon.change_source_changed",
            Self::CandidateInvalid => "canon.change_candidate_invalid",
            Self::DuplicateTarget => "canon.change_duplicate_target",
            Self::RevisionConflict => "canon.change_revision_conflict",
            Self::RecoveryRequired => "canon.change_recovery_required",
            Self::StoreFull => "canon.change_store_full",
            Self::Io => "canon.change_io",
            Self::WriterBusy => "canon.change_writer_busy",
            Self::Cancelled => "canon.change_cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeEligibility {
    AutomaticEligible,
    ConfirmationRequired,
    ConversationOnly,
}

/// This is only a verified preflight, not a committed operation.
pub struct ValidatedChange {
    pub request: ProposeChangeRequest,
    pub eligibility: ChangeEligibility,
    pub request_hash: String,
}

#[derive(Clone)]
struct ReadEvidence {
    target_id: String,
    entity_kind: CanonEntityKind,
    relative_path: String,
    content: String,
    source_hash: String,
    dirty_revision: Option<u64>,
}

/// Host-owned per-Turn evidence. It is intentionally not deserializable.
#[derive(Clone)]
pub struct ChangeEvidence {
    project_id: String,
    session_id: String,
    turn_id: String,
    sources: BTreeMap<String, (ConversationRole, String)>,
    current_user_messages: BTreeSet<String>,
    methods: BTreeMap<String, String>,
    reads: BTreeMap<String, ReadEvidence>,
    source_catalog: Vec<Value>,
}

impl ChangeEvidence {
    /// Caller supplies the authoritative persisted Conversation, never tool JSON.
    pub fn from_conversation(
        session: &ConversationSession,
        turn_id: &str,
    ) -> Result<Self, ChangeError> {
        if !valid_id(&session.project_id) || !valid_id(&session.session_id) || !valid_id(turn_id) {
            return Err(ChangeError::SourceInvalid);
        }
        let turn_index = session
            .turns
            .iter()
            .position(|turn| turn.turn_id == turn_id)
            .ok_or(ChangeError::SourceInvalid)?;
        let current = &session.turns[turn_index];
        let mut sources = BTreeMap::new();
        let mut source_catalog = Vec::new();
        // References are a compact recent projection, not a copy of all History.
        for turn in session.turns[..=turn_index].iter().rev().take(8) {
            for message in turn.messages.iter().rev() {
                if sources.len() >= MAX_CHANGE_REFERENCES {
                    break;
                }
                if !valid_id(&message.message_id)
                    || sources
                        .insert(
                            message.message_id.clone(),
                            (message.role, sha256_hex(message.content.as_bytes())),
                        )
                        .is_some()
                {
                    return Err(ChangeError::SourceInvalid);
                }
                source_catalog.push(json!({
                    "messageId": message.message_id,
                    "contentHash": sha256_hex(message.content.as_bytes()),
                    "role": message.role,
                    "currentTurn": turn.turn_id == turn_id,
                    "excerpt": message.content.chars().take(160).collect::<String>(),
                    "excerptTruncated": message.content.chars().count() > 160
                }));
            }
        }
        let current_user_messages = current
            .messages
            .iter()
            .filter(|message| message.role == ConversationRole::User)
            .map(|message| message.message_id.clone())
            .collect();
        Ok(Self {
            project_id: session.project_id.clone(),
            session_id: session.session_id.clone(),
            turn_id: turn_id.to_owned(),
            sources,
            current_user_messages,
            methods: BTreeMap::new(),
            reads: BTreeMap::new(),
            source_catalog,
        })
    }

    pub fn matches_context(&self, project_id: &str, session_id: &str, turn_id: &str) -> bool {
        self.project_id == project_id && self.session_id == session_id && self.turn_id == turn_id
    }

    pub fn source_references(&self) -> Vec<SourceReference> {
        self.sources
            .iter()
            .map(|(message_id, (_, content_hash))| SourceReference {
                message_id: message_id.clone(),
                content_hash: content_hash.clone(),
            })
            .collect()
    }

    /// Bounded identifiers for actual persisted messages, not model-invented IDs.
    pub fn source_catalog(&self) -> &[Value] {
        &self.source_catalog
    }

    /// Call only after delivering the complete validated instruction resource.
    pub fn record_method(
        &mut self,
        pack: &ValidatedCapabilityPack,
        capability_id: &str,
        delivered_hash: &str,
    ) -> Result<String, ChangeError> {
        let capability = pack
            .capability(capability_id)
            .ok_or(ChangeError::MethodNotRead)?;
        if pack.resource_sha256(&capability.instruction_resource) != Some(delivered_hash) {
            return Err(ChangeError::MethodNotRead);
        }
        if self.methods.len() >= 8 {
            return Err(ChangeError::BudgetExceeded);
        }
        let receipt = Uuid::new_v4().to_string();
        self.methods
            .insert(receipt.clone(), capability_id.to_owned());
        Ok(receipt)
    }

    /// The caller must assemble a full file, then compare its hash with the
    /// authoritative disk hash. Fragmentary reads deliberately cannot qualify.
    pub fn record_complete_read(
        &mut self,
        kind: CanonEntityKind,
        path: &str,
        content: &str,
        source_hash: &str,
        dirty_revision: Option<u64>,
    ) -> Result<(String, String), ChangeError> {
        kind.validate_path(path)?;
        if content.len() > MAX_CHANGE_FILE_BYTES || self.reads.len() >= 8 {
            return Err(ChangeError::BudgetExceeded);
        }
        if sha256_hex(content.as_bytes()) != source_hash {
            return Err(ChangeError::IncompleteRead);
        }
        validate_candidate(kind, content)?;
        let target = Uuid::new_v4().to_string();
        let receipt = Uuid::new_v4().to_string();
        self.reads.insert(
            receipt.clone(),
            ReadEvidence {
                target_id: target.clone(),
                entity_kind: kind,
                relative_path: path.to_owned(),
                content: content.to_owned(),
                source_hash: source_hash.to_owned(),
                dirty_revision,
            },
        );
        Ok((target, receipt))
    }

    pub fn target_path(&self, receipt_id: &str) -> Option<&str> {
        self.reads
            .get(receipt_id)
            .map(|read| read.relative_path.as_str())
    }

    pub fn validate_request(&self, value: &Value) -> Result<ValidatedChange, ChangeError> {
        let bytes = serde_json::to_vec(value).map_err(|_| ChangeError::InvalidRequest)?;
        if bytes.len() > MAX_TOOL_ARGUMENTS_BYTES {
            return Err(ChangeError::BudgetExceeded);
        }
        // Schema enforces required nullable fields; serde alone would accept
        // omitted Option fields, which is unsafe for mutation requests.
        propose_change_definition()
            .validate_arguments(value)
            .map_err(|_| ChangeError::InvalidRequest)?;
        let request: ProposeChangeRequest =
            serde_json::from_value(value.clone()).map_err(|_| ChangeError::InvalidRequest)?;
        if request.schema_version != CHANGE_SCHEMA_VERSION
            || request.operations.is_empty()
            || request.operations.len() > MAX_CHANGE_TARGETS
            || request.sources.is_empty()
            || request.sources.len() > MAX_CHANGE_REFERENCES
        {
            return Err(ChangeError::InvalidRequest);
        }
        if request.source_kind == ChangeSourceKind::ChapterCommit {
            return Err(ChangeError::ChapterSourceUnavailable);
        }
        let mut seen = BTreeSet::new();
        let mut has_current_user = false;
        let mut has_assistant = false;
        for source in &request.sources {
            if !seen.insert(&source.message_id) {
                return Err(ChangeError::SourceInvalid);
            }
            let (role, hash) = self
                .sources
                .get(&source.message_id)
                .ok_or(ChangeError::SourceInvalid)?;
            if hash != &source.content_hash {
                return Err(ChangeError::SourceInvalid);
            }
            has_current_user |= *role == ConversationRole::User
                && self.current_user_messages.contains(&source.message_id);
            has_assistant |= *role == ConversationRole::Assistant;
        }
        if request.source_kind != ChangeSourceKind::AiSuggestion && !has_current_user
            || request.source_kind == ChangeSourceKind::AcceptedSuggestion && !has_assistant
        {
            return Err(ChangeError::SourceInvalid);
        }
        let mut eligibility = match (request.source_kind, request.risk) {
            (ChangeSourceKind::AiSuggestion, _) => ChangeEligibility::ConversationOnly,
            (_, ChangeRisk::Low) => ChangeEligibility::AutomaticEligible,
            _ => ChangeEligibility::ConfirmationRequired,
        };
        let mut targets = BTreeSet::new();
        let mut candidate_bytes = 0usize;
        for operation in &request.operations {
            if self.methods.get(&operation.method_receipt_id) != Some(&operation.capability_id)
                || operation.capability_id != operation.entity_kind.capability_id()
            {
                return Err(ChangeError::MethodNotRead);
            }
            candidate_bytes = candidate_bytes
                .checked_add(operation.proposed_markdown.len())
                .ok_or(ChangeError::BudgetExceeded)?;
            if candidate_bytes > MAX_CHANGE_BODY_BYTES {
                return Err(ChangeError::BudgetExceeded);
            }
            validate_candidate(operation.entity_kind, &operation.proposed_markdown)?;
            let has_target = operation.target_id.is_some();
            match operation.kind {
                MutationKind::Create if has_target => return Err(ChangeError::TargetInvalid),
                MutationKind::Update if !has_target => return Err(ChangeError::TargetInvalid),
                _ => {}
            }
            if !has_target {
                if operation.read_receipt_id.is_some()
                    || operation.expected_source_hash.is_some()
                    || metadata_field(&operation.proposed_markdown, "id").as_deref() != Some("new")
                {
                    return Err(ChangeError::TargetInvalid);
                }
                // New identity/path is allocated under the executor's writer lease.
                let title = metadata_field(&operation.proposed_markdown, "title")
                    .ok_or(ChangeError::CandidateInvalid)?;
                let key = format!(
                    "{}/new/{}",
                    operation.entity_kind.directory(),
                    title.to_lowercase()
                );
                if !targets.insert(key) {
                    return Err(ChangeError::DuplicateTarget);
                }
                continue;
            }
            let read = operation
                .read_receipt_id
                .as_ref()
                .and_then(|id| self.reads.get(id))
                .ok_or(ChangeError::IncompleteRead)?;
            if operation.target_id.as_deref() != Some(&read.target_id)
                || operation.entity_kind != read.entity_kind
            {
                return Err(ChangeError::TargetInvalid);
            }
            if operation.expected_source_hash.as_deref() != Some(&read.source_hash) {
                return Err(ChangeError::SourceChanged);
            }
            if !targets.insert(read.relative_path.to_lowercase()) {
                return Err(ChangeError::DuplicateTarget);
            }
            if metadata_field(&read.content, "id")
                != metadata_field(&operation.proposed_markdown, "id")
            {
                return Err(ChangeError::TargetInvalid);
            }
            if eligibility == ChangeEligibility::AutomaticEligible
                && (read.dirty_revision.is_some()
                    || structural_loss(&read.content, &operation.proposed_markdown))
            {
                eligibility = ChangeEligibility::ConfirmationRequired;
            }
        }
        Ok(ValidatedChange {
            request,
            eligibility,
            request_hash: sha256_hex(&bytes),
        })
    }
}

pub fn propose_change_definition() -> ToolDefinition {
    let string = json!({"type":"string"});
    let nullable_string = json!({"type":["string","null"]});
    let source = json!({
        "type":"object", "properties":{"messageId":string,"contentHash":string},
        "required":["messageId","contentHash"],"additionalProperties":false
    });
    let operation = json!({
        "type":"object",
        "properties":{
            "kind":{"type":"string","enum":["create","update","upsert"]},
            "entityKind":{"type":"string","enum":["character","world","material","plot","outline"]},
            "capabilityId":string,"methodReceiptId":string,
            "targetId":nullable_string,"readReceiptId":nullable_string,
            "expectedSourceHash":nullable_string,"proposedMarkdown":string
        },
        "required":["kind","entityKind","capabilityId","methodReceiptId","targetId",
            "readReceiptId","expectedSourceHash","proposedMarkdown"],
        "additionalProperties":false
    });
    ToolDefinition {
        name: PROPOSE_CHANGE_TOOL_NAME.to_owned(), version: 1,
        description: "提交有来源的资料变更。先读取能力方法及完整目标；新建使用 id: new 和三个 null 目标字段。最多四项，正文合计 8192 字节。用户明确决定可自动保存；建议不保存，冲突等确认。仅 committed 回执表示已保存。".to_owned(),
        display_label: "维护小说资料".to_owned(),
        input_schema: json!({
            "type":"object",
            "properties":{
                "schemaVersion":{"type":"integer","enum":[1]},
                "sourceKind":{"type":"string","enum":["user_decision","accepted_suggestion","ai_suggestion","chapter_commit"]},
                "sources":{"type":"array","items":source},
                "risk":{"type":"string","enum":["low","semantic_conflict","ambiguous_target","broad_replacement"]},
                "operations":{"type":"array","items":operation}
            },
            "required":["schemaVersion","sourceKind","sources","risk","operations"],
            "additionalProperties":false
        }),
    }
}

pub fn validate_candidate(kind: CanonEntityKind, content: &str) -> Result<(), ChangeError> {
    if content.is_empty()
        || content.len() > MAX_CHANGE_FILE_BYTES
        || content.starts_with('\u{feff}')
        || content.contains(['\r', '\0'])
        || !content.ends_with('\n')
    {
        return Err(ChangeError::CandidateInvalid);
    }
    let path = format!("{}/validation.md", kind.directory());
    let validation = validate_source(&path, content);
    let id = metadata_field(content, "id").ok_or(ChangeError::CandidateInvalid)?;
    if !validation.eligible_for_ai_apply
        || !validation.diagnostics.is_empty()
        || !valid_identity(&id)
        || metadata_field(content, "kind").as_deref() != Some(kind.metadata_kind())
        || metadata_field(content, "schemaVersion").as_deref() != Some(kind.metadata_version())
    {
        return Err(ChangeError::CandidateInvalid);
    }
    Ok(())
}

pub fn metadata_field(content: &str, key: &str) -> Option<String> {
    let frontmatter = content.strip_prefix("---\n")?.split_once("\n---")?.0;
    let prefix = format!("{key}:");
    let value = frontmatter
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))?
        .trim();
    if value.starts_with('"') {
        serde_json::from_str::<String>(value).ok()
    } else if let Some(quoted) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        Some(quoted.replace("''", "'"))
    } else if !value.is_empty() && !value.contains(['[', ']', '{', '}', '#']) {
        Some(value.to_owned())
    } else {
        None
    }
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn valid_id(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn structural_loss(before: &str, after: &str) -> bool {
    let fields = |content: &str| -> BTreeSet<String> {
        content
            .strip_prefix("---\n")
            .and_then(|v| v.split_once("\n---"))
            .map(|(frontmatter, _)| {
                frontmatter
                    .lines()
                    .filter(|line| !line.starts_with(char::is_whitespace))
                    .filter_map(|line| line.split_once(':').map(|(key, _)| key.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    };
    !fields(before).is_subset(&fields(after))
        || before
            .lines()
            .filter(|line| line.starts_with("## "))
            .any(|heading| !after.lines().any(|line| line == heading))
        || after.len().saturating_mul(2) < before.len()
}

#[cfg(test)]
mod tests;
