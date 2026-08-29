use crate::action_contracts::ProjectRelativePathV1;
use crate::authorization::{GovernanceErrorCodeV1, GOVERNANCE_SCHEMA_VERSION};
use crate::contracts::{
    AuthorizationPolicyV1, ContentHashV1, EpochMillisV1, ProjectIdV1, SessionIdV1, SummaryIdV1,
    TurnIdV1, TurnStateV1, RECENT_TURN_INDEX_LIMIT, SESSION_SUMMARY_MAX_UTF8_BYTES,
};
use crate::transaction_contracts::{MultiFileTransactionPlanV1, MAX_UNDO_BYTES_PER_TURN};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;

pub const MESSAGE_INDEX_LIMIT: usize = 200;
pub const UNDO_RECORD_LIMIT: usize = 20;
pub const UNDO_RETENTION_DAYS: u64 = 30;
pub const UNDO_RETENTION_MS: u64 = UNDO_RETENTION_DAYS * 24 * 60 * 60 * 1000;
pub const MAX_UNDO_BYTES_PER_PROJECT: u32 = 20 * 1024 * 1024;
pub const MAX_FULL_CHAT_BYTES_PER_PROJECT: u32 = 20 * 1024 * 1024;
pub const MAX_FULL_CHAT_RETENTION_DAYS: u16 = 30;

macro_rules! governance_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, GovernanceErrorCodeV1> {
                let value = value.into();
                if value.len() < $prefix.len() + 8
                    || value.len() > 96
                    || !value.starts_with($prefix)
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                {
                    return Err(GovernanceErrorCodeV1::InvalidField);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

governance_id!(MessageIdV1, "msg_");
governance_id!(UndoBlobIdV1, "ubl_");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRoleV1 {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedSessionSummaryV1 {
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub summary_id: SummaryIdV1,
    pub version: u32,
    pub text: String,
    pub content_hash: ContentHashV1,
    pub utf8_bytes: u32,
}

impl PersistedSessionSummaryV1 {
    pub fn validate(
        &self,
        project_id: &ProjectIdV1,
        session_id: &SessionIdV1,
    ) -> Result<(), GovernanceErrorCodeV1> {
        if &self.project_id != project_id || &self.session_id != session_id {
            return Err(GovernanceErrorCodeV1::CrossProjectReference);
        }
        if self.version == 0
            || self.text.is_empty()
            || self.text.len() > SESSION_SUMMARY_MAX_UTF8_BYTES as usize
            || self.utf8_bytes != self.text.len() as u32
            || self.content_hash != ContentHashV1::digest(self.text.as_bytes())
        {
            return Err(GovernanceErrorCodeV1::PersistenceLimitExceeded);
        }
        reject_sensitive_text(&self.text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageIndexEntryV1 {
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub message_id: MessageIdV1,
    pub turn_id: TurnIdV1,
    pub ordinal: u32,
    pub role: MessageRoleV1,
    pub content_hash: ContentHashV1,
    pub utf8_bytes: u32,
    pub created_at_ms: EpochMillisV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedTurnMetadataV1 {
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub turn_id: TurnIdV1,
    pub terminal_state: TurnStateV1,
    pub skill_count: u16,
    pub action_count: u16,
    pub tool_call_count: u16,
    pub modified_file_count: u16,
    pub action_plan_hash: ContentHashV1,
    pub transaction_hash: Option<ContentHashV1>,
    pub updated_at_ms: EpochMillisV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullChatProtectionV1 {
    Disabled,
    WindowsUserContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FullChatPersistenceV1 {
    pub enabled: bool,
    pub user_opt_in: bool,
    pub protection: FullChatProtectionV1,
    pub max_utf8_bytes: u32,
    pub retention_days: u16,
    pub privacy_notice_version: Option<u16>,
    pub clear_generation: u32,
}

impl Default for FullChatPersistenceV1 {
    fn default() -> Self {
        Self {
            enabled: false,
            user_opt_in: false,
            protection: FullChatProtectionV1::Disabled,
            max_utf8_bytes: 0,
            retention_days: 0,
            privacy_notice_version: None,
            clear_generation: 0,
        }
    }
}

impl FullChatPersistenceV1 {
    pub fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if !self.enabled {
            if self.user_opt_in
                || self.protection != FullChatProtectionV1::Disabled
                || self.max_utf8_bytes != 0
                || self.retention_days != 0
                || self.privacy_notice_version.is_some()
            {
                return Err(GovernanceErrorCodeV1::PlaintextDowngradeForbidden);
            }
            return Ok(());
        }
        if !self.user_opt_in
            || self.protection != FullChatProtectionV1::WindowsUserContext
            || self.max_utf8_bytes == 0
            || self.max_utf8_bytes > MAX_FULL_CHAT_BYTES_PER_PROJECT
            || self.retention_days == 0
            || self.retention_days > MAX_FULL_CHAT_RETENTION_DAYS
            || self.privacy_notice_version.unwrap_or(0) == 0
        {
            return Err(GovernanceErrorCodeV1::PlaintextDowngradeForbidden);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionPersistenceSnapshotV1 {
    pub schema_version: u16,
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub authorization_policy: AuthorizationPolicyV1,
    pub summary: Option<PersistedSessionSummaryV1>,
    pub message_index: Vec<MessageIndexEntryV1>,
    pub terminal_turns: Vec<PersistedTurnMetadataV1>,
    pub full_chat: FullChatPersistenceV1,
    pub clear_generation: u32,
}

impl SessionPersistenceSnapshotV1 {
    pub fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if self.schema_version != GOVERNANCE_SCHEMA_VERSION {
            return Err(GovernanceErrorCodeV1::InvalidSchemaVersion);
        }
        if self.message_index.len() > MESSAGE_INDEX_LIMIT
            || self.terminal_turns.len() > RECENT_TURN_INDEX_LIMIT
        {
            return Err(GovernanceErrorCodeV1::PersistenceLimitExceeded);
        }
        if let Some(summary) = &self.summary {
            summary.validate(&self.project_id, &self.session_id)?;
        }
        self.full_chat.validate()?;
        if self.full_chat.clear_generation != self.clear_generation {
            return Err(GovernanceErrorCodeV1::InvalidField);
        }

        let mut message_ids = BTreeSet::new();
        let mut message_ordinals = BTreeSet::new();
        for message in &self.message_index {
            if message.project_id != self.project_id
                || message.session_id != self.session_id
                || message.ordinal == 0
                || !message_ids.insert(&message.message_id)
                || !message_ordinals.insert(message.ordinal)
            {
                return Err(GovernanceErrorCodeV1::CrossProjectReference);
            }
        }
        let mut turn_ids = BTreeSet::new();
        for turn in &self.terminal_turns {
            if turn.project_id != self.project_id
                || turn.session_id != self.session_id
                || !turn.terminal_state.is_terminal()
                || !turn_ids.insert(&turn.turn_id)
            {
                return Err(GovernanceErrorCodeV1::CrossProjectReference);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionClearReceiptV1 {
    pub schema_version: u16,
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub previous_clear_generation: u32,
    pub next_clear_generation: u32,
    pub summaries_removed: u16,
    pub message_indexes_removed: u16,
    pub terminal_turns_removed: u16,
    pub undo_records_removed: u16,
    pub canon_write_count: u16,
}

impl SessionClearReceiptV1 {
    pub fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if self.schema_version != GOVERNANCE_SCHEMA_VERSION
            || self.next_clear_generation != self.previous_clear_generation.saturating_add(1)
            || self.canon_write_count != 0
        {
            return Err(GovernanceErrorCodeV1::InvalidField);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UndoProtectionV1 {
    WindowsUserContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UndoRecordStatusV1 {
    Ready,
    Consumed,
    Expired,
    HashConflict,
    DecryptFailed,
    Cleared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoTargetV1 {
    pub path: ProjectRelativePathV1,
    pub expected_current_hash: ContentHashV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoRecordV1 {
    pub schema_version: u16,
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub turn_id: TurnIdV1,
    pub transaction_hash: ContentHashV1,
    pub blob_id: UndoBlobIdV1,
    pub blob_hash: ContentHashV1,
    pub encrypted_bytes: u32,
    pub protection: UndoProtectionV1,
    pub targets: Vec<UndoTargetV1>,
    pub target_set_hash: ContentHashV1,
    pub created_at_ms: EpochMillisV1,
    pub expires_at_ms: EpochMillisV1,
    pub status: UndoRecordStatusV1,
    pub clear_generation: u32,
}

impl UndoRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        project_id: ProjectIdV1,
        session_id: SessionIdV1,
        turn_id: TurnIdV1,
        transaction_hash: ContentHashV1,
        blob_id: UndoBlobIdV1,
        blob_hash: ContentHashV1,
        encrypted_bytes: u32,
        mut targets: Vec<UndoTargetV1>,
        created_at_ms: EpochMillisV1,
        expires_at_ms: EpochMillisV1,
        clear_generation: u32,
    ) -> Result<Self, GovernanceErrorCodeV1> {
        targets.sort_by(|left, right| left.path.cmp(&right.path));
        let target_set_hash = target_set_hash(&targets)?;
        let record = Self {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            project_id,
            session_id,
            turn_id,
            transaction_hash,
            blob_id,
            blob_hash,
            encrypted_bytes,
            protection: UndoProtectionV1::WindowsUserContext,
            targets,
            target_set_hash,
            created_at_ms,
            expires_at_ms,
            status: UndoRecordStatusV1::Ready,
            clear_generation,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if self.schema_version != GOVERNANCE_SCHEMA_VERSION {
            return Err(GovernanceErrorCodeV1::InvalidSchemaVersion);
        }
        if self.encrypted_bytes == 0 || self.encrypted_bytes > MAX_UNDO_BYTES_PER_TURN {
            return Err(GovernanceErrorCodeV1::UndoLimitExceeded);
        }
        if self.expires_at_ms <= self.created_at_ms
            || self.expires_at_ms.0.saturating_sub(self.created_at_ms.0) > UNDO_RETENTION_MS
            || self.target_set_hash != target_set_hash(&self.targets)?
        {
            return Err(GovernanceErrorCodeV1::InvalidField);
        }
        Ok(())
    }
}

fn target_set_hash(targets: &[UndoTargetV1]) -> Result<ContentHashV1, GovernanceErrorCodeV1> {
    if targets.is_empty() {
        return Err(GovernanceErrorCodeV1::InvalidField);
    }
    let mut paths = BTreeSet::new();
    if !targets.iter().all(|target| paths.insert(&target.path)) {
        return Err(GovernanceErrorCodeV1::InvalidField);
    }
    if !targets.windows(2).all(|pair| pair[0].path < pair[1].path) {
        return Err(GovernanceErrorCodeV1::InvalidField);
    }
    Ok(ContentHashV1::digest(
        &serde_json::to_vec(targets).expect("undo target serialization is infallible"),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoCatalogV1 {
    pub schema_version: u16,
    pub project_id: ProjectIdV1,
    pub records: Vec<UndoRecordV1>,
    pub total_encrypted_bytes: u32,
    pub clear_generation: u32,
}

impl UndoCatalogV1 {
    pub fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if self.schema_version != GOVERNANCE_SCHEMA_VERSION {
            return Err(GovernanceErrorCodeV1::InvalidSchemaVersion);
        }
        if self.records.len() > UNDO_RECORD_LIMIT {
            return Err(GovernanceErrorCodeV1::UndoLimitExceeded);
        }
        let mut turns = BTreeSet::new();
        let mut blobs = BTreeSet::new();
        let mut total = 0_u32;
        for record in &self.records {
            record.validate()?;
            if record.project_id != self.project_id
                || record.clear_generation != self.clear_generation
                || !turns.insert(&record.turn_id)
                || !blobs.insert(&record.blob_id)
            {
                return Err(GovernanceErrorCodeV1::CrossProjectReference);
            }
            total = total
                .checked_add(record.encrypted_bytes)
                .ok_or(GovernanceErrorCodeV1::UndoLimitExceeded)?;
        }
        if total != self.total_encrypted_bytes || total > MAX_UNDO_BYTES_PER_PROJECT {
            return Err(GovernanceErrorCodeV1::UndoLimitExceeded);
        }
        Ok(())
    }

    pub fn admits_auto_apply(
        &self,
        plan: &MultiFileTransactionPlanV1,
        prepared_record: &UndoRecordV1,
    ) -> bool {
        self.validate().is_ok()
            && plan.validate().is_ok()
            && prepared_record.validate().is_ok()
            && prepared_record.status == UndoRecordStatusV1::Ready
            && prepared_record.project_id == self.project_id
            && prepared_record.project_id == plan.project_id
            && prepared_record.session_id == plan.session_id
            && prepared_record.turn_id == plan.turn_id
            && prepared_record.transaction_hash == plan.transaction_hash
            && prepared_record.clear_generation == self.clear_generation
            && prepared_record.encrypted_bytes > 0
            && prepared_record.encrypted_bytes <= plan.total_estimated_undo_bytes
            && prepared_record.targets.len() == plan.mutations.len()
            && prepared_record
                .targets
                .iter()
                .zip(&plan.mutations)
                .all(|(target, mutation)| {
                    target.path == mutation.frozen.path
                        && target.expected_current_hash == mutation.planned_result_hash
                })
            && !self
                .records
                .iter()
                .any(|record| record.turn_id == prepared_record.turn_id)
            && !self
                .records
                .iter()
                .any(|record| record.blob_id == prepared_record.blob_id)
            && self.records.len() < UNDO_RECORD_LIMIT
            && self
                .total_encrypted_bytes
                .checked_add(prepared_record.encrypted_bytes)
                .is_some_and(|total| total <= MAX_UNDO_BYTES_PER_PROJECT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UndoEligibilityV1 {
    Ready,
    Expired,
    HashConflict,
    DecryptFailed,
    Cleared,
    AlreadyConsumed,
}

pub fn evaluate_undo(
    record: &UndoRecordV1,
    current_target_set_hash: &ContentHashV1,
    can_decrypt: bool,
    now_ms: EpochMillisV1,
    current_clear_generation: u32,
) -> Result<UndoEligibilityV1, GovernanceErrorCodeV1> {
    record.validate()?;
    if record.clear_generation != current_clear_generation
        || record.status == UndoRecordStatusV1::Cleared
    {
        return Ok(UndoEligibilityV1::Cleared);
    }
    if record.status == UndoRecordStatusV1::Consumed {
        return Ok(UndoEligibilityV1::AlreadyConsumed);
    }
    if now_ms >= record.expires_at_ms || record.status == UndoRecordStatusV1::Expired {
        return Ok(UndoEligibilityV1::Expired);
    }
    if !can_decrypt || record.status == UndoRecordStatusV1::DecryptFailed {
        return Ok(UndoEligibilityV1::DecryptFailed);
    }
    if current_target_set_hash != &record.target_set_hash
        || record.status == UndoRecordStatusV1::HashConflict
    {
        return Ok(UndoEligibilityV1::HashConflict);
    }
    Ok(UndoEligibilityV1::Ready)
}

pub fn reject_sensitive_text(value: &str) -> Result<(), GovernanceErrorCodeV1> {
    let lower = value.to_ascii_lowercase();
    let forbidden = [
        "api_key",
        "apikey",
        "authorization:",
        "bearer ",
        "credential_ref",
        "rawproviderresponse",
        "raw_provider_response",
        "<think",
        "sk-secret",
    ];
    if forbidden.iter().any(|marker| lower.contains(marker))
        || has_provider_key_shape(value)
        || has_windows_absolute_path(value)
        || value.contains("\\\\")
    {
        return Err(GovernanceErrorCodeV1::SensitiveDataForbidden);
    }
    Ok(())
}

fn has_provider_key_shape(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.windows(3).enumerate().any(|(index, prefix)| {
        prefix.eq_ignore_ascii_case(b"sk-")
            && (index == 0 || !bytes[index - 1].is_ascii_alphanumeric())
            && bytes[index + 3..]
                .iter()
                .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                .count()
                >= 16
    })
}

fn has_windows_absolute_path(value: &str) -> bool {
    value.as_bytes().windows(3).any(|window| {
        window[0].is_ascii_alphabetic() && window[1] == b':' && matches!(window[2], b'/' | b'\\')
    })
}
