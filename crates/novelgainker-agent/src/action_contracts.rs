use crate::contracts::{
    ContentHashV1, EpochMillisV1, ProjectIdV1, ProviderSnapshotV1, SessionIdV1, SkillIdV1,
    SkillPackSnapshotV1, TurnIdV1,
};
use crate::SKILL_IDS;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub const AGENT_ACTION_SCHEMA_VERSION: u16 = 1;
pub const MAX_VISIBLE_TEXT_UTF8_BYTES: usize = 32 * 1024;
pub const MAX_TOOL_PURPOSE_UTF8_BYTES: usize = 512;
pub const MAX_TOOL_CONTENT_UTF8_BYTES: usize = 64 * 1024;
pub const MAX_SEARCH_QUERY_UTF8_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentActionErrorCodeV1 {
    UnsupportedVersion,
    InvalidPayload,
    UnknownSkill,
    DuplicateSkill,
    InvalidCallId,
    DuplicateCallId,
    UnknownTool,
    ToolArgumentsMismatch,
    AbsolutePathForbidden,
    PathTraversalForbidden,
    ForbiddenAuthorityField,
    EmptyToolBatch,
    ToolBatchLimitExceeded,
    AgentStepBudgetExceeded,
    ToolCallBudgetExceeded,
    ProjectFileBudgetExceeded,
    ModifiedFileBudgetExceeded,
    WriteTransactionBudgetExceeded,
    ProviderRequestBudgetExceeded,
    CrossProviderFallbackForbidden,
    RepairExhausted,
    ActionChainInvalid,
    ToolResultMismatch,
    InvalidToolResult,
}

impl AgentActionErrorCodeV1 {
    pub fn stable_code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "agent-action.unsupported-version",
            Self::InvalidPayload => "agent-action.invalid-payload",
            Self::UnknownSkill => "agent-action.unknown-skill",
            Self::DuplicateSkill => "agent-action.duplicate-skill",
            Self::InvalidCallId => "agent-action.invalid-call-id",
            Self::DuplicateCallId => "agent-action.duplicate-call-id",
            Self::UnknownTool => "agent-action.unknown-tool",
            Self::ToolArgumentsMismatch => "agent-action.tool-arguments-mismatch",
            Self::AbsolutePathForbidden => "agent-action.absolute-path-forbidden",
            Self::PathTraversalForbidden => "agent-action.path-traversal-forbidden",
            Self::ForbiddenAuthorityField => "agent-action.forbidden-authority-field",
            Self::EmptyToolBatch => "agent-action.empty-tool-batch",
            Self::ToolBatchLimitExceeded => "agent-action.tool-batch-limit-exceeded",
            Self::AgentStepBudgetExceeded => "agent-action.agent-step-budget-exceeded",
            Self::ToolCallBudgetExceeded => "agent-action.tool-call-budget-exceeded",
            Self::ProjectFileBudgetExceeded => "agent-action.project-file-budget-exceeded",
            Self::ModifiedFileBudgetExceeded => "agent-action.modified-file-budget-exceeded",
            Self::WriteTransactionBudgetExceeded => {
                "agent-action.write-transaction-budget-exceeded"
            }
            Self::ProviderRequestBudgetExceeded => "agent-action.provider-request-budget-exceeded",
            Self::CrossProviderFallbackForbidden => {
                "agent-action.cross-provider-fallback-forbidden"
            }
            Self::RepairExhausted => "agent-action.repair-exhausted",
            Self::ActionChainInvalid => "agent-action.action-chain-invalid",
            Self::ToolResultMismatch => "agent-action.tool-result-mismatch",
            Self::InvalidToolResult => "agent-action.invalid-tool-result",
        }
    }
}

impl fmt::Display for AgentActionErrorCodeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

impl std::error::Error for AgentActionErrorCodeV1 {}

fn valid_opaque_id(value: &str, prefix: &str) -> bool {
    value.len() >= prefix.len() + 8
        && value.len() <= 96
        && value.starts_with(prefix)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

macro_rules! action_opaque_id {
    ($name:ident, $prefix:literal, $error:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, AgentActionErrorCodeV1> {
                let value = value.into();
                if !valid_opaque_id(&value, $prefix) {
                    return Err($error);
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

action_opaque_id!(
    AgentActionIdV1,
    "act_",
    AgentActionErrorCodeV1::InvalidPayload
);
action_opaque_id!(ToolCallIdV1, "call_", AgentActionErrorCodeV1::InvalidCallId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ProjectRelativePathV1(String);

impl ProjectRelativePathV1 {
    pub fn parse(value: impl Into<String>) -> Result<Self, AgentActionErrorCodeV1> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 512
            || value.starts_with('/')
            || value.starts_with('\\')
            || value.contains(':')
            || value.contains('\0')
        {
            return Err(AgentActionErrorCodeV1::AbsolutePathForbidden);
        }
        if value.contains('\\')
            || value
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err(AgentActionErrorCodeV1::PathTraversalForbidden);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProjectRelativePathV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RegisteredToolIdV1 {
    #[serde(rename = "project.list_markdown")]
    ProjectListMarkdown,
    #[serde(rename = "project.read_markdown")]
    ProjectReadMarkdown,
    #[serde(rename = "project.search_markdown")]
    ProjectSearchMarkdown,
    #[serde(rename = "project.validate_markdown")]
    ProjectValidateMarkdown,
    #[serde(rename = "project.create_markdown")]
    ProjectCreateMarkdown,
    #[serde(rename = "project.patch_markdown")]
    ProjectPatchMarkdown,
}

impl RegisteredToolIdV1 {
    pub fn parse(value: &str) -> Result<Self, AgentActionErrorCodeV1> {
        match value {
            "project.list_markdown" => Ok(Self::ProjectListMarkdown),
            "project.read_markdown" => Ok(Self::ProjectReadMarkdown),
            "project.search_markdown" => Ok(Self::ProjectSearchMarkdown),
            "project.validate_markdown" => Ok(Self::ProjectValidateMarkdown),
            "project.create_markdown" => Ok(Self::ProjectCreateMarkdown),
            "project.patch_markdown" => Ok(Self::ProjectPatchMarkdown),
            _ => Err(AgentActionErrorCodeV1::UnknownTool),
        }
    }

    pub fn is_write(self) -> bool {
        matches!(
            self,
            Self::ProjectCreateMarkdown | Self::ProjectPatchMarkdown
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolArgumentsV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<ProjectRelativePathV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<ProjectRelativePathV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_source_hash: Option<ContentHashV1>,
}

impl ToolArgumentsV1 {
    pub fn validate(&self, tool_id: RegisteredToolIdV1) -> Result<(), AgentActionErrorCodeV1> {
        let shape_matches = match tool_id {
            RegisteredToolIdV1::ProjectListMarkdown => {
                self.directory.is_some()
                    && self.path.is_none()
                    && self.query.is_none()
                    && self.content.is_none()
                    && self.patch.is_none()
                    && self.expected_source_hash.is_none()
            }
            RegisteredToolIdV1::ProjectReadMarkdown
            | RegisteredToolIdV1::ProjectValidateMarkdown => {
                self.path.is_some()
                    && self.directory.is_none()
                    && self.query.is_none()
                    && self.content.is_none()
                    && self.patch.is_none()
                    && self.expected_source_hash.is_none()
            }
            RegisteredToolIdV1::ProjectSearchMarkdown => {
                self.directory.is_some()
                    && self.query.is_some()
                    && self.path.is_none()
                    && self.content.is_none()
                    && self.patch.is_none()
                    && self.expected_source_hash.is_none()
            }
            RegisteredToolIdV1::ProjectCreateMarkdown => {
                self.path.is_some()
                    && self.content.is_some()
                    && self.directory.is_none()
                    && self.query.is_none()
                    && self.patch.is_none()
                    && self.expected_source_hash.is_none()
            }
            RegisteredToolIdV1::ProjectPatchMarkdown => {
                self.path.is_some()
                    && self.patch.is_some()
                    && self.expected_source_hash.is_some()
                    && self.directory.is_none()
                    && self.query.is_none()
                    && self.content.is_none()
            }
        };
        if !shape_matches {
            return Err(AgentActionErrorCodeV1::ToolArgumentsMismatch);
        }
        if self
            .query
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > MAX_SEARCH_QUERY_UTF8_BYTES)
            || self
                .content
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > MAX_TOOL_CONTENT_UTF8_BYTES)
            || self
                .patch
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > MAX_TOOL_CONTENT_UTF8_BYTES)
        {
            return Err(AgentActionErrorCodeV1::InvalidPayload);
        }
        Ok(())
    }

    pub fn referenced_paths(&self) -> impl Iterator<Item = &ProjectRelativePathV1> {
        self.path.iter().chain(self.directory.iter())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelToolRequestV1 {
    pub call_id: ToolCallIdV1,
    pub tool_id: RegisteredToolIdV1,
    pub arguments: ToolArgumentsV1,
    pub purpose: String,
}

impl ModelToolRequestV1 {
    pub fn validate(&self) -> Result<(), AgentActionErrorCodeV1> {
        if self.purpose.is_empty() || self.purpose.len() > MAX_TOOL_PURPOSE_UTF8_BYTES {
            return Err(AgentActionErrorCodeV1::InvalidPayload);
        }
        self.arguments.validate(self.tool_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentActionPayloadV1 {
    Respond {
        #[serde(rename = "schemaVersion")]
        schema_version: u16,
        #[serde(rename = "skillIds")]
        skill_ids: Vec<SkillIdV1>,
        #[serde(rename = "visibleText")]
        visible_text: String,
    },
    RequestTools {
        #[serde(rename = "schemaVersion")]
        schema_version: u16,
        #[serde(rename = "skillIds")]
        skill_ids: Vec<SkillIdV1>,
        #[serde(rename = "toolRequests")]
        tool_requests: Vec<ModelToolRequestV1>,
    },
    Complete {
        #[serde(rename = "schemaVersion")]
        schema_version: u16,
        #[serde(rename = "skillIds")]
        skill_ids: Vec<SkillIdV1>,
        #[serde(rename = "visibleSummary")]
        visible_summary: String,
    },
}

impl AgentActionPayloadV1 {
    pub fn validate(&self) -> Result<(), AgentActionErrorCodeV1> {
        if self.schema_version() != AGENT_ACTION_SCHEMA_VERSION {
            return Err(AgentActionErrorCodeV1::UnsupportedVersion);
        }
        validate_skill_ids(self.skill_ids())?;
        match self {
            Self::Respond { visible_text, .. } => validate_visible_text(visible_text),
            Self::Complete {
                visible_summary, ..
            } => validate_visible_text(visible_summary),
            Self::RequestTools { tool_requests, .. } => {
                if tool_requests.is_empty() {
                    return Err(AgentActionErrorCodeV1::EmptyToolBatch);
                }
                if tool_requests.len() > crate::contracts::MAX_TOOL_CALLS_PER_BATCH as usize {
                    return Err(AgentActionErrorCodeV1::ToolBatchLimitExceeded);
                }
                let mut call_ids = BTreeSet::new();
                for request in tool_requests {
                    request.validate()?;
                    if !call_ids.insert(&request.call_id) {
                        return Err(AgentActionErrorCodeV1::DuplicateCallId);
                    }
                }
                Ok(())
            }
        }
    }

    pub fn schema_version(&self) -> u16 {
        match self {
            Self::Respond { schema_version, .. }
            | Self::RequestTools { schema_version, .. }
            | Self::Complete { schema_version, .. } => *schema_version,
        }
    }

    pub fn skill_ids(&self) -> &[SkillIdV1] {
        match self {
            Self::Respond { skill_ids, .. }
            | Self::RequestTools { skill_ids, .. }
            | Self::Complete { skill_ids, .. } => skill_ids,
        }
    }

    pub fn tool_requests(&self) -> &[ModelToolRequestV1] {
        match self {
            Self::RequestTools { tool_requests, .. } => tool_requests,
            _ => &[],
        }
    }

    pub fn payload_hash(&self) -> ContentHashV1 {
        ContentHashV1::digest(
            &serde_json::to_vec(self).expect("AgentActionPayloadV1 serialization is infallible"),
        )
    }
}

fn validate_visible_text(value: &str) -> Result<(), AgentActionErrorCodeV1> {
    if value.is_empty() || value.len() > MAX_VISIBLE_TEXT_UTF8_BYTES {
        Err(AgentActionErrorCodeV1::InvalidPayload)
    } else {
        Ok(())
    }
}

fn validate_skill_ids(skill_ids: &[SkillIdV1]) -> Result<(), AgentActionErrorCodeV1> {
    if skill_ids.len() > SKILL_IDS.len() {
        return Err(AgentActionErrorCodeV1::UnknownSkill);
    }
    let mut unique = BTreeSet::new();
    for skill_id in skill_ids {
        if !SKILL_IDS.contains(&skill_id.as_str()) {
            return Err(AgentActionErrorCodeV1::UnknownSkill);
        }
        if !unique.insert(skill_id) {
            return Err(AgentActionErrorCodeV1::DuplicateSkill);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectToolBindingV1 {
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub turn_id: TurnIdV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolRequestV1 {
    pub schema_version: u16,
    pub binding: ProjectToolBindingV1,
    pub serial_ordinal: u16,
    pub call_id: ToolCallIdV1,
    pub tool_id: RegisteredToolIdV1,
    pub arguments: ToolArgumentsV1,
    pub purpose: String,
}

impl ToolRequestV1 {
    pub fn validate(&self) -> Result<(), AgentActionErrorCodeV1> {
        if self.schema_version != AGENT_ACTION_SCHEMA_VERSION {
            return Err(AgentActionErrorCodeV1::UnsupportedVersion);
        }
        if self.purpose.is_empty() || self.purpose.len() > MAX_TOOL_PURPOSE_UTF8_BYTES {
            return Err(AgentActionErrorCodeV1::InvalidPayload);
        }
        self.arguments.validate(self.tool_id)
    }

    pub fn request_hash(&self) -> ContentHashV1 {
        ContentHashV1::digest(
            &serde_json::to_vec(self).expect("ToolRequestV1 serialization is infallible"),
        )
    }
}

pub fn bind_tool_requests(
    payload: &AgentActionPayloadV1,
    binding: &ProjectToolBindingV1,
) -> Result<Vec<ToolRequestV1>, AgentActionErrorCodeV1> {
    payload.validate()?;
    Ok(payload
        .tool_requests()
        .iter()
        .enumerate()
        .map(|(ordinal, request)| ToolRequestV1 {
            schema_version: AGENT_ACTION_SCHEMA_VERSION,
            binding: binding.clone(),
            serial_ordinal: ordinal as u16,
            call_id: request.call_id.clone(),
            tool_id: request.tool_id,
            arguments: request.arguments.clone(),
            purpose: request.purpose.clone(),
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppPolicyDispositionV1 {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionStateV1 {
    Redacted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentActionRecordContextV1 {
    pub action_id: AgentActionIdV1,
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub turn_id: TurnIdV1,
    pub sequence: u16,
    pub provider: ProviderSnapshotV1,
    pub skill_pack: SkillPackSnapshotV1,
    pub effective_skill_ids: Vec<SkillIdV1>,
    pub policy_disposition: AppPolicyDispositionV1,
    pub created_at_ms: EpochMillisV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentActionRecordV1 {
    pub schema_version: u16,
    pub context: AgentActionRecordContextV1,
    pub payload: AgentActionPayloadV1,
    pub payload_hash: ContentHashV1,
    pub previous_action_hash: Option<ContentHashV1>,
    pub action_hash: ContentHashV1,
    pub redaction_state: RedactionStateV1,
}

impl AgentActionRecordV1 {
    pub fn wrap(
        context: AgentActionRecordContextV1,
        payload: AgentActionPayloadV1,
        previous_action_hash: Option<ContentHashV1>,
    ) -> Result<Self, AgentActionErrorCodeV1> {
        payload.validate()?;
        validate_skill_ids(&context.effective_skill_ids)?;
        if context.sequence == 0
            || context.sequence > crate::contracts::MAX_AGENT_STEPS_PER_TURN
            || (context.sequence == 1) != previous_action_hash.is_none()
        {
            return Err(AgentActionErrorCodeV1::ActionChainInvalid);
        }
        let payload_hash = payload.payload_hash();
        let action_hash = hash_json(&(
            AGENT_ACTION_SCHEMA_VERSION,
            &context,
            &payload_hash,
            &previous_action_hash,
            RedactionStateV1::Redacted,
        ));
        Ok(Self {
            schema_version: AGENT_ACTION_SCHEMA_VERSION,
            context,
            payload,
            payload_hash,
            previous_action_hash,
            action_hash,
            redaction_state: RedactionStateV1::Redacted,
        })
    }

    pub fn validate(&self) -> Result<(), AgentActionErrorCodeV1> {
        if self.schema_version != AGENT_ACTION_SCHEMA_VERSION {
            return Err(AgentActionErrorCodeV1::UnsupportedVersion);
        }
        self.payload.validate()?;
        validate_skill_ids(&self.context.effective_skill_ids)?;
        if self.context.sequence == 0
            || self.context.sequence > crate::contracts::MAX_AGENT_STEPS_PER_TURN
            || (self.context.sequence == 1) != self.previous_action_hash.is_none()
        {
            return Err(AgentActionErrorCodeV1::ActionChainInvalid);
        }
        let expected = hash_json(&(
            AGENT_ACTION_SCHEMA_VERSION,
            &self.context,
            &self.payload_hash,
            &self.previous_action_hash,
            self.redaction_state,
        ));
        if self.payload_hash != self.payload.payload_hash() || self.action_hash != expected {
            return Err(AgentActionErrorCodeV1::ActionChainInvalid);
        }
        Ok(())
    }
}

fn hash_json(value: &impl Serialize) -> ContentHashV1 {
    ContentHashV1::digest(&serde_json::to_vec(value).expect("contract serialization is infallible"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolResultStatusV1 {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolErrorCodeV1 {
    NotFound,
    HashConflict,
    InvalidMarkdown,
    PolicyBlocked,
    Cancelled,
    InternalRedacted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncationReasonV1 {
    ItemLimit,
    ByteLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolResultTrustV1 {
    UntrustedToolData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolResultDataV1 {
    pub item_count: u16,
    pub content_hash: ContentHashV1,
    pub affected_paths: Vec<ProjectRelativePathV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolResultV1 {
    pub schema_version: u16,
    pub binding: ProjectToolBindingV1,
    pub call_id: ToolCallIdV1,
    pub tool_id: RegisteredToolIdV1,
    pub status: ToolResultStatusV1,
    pub error_code: Option<ToolErrorCodeV1>,
    pub data: Option<ToolResultDataV1>,
    pub result_hash: ContentHashV1,
    pub truncated: bool,
    pub truncation_reason: Option<TruncationReasonV1>,
    pub trust: ToolResultTrustV1,
}

impl ToolResultV1 {
    pub fn build(
        request: &ToolRequestV1,
        status: ToolResultStatusV1,
        error_code: Option<ToolErrorCodeV1>,
        data: Option<ToolResultDataV1>,
        truncated: bool,
        truncation_reason: Option<TruncationReasonV1>,
    ) -> Result<Self, AgentActionErrorCodeV1> {
        let trust = ToolResultTrustV1::UntrustedToolData;
        let result_hash = hash_json(&(
            &request.binding,
            &request.call_id,
            request.tool_id,
            status,
            error_code,
            &data,
            truncated,
            truncation_reason,
            trust,
        ));
        let result = Self {
            schema_version: AGENT_ACTION_SCHEMA_VERSION,
            binding: request.binding.clone(),
            call_id: request.call_id.clone(),
            tool_id: request.tool_id,
            status,
            error_code,
            data,
            result_hash,
            truncated,
            truncation_reason,
            trust,
        };
        result.validate_against(request)?;
        Ok(result)
    }

    pub fn validate_against(&self, request: &ToolRequestV1) -> Result<(), AgentActionErrorCodeV1> {
        if self.schema_version != AGENT_ACTION_SCHEMA_VERSION {
            return Err(AgentActionErrorCodeV1::UnsupportedVersion);
        }
        if self.binding != request.binding
            || self.call_id != request.call_id
            || self.tool_id != request.tool_id
        {
            return Err(AgentActionErrorCodeV1::ToolResultMismatch);
        }
        let status_valid = match self.status {
            ToolResultStatusV1::Succeeded => self.error_code.is_none() && self.data.is_some(),
            ToolResultStatusV1::Failed | ToolResultStatusV1::Cancelled => self.error_code.is_some(),
        };
        let expected_hash = hash_json(&(
            &self.binding,
            &self.call_id,
            self.tool_id,
            self.status,
            self.error_code,
            &self.data,
            self.truncated,
            self.truncation_reason,
            self.trust,
        ));
        if !status_valid
            || self.truncated != self.truncation_reason.is_some()
            || self.trust != ToolResultTrustV1::UntrustedToolData
            || self.result_hash != expected_hash
        {
            return Err(AgentActionErrorCodeV1::InvalidToolResult);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RedactedAgentErrorLogV1 {
    pub schema_version: u16,
    pub code: AgentActionErrorCodeV1,
    pub source_hash: ContentHashV1,
    pub attempt_count: u8,
    pub redacted: bool,
}

impl RedactedAgentErrorLogV1 {
    pub fn from_source(code: AgentActionErrorCodeV1, source: &[u8], attempt_count: u8) -> Self {
        Self {
            schema_version: AGENT_ACTION_SCHEMA_VERSION,
            code,
            source_hash: ContentHashV1::digest(source),
            attempt_count,
            redacted: true,
        }
    }
}
