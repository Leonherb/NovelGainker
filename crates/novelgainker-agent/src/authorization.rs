use crate::action_contracts::{
    ProjectRelativePathV1, ProjectToolBindingV1, RegisteredToolIdV1, ToolRequestV1,
};
use crate::contracts::{AuthorizationPolicyV1, ContentHashV1, ProjectIdV1, SessionIdV1, TurnIdV1};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub const GOVERNANCE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GovernanceErrorCodeV1 {
    InvalidSchemaVersion,
    InvalidField,
    DuplicateAction,
    PolicyBlocked,
    ConfirmationRequired,
    ForbiddenV1,
    ConfirmationInvalid,
    ConfirmationAlreadyUsed,
    CrossProjectReference,
    SensitiveDataForbidden,
    PersistenceLimitExceeded,
    PlaintextDowngradeForbidden,
    TransactionInvalid,
    PreflightFailed,
    UndoLimitExceeded,
    UndoExpired,
    UndoHashConflict,
    UndoDecryptFailed,
    UndoCleared,
}

impl fmt::Display for GovernanceErrorCodeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::InvalidSchemaVersion => "agent-governance.invalid-schema-version",
            Self::InvalidField => "agent-governance.invalid-field",
            Self::DuplicateAction => "agent-governance.duplicate-action",
            Self::PolicyBlocked => "agent-governance.policy-blocked",
            Self::ConfirmationRequired => "agent-governance.confirmation-required",
            Self::ForbiddenV1 => "agent-governance.forbidden-v1",
            Self::ConfirmationInvalid => "agent-governance.confirmation-invalid",
            Self::ConfirmationAlreadyUsed => "agent-governance.confirmation-already-used",
            Self::CrossProjectReference => "agent-governance.cross-project-reference",
            Self::SensitiveDataForbidden => "agent-governance.sensitive-data-forbidden",
            Self::PersistenceLimitExceeded => "agent-governance.persistence-limit-exceeded",
            Self::PlaintextDowngradeForbidden => "agent-governance.plaintext-downgrade-forbidden",
            Self::TransactionInvalid => "agent-governance.transaction-invalid",
            Self::PreflightFailed => "agent-governance.preflight-failed",
            Self::UndoLimitExceeded => "agent-governance.undo-limit-exceeded",
            Self::UndoExpired => "agent-governance.undo-expired",
            Self::UndoHashConflict => "agent-governance.undo-hash-conflict",
            Self::UndoDecryptFailed => "agent-governance.undo-decrypt-failed",
            Self::UndoCleared => "agent-governance.undo-cleared",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for GovernanceErrorCodeV1 {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskLevelV1 {
    ReadSafe,
    WriteSafe,
    AlwaysConfirm,
    ForbiddenV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernedActionV1 {
    ListManagedMarkdown,
    ReadManagedMarkdown,
    SearchManagedMarkdown,
    ValidateManagedMarkdown,
    CreateEntityMarkdown,
    PatchEntityMarkdown,
    CreateDraftChapter,
    ReplaceWholeFile,
    ReplaceWholeChapter,
    PromoteChapterState,
    UpdateStoryLedger,
    ModifyProjectRules,
    ModifyCriticalMetadata,
    ImportOutsideProject,
    PublishOrExport,
    DeletePath,
    MovePath,
    RenamePath,
    ArbitraryFilesystem,
    SystemCapability,
    ModifyAuthorizationOrSkillPack,
}

impl GovernedActionV1 {
    pub const ALL: [Self; 21] = [
        Self::ListManagedMarkdown,
        Self::ReadManagedMarkdown,
        Self::SearchManagedMarkdown,
        Self::ValidateManagedMarkdown,
        Self::CreateEntityMarkdown,
        Self::PatchEntityMarkdown,
        Self::CreateDraftChapter,
        Self::ReplaceWholeFile,
        Self::ReplaceWholeChapter,
        Self::PromoteChapterState,
        Self::UpdateStoryLedger,
        Self::ModifyProjectRules,
        Self::ModifyCriticalMetadata,
        Self::ImportOutsideProject,
        Self::PublishOrExport,
        Self::DeletePath,
        Self::MovePath,
        Self::RenamePath,
        Self::ArbitraryFilesystem,
        Self::SystemCapability,
        Self::ModifyAuthorizationOrSkillPack,
    ];

    pub fn risk(self) -> RiskLevelV1 {
        match self {
            Self::ListManagedMarkdown
            | Self::ReadManagedMarkdown
            | Self::SearchManagedMarkdown
            | Self::ValidateManagedMarkdown => RiskLevelV1::ReadSafe,
            Self::CreateEntityMarkdown | Self::PatchEntityMarkdown | Self::CreateDraftChapter => {
                RiskLevelV1::WriteSafe
            }
            Self::ReplaceWholeFile
            | Self::ReplaceWholeChapter
            | Self::PromoteChapterState
            | Self::UpdateStoryLedger
            | Self::ModifyProjectRules
            | Self::ModifyCriticalMetadata
            | Self::ImportOutsideProject
            | Self::PublishOrExport => RiskLevelV1::AlwaysConfirm,
            Self::DeletePath
            | Self::MovePath
            | Self::RenamePath
            | Self::ArbitraryFilesystem
            | Self::SystemCapability
            | Self::ModifyAuthorizationOrSkillPack => RiskLevelV1::ForbiddenV1,
        }
    }
}

pub fn registered_tool_risk(tool_id: RegisteredToolIdV1) -> Option<RiskLevelV1> {
    match tool_id {
        RegisteredToolIdV1::ProjectListMarkdown
        | RegisteredToolIdV1::ProjectReadMarkdown
        | RegisteredToolIdV1::ProjectSearchMarkdown
        | RegisteredToolIdV1::ProjectValidateMarkdown => Some(RiskLevelV1::ReadSafe),
        RegisteredToolIdV1::ProjectCreateMarkdown | RegisteredToolIdV1::ProjectPatchMarkdown => {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatedMutationShapeV1 {
    CreateEntityMarkdown,
    CreateDraftChapter,
    PatchHunks,
    ReplaceWholeFile,
    ReplaceWholeChapter,
    PromoteChapterState,
    UpdateStoryLedger,
    ModifyProjectRules,
    ModifyCriticalMetadata,
    PublishOrExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteToolClassificationV1 {
    pub schema_version: u16,
    pub binding: ProjectToolBindingV1,
    pub tool_request_hash: ContentHashV1,
    pub tool_id: RegisteredToolIdV1,
    pub path: ProjectRelativePathV1,
    pub mutation_shape: ValidatedMutationShapeV1,
    pub action: GovernedActionV1,
    pub risk: RiskLevelV1,
    pub classification_hash: ContentHashV1,
}

impl WriteToolClassificationV1 {
    pub fn build(
        request: &ToolRequestV1,
        mutation_shape: ValidatedMutationShapeV1,
    ) -> Result<Self, GovernanceErrorCodeV1> {
        request
            .validate()
            .map_err(|_| GovernanceErrorCodeV1::InvalidField)?;
        if !request.tool_id.is_write() {
            return Err(GovernanceErrorCodeV1::InvalidField);
        }
        let path = request
            .arguments
            .path
            .clone()
            .ok_or(GovernanceErrorCodeV1::InvalidField)?;
        let action = classify_write_action(request.tool_id, &path, mutation_shape)?;
        let risk = action.risk();
        let tool_request_hash = request.request_hash();
        let classification_hash = write_classification_hash(
            &request.binding,
            &tool_request_hash,
            request.tool_id,
            &path,
            mutation_shape,
            action,
            risk,
        );
        Ok(Self {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            binding: request.binding.clone(),
            tool_request_hash,
            tool_id: request.tool_id,
            path,
            mutation_shape,
            action,
            risk,
            classification_hash,
        })
    }

    pub fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if self.schema_version != GOVERNANCE_SCHEMA_VERSION {
            return Err(GovernanceErrorCodeV1::InvalidSchemaVersion);
        }
        let action = classify_write_action(self.tool_id, &self.path, self.mutation_shape)?;
        if self.action != action
            || self.risk != action.risk()
            || self.classification_hash
                != write_classification_hash(
                    &self.binding,
                    &self.tool_request_hash,
                    self.tool_id,
                    &self.path,
                    self.mutation_shape,
                    self.action,
                    self.risk,
                )
        {
            return Err(GovernanceErrorCodeV1::InvalidField);
        }
        Ok(())
    }
}

fn classify_write_action(
    tool_id: RegisteredToolIdV1,
    path: &ProjectRelativePathV1,
    mutation_shape: ValidatedMutationShapeV1,
) -> Result<GovernedActionV1, GovernanceErrorCodeV1> {
    if !tool_id.is_write() {
        return Err(GovernanceErrorCodeV1::InvalidField);
    }
    let normalized = path.as_str().to_ascii_lowercase();
    let segments = normalized.split('/').collect::<Vec<_>>();
    let is_create = tool_id == RegisteredToolIdV1::ProjectCreateMarkdown;
    let is_patch = tool_id == RegisteredToolIdV1::ProjectPatchMarkdown;

    if segments.iter().any(|segment| segment.starts_with('.')) {
        return Ok(GovernedActionV1::ModifyAuthorizationOrSkillPack);
    }
    if matches!(normalized.as_str(), "agents.md" | "claude.md") {
        return Ok(GovernedActionV1::ModifyProjectRules);
    }
    if normalized == "project.md" {
        return Ok(GovernedActionV1::ModifyCriticalMetadata);
    }
    if normalized.starts_with("materials/000.") {
        return Ok(GovernedActionV1::UpdateStoryLedger);
    }
    if normalized.starts_with("published/") {
        return Ok(GovernedActionV1::PublishOrExport);
    }
    let managed_root = segments.first().is_some_and(|root| {
        matches!(
            *root,
            "characters" | "materials" | "plot" | "outline" | "world" | "chapters"
        )
    });
    if !normalized.ends_with(".md") || !managed_root {
        return Ok(GovernedActionV1::ArbitraryFilesystem);
    }

    let action = match mutation_shape {
        ValidatedMutationShapeV1::CreateEntityMarkdown
            if is_create && !normalized.starts_with("chapters/") =>
        {
            GovernedActionV1::CreateEntityMarkdown
        }
        ValidatedMutationShapeV1::CreateDraftChapter
            if is_create && normalized.starts_with("chapters/") =>
        {
            GovernedActionV1::CreateDraftChapter
        }
        ValidatedMutationShapeV1::PatchHunks if is_patch => GovernedActionV1::PatchEntityMarkdown,
        ValidatedMutationShapeV1::ReplaceWholeFile if is_patch => {
            GovernedActionV1::ReplaceWholeFile
        }
        ValidatedMutationShapeV1::ReplaceWholeChapter
            if is_patch && normalized.starts_with("chapters/") =>
        {
            GovernedActionV1::ReplaceWholeChapter
        }
        ValidatedMutationShapeV1::PromoteChapterState
            if is_patch && normalized.starts_with("chapters/") =>
        {
            GovernedActionV1::PromoteChapterState
        }
        ValidatedMutationShapeV1::UpdateStoryLedger if is_patch => {
            GovernedActionV1::UpdateStoryLedger
        }
        ValidatedMutationShapeV1::ModifyProjectRules if is_patch => {
            GovernedActionV1::ModifyProjectRules
        }
        ValidatedMutationShapeV1::ModifyCriticalMetadata if is_patch => {
            GovernedActionV1::ModifyCriticalMetadata
        }
        ValidatedMutationShapeV1::PublishOrExport => GovernedActionV1::PublishOrExport,
        _ => return Err(GovernanceErrorCodeV1::InvalidField),
    };
    Ok(action)
}

#[allow(clippy::too_many_arguments)]
fn write_classification_hash(
    binding: &ProjectToolBindingV1,
    tool_request_hash: &ContentHashV1,
    tool_id: RegisteredToolIdV1,
    path: &ProjectRelativePathV1,
    mutation_shape: ValidatedMutationShapeV1,
    action: GovernedActionV1,
    risk: RiskLevelV1,
) -> ContentHashV1 {
    ContentHashV1::digest(
        &serde_json::to_vec(&(
            GOVERNANCE_SCHEMA_VERSION,
            binding,
            tool_request_hash,
            tool_id,
            path,
            mutation_shape,
            action,
            risk,
        ))
        .expect("write classification serialization is infallible"),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationEffectV1 {
    Execute,
    AwaitConfirmation,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorizationReasonV1 {
    ReadSafe,
    AutoApplySafe,
    ReviewEveryWrite,
    HighRiskConfirmation,
    PlanOnlyWriteBlocked,
    ForbiddenV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorizationDecisionV1 {
    pub action: GovernedActionV1,
    pub risk: RiskLevelV1,
    pub policy: AuthorizationPolicyV1,
    pub effect: AuthorizationEffectV1,
    pub reason: AuthorizationReasonV1,
}

pub fn authorize(
    policy: AuthorizationPolicyV1,
    action: GovernedActionV1,
) -> AuthorizationDecisionV1 {
    let risk = action.risk();
    let (effect, reason) = match (policy, risk) {
        (_, RiskLevelV1::ReadSafe) => (
            AuthorizationEffectV1::Execute,
            AuthorizationReasonV1::ReadSafe,
        ),
        (_, RiskLevelV1::ForbiddenV1) => (
            AuthorizationEffectV1::Block,
            AuthorizationReasonV1::ForbiddenV1,
        ),
        (AuthorizationPolicyV1::PlanOnly, _) => (
            AuthorizationEffectV1::Block,
            AuthorizationReasonV1::PlanOnlyWriteBlocked,
        ),
        (AuthorizationPolicyV1::ReviewEveryWrite, RiskLevelV1::WriteSafe) => (
            AuthorizationEffectV1::AwaitConfirmation,
            AuthorizationReasonV1::ReviewEveryWrite,
        ),
        (AuthorizationPolicyV1::AutoApplySafe, RiskLevelV1::WriteSafe) => (
            AuthorizationEffectV1::Execute,
            AuthorizationReasonV1::AutoApplySafe,
        ),
        (_, RiskLevelV1::AlwaysConfirm) => (
            AuthorizationEffectV1::AwaitConfirmation,
            AuthorizationReasonV1::HighRiskConfirmation,
        ),
    };
    AuthorizationDecisionV1 {
        action,
        risk,
        policy,
        effect,
        reason,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnAuthorizationV1 {
    pub schema_version: u16,
    pub policy: AuthorizationPolicyV1,
    pub decisions: Vec<AuthorizationDecisionV1>,
    pub transaction_effect: AuthorizationEffectV1,
    pub preconfirmation_write_count: u16,
    pub atomic_write_transaction: bool,
}

pub fn authorize_turn(
    policy: AuthorizationPolicyV1,
    actions: &[GovernedActionV1],
) -> Result<TurnAuthorizationV1, GovernanceErrorCodeV1> {
    if actions.is_empty() {
        return Err(GovernanceErrorCodeV1::InvalidField);
    }
    let mut unique = BTreeSet::new();
    let mut decisions = Vec::with_capacity(actions.len());
    for action in actions {
        if !unique.insert(*action) {
            return Err(GovernanceErrorCodeV1::DuplicateAction);
        }
        decisions.push(authorize(policy, *action));
    }

    let transaction_effect = if decisions
        .iter()
        .any(|decision| decision.effect == AuthorizationEffectV1::Block)
    {
        AuthorizationEffectV1::Block
    } else if decisions
        .iter()
        .any(|decision| decision.effect == AuthorizationEffectV1::AwaitConfirmation)
    {
        AuthorizationEffectV1::AwaitConfirmation
    } else {
        AuthorizationEffectV1::Execute
    };

    Ok(TurnAuthorizationV1 {
        schema_version: GOVERNANCE_SCHEMA_VERSION,
        policy,
        decisions,
        transaction_effect,
        preconfirmation_write_count: 0,
        atomic_write_transaction: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ConfirmationIdV1(String);

impl ConfirmationIdV1 {
    pub fn parse(value: impl Into<String>) -> Result<Self, GovernanceErrorCodeV1> {
        let value = value.into();
        if value.len() < 12
            || value.len() > 96
            || !value.starts_with("cfm_")
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

impl<'de> Deserialize<'de> for ConfirmationIdV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationAuthorityV1 {
    UserViaApplication,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmationBindingV1 {
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub turn_id: TurnIdV1,
    pub action_plan_hash: ContentHashV1,
    pub source_set_hash: ContentHashV1,
    pub policy_snapshot_hash: ContentHashV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HighRiskConfirmationV1 {
    pub schema_version: u16,
    pub confirmation_id: ConfirmationIdV1,
    pub authority: ConfirmationAuthorityV1,
    pub binding: ConfirmationBindingV1,
    pub issued_at_ms: u64,
    pub used: bool,
    pub confirmation_hash: ContentHashV1,
}

impl HighRiskConfirmationV1 {
    pub fn issue(
        confirmation_id: ConfirmationIdV1,
        binding: ConfirmationBindingV1,
        issued_at_ms: u64,
    ) -> Self {
        let confirmation_hash = confirmation_hash(&confirmation_id, &binding, issued_at_ms);
        Self {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            confirmation_id,
            authority: ConfirmationAuthorityV1::UserViaApplication,
            binding,
            issued_at_ms,
            used: false,
            confirmation_hash,
        }
    }

    pub fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if self.schema_version != GOVERNANCE_SCHEMA_VERSION {
            return Err(GovernanceErrorCodeV1::InvalidSchemaVersion);
        }
        if self.confirmation_hash
            != confirmation_hash(&self.confirmation_id, &self.binding, self.issued_at_ms)
        {
            return Err(GovernanceErrorCodeV1::ConfirmationInvalid);
        }
        Ok(())
    }

    pub fn consume(
        &mut self,
        current_binding: &ConfirmationBindingV1,
    ) -> Result<(), GovernanceErrorCodeV1> {
        self.validate()?;
        if self.used {
            return Err(GovernanceErrorCodeV1::ConfirmationAlreadyUsed);
        }
        if &self.binding != current_binding {
            return Err(GovernanceErrorCodeV1::ConfirmationInvalid);
        }
        self.used = true;
        Ok(())
    }
}

fn confirmation_hash(
    confirmation_id: &ConfirmationIdV1,
    binding: &ConfirmationBindingV1,
    issued_at_ms: u64,
) -> ContentHashV1 {
    ContentHashV1::digest(
        &serde_json::to_vec(&(
            GOVERNANCE_SCHEMA_VERSION,
            confirmation_id,
            ConfirmationAuthorityV1::UserViaApplication,
            binding,
            issued_at_ms,
        ))
        .expect("confirmation serialization is infallible"),
    )
}
