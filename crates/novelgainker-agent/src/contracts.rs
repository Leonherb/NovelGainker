use crate::SKILL_IDS;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

pub const AGENT_CONTRACT_SCHEMA_VERSION: u16 = 1;
pub const SESSION_SUMMARY_MAX_UTF8_BYTES: u32 = 32 * 1024;
pub const RECENT_TURN_INDEX_LIMIT: usize = 100;
pub const MAX_AGENT_STEPS_PER_TURN: u16 = 8;
pub const MAX_TOOL_CALLS_PER_TURN: u16 = 16;
pub const MAX_TOOL_CALLS_PER_BATCH: u16 = 8;
pub const MAX_UNIQUE_PROJECT_FILES_PER_TURN: u16 = 24;
pub const MAX_MODIFIED_PROJECT_FILES_PER_TURN: u16 = 8;
pub const MAX_WRITE_TRANSACTIONS_PER_TURN: u16 = 1;
pub const MAX_PROVIDER_REQUEST_BYTES: u32 = 96 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    InvalidSchemaVersion,
    InvalidField(&'static str),
    InvalidBudget,
    UnknownSkill,
    DuplicateSkill,
    ZeroSkillNotAllowed,
    CrossProjectReference,
    SessionMismatch,
    SessionNotActive,
    TurnAlreadyActive,
    TurnNotActive,
    WriterUnavailable,
    InvalidTransition,
    TerminalStateReuse,
    SnapshotDrift,
    RecoveryNotRequired,
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::InvalidSchemaVersion => "agent-contract.invalid-schema-version",
            Self::InvalidField(field) => {
                return write!(formatter, "agent-contract.invalid-field:{field}")
            }
            Self::InvalidBudget => "agent-contract.invalid-budget",
            Self::UnknownSkill => "agent-contract.unknown-skill",
            Self::DuplicateSkill => "agent-contract.duplicate-skill",
            Self::ZeroSkillNotAllowed => "agent-contract.zero-skill-not-allowed",
            Self::CrossProjectReference => "agent-contract.cross-project-reference",
            Self::SessionMismatch => "agent-contract.session-mismatch",
            Self::SessionNotActive => "agent-contract.session-not-active",
            Self::TurnAlreadyActive => "agent-contract.turn-already-active",
            Self::TurnNotActive => "agent-contract.turn-not-active",
            Self::WriterUnavailable => "agent-contract.writer-unavailable",
            Self::InvalidTransition => "agent-contract.invalid-transition",
            Self::TerminalStateReuse => "agent-contract.terminal-state-reuse",
            Self::SnapshotDrift => "agent-contract.snapshot-drift",
            Self::RecoveryNotRequired => "agent-contract.recovery-not-required",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for ContractError {}

fn valid_opaque_id(value: &str, prefix: &str) -> bool {
    value.len() >= prefix.len() + 8
        && value.len() <= 96
        && value.starts_with(prefix)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
        })
}

macro_rules! opaque_id {
    ($name:ident, $prefix:literal, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
                let value = value.into();
                if !valid_opaque_id(&value, $prefix) {
                    return Err(ContractError::InvalidField($field));
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

macro_rules! bounded_token {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
                let value = value.into();
                if !valid_token(&value) {
                    return Err(ContractError::InvalidField($field));
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

opaque_id!(ProjectIdV1, "prj_", "projectId");
opaque_id!(SessionIdV1, "ses_", "sessionId");
opaque_id!(TurnIdV1, "trn_", "turnId");
opaque_id!(SummaryIdV1, "sum_", "summaryId");

bounded_token!(ProviderIdV1, "providerId");
bounded_token!(PresetVersionV1, "presetVersion");
bounded_token!(ModelIdV1, "modelId");
bounded_token!(SkillPackVersionV1, "skillPackVersion");
bounded_token!(SkillIdV1, "skillId");
bounded_token!(ProjectReferenceIdV1, "referenceId");

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ContentHashV1(String);

impl ContentHashV1 {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ContractError::InvalidField("contentHash"));
        }
        Ok(Self(value))
    }

    pub fn digest(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContentHashV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EpochMillisV1(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationPolicyV1 {
    PlanOnly,
    ReviewEveryWrite,
    AutoApplySafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatusV1 {
    Active,
    RecoveryRequired,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionTerminationReasonV1 {
    ProjectClosed,
    UserEnded,
    FatalContractError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStateV1 {
    Idle,
    Planning,
    Reading,
    Acting,
    AwaitingHighRiskConfirmation,
    Applying,
    Validating,
    Completed,
    Cancelled,
    Failed,
    RecoveryRequired,
}

impl TurnStateV1 {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::RecoveryRequired
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnIntentV1 {
    Discussion,
    Clarification,
    ProductHelp,
    ProjectWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectReferenceKindV1 {
    Summary,
    ToolResult,
    Transaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnFailureCodeV1 {
    ProviderSnapshotDrift,
    SkillPackSnapshotDrift,
    AuthorizationPolicySnapshotDrift,
    ContractViolation,
    WriteRecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSnapshotV1 {
    pub provider_id: ProviderIdV1,
    pub preset_version: PresetVersionV1,
    pub model_id: ModelIdV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillPackSnapshotV1 {
    pub version: SkillPackVersionV1,
    pub hash: ContentHashV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnBudgetV1 {
    pub max_agent_steps: u16,
    pub max_tool_calls: u16,
    pub max_tool_calls_per_batch: u16,
    pub max_unique_project_files: u16,
    pub max_modified_project_files: u16,
    pub max_write_transactions: u16,
    pub max_provider_request_bytes: u32,
}

impl Default for TurnBudgetV1 {
    fn default() -> Self {
        Self {
            max_agent_steps: MAX_AGENT_STEPS_PER_TURN,
            max_tool_calls: MAX_TOOL_CALLS_PER_TURN,
            max_tool_calls_per_batch: MAX_TOOL_CALLS_PER_BATCH,
            max_unique_project_files: MAX_UNIQUE_PROJECT_FILES_PER_TURN,
            max_modified_project_files: MAX_MODIFIED_PROJECT_FILES_PER_TURN,
            max_write_transactions: MAX_WRITE_TRANSACTIONS_PER_TURN,
            max_provider_request_bytes: MAX_PROVIDER_REQUEST_BYTES,
        }
    }
}

impl TurnBudgetV1 {
    pub fn validate(self) -> Result<(), ContractError> {
        let within_limit = self.max_agent_steps > 0
            && self.max_agent_steps <= MAX_AGENT_STEPS_PER_TURN
            && self.max_tool_calls > 0
            && self.max_tool_calls <= MAX_TOOL_CALLS_PER_TURN
            && self.max_tool_calls_per_batch > 0
            && self.max_tool_calls_per_batch <= MAX_TOOL_CALLS_PER_BATCH
            && self.max_tool_calls_per_batch <= self.max_tool_calls
            && self.max_unique_project_files > 0
            && self.max_unique_project_files <= MAX_UNIQUE_PROJECT_FILES_PER_TURN
            && self.max_modified_project_files > 0
            && self.max_modified_project_files <= MAX_MODIFIED_PROJECT_FILES_PER_TURN
            && self.max_modified_project_files <= self.max_unique_project_files
            && self.max_write_transactions == MAX_WRITE_TRANSACTIONS_PER_TURN
            && self.max_provider_request_bytes > 0
            && self.max_provider_request_bytes <= MAX_PROVIDER_REQUEST_BYTES;
        if !within_limit {
            return Err(ContractError::InvalidBudget);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenTurnSnapshotV1 {
    pub provider: ProviderSnapshotV1,
    pub skill_pack: SkillPackSnapshotV1,
    pub authorization_policy: AuthorizationPolicyV1,
    pub user_request_hash: ContentHashV1,
    pub budget: TurnBudgetV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSummaryRefV1 {
    pub project_id: ProjectIdV1,
    pub summary_id: SummaryIdV1,
    pub version: u32,
    pub content_hash: ContentHashV1,
    pub utf8_bytes: u32,
}

impl SessionSummaryRefV1 {
    fn validate_for_project(&self, project_id: &ProjectIdV1) -> Result<(), ContractError> {
        if &self.project_id != project_id {
            return Err(ContractError::CrossProjectReference);
        }
        if self.version == 0 || self.utf8_bytes > SESSION_SUMMARY_MAX_UTF8_BYTES {
            return Err(ContractError::InvalidField("summary"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnIndexEntryV1 {
    pub project_id: ProjectIdV1,
    pub turn_id: TurnIdV1,
    pub state: TurnStateV1,
    pub updated_at_ms: EpochMillisV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WriterLeaseV1 {
    Released,
    Held { turn_id: TurnIdV1 },
    RecoveryRequired { turn_id: TurnIdV1 },
}

impl WriterLeaseV1 {
    pub fn turn_id(&self) -> Option<&TurnIdV1> {
        match self {
            Self::Released => None,
            Self::Held { turn_id } | Self::RecoveryRequired { turn_id } => Some(turn_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAgentSessionV1 {
    pub schema_version: u16,
    pub session_id: SessionIdV1,
    pub project_id: ProjectIdV1,
    pub status: SessionStatusV1,
    pub provider: ProviderSnapshotV1,
    pub skill_pack: SkillPackSnapshotV1,
    pub authorization_policy: AuthorizationPolicyV1,
    pub summary: Option<SessionSummaryRefV1>,
    pub recent_turns: Vec<TurnIndexEntryV1>,
    pub active_turn_id: Option<TurnIdV1>,
    pub writer_lease: WriterLeaseV1,
    pub created_at_ms: EpochMillisV1,
    pub updated_at_ms: EpochMillisV1,
    pub termination_reason: Option<SessionTerminationReasonV1>,
}

impl ProjectAgentSessionV1 {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != AGENT_CONTRACT_SCHEMA_VERSION {
            return Err(ContractError::InvalidSchemaVersion);
        }
        if self.updated_at_ms < self.created_at_ms {
            return Err(ContractError::InvalidField("updatedAtMs"));
        }
        if let Some(summary) = &self.summary {
            summary.validate_for_project(&self.project_id)?;
        }
        if self.recent_turns.len() > RECENT_TURN_INDEX_LIMIT {
            return Err(ContractError::InvalidField("recentTurns"));
        }
        let mut turn_ids = BTreeSet::new();
        for entry in &self.recent_turns {
            if entry.project_id != self.project_id {
                return Err(ContractError::CrossProjectReference);
            }
            if !entry.state.is_terminal() || !turn_ids.insert(&entry.turn_id) {
                return Err(ContractError::InvalidField("recentTurns"));
            }
        }

        match self.status {
            SessionStatusV1::Active => {
                if self.termination_reason.is_some()
                    || matches!(self.writer_lease, WriterLeaseV1::RecoveryRequired { .. })
                {
                    return Err(ContractError::InvalidField("status"));
                }
            }
            SessionStatusV1::RecoveryRequired => {
                if self.termination_reason.is_some()
                    || !matches!(self.writer_lease, WriterLeaseV1::RecoveryRequired { .. })
                    || self.active_turn_id.is_none()
                {
                    return Err(ContractError::InvalidField("status"));
                }
            }
            SessionStatusV1::Terminated => {
                if self.termination_reason.is_none()
                    || self.active_turn_id.is_some()
                    || !matches!(self.writer_lease, WriterLeaseV1::Released)
                {
                    return Err(ContractError::InvalidField("status"));
                }
            }
        }

        match (&self.active_turn_id, self.writer_lease.turn_id()) {
            (None, None) => {}
            (Some(_), None) => {}
            (Some(active), Some(writer)) if active == writer => {}
            _ => return Err(ContractError::WriterUnavailable),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectScopedReferenceV1 {
    pub project_id: ProjectIdV1,
    pub kind: ProjectReferenceKindV1,
    pub reference_id: ProjectReferenceIdV1,
    pub content_hash: ContentHashV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnFailureV1 {
    pub code: TurnFailureCodeV1,
    pub detail_hash: ContentHashV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTurnV1 {
    pub schema_version: u16,
    pub turn_id: TurnIdV1,
    pub session_id: SessionIdV1,
    pub project_id: ProjectIdV1,
    pub state: TurnStateV1,
    pub intent: TurnIntentV1,
    pub selected_skill_ids: Vec<SkillIdV1>,
    pub frozen: FrozenTurnSnapshotV1,
    pub references: Vec<ProjectScopedReferenceV1>,
    pub cancellation_requested: bool,
    pub write_transaction_started: bool,
    pub failure: Option<TurnFailureV1>,
    pub created_at_ms: EpochMillisV1,
    pub updated_at_ms: EpochMillisV1,
}

impl AgentTurnV1 {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != AGENT_CONTRACT_SCHEMA_VERSION {
            return Err(ContractError::InvalidSchemaVersion);
        }
        if self.updated_at_ms < self.created_at_ms {
            return Err(ContractError::InvalidField("updatedAtMs"));
        }
        self.frozen.budget.validate()?;
        ProjectAgentBasePolicyV1::locked()
            .validate_skill_selection(self.intent, &self.selected_skill_ids)?;

        let mut reference_keys = BTreeSet::new();
        for reference in &self.references {
            if reference.project_id != self.project_id {
                return Err(ContractError::CrossProjectReference);
            }
            if !reference_keys.insert((reference.kind, &reference.reference_id)) {
                return Err(ContractError::InvalidField("references"));
            }
        }

        if matches!(
            self.state,
            TurnStateV1::Applying | TurnStateV1::Validating | TurnStateV1::RecoveryRequired
        ) && !self.write_transaction_started
        {
            return Err(ContractError::InvalidField("writeTransactionStarted"));
        }
        if self.write_transaction_started
            && matches!(self.state, TurnStateV1::Cancelled | TurnStateV1::Failed)
        {
            return Err(ContractError::InvalidField("state"));
        }
        if self.state == TurnStateV1::Cancelled && !self.cancellation_requested {
            return Err(ContractError::InvalidField("cancellationRequested"));
        }
        if matches!(
            self.state,
            TurnStateV1::Failed | TurnStateV1::RecoveryRequired
        ) != self.failure.is_some()
        {
            return Err(ContractError::InvalidField("failure"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosedProjectActionV1 {
    CreateProject,
    OpenProject,
    ProductHelp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectAgentBasePolicyV1 {
    pub schema_version: u16,
    pub policy_id: String,
    pub is_skill: bool,
    pub requires_project_session: bool,
    pub allows_bypass_chat: bool,
    pub allows_zero_skill_direct_response: bool,
    pub allowed_without_open_project: Vec<ClosedProjectActionV1>,
}

impl ProjectAgentBasePolicyV1 {
    pub fn locked() -> Self {
        Self {
            schema_version: AGENT_CONTRACT_SCHEMA_VERSION,
            policy_id: "project-agent-base-policy-v1".to_owned(),
            is_skill: false,
            requires_project_session: true,
            allows_bypass_chat: false,
            allows_zero_skill_direct_response: true,
            allowed_without_open_project: vec![
                ClosedProjectActionV1::CreateProject,
                ClosedProjectActionV1::OpenProject,
                ClosedProjectActionV1::ProductHelp,
            ],
        }
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self != &Self::locked() || SKILL_IDS.contains(&self.policy_id.as_str()) {
            return Err(ContractError::InvalidField("basePolicy"));
        }
        Ok(())
    }

    pub fn validate_skill_selection(
        &self,
        intent: TurnIntentV1,
        selected_skill_ids: &[SkillIdV1],
    ) -> Result<(), ContractError> {
        self.validate()?;
        if selected_skill_ids.is_empty()
            && !matches!(
                intent,
                TurnIntentV1::Discussion | TurnIntentV1::Clarification | TurnIntentV1::ProductHelp
            )
        {
            return Err(ContractError::ZeroSkillNotAllowed);
        }
        if selected_skill_ids.len() > SKILL_IDS.len() {
            return Err(ContractError::InvalidField("selectedSkillIds"));
        }
        let mut unique = BTreeSet::new();
        for skill_id in selected_skill_ids {
            if !SKILL_IDS.contains(&skill_id.as_str()) {
                return Err(ContractError::UnknownSkill);
            }
            if !unique.insert(skill_id) {
                return Err(ContractError::DuplicateSkill);
            }
        }
        Ok(())
    }
}
