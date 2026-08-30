use super::{session_handle, session_writer_handle, ProjectRegistry};
use app_core::agent_anchor::{
    resolve_anchor_intent, AnchorReferenceV1, AnchorResolutionV1, AnchorTargetKindV1,
    ResolveAnchorInputV1,
};
use app_core::agent_canon::{
    bind_canon_proposal, chapter_proposal_from_artifact, discard_persisted_candidate,
    materialize_draft_artifact, normalize_canon_target, persist_canon_proposal,
    persist_draft_artifact, DraftArtifactV1, MaterializeDraftInputV1, PersistedCanonProposalV1,
};
use app_core::agent_conflict::{
    detect_conflicts, evidence_from_markdown_facts, resolve_conflict_set,
    ConflictResolutionChoiceV1, ConflictResolutionResultV1, ConflictSetV1,
};
use app_core::agent_marks::{
    archive_session_marks, reconcile_context_marks, remove_context_mark, upsert_context_mark,
    ContextMarkCreatorV1, ContextMarkPriorityV1, ContextMarkScopeV1, ContextMarkStatusV1,
    ContextMarkStoreV1, ContextMarkV1, UpsertContextMarkInputV1,
};
use app_core::agent_transaction::{
    apply_agent_undo, commit_prepared_agent_write, AgentUndoApplyResultV1,
};
use app_core::agent_v2::{
    append_completed_turn, clear_agent_history, compact_memory, last_completed_skill_ids,
    load_agent_session, memory_for_compaction, persist_agent_write_policy, resolve_history_branch,
    source_ref_by_id, AgentHistoryClearResultV2, AgentHistoryClearScopeV2, AgentSessionSnapshotV2,
    CompletedTurnInputV2, ContextQueryResultV2, ConversationMessageV2, ProjectContextIndexV2,
    SessionMemoryV2, SourceRefV2,
};
use app_core::agent_write_plan_v2::{
    build_write_plan_v2, validate_write_confirmation, BuildWritePlanInputV2, PreparedWritePlanV2,
    WritePlanV2,
};
use app_core::credential_vault::ProviderId;
use app_core::project::ProjectHandle;
use app_core::provider_core::{
    preset, CancellationToken, FinishReason, MessageRole, NormalizedEvent, NormalizedMessage,
    NormalizedRequest, NormalizedResponse, ProviderError, ProviderHttpClient, MAX_OUTPUT_TOKENS,
};
use app_core::provider_state::{load_provider_state, ProviderConnectionStatus};
use app_core::AppError;
use novelgainker_agent::authorization::AuthorizationEffectV1;
use novelgainker_agent::contracts::{AuthorizationPolicyV1, EpochMillisV1};
use novelgainker_agent::persistence_contracts::UndoRecordStatusV1;
use novelgainker_agent::runtime_registry_v2::{BundledSkillResourceKindV2, RuntimeSkillRegistryV2};
use novelgainker_agent::v2::{
    AgentControlActionV2, BudgetProfileV2, CanonProposalV1, CanonTargetKindV1,
    CompletionContractV2, MemoryCompactionV1, ProviderInvocationV2, ProviderModeV2,
    SkillIntentClassV2, SkillSelectionV2, TurnBudgetTrackerV2, TurnBudgetUsageV2,
};
use novelgainker_agent::{
    agent_control_action_v2_schema_source, canon_proposal_v1_schema_source,
    memory_compaction_v1_schema_source, skill_selection_v2_schema_source,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use uuid::Uuid;

const AGENT_TURN_EVENT_CHANNEL: &str = "agent-turn-v2-event";
const AGENT_RUNTIME_SCHEMA_VERSION: u16 = 2;
const MAX_AGENT_USER_INPUT_BYTES: usize = 16 * 1024;
const MAX_CONTROL_ROUNDS: usize = 4;
const MAX_CONTEXT_SOURCES_PER_QUERY: usize = 8;

#[derive(Default)]
pub(super) struct AgentCanonRuntimeV2 {
    policies: HashMap<String, AgentPolicyRuntimeV2>,
    pending_plans: HashMap<String, PendingWriteRuntimeV2>,
    pending_conflicts: HashMap<String, PendingConflictRuntimeV2>,
}

struct AgentPolicyRuntimeV2 {
    project_session_id: String,
    session_id: String,
    policy: AuthorizationPolicyV1,
    policy_hash: String,
}

struct PendingWriteRuntimeV2 {
    project_session_id: String,
    prepared: PreparedWritePlanV2,
}

struct PendingConflictRuntimeV2 {
    project_session_id: String,
    candidate: PersistedCanonProposalV1,
    conflict_set: ConflictSetV1,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWritePolicySnapshotV2 {
    pub policy: AuthorizationPolicyV1,
    pub policy_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentWritePolicySetInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub policy: AuthorizationPolicyV1,
    pub expected_policy_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentContextMarkUpsertInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub source_id: String,
    pub source_binding_hash: String,
    pub scope: ContextMarkScopeV1,
    pub priority: ContextMarkPriorityV1,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentContextMarkRemoveInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub mark_id: String,
    pub binding_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentConflictResolutionInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub conflict_set_id: String,
    pub conflict_set_hash: String,
    pub resolution: ConflictResolutionChoiceV1,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConflictResolutionResponseV2 {
    pub resolution: ConflictResolutionResultV1,
    pub write_plan: Option<WritePlanV2>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentWriteConfirmationInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub pending_id: String,
    pub plan_hash: String,
    pub source_binding_hash: String,
    pub policy_hash: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentWritePlanRejectInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub pending_id: String,
    pub plan_hash: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWritePlanRejectResultV2 {
    pub rejected: bool,
    pub project_write_count: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentWriteUndoInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub receipt_id: String,
    pub result_hash: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommitReceiptV2 {
    pub schema_version: u16,
    pub receipt_id: String,
    pub transaction_hash: String,
    pub result_hash: String,
    pub relative_paths: Vec<String>,
    pub committed_at_ms: u64,
    pub undo_expires_at_ms: u64,
    pub project_write_count: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkillResourceSnapshotV2 {
    pub path: String,
    pub kind: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkillSnapshotV2 {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub trigger_summary: String,
    pub manifest_sha256: String,
    pub resources: Vec<AgentSkillResourceSnapshotV2>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkillRegistrySnapshotV2 {
    pub schema_version: u16,
    pub pack_id: String,
    pub pack_version: String,
    pub pack_hash: String,
    pub source_commit: String,
    pub runtime_source: String,
    pub runtime_enabled: bool,
    pub eligible_skill_ids: Vec<String>,
    pub skills: Vec<AgentSkillSnapshotV2>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTurnStartInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub user_text: String,
    pub expected_provider_id: ProviderId,
    #[serde(default)]
    pub anchor_reference: Option<AnchorReferenceV1>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnStartedV2 {
    pub schema_version: u16,
    pub run_id: String,
    pub turn_id: String,
    pub provider_id: ProviderId,
    pub model: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnCancelResultV2 {
    pub cancel_requested: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentMemoryCompactInputV2 {
    pub schema_version: u16,
    pub project_session_id: String,
    pub session_id: String,
    pub memory_hash: String,
    pub expected_provider_id: ProviderId,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum AgentTurnEventBodyV2 {
    TurnState {
        state: String,
        visible_reason: Option<String>,
    },
    SkillSelection {
        selected_skill_ids: Vec<String>,
        intent_class: SkillIntentClassV2,
        visible_reason: String,
    },
    ContextSources {
        sources: Vec<SourceRefV2>,
        index_hash: String,
    },
    AnchorCandidate {
        resolution: AnchorResolutionV1,
    },
    CanonCandidate {
        artifact: DraftArtifactV1,
        proposal: PersistedCanonProposalV1,
    },
    ContextMarks {
        store: ContextMarkStoreV1,
    },
    Conflict {
        conflict_set: ConflictSetV1,
    },
    WritePlan {
        plan: WritePlanV2,
    },
    CommitReceipt {
        receipt: AgentCommitReceiptV2,
    },
    ConversationDelta {
        delta: String,
    },
    Completed {
        message: ConversationMessageV2,
        sources: Vec<SourceRefV2>,
        usage: TurnBudgetUsageV2,
        agent_canon_write_count: u16,
    },
    Failed {
        message: Option<ConversationMessageV2>,
        error: AppError,
        usage: TurnBudgetUsageV2,
        agent_canon_write_count: u16,
    },
    Cancelled {
        message: Option<ConversationMessageV2>,
        usage: TurnBudgetUsageV2,
        agent_canon_write_count: u16,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentTurnEventV2 {
    schema_version: u16,
    run_id: String,
    turn_id: String,
    project_session_id: String,
    provider_id: ProviderId,
    model: String,
    sequence: u32,
    #[serde(flatten)]
    event: AgentTurnEventBodyV2,
}

struct AgentWorkerContext {
    run_id: String,
    turn_id: String,
    project_session_id: String,
    session_id: String,
    branch_id: String,
    handle: ProjectHandle,
    provider_id: ProviderId,
    model: String,
    user_text: String,
    current_user_message_id: String,
    anchor_resolution: AnchorResolutionV1,
    prior_selected_skill_ids: Vec<String>,
    cancellation: CancellationToken,
    cancellation_id: String,
}

#[tauri::command]
pub fn agent_skill_registry_load() -> AgentSkillRegistrySnapshotV2 {
    let registry = RuntimeSkillRegistryV2::load_gar1_read_only();
    let hashes = registry.resource_hashes();
    AgentSkillRegistrySnapshotV2 {
        schema_version: AGENT_RUNTIME_SCHEMA_VERSION,
        pack_id: registry.pack_id.to_owned(),
        pack_version: registry.pack_version.to_owned(),
        pack_hash: registry.pack_hash.to_owned(),
        source_commit: registry.source_commit.to_owned(),
        runtime_source: registry.runtime_source.to_owned(),
        runtime_enabled: true,
        eligible_skill_ids: registry
            .eligible_skill_ids
            .iter()
            .map(|id| (*id).to_owned())
            .collect(),
        skills: registry
            .skills
            .iter()
            .map(|skill| AgentSkillSnapshotV2 {
                id: skill.id.to_owned(),
                display_name: skill.display_name.to_owned(),
                description: skill.description.to_owned(),
                trigger_summary: skill.trigger_summary.to_owned(),
                manifest_sha256: hashes
                    .iter()
                    .find(|(path, _)| path == &format!("manifests/{}.json", skill.id))
                    .map(|(_, hash)| hash.clone())
                    .unwrap_or_default(),
                resources: skill
                    .resources
                    .iter()
                    .map(|resource| AgentSkillResourceSnapshotV2 {
                        path: resource.path.to_owned(),
                        kind: match resource.kind {
                            BundledSkillResourceKindV2::Instruction => "instruction",
                            BundledSkillResourceKindV2::Template => "template",
                            BundledSkillResourceKindV2::Reference => "reference",
                        }
                        .to_owned(),
                        sha256: hashes
                            .iter()
                            .find(|(path, _)| path == resource.path)
                            .map(|(_, hash)| hash.clone())
                            .unwrap_or_default(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[tauri::command]
pub fn agent_session_load(
    state: tauri::State<'_, ProjectRegistry>,
    project_session_id: String,
    history_consent: bool,
) -> Result<AgentSessionSnapshotV2, AppError> {
    let handle = session_handle(&state, &project_session_id)?;
    load_agent_session(&handle, history_consent, now_ms())
}

#[tauri::command]
pub fn agent_history_branch_resolve(
    state: tauri::State<'_, ProjectRegistry>,
    project_session_id: String,
    branch_id: String,
) -> Result<AgentSessionSnapshotV2, AppError> {
    let handle = session_handle(&state, &project_session_id)?;
    resolve_history_branch(&handle, &branch_id)
}

#[tauri::command]
pub fn agent_session_clear(
    state: tauri::State<'_, ProjectRegistry>,
    project_session_id: String,
    scope: AgentHistoryClearScopeV2,
) -> Result<AgentHistoryClearResultV2, AppError> {
    if state.active_provider_id().is_some() {
        return Err(AppError::provider_run_active());
    }
    let handle = session_handle(&state, &project_session_id)?;
    if project_has_pending_action(&state, handle.project_id())? {
        return Err(AppError::agent_contract("agent.pending_author_decision"));
    }
    if let Some(session_id) = load_agent_session(&handle, false, now_ms())?.session_id {
        archive_session_marks(&handle, &session_id, now_ms())?;
    }
    clear_agent_history(&handle, scope, now_ms())
}

#[tauri::command]
pub fn agent_turn_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProjectRegistry>,
    input: AgentTurnStartInputV2,
) -> Result<AgentTurnStartedV2, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION
        || input.user_text.trim().is_empty()
        || input.user_text.len() > MAX_AGENT_USER_INPUT_BYTES
        || input
            .user_text
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(AppError::agent_contract("agent.turn_input_invalid"));
    }
    let handle = session_handle(&state, &input.project_session_id)?;
    if project_has_pending_action(&state, handle.project_id())? {
        return Err(AppError::agent_contract("agent.pending_author_decision"));
    }
    let session = load_agent_session(&handle, false, now_ms())?;
    if session.history_consent_required || session.branch_selection_required {
        return Err(AppError::agent_contract("agent.session_not_ready"));
    }
    let session_id = session
        .session_id
        .clone()
        .ok_or_else(|| AppError::agent_contract("agent.session_not_ready"))?;
    let branch_id = session
        .active_branch_id
        .clone()
        .ok_or_else(|| AppError::agent_contract("agent.session_not_ready"))?;
    let prior_selected_skill_ids = last_completed_skill_ids(&handle)?;
    let provider_id = selected_ready_provider(&state, input.expected_provider_id)?;
    let run_id = format!("run_{}", Uuid::new_v4().simple());
    let turn_id = format!("trn_{}", Uuid::new_v4().simple());
    let current_user_message_id = format!("msg_{}", Uuid::new_v4().simple());
    let anchor_resolution = resolve_anchor_intent(ResolveAnchorInputV1 {
        snapshot: &session,
        turn_id: &turn_id,
        current_user_message_id: &current_user_message_id,
        user_text: &input.user_text,
        reference: input.anchor_reference.as_ref(),
        now_ms: now_ms(),
    })?;
    let cancellation_id = format!("can_{}", Uuid::new_v4().simple());
    let cancellation = CancellationToken::default();
    state.begin_provider_run(provider_id, run_id.clone(), cancellation.clone())?;
    let model = preset(provider_id).model.to_owned();
    let worker = AgentWorkerContext {
        run_id: run_id.clone(),
        turn_id: turn_id.clone(),
        project_session_id: input.project_session_id,
        session_id,
        branch_id,
        handle,
        provider_id,
        model: model.clone(),
        user_text: input.user_text,
        current_user_message_id,
        anchor_resolution,
        prior_selected_skill_ids,
        cancellation,
        cancellation_id,
    };
    tauri::async_runtime::spawn_blocking(move || execute_agent_turn(app, worker));
    Ok(AgentTurnStartedV2 {
        schema_version: AGENT_RUNTIME_SCHEMA_VERSION,
        run_id,
        turn_id,
        provider_id,
        model,
    })
}

#[tauri::command]
pub fn agent_turn_cancel(
    state: tauri::State<'_, ProjectRegistry>,
    run_id: String,
) -> Result<AgentTurnCancelResultV2, AppError> {
    state.cancel_provider_run(&run_id)?;
    Ok(AgentTurnCancelResultV2 {
        cancel_requested: true,
    })
}

#[tauri::command]
pub async fn agent_memory_compact(
    app: tauri::AppHandle,
    input: AgentMemoryCompactInputV2,
) -> Result<SessionMemoryV2, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION {
        return Err(AppError::agent_contract("agent.memory_input_invalid"));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ProjectRegistry>();
        if state.active_provider_id().is_some() {
            return Err(AppError::provider_run_active());
        }
        let handle = session_handle(&state, &input.project_session_id)?;
        let memory = memory_for_compaction(&handle, &input.session_id, &input.memory_hash)?;
        let provider_id = selected_ready_provider(&state, input.expected_provider_id)?;
        let last_turn_id = memory
            .last_turn_id
            .clone()
            .ok_or_else(|| AppError::agent_contract("agent.memory_compaction_not_needed"))?;
        let request_id = format!("req_{}", Uuid::new_v4().simple());
        let request = NormalizedRequest::agent_v2_memory_compaction(
            serde_json::to_string(&memory)
                .map_err(|_| AppError::agent_contract("agent.memory_invalid"))?,
            &request_id,
            2_048,
            memory_compaction_v1_schema_source(),
        )
        .map_err(AppError::from_provider_error)?;
        let run_id = format!("run_{}", Uuid::new_v4().simple());
        let cancellation = CancellationToken::default();
        state.begin_provider_run(provider_id, run_id.clone(), cancellation.clone())?;
        let result = (|| {
            let client = ProviderHttpClient::production().map_err(AppError::from_provider_error)?;
            let response =
                complete_with_vault(&state, &client, provider_id, &request, &cancellation)?;
            let compacted: MemoryCompactionV1 = serde_json::from_str(&response.text)
                .map_err(|_| AppError::agent_contract("agent.memory_compaction_invalid"))?;
            compacted
                .validate(&request_id, &input.memory_hash)
                .map_err(|_| AppError::agent_contract("agent.memory_compaction_invalid"))?;
            if compacted.compacted_through_turn_id != last_turn_id {
                return Err(AppError::agent_contract(
                    "agent.memory_compaction_turn_mismatch",
                ));
            }
            compact_memory(
                &handle,
                &input.session_id,
                &input.memory_hash,
                &compacted.summary_text,
                &compacted.compacted_through_turn_id,
            )
        })();
        state.finish_provider_run(&run_id);
        result
    })
    .await
    .map_err(|_| AppError::runtime_unavailable())?
}

#[tauri::command]
pub fn agent_write_policy_set(
    state: tauri::State<'_, ProjectRegistry>,
    input: AgentWritePolicySetInputV2,
) -> Result<AgentWritePolicySnapshotV2, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION || state.active_provider_id().is_some()
    {
        return Err(AppError::agent_contract("agent-write-policy.input-invalid"));
    }
    let handle = session_handle(&state, &input.project_session_id)?;
    let session = load_agent_session(&handle, false, now_ms())?;
    if session.session_id.is_none() {
        return Err(AppError::agent_contract("agent.session_not_ready"));
    }
    let mut runtime = lock_canon_runtime(&state)?;
    if runtime.pending_plans.contains_key(handle.project_id())
        || runtime.pending_conflicts.contains_key(handle.project_id())
    {
        return Err(AppError::agent_contract(
            "agent-write-policy.pending-action",
        ));
    }
    let current = (
        session.write_policy,
        session
            .write_policy_hash
            .clone()
            .ok_or_else(|| AppError::agent_contract("agent.session_not_ready"))?,
    );
    if current.1 != input.expected_policy_hash {
        return Err(AppError::agent_contract("agent-write-policy.hash-changed"));
    }
    let persisted =
        persist_agent_write_policy(&handle, &input.expected_policy_hash, input.policy, now_ms())?;
    let next_hash = persisted
        .write_policy_hash
        .ok_or_else(|| AppError::agent_contract("agent.session_not_ready"))?;
    runtime.policies.insert(
        handle.project_id().to_owned(),
        AgentPolicyRuntimeV2 {
            project_session_id: input.project_session_id,
            session_id: session
                .session_id
                .ok_or_else(|| AppError::agent_contract("agent.session_not_ready"))?,
            policy: input.policy,
            policy_hash: next_hash.clone(),
        },
    );
    Ok(AgentWritePolicySnapshotV2 {
        policy: input.policy,
        policy_hash: next_hash,
    })
}

#[tauri::command]
pub fn agent_context_mark_upsert(
    state: tauri::State<'_, ProjectRegistry>,
    input: AgentContextMarkUpsertInputV2,
) -> Result<ContextMarkStoreV1, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION || state.active_provider_id().is_some()
    {
        return Err(AppError::agent_contract("agent-mark.input-invalid"));
    }
    let handle = session_handle(&state, &input.project_session_id)?;
    let session = load_agent_session(&handle, false, now_ms())?;
    let source = source_ref_by_id(&handle, &input.source_id)?;
    if input.source_binding_hash != source.content_hash {
        return Err(AppError::agent_contract("agent-mark.binding-invalid"));
    }
    upsert_context_mark(
        &handle,
        UpsertContextMarkInputV1 {
            session_id: session
                .session_id
                .as_deref()
                .ok_or_else(|| AppError::agent_contract("agent.session_not_ready"))?,
            branch_id: session
                .active_branch_id
                .as_deref()
                .ok_or_else(|| AppError::agent_contract("agent.session_not_ready"))?,
            source: &source,
            scope: input.scope,
            priority: input.priority,
            creator: ContextMarkCreatorV1::User,
            now_ms: now_ms(),
        },
    )
}

#[tauri::command]
pub fn agent_context_mark_remove(
    state: tauri::State<'_, ProjectRegistry>,
    input: AgentContextMarkRemoveInputV2,
) -> Result<ContextMarkStoreV1, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION || state.active_provider_id().is_some()
    {
        return Err(AppError::agent_contract("agent-mark.input-invalid"));
    }
    let handle = session_handle(&state, &input.project_session_id)?;
    remove_context_mark(
        &handle,
        &input.mark_id,
        &input.binding_hash,
        ContextMarkCreatorV1::User,
    )
}

#[tauri::command]
pub fn agent_conflict_resolution_submit(
    state: tauri::State<'_, ProjectRegistry>,
    input: AgentConflictResolutionInputV2,
) -> Result<AgentConflictResolutionResponseV2, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION || state.active_provider_id().is_some()
    {
        return Err(AppError::agent_contract("agent-conflict.input-invalid"));
    }
    let handle = session_handle(&state, &input.project_session_id)?;
    let (resolution, write_plan) = {
        let mut runtime = lock_canon_runtime(&state)?;
        let pending = runtime
            .pending_conflicts
            .get(handle.project_id())
            .ok_or_else(|| AppError::agent_contract("agent-conflict.pending-missing"))?;
        if pending.project_session_id != input.project_session_id
            || pending.conflict_set.conflict_set_id != input.conflict_set_id
        {
            return Err(AppError::agent_contract("agent-conflict.binding-invalid"));
        }
        let resolution = resolve_conflict_set(
            &pending.conflict_set,
            &input.conflict_set_hash,
            input.resolution,
            input.confirmed,
        )?;
        let prepared = if resolution.requires_write_confirmation
            && input.resolution == ConflictResolutionChoiceV1::AdoptCandidate
        {
            Some(build_write_plan_v2(BuildWritePlanInputV2 {
                handle: &handle,
                persisted: &pending.candidate,
                policy: AuthorizationPolicyV1::ReviewEveryWrite,
                conflict_set_hash: Some(&pending.conflict_set.conflict_set_hash),
            })?)
        } else {
            discard_persisted_candidate(
                &handle,
                &pending.candidate.artifact_id,
                &pending.candidate.proposal.proposal_id,
            )?;
            None
        };
        runtime.pending_conflicts.remove(handle.project_id());
        let write_plan = prepared.map(|prepared| {
            let plan = prepared.plan.clone();
            runtime.pending_plans.insert(
                handle.project_id().to_owned(),
                PendingWriteRuntimeV2 {
                    project_session_id: input.project_session_id,
                    prepared,
                },
            );
            plan
        });
        (resolution, write_plan)
    };
    Ok(AgentConflictResolutionResponseV2 {
        resolution,
        write_plan,
    })
}

#[tauri::command]
pub fn agent_write_confirmation_submit(
    state: tauri::State<'_, ProjectRegistry>,
    input: AgentWriteConfirmationInputV2,
) -> Result<AgentCommitReceiptV2, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION || state.active_provider_id().is_some()
    {
        return Err(AppError::agent_contract("agent-write-plan.input-invalid"));
    }
    let handle = session_handle(&state, &input.project_session_id)?;
    let pending = {
        let mut runtime = lock_canon_runtime(&state)?;
        let pending = runtime
            .pending_plans
            .get(handle.project_id())
            .ok_or_else(|| AppError::agent_contract("agent-write-plan.pending-missing"))?;
        if pending.project_session_id != input.project_session_id {
            return Err(AppError::agent_contract(
                "agent-write-plan.confirmation-invalid",
            ));
        }
        validate_write_confirmation(
            &pending.prepared,
            &input.pending_id,
            &input.plan_hash,
            &input.source_binding_hash,
            &input.policy_hash,
            input.confirmed,
        )?;
        if !pending.prepared.plan.preview.can_confirm {
            return Err(AppError::agent_contract("agent-write-plan.policy-blocked"));
        }
        runtime
            .pending_plans
            .remove(handle.project_id())
            .expect("validated pending plan remains present while locked")
    };
    commit_write_plan(&state, &input.project_session_id, &pending.prepared)
}

#[tauri::command]
pub fn agent_write_plan_reject(
    state: tauri::State<'_, ProjectRegistry>,
    input: AgentWritePlanRejectInputV2,
) -> Result<AgentWritePlanRejectResultV2, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION || state.active_provider_id().is_some()
    {
        return Err(AppError::agent_contract("agent-write-plan.input-invalid"));
    }
    let handle = session_handle(&state, &input.project_session_id)?;
    let mut runtime = lock_canon_runtime(&state)?;
    let pending = runtime
        .pending_plans
        .get(handle.project_id())
        .ok_or_else(|| AppError::agent_contract("agent-write-plan.pending-missing"))?;
    if pending.project_session_id != input.project_session_id
        || pending.prepared.plan.pending_id != input.pending_id
        || pending.prepared.plan.plan_hash != input.plan_hash
    {
        return Err(AppError::agent_contract("agent-write-plan.binding-invalid"));
    }
    discard_persisted_candidate(
        &handle,
        &pending.prepared.plan.artifact_id,
        &pending.prepared.plan.proposal_id,
    )?;
    runtime.pending_plans.remove(handle.project_id());
    Ok(AgentWritePlanRejectResultV2 {
        rejected: true,
        project_write_count: 0,
    })
}

#[tauri::command]
pub fn agent_write_undo_apply(
    state: tauri::State<'_, ProjectRegistry>,
    input: AgentWriteUndoInputV2,
) -> Result<AgentUndoApplyResultV1, AppError> {
    if input.schema_version != AGENT_RUNTIME_SCHEMA_VERSION
        || !input.confirmed
        || state.active_provider_id().is_some()
        || !input.receipt_id.starts_with("rcpt_")
    {
        return Err(AppError::agent_contract("agent-undo.input-invalid"));
    }
    let handle = session_writer_handle(&state, &input.project_session_id)?;
    let transaction_hash = input
        .receipt_id
        .strip_prefix("rcpt_")
        .ok_or_else(|| AppError::agent_contract("agent-undo.binding-invalid"))?;
    let record = state
        .app_state
        .agent_undo_records(handle.project_id())?
        .into_iter()
        .find(|record| {
            record.transaction_hash.as_str() == transaction_hash
                && record.target_set_hash.as_str() == input.result_hash
                && record.status == UndoRecordStatusV1::Ready
        })
        .ok_or_else(|| AppError::agent_contract("agent-undo.binding-invalid"))?;
    let result = apply_agent_undo(
        &handle,
        &record,
        &state.recovery_store,
        &state.agent_undo_store,
    )?;
    state.app_state.set_agent_undo_status(
        handle.project_id(),
        record.transaction_hash.as_str(),
        UndoRecordStatusV1::Consumed,
    )?;
    Ok(result)
}

fn selected_ready_provider(
    state: &ProjectRegistry,
    expected_provider_id: ProviderId,
) -> Result<ProviderId, AppError> {
    let snapshot = load_provider_state(&state.app_state, state.credential_vault.as_ref())?;
    let selected = snapshot
        .providers
        .iter()
        .find(|provider| provider.selected)
        .ok_or_else(AppError::provider_not_ready)?;
    if selected.provider_id != expected_provider_id
        || !selected.configured
        || selected.connection_status != ProviderConnectionStatus::Connected
    {
        return Err(AppError::provider_not_ready());
    }
    Ok(selected.provider_id)
}

fn lock_canon_runtime(
    state: &ProjectRegistry,
) -> Result<std::sync::MutexGuard<'_, AgentCanonRuntimeV2>, AppError> {
    state
        .agent_canon_runtime
        .lock()
        .map_err(|_| AppError::runtime_unavailable())
}

fn current_write_policy(
    state: &ProjectRegistry,
    handle: &ProjectHandle,
    project_session_id: &str,
    session_id: &str,
) -> Result<AgentWritePolicySnapshotV2, AppError> {
    let runtime = lock_canon_runtime(state)?;
    if let Some(entry) = runtime.policies.get(handle.project_id()) {
        if entry.project_session_id == project_session_id && entry.session_id == session_id {
            return Ok(AgentWritePolicySnapshotV2 {
                policy: entry.policy,
                policy_hash: entry.policy_hash.clone(),
            });
        }
    }
    drop(runtime);
    let session = load_agent_session(handle, false, now_ms())?;
    if session.session_id.as_deref() != Some(session_id) {
        return Err(AppError::agent_contract("agent.session_not_ready"));
    }
    Ok(AgentWritePolicySnapshotV2 {
        policy: session.write_policy,
        policy_hash: session
            .write_policy_hash
            .ok_or_else(|| AppError::agent_contract("agent.session_not_ready"))?,
    })
}

fn commit_write_plan(
    state: &ProjectRegistry,
    project_session_id: &str,
    prepared: &PreparedWritePlanV2,
) -> Result<AgentCommitReceiptV2, AppError> {
    let handle = session_writer_handle(state, project_session_id)?;
    if handle.project_id() != prepared.plan.project_id {
        return Err(AppError::agent_contract(
            "agent-write-plan.project-mismatch",
        ));
    }
    let committed = commit_prepared_agent_write(
        &handle,
        &prepared.prepared,
        &state.recovery_store,
        &state.agent_undo_store,
        EpochMillisV1(now_ms()),
        0,
    )?;
    if let Err(error) = state
        .app_state
        .save_agent_undo_record(handle.project_id(), &committed.undo_record)
    {
        let rollback = apply_agent_undo(
            &handle,
            &committed.undo_record,
            &state.recovery_store,
            &state.agent_undo_store,
        );
        return Err(if rollback.is_ok() {
            error
        } else {
            AppError::project_recovery_required()
        });
    }
    let receipt = AgentCommitReceiptV2 {
        schema_version: 2,
        receipt_id: format!("rcpt_{}", committed.receipt.transaction_hash.as_str()),
        transaction_hash: committed.receipt.transaction_hash.as_str().to_owned(),
        result_hash: committed.undo_record.target_set_hash.as_str().to_owned(),
        relative_paths: committed
            .receipt
            .committed_results
            .iter()
            .map(|result| result.path.as_str().to_owned())
            .collect(),
        committed_at_ms: committed.receipt.committed_at_ms.0,
        undo_expires_at_ms: committed.undo_record.expires_at_ms.0,
        project_write_count: committed.project_write_count,
    };
    if let Err(error) = refresh_committed_context(&handle, prepared, &receipt) {
        let rollback = apply_agent_undo(
            &handle,
            &committed.undo_record,
            &state.recovery_store,
            &state.agent_undo_store,
        );
        if rollback.is_ok() {
            let _ = state.app_state.set_agent_undo_status(
                handle.project_id(),
                committed.undo_record.transaction_hash.as_str(),
                UndoRecordStatusV1::Consumed,
            );
            let _ = reconcile_context_marks(&handle, now_ms());
            return Err(error);
        }
        return Err(AppError::project_recovery_required());
    }
    Ok(receipt)
}

fn refresh_committed_context(
    handle: &ProjectHandle,
    prepared: &PreparedWritePlanV2,
    receipt: &AgentCommitReceiptV2,
) -> Result<(), AppError> {
    let index = ProjectContextIndexV2::build(handle)?;
    let sources = index.sources_for_relative_paths(
        &prepared.plan.turn_id,
        &receipt.relative_paths,
        receipt.relative_paths.len().max(1),
    )?;
    if sources.len() != receipt.relative_paths.len() {
        return Err(AppError::agent_contract("agent.context_refresh_incomplete"));
    }
    for source in &sources {
        upsert_context_mark(
            handle,
            UpsertContextMarkInputV1 {
                session_id: &prepared.plan.session_id,
                branch_id: &prepared.plan.branch_id,
                source,
                scope: ContextMarkScopeV1::Session,
                priority: ContextMarkPriorityV1::Medium,
                creator: ContextMarkCreatorV1::Agent,
                now_ms: now_ms(),
            },
        )?;
    }
    reconcile_context_marks(handle, now_ms())?;
    discard_persisted_candidate(
        handle,
        &prepared.plan.artifact_id,
        &prepared.plan.proposal_id,
    )?;
    Ok(())
}

fn project_has_pending_action(state: &ProjectRegistry, project_id: &str) -> Result<bool, AppError> {
    let runtime = lock_canon_runtime(state)?;
    Ok(runtime.pending_plans.contains_key(project_id)
        || runtime.pending_conflicts.contains_key(project_id))
}

fn context_mark_applies(mark: &ContextMarkV1, session_id: &str, branch_id: &str) -> bool {
    matches!(
        mark.status,
        ContextMarkStatusV1::Active | ContextMarkStatusV1::ReRead | ContextMarkStatusV1::Rebound
    ) && (mark.scope == ContextMarkScopeV1::Project
        || (mark.session_id == session_id && mark.branch_id == branch_id))
}

pub(super) fn clear_project_agent_runtime(
    state: &ProjectRegistry,
    handle: &ProjectHandle,
) -> Result<(), AppError> {
    let project_id = handle.project_id();
    let mut runtime = lock_canon_runtime(state)?;
    runtime.policies.remove(project_id);
    let pending_plan = runtime.pending_plans.remove(project_id).map(|pending| {
        (
            pending.prepared.plan.artifact_id,
            pending.prepared.plan.proposal_id,
        )
    });
    let pending_conflict = runtime.pending_conflicts.remove(project_id).map(|pending| {
        (
            pending.candidate.artifact_id,
            pending.candidate.proposal.proposal_id,
        )
    });
    drop(runtime);
    for (artifact_id, proposal_id) in pending_plan.into_iter().chain(pending_conflict) {
        discard_persisted_candidate(handle, &artifact_id, &proposal_id)?;
    }
    Ok(())
}

fn execute_agent_turn(app: tauri::AppHandle, context: AgentWorkerContext) {
    let state = app.state::<ProjectRegistry>();
    let mut sequence = 0_u32;
    let mut agent_canon_write_count = 0_u16;
    let mut tracker = match TurnBudgetTrackerV2::new(BudgetProfileV2::gar2_canon()) {
        Ok(tracker) => tracker,
        Err(_) => {
            finish_worker_with_error(
                &app,
                &state,
                &context,
                &mut sequence,
                AgentWorkerFailure {
                    usage: TurnBudgetUsageV2::default(),
                    error: AppError::agent_contract("agent.budget_invalid"),
                    selected_skill_ids: Vec::new(),
                    sources: Vec::new(),
                    agent_canon_write_count,
                },
            );
            return;
        }
    };
    let result = execute_agent_turn_inner(
        &app,
        &state,
        &context,
        &mut sequence,
        &mut tracker,
        &mut agent_canon_write_count,
    );
    if let Err((error, selected_skill_ids, sources)) = result {
        finish_worker_with_error(
            &app,
            &state,
            &context,
            &mut sequence,
            AgentWorkerFailure {
                usage: tracker.usage(),
                error,
                selected_skill_ids,
                sources,
                agent_canon_write_count,
            },
        );
        return;
    }
    state.finish_provider_run(&context.run_id);
}

fn execute_agent_turn_inner(
    app: &tauri::AppHandle,
    state: &ProjectRegistry,
    context: &AgentWorkerContext,
    sequence: &mut u32,
    tracker: &mut TurnBudgetTrackerV2,
    agent_canon_write_count: &mut u16,
) -> Result<(), (AppError, Vec<String>, Vec<SourceRefV2>)> {
    emit_turn_event(
        app,
        context,
        sequence,
        AgentTurnEventBodyV2::TurnState {
            state: "selecting_skills".to_owned(),
            visible_reason: None,
        },
    )
    .map_err(|error| (error, Vec::new(), Vec::new()))?;
    emit_turn_event(
        app,
        context,
        sequence,
        AgentTurnEventBodyV2::AnchorCandidate {
            resolution: context.anchor_resolution.clone(),
        },
    )
    .map_err(|error| (error, Vec::new(), Vec::new()))?;

    let client = ProviderHttpClient::production()
        .map_err(|error| (AppError::from_provider_error(error), Vec::new(), Vec::new()))?;
    let registry = RuntimeSkillRegistryV2::load_gar1_read_only();
    let catalog = registry
        .skills
        .iter()
        .map(|skill| {
            format!(
                "- {}：{}；{}",
                skill.id, skill.description, skill.trigger_summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let selection_request_id = format!("req_{}", Uuid::new_v4().simple());
    validate_invocation(
        &context.cancellation_id,
        &selection_request_id,
        ProviderModeV2::Control,
        CompletionContractV2::SkillSelectionV2,
    )
    .map_err(|error| (error, Vec::new(), Vec::new()))?;
    let selection = structured_selection(
        state,
        &client,
        context,
        tracker,
        &registry,
        &catalog,
        &selection_request_id,
    )
    .map_err(|error| (error, Vec::new(), Vec::new()))?;
    let selected_skill_ids = selection.selected_skill_ids.clone();
    emit_turn_event(
        app,
        context,
        sequence,
        AgentTurnEventBodyV2::SkillSelection {
            selected_skill_ids: selected_skill_ids.clone(),
            intent_class: selection.intent_class,
            visible_reason: selection.visible_reason.clone(),
        },
    )
    .map_err(|error| (error, selected_skill_ids.clone(), Vec::new()))?;

    let skill_bundle = registry
        .selected_provider_bundle(&selected_skill_ids)
        .map_err(|_| {
            (
                AppError::agent_contract("agent.skill_selection_invalid"),
                selected_skill_ids.clone(),
                Vec::new(),
            )
        })?;
    let skill_bundle = if skill_bundle.trim().is_empty() {
        "（无 Skill；仅允许产品帮助或不涉及项目 Canon 的普通交流）".to_owned()
    } else {
        skill_bundle
    };

    let initial_marks = reconcile_context_marks(&context.handle, now_ms())
        .map_err(|error| (error, selected_skill_ids.clone(), Vec::new()))?;
    let mut marked_paths = initial_marks
        .marks
        .iter()
        .filter(|mark| context_mark_applies(mark, &context.session_id, &context.branch_id))
        .collect::<Vec<_>>();
    marked_paths.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    let marked_paths = marked_paths
        .into_iter()
        .map(|mark| mark.relative_path.clone())
        .collect::<Vec<_>>();
    let mut all_sources = Vec::<SourceRefV2>::new();
    let mut query_history = Vec::<ContextQueryResultV2>::new();
    let mut index: Option<ProjectContextIndexV2> = None;
    let mut action_reason = selection.visible_reason.clone();
    let mut start_response_source_ids = Vec::new();
    let mut canon_directive: Option<(CanonTargetKindV1, Option<String>, String, Vec<String>)> =
        None;
    let mut mark_suggestions_to_apply = Vec::new();
    if !marked_paths.is_empty() {
        let context_index = ProjectContextIndexV2::build(&context.handle)
            .map_err(|error| (error, selected_skill_ids.clone(), Vec::new()))?;
        let marked_sources = context_index
            .sources_for_relative_paths(&context.turn_id, &marked_paths, 16)
            .map_err(|error| (error, selected_skill_ids.clone(), Vec::new()))?;
        let bytes = serde_json::to_vec(&marked_sources)
            .map_err(|_| {
                (
                    AppError::agent_contract("agent.context_result_invalid"),
                    selected_skill_ids.clone(),
                    Vec::new(),
                )
            })?
            .len() as u64;
        tracker.record_context_result(bytes).map_err(|_| {
            (
                AppError::agent_contract("agent.budget_exceeded"),
                selected_skill_ids.clone(),
                Vec::new(),
            )
        })?;
        if !marked_sources.is_empty() {
            emit_turn_event(
                app,
                context,
                sequence,
                AgentTurnEventBodyV2::ContextSources {
                    sources: marked_sources.clone(),
                    index_hash: context_index.stats().index_hash.clone(),
                },
            )
            .map_err(|error| (error, selected_skill_ids.clone(), Vec::new()))?;
            all_sources.extend(marked_sources);
        }
        index = Some(context_index);
    }
    for round in 0..MAX_CONTROL_ROUNDS {
        emit_turn_event(
            app,
            context,
            sequence,
            AgentTurnEventBodyV2::TurnState {
                state: if all_sources.is_empty() {
                    "planning_context"
                } else {
                    "gathering_context"
                }
                .to_owned(),
                visible_reason: Some(action_reason.clone()),
            },
        )
        .map_err(|error| (error, selected_skill_ids.clone(), all_sources.clone()))?;

        let action_request_id = format!("req_{}", Uuid::new_v4().simple());
        validate_invocation(
            &context.cancellation_id,
            &action_request_id,
            ProviderModeV2::Control,
            CompletionContractV2::AgentActionV2,
        )
        .map_err(|error| (error, selected_skill_ids.clone(), all_sources.clone()))?;
        let host_state = control_host_state(
            &selection,
            &all_sources,
            &query_history,
            &context.anchor_resolution,
        );
        let action = structured_action(
            state,
            &client,
            context,
            tracker,
            &selected_skill_ids,
            &skill_bundle,
            &host_state,
            &action_request_id,
        )
        .map_err(|error| (error, selected_skill_ids.clone(), all_sources.clone()))?;

        match action {
            AgentControlActionV2::RequestContext {
                visible_reason,
                queries,
                ..
            } => {
                action_reason = visible_reason;
                let context_index = match index.take() {
                    Some(index) => index,
                    None => ProjectContextIndexV2::build(&context.handle).map_err(|error| {
                        (error, selected_skill_ids.clone(), all_sources.clone())
                    })?,
                };
                let results = context_index
                    .query(&context.turn_id, &queries, MAX_CONTEXT_SOURCES_PER_QUERY)
                    .map_err(|error| (error, selected_skill_ids.clone(), all_sources.clone()))?;
                let new_sources = flatten_unique_sources(&all_sources, &results);
                let bytes = serde_json::to_vec(&new_sources)
                    .map_err(|_| {
                        (
                            AppError::agent_contract("agent.context_result_invalid"),
                            selected_skill_ids.clone(),
                            all_sources.clone(),
                        )
                    })?
                    .len() as u64;
                tracker.record_context_result(bytes).map_err(|_| {
                    (
                        AppError::agent_contract("agent.budget_exceeded"),
                        selected_skill_ids.clone(),
                        all_sources.clone(),
                    )
                })?;
                all_sources.extend(new_sources.clone());
                emit_turn_event(
                    app,
                    context,
                    sequence,
                    AgentTurnEventBodyV2::ContextSources {
                        sources: new_sources,
                        index_hash: context_index.stats().index_hash.clone(),
                    },
                )
                .map_err(|error| (error, selected_skill_ids.clone(), all_sources.clone()))?;
                query_history.extend(results);
                index = Some(context_index);
            }
            AgentControlActionV2::StartResponse {
                visible_reason,
                response_brief,
                source_ids,
                mark_suggestions,
                ..
            } => {
                let known = all_sources
                    .iter()
                    .map(|source| source.source_id.as_str())
                    .collect::<BTreeSet<_>>();
                if source_ids
                    .iter()
                    .any(|source| !known.contains(source.as_str()))
                {
                    return Err((
                        AppError::agent_contract("agent.source_binding_invalid"),
                        selected_skill_ids,
                        all_sources,
                    ));
                }
                if mark_suggestions
                    .iter()
                    .any(|suggestion| !known.contains(suggestion.source_id.as_str()))
                {
                    return Err((
                        AppError::agent_contract("agent.source_binding_invalid"),
                        selected_skill_ids,
                        all_sources,
                    ));
                }
                mark_suggestions_to_apply = mark_suggestions;
                action_reason = visible_reason;
                start_response_source_ids = source_ids.clone();
                if let AnchorResolutionV1::Ready { anchor } = &context.anchor_resolution {
                    canon_directive = Some((
                        canon_target(anchor.target_kind),
                        anchor.display_name.clone(),
                        response_brief,
                        source_ids,
                    ));
                }
                break;
            }
            AgentControlActionV2::SubmitCanonProposal {
                visible_reason,
                target_kind,
                display_name,
                proposal_brief,
                source_ids,
                ..
            } => {
                let AnchorResolutionV1::Ready { anchor } = &context.anchor_resolution else {
                    return Err((
                        AppError::agent_contract("agent-canon.anchor-required"),
                        selected_skill_ids,
                        all_sources,
                    ));
                };
                if canon_target(anchor.target_kind) != target_kind {
                    return Err((
                        AppError::agent_contract("agent-canon.target-mismatch"),
                        selected_skill_ids,
                        all_sources,
                    ));
                }
                let known = all_sources
                    .iter()
                    .map(|source| source.source_id.as_str())
                    .collect::<BTreeSet<_>>();
                if source_ids
                    .iter()
                    .any(|source| !known.contains(source.as_str()))
                {
                    return Err((
                        AppError::agent_contract("agent.source_binding_invalid"),
                        selected_skill_ids,
                        all_sources,
                    ));
                }
                action_reason = visible_reason;
                start_response_source_ids = source_ids.clone();
                canon_directive = Some((target_kind, display_name, proposal_brief, source_ids));
                break;
            }
        }
        if round + 1 == MAX_CONTROL_ROUNDS {
            return Err((
                AppError::agent_contract("agent.control_round_limit"),
                selected_skill_ids,
                all_sources,
            ));
        }
    }

    if let Some(index) = &index {
        index
            .verify_unchanged(&context.handle)
            .map_err(|error| (error, selected_skill_ids.clone(), all_sources.clone()))?;
    }
    let conversation_sources = all_sources
        .iter()
        .filter(|source| start_response_source_ids.contains(&source.source_id))
        .cloned()
        .collect::<Vec<_>>();
    let canon_candidate = if let Some((target_kind, display_name, proposal_brief, _)) =
        canon_directive
    {
        let AnchorResolutionV1::Ready { anchor } = &context.anchor_resolution else {
            unreachable!("canon directive is accepted only for a ready Anchor");
        };
        emit_turn_event(
            app,
            context,
            sequence,
            AgentTurnEventBodyV2::TurnState {
                state: "reconciling".to_owned(),
                visible_reason: Some(action_reason.clone()),
            },
        )
        .map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        let session = load_agent_session(&context.handle, false, now_ms()).map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        let artifact = materialize_draft_artifact(MaterializeDraftInputV1 {
            snapshot: &session,
            anchor,
            current_user_message_id: &context.current_user_message_id,
            current_user_text: &context.user_text,
            now_ms: now_ms(),
        })
        .map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        persist_draft_artifact(&context.handle, &artifact).map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        let proposal_request_id = format!("req_{}", Uuid::new_v4().simple());
        let proposal = if target_kind == CanonTargetKindV1::Chapter {
            chapter_proposal_from_artifact(
                proposal_request_id,
                display_name.unwrap_or_else(|| "新章节".to_owned()),
                &artifact,
            )
        } else {
            validate_invocation(
                &context.cancellation_id,
                &proposal_request_id,
                ProviderModeV2::Reconciliation,
                CompletionContractV2::CanonProposalV1,
            )
            .map_err(|error| {
                (
                    error,
                    selected_skill_ids.clone(),
                    conversation_sources.clone(),
                )
            })?;
            structured_reconciliation(
                state,
                &client,
                context,
                tracker,
                &selected_skill_ids,
                &skill_bundle,
                &artifact,
                target_kind,
                display_name.as_deref(),
                &proposal_brief,
                &conversation_sources,
                &proposal_request_id,
            )
            .map_err(|error| {
                (
                    error,
                    selected_skill_ids.clone(),
                    conversation_sources.clone(),
                )
            })?
        };
        let allowed_sources = conversation_sources
            .iter()
            .map(|source| (source.source_id.clone(), source.excerpt_hash.clone()))
            .collect::<Vec<_>>();
        let persisted = bind_canon_proposal(
            &context.handle,
            &session,
            anchor,
            &artifact,
            proposal,
            &allowed_sources,
            now_ms(),
        )
        .map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        persist_canon_proposal(&context.handle, &persisted).map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        emit_turn_event(
            app,
            context,
            sequence,
            AgentTurnEventBodyV2::CanonCandidate {
                artifact: artifact.clone(),
                proposal: persisted.clone(),
            },
        )
        .map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        Some((artifact, persisted))
    } else {
        None
    };
    if !mark_suggestions_to_apply.is_empty() {
        let session = load_agent_session(&context.handle, false, now_ms()).map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        for suggestion in &mark_suggestions_to_apply {
            let source = all_sources
                .iter()
                .find(|source| source.source_id == suggestion.source_id)
                .ok_or_else(|| {
                    (
                        AppError::agent_contract("agent.source_binding_invalid"),
                        selected_skill_ids.clone(),
                        conversation_sources.clone(),
                    )
                })?;
            upsert_context_mark(
                &context.handle,
                UpsertContextMarkInputV1 {
                    session_id: session.session_id.as_deref().ok_or_else(|| {
                        (
                            AppError::agent_contract("agent.session_not_ready"),
                            selected_skill_ids.clone(),
                            conversation_sources.clone(),
                        )
                    })?,
                    branch_id: session.active_branch_id.as_deref().ok_or_else(|| {
                        (
                            AppError::agent_contract("agent.session_not_ready"),
                            selected_skill_ids.clone(),
                            conversation_sources.clone(),
                        )
                    })?,
                    source,
                    scope: ContextMarkScopeV1::Session,
                    priority: ContextMarkPriorityV1::Medium,
                    creator: ContextMarkCreatorV1::Agent,
                    now_ms: now_ms(),
                },
            )
            .map_err(|error| {
                (
                    error,
                    selected_skill_ids.clone(),
                    conversation_sources.clone(),
                )
            })?;
        }
    }
    let marks = reconcile_context_marks(&context.handle, now_ms()).map_err(|error| {
        (
            error,
            selected_skill_ids.clone(),
            conversation_sources.clone(),
        )
    })?;
    emit_turn_event(
        app,
        context,
        sequence,
        AgentTurnEventBodyV2::ContextMarks { store: marks },
    )
    .map_err(|error| {
        (
            error,
            selected_skill_ids.clone(),
            conversation_sources.clone(),
        )
    })?;
    let mut canon_write_state = None::<String>;
    if let Some((_, persisted)) = canon_candidate.as_ref() {
        emit_turn_event(
            app,
            context,
            sequence,
            AgentTurnEventBodyV2::TurnState {
                state: "conflict_check".to_owned(),
                visible_reason: Some("正在核对现有 Canon 与新候选。".to_owned()),
            },
        )
        .map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        let target =
            normalize_canon_target(&context.handle, &persisted.proposal).map_err(|error| {
                (
                    error,
                    selected_skill_ids.clone(),
                    conversation_sources.clone(),
                )
            })?;
        let (current_hash, evidence) = if target.existing_source_hash.is_some() {
            let document =
                app_core::workspace::read_project_file(&context.handle, &target.relative_path)
                    .map_err(|error| {
                        (
                            error,
                            selected_skill_ids.clone(),
                            conversation_sources.clone(),
                        )
                    })?;
            let source_id = format!("src_existing_{}", &document.content_hash[..12]);
            (
                Some(document.content_hash.clone()),
                evidence_from_markdown_facts(
                    &target.relative_path,
                    &source_id,
                    &document.content_hash,
                    &document.content,
                ),
            )
        } else {
            (None, Vec::new())
        };
        let conflicts = detect_conflicts(
            persisted,
            &target,
            current_hash.as_deref(),
            &evidence,
            now_ms(),
        )
        .map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
        if let Some(conflict_set) = conflicts {
            if conflict_set.requires_resolution {
                lock_canon_runtime(state)
                    .map_err(|error| {
                        (
                            error,
                            selected_skill_ids.clone(),
                            conversation_sources.clone(),
                        )
                    })?
                    .pending_conflicts
                    .insert(
                        context.handle.project_id().to_owned(),
                        PendingConflictRuntimeV2 {
                            project_session_id: context.project_session_id.clone(),
                            candidate: persisted.clone(),
                            conflict_set: conflict_set.clone(),
                        },
                    );
            }
            emit_turn_event(
                app,
                context,
                sequence,
                AgentTurnEventBodyV2::Conflict {
                    conflict_set: conflict_set.clone(),
                },
            )
            .map_err(|error| {
                (
                    error,
                    selected_skill_ids.clone(),
                    conversation_sources.clone(),
                )
            })?;
            canon_write_state = Some(if conflict_set.requires_resolution {
                "awaiting_conflict_resolution".to_owned()
            } else {
                "clarification_required".to_owned()
            });
        } else {
            let session_id = load_agent_session(&context.handle, false, now_ms())
                .map_err(|error| {
                    (
                        error,
                        selected_skill_ids.clone(),
                        conversation_sources.clone(),
                    )
                })?
                .session_id
                .ok_or_else(|| {
                    (
                        AppError::agent_contract("agent.session_not_ready"),
                        selected_skill_ids.clone(),
                        conversation_sources.clone(),
                    )
                })?;
            let policy = current_write_policy(
                state,
                &context.handle,
                &context.project_session_id,
                &session_id,
            )
            .map_err(|error| {
                (
                    error,
                    selected_skill_ids.clone(),
                    conversation_sources.clone(),
                )
            })?;
            let prepared = build_write_plan_v2(BuildWritePlanInputV2 {
                handle: &context.handle,
                persisted,
                policy: policy.policy,
                conflict_set_hash: None,
            })
            .map_err(|error| {
                (
                    error,
                    selected_skill_ids.clone(),
                    conversation_sources.clone(),
                )
            })?;
            emit_turn_event(
                app,
                context,
                sequence,
                AgentTurnEventBodyV2::WritePlan {
                    plan: prepared.plan.clone(),
                },
            )
            .map_err(|error| {
                (
                    error,
                    selected_skill_ids.clone(),
                    conversation_sources.clone(),
                )
            })?;
            if prepared.plan.preview.authorization_effect == AuthorizationEffectV1::Execute
                && !prepared.plan.preview.requires_confirmation
            {
                if context.cancellation.is_cancelled() {
                    return Err((
                        AppError::agent_contract("agent.cancelled"),
                        selected_skill_ids,
                        conversation_sources,
                    ));
                }
                emit_turn_event(
                    app,
                    context,
                    sequence,
                    AgentTurnEventBodyV2::TurnState {
                        state: "committing".to_owned(),
                        visible_reason: Some("正在以单个原子事务提交安全变更。".to_owned()),
                    },
                )
                .map_err(|error| {
                    (
                        error,
                        selected_skill_ids.clone(),
                        conversation_sources.clone(),
                    )
                })?;
                tracker
                    .reserve_canon_writes(prepared.prepared.mutations.len() as u16)
                    .map_err(|_| {
                        (
                            AppError::agent_contract("agent.budget_exceeded"),
                            selected_skill_ids.clone(),
                            conversation_sources.clone(),
                        )
                    })?;
                let receipt = commit_write_plan(state, &context.project_session_id, &prepared)
                    .map_err(|error| {
                        (
                            error,
                            selected_skill_ids.clone(),
                            conversation_sources.clone(),
                        )
                    })?;
                *agent_canon_write_count =
                    agent_canon_write_count.saturating_add(receipt.project_write_count);
                emit_turn_event(
                    app,
                    context,
                    sequence,
                    AgentTurnEventBodyV2::CommitReceipt {
                        receipt: receipt.clone(),
                    },
                )
                .map_err(|error| {
                    (
                        error,
                        selected_skill_ids.clone(),
                        conversation_sources.clone(),
                    )
                })?;
                let refreshed_marks =
                    reconcile_context_marks(&context.handle, now_ms()).map_err(|error| {
                        (
                            error,
                            selected_skill_ids.clone(),
                            conversation_sources.clone(),
                        )
                    })?;
                emit_turn_event(
                    app,
                    context,
                    sequence,
                    AgentTurnEventBodyV2::ContextMarks {
                        store: refreshed_marks,
                    },
                )
                .map_err(|error| {
                    (
                        error,
                        selected_skill_ids.clone(),
                        conversation_sources.clone(),
                    )
                })?;
                canon_write_state = Some("committed".to_owned());
            } else {
                let state_name = if prepared.plan.preview.can_confirm {
                    "awaiting_write_confirmation"
                } else {
                    "plan_only"
                };
                lock_canon_runtime(state)
                    .map_err(|error| {
                        (
                            error,
                            selected_skill_ids.clone(),
                            conversation_sources.clone(),
                        )
                    })?
                    .pending_plans
                    .insert(
                        context.handle.project_id().to_owned(),
                        PendingWriteRuntimeV2 {
                            project_session_id: context.project_session_id.clone(),
                            prepared,
                        },
                    );
                canon_write_state = Some(state_name.to_owned());
            }
        }
    }
    emit_turn_event(
        app,
        context,
        sequence,
        AgentTurnEventBodyV2::TurnState {
            state: "responding".to_owned(),
            visible_reason: Some(action_reason),
        },
    )
    .map_err(|error| {
        (
            error,
            selected_skill_ids.clone(),
            conversation_sources.clone(),
        )
    })?;

    let request_id = format!("req_{}", Uuid::new_v4().simple());
    validate_invocation(
        &context.cancellation_id,
        &request_id,
        ProviderModeV2::Conversation,
        CompletionContractV2::ConversationTextV1,
    )
    .map_err(|error| {
        (
            error,
            selected_skill_ids.clone(),
            conversation_sources.clone(),
        )
    })?;
    let session = load_agent_session(&context.handle, false, now_ms()).map_err(|error| {
        (
            error,
            selected_skill_ids.clone(),
            conversation_sources.clone(),
        )
    })?;
    let mut messages = session.messages.iter().rev().take(30).collect::<Vec<_>>();
    messages.reverse();
    let mut provider_messages = messages
        .into_iter()
        .map(|message| NormalizedMessage {
            role: match message.role {
                app_core::agent_v2::ConversationRoleV2::User => MessageRole::User,
                app_core::agent_v2::ConversationRoleV2::Assistant => MessageRole::Assistant,
            },
            content: message.body.clone(),
        })
        .collect::<Vec<_>>();
    provider_messages.push(NormalizedMessage {
        role: MessageRole::User,
        content: context.user_text.clone(),
    });
    let host_context = conversation_host_context(
        session.memory.as_ref(),
        &conversation_sources,
        &context.anchor_resolution,
        canon_candidate.as_ref(),
        canon_write_state.as_deref(),
    );
    let request = NormalizedRequest::agent_v2_conversation(
        provider_messages,
        skill_bundle,
        host_context,
        MAX_OUTPUT_TOKENS,
    )
    .map_err(|error| {
        (
            AppError::from_provider_error(error),
            selected_skill_ids.clone(),
            conversation_sources.clone(),
        )
    })?;
    reserve_request(tracker, &request).map_err(|error| {
        (
            error,
            selected_skill_ids.clone(),
            conversation_sources.clone(),
        )
    })?;
    let response =
        stream_with_vault(state, &client, context, &request, app, sequence).map_err(|error| {
            (
                error,
                selected_skill_ids.clone(),
                conversation_sources.clone(),
            )
        })?;
    record_response(tracker, &response).map_err(|error| {
        (
            error,
            selected_skill_ids.clone(),
            conversation_sources.clone(),
        )
    })?;
    if response.finish_reason != FinishReason::Stop || response.text.trim().is_empty() {
        return Err((
            AppError::agent_contract("agent.conversation_incomplete"),
            selected_skill_ids,
            conversation_sources,
        ));
    }
    let persisted = append_completed_turn(
        &context.handle,
        CompletedTurnInputV2 {
            turn_id: &context.turn_id,
            user_message_id: Some(&context.current_user_message_id),
            user_content: &context.user_text,
            assistant_content: &response.text,
            selected_skill_ids,
            source_refs: conversation_sources.clone(),
            terminal_state: "completed",
            now_ms: now_ms(),
        },
    )
    .map_err(|error| (error, Vec::new(), conversation_sources.clone()))?;
    let message = persisted.messages.last().cloned().ok_or_else(|| {
        (
            AppError::agent_contract("agent.history_invalid"),
            Vec::new(),
            conversation_sources.clone(),
        )
    })?;
    emit_turn_event(
        app,
        context,
        sequence,
        AgentTurnEventBodyV2::Completed {
            message,
            sources: conversation_sources,
            usage: tracker.usage(),
            agent_canon_write_count: *agent_canon_write_count,
        },
    )
    .map_err(|error| (error, Vec::new(), Vec::new()))?;
    Ok(())
}

fn structured_selection(
    state: &ProjectRegistry,
    client: &ProviderHttpClient,
    context: &AgentWorkerContext,
    tracker: &mut TurnBudgetTrackerV2,
    registry: &RuntimeSkillRegistryV2,
    catalog: &str,
    request_id: &str,
) -> Result<SkillSelectionV2, AppError> {
    for attempt in 0..=1 {
        if attempt == 1 {
            tracker
                .record_structured_repair()
                .map_err(|_| AppError::agent_contract("agent.budget_exceeded"))?;
        }
        let user_text = if attempt == 0 {
            build_skill_selection_input(
                context.user_text.as_str(),
                &context.prior_selected_skill_ids,
            )
        } else {
            format!(
                "{}\n\n上一次输出未通过当前 SkillSelectionV2；只按当前固定 Schema 重试。除 product_help 外 selectedSkillIds 不得为空；“继续/重试”应参考 priorSelectedSkillIds 重新判断。",
                build_skill_selection_input(
                    context.user_text.as_str(),
                    &context.prior_selected_skill_ids,
                )
            )
        };
        let request = NormalizedRequest::agent_v2_skill_selection(
            user_text,
            catalog.to_owned(),
            request_id,
            1_024,
            skill_selection_v2_schema_source(),
        )
        .map_err(AppError::from_provider_error)?;
        reserve_request(tracker, &request)?;
        let response = complete_with_vault(
            state,
            client,
            context.provider_id,
            &request,
            &context.cancellation,
        )?;
        record_response(tracker, &response)?;
        if let Ok(selection) = serde_json::from_str::<SkillSelectionV2>(&response.text) {
            let project_context_requested =
                selection.intent_class != SkillIntentClassV2::ProductHelp;
            if registry
                .validate_selection(&selection, request_id, project_context_requested)
                .is_ok()
            {
                return Ok(selection);
            }
        }
    }
    Err(AppError::agent_contract("agent.skill_selection_invalid"))
}

#[allow(clippy::too_many_arguments)]
fn structured_action(
    state: &ProjectRegistry,
    client: &ProviderHttpClient,
    context: &AgentWorkerContext,
    tracker: &mut TurnBudgetTrackerV2,
    selected_skill_ids: &[String],
    skill_bundle: &str,
    host_state: &str,
    request_id: &str,
) -> Result<AgentControlActionV2, AppError> {
    for attempt in 0..=1 {
        if attempt == 1 {
            tracker
                .record_structured_repair()
                .map_err(|_| AppError::agent_contract("agent.budget_exceeded"))?;
        }
        let user_text = if attempt == 0 {
            context.user_text.clone()
        } else {
            format!(
                "{}\n\n上一次输出未通过当前 AgentControlActionV2；只按当前固定 Schema 重试。",
                context.user_text
            )
        };
        let request = NormalizedRequest::agent_v2_control(
            user_text,
            skill_bundle.to_owned(),
            host_state.to_owned(),
            request_id,
            2_048,
            agent_control_action_v2_schema_source(),
        )
        .map_err(AppError::from_provider_error)?;
        reserve_request(tracker, &request)?;
        let response = complete_with_vault(
            state,
            client,
            context.provider_id,
            &request,
            &context.cancellation,
        )?;
        record_response(tracker, &response)?;
        if let Ok(action) = serde_json::from_str::<AgentControlActionV2>(&response.text) {
            if action.validate(request_id, selected_skill_ids).is_ok() {
                return Ok(action);
            }
        }
    }
    Err(AppError::agent_contract("agent.control_action_invalid"))
}

#[allow(clippy::too_many_arguments)]
fn structured_reconciliation(
    state: &ProjectRegistry,
    client: &ProviderHttpClient,
    context: &AgentWorkerContext,
    tracker: &mut TurnBudgetTrackerV2,
    selected_skill_ids: &[String],
    skill_bundle: &str,
    artifact: &DraftArtifactV1,
    target_kind: CanonTargetKindV1,
    display_name: Option<&str>,
    proposal_brief: &str,
    sources: &[SourceRefV2],
    request_id: &str,
) -> Result<CanonProposalV1, AppError> {
    for attempt in 0..=1 {
        if attempt == 1 {
            tracker
                .record_structured_repair()
                .map_err(|_| AppError::agent_contract("agent.budget_exceeded"))?;
        }
        let anchor_input = serde_json::json!({
            "targetKind": target_kind,
            "displayName": display_name,
            "proposalBrief": proposal_brief,
            "artifact": {
                "artifactId": artifact.artifact_id,
                "content": artifact.content,
                "contentHash": artifact.content_hash,
                "sourceBindings": artifact.source_bindings
            },
            "repair": attempt == 1
        })
        .to_string();
        let mut source_data = sources
            .iter()
            .map(|source| {
                serde_json::json!({
                    "sourceId": source.source_id,
                    "sourceHash": source.excerpt_hash,
                    "excerpt": source.excerpt,
                    "trust": "untrusted_data"
                })
            })
            .collect::<Vec<_>>();
        source_data.push(serde_json::json!({
            "sourceId": artifact.artifact_id,
            "sourceHash": artifact.content_hash,
            "trust": "host_verified_visible_artifact"
        }));
        let request = NormalizedRequest::agent_v2_reconciliation(
            anchor_input,
            skill_bundle.to_owned(),
            serde_json::Value::Array(source_data).to_string(),
            request_id,
            4_096,
            canon_proposal_v1_schema_source(),
        )
        .map_err(AppError::from_provider_error)?;
        reserve_request(tracker, &request)?;
        let response = complete_with_vault(
            state,
            client,
            context.provider_id,
            &request,
            &context.cancellation,
        )?;
        record_response(tracker, &response)?;
        if let Ok(proposal) = serde_json::from_str::<CanonProposalV1>(&response.text) {
            let mut allowed = sources
                .iter()
                .map(|source| (source.source_id.clone(), source.excerpt_hash.clone()))
                .collect::<Vec<_>>();
            allowed.push((artifact.artifact_id.clone(), artifact.content_hash.clone()));
            for binding in &artifact.source_bindings {
                allowed.push((
                    binding.source_message_id.clone(),
                    binding.message_body_hash.clone(),
                ));
            }
            if proposal.target_kind == target_kind
                && proposal.validate(request_id, &allowed).is_ok()
                && !selected_skill_ids.is_empty()
            {
                return Ok(proposal);
            }
        }
    }
    Err(AppError::agent_contract("agent-canon.proposal-invalid"))
}

fn validate_invocation(
    cancellation_id: &str,
    request_id: &str,
    mode: ProviderModeV2,
    completion_contract: CompletionContractV2,
) -> Result<(), AppError> {
    ProviderInvocationV2 {
        request_id: request_id.to_owned(),
        mode,
        completion_contract,
        budget_lease_id: format!("bud_{}", Uuid::new_v4().simple()),
        cancellation_id: cancellation_id.to_owned(),
    }
    .validate()
    .map_err(|_| AppError::agent_contract("agent.provider_invocation_invalid"))
}

fn reserve_request(
    tracker: &mut TurnBudgetTrackerV2,
    request: &NormalizedRequest,
) -> Result<(), AppError> {
    let input_bytes = request
        .messages
        .iter()
        .map(|message| message.content.len() as u64)
        .sum::<u64>();
    tracker
        .reserve_provider_call(input_bytes, input_bytes.div_ceil(4))
        .map_err(|_| AppError::agent_contract("agent.budget_exceeded"))
}

fn record_response(
    tracker: &mut TurnBudgetTrackerV2,
    response: &NormalizedResponse,
) -> Result<(), AppError> {
    let output_bytes = response.text.len() as u64;
    let output_tokens = response
        .usage
        .map(|usage| usage.output_tokens)
        .unwrap_or_else(|| output_bytes.div_ceil(4));
    tracker
        .record_provider_output(output_bytes, output_tokens)
        .map_err(|_| AppError::agent_contract("agent.budget_exceeded"))
}

fn complete_with_vault(
    state: &ProjectRegistry,
    client: &ProviderHttpClient,
    provider_id: ProviderId,
    request: &NormalizedRequest,
    cancellation: &CancellationToken,
) -> Result<NormalizedResponse, AppError> {
    let mut result = None;
    state
        .credential_vault
        .with_secret(provider_id, &mut |key| {
            result = Some(
                client
                    .complete(provider_id, key, request, cancellation)
                    .map_err(AppError::from_provider_error),
            );
            Ok(())
        })?;
    result.ok_or_else(AppError::provider_vault_unavailable)?
}

fn stream_with_vault(
    state: &ProjectRegistry,
    client: &ProviderHttpClient,
    context: &AgentWorkerContext,
    request: &NormalizedRequest,
    app: &tauri::AppHandle,
    sequence: &mut u32,
) -> Result<NormalizedResponse, AppError> {
    let mut result = None;
    state
        .credential_vault
        .with_secret(context.provider_id, &mut |key| {
            result = Some(
                client
                    .stream(
                        context.provider_id,
                        key,
                        request,
                        &context.cancellation,
                        &mut |event| {
                            if let NormalizedEvent::TextDelta { delta, .. } = event {
                                emit_turn_event(
                                    app,
                                    context,
                                    sequence,
                                    AgentTurnEventBodyV2::ConversationDelta { delta },
                                )
                                .map_err(|_| ProviderError::cancelled())?;
                            }
                            Ok(())
                        },
                    )
                    .map_err(AppError::from_provider_error),
            );
            Ok(())
        })?;
    result.ok_or_else(AppError::provider_vault_unavailable)?
}

fn control_host_state(
    selection: &SkillSelectionV2,
    sources: &[SourceRefV2],
    query_history: &[ContextQueryResultV2],
    anchor_resolution: &AnchorResolutionV1,
) -> String {
    let source_data = sources
        .iter()
        .map(|source| {
            serde_json::json!({
                "sourceId": source.source_id,
                "entity": source.entity,
                "excerpt": source.excerpt,
                "excerptHash": source.excerpt_hash,
                "trust": "untrusted_data"
            })
        })
        .collect::<Vec<_>>();
    let query_data = query_history
        .iter()
        .map(|result| {
            serde_json::json!({
                "kind": result.query.kind,
                "terms": result.query.terms,
                "purpose": result.query.purpose,
                "matchedSourceIds": result
                    .sources
                    .iter()
                    .map(|source| source.source_id.as_str())
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let last_query_had_no_matches = query_history
        .last()
        .is_some_and(|result| result.sources.is_empty());
    serde_json::json!({
        "schemaVersion": 2,
        "amR2Anchor": true,
        "selectedSkillIds": selection.selected_skill_ids,
        "intentClass": selection.intent_class,
        "contextSources": source_data,
        "completedContextQueries": query_data,
        "lastQueryHadNoMatches": last_query_had_no_matches,
        "nextActionGuidance": if last_query_had_no_matches {
            "No matching project source was found. Do not repeat the same query; return start_response with the verified sourceIds already available, which may be empty."
        } else {
            "Request only additional context that is still necessary, otherwise return start_response."
        },
        "anchorResolution": anchor_resolution,
        "canonProposalAllowed": matches!(anchor_resolution, AnchorResolutionV1::Ready { .. }),
        "canonWritesAllowed": false,
        "marksAllowed": false
    })
    .to_string()
}

fn build_skill_selection_input(current_user_text: &str, prior_skill_ids: &[String]) -> String {
    let prior = serde_json::to_string(prior_skill_ids)
        .expect("validated prior Skill IDs always serialize to JSON");
    format!(
        "Host 承接元数据（不含历史正文，不授予权限）：\npriorSelectedSkillIds: {prior}\n\n当前作者消息：\n{current_user_text}"
    )
}

fn conversation_host_context(
    memory: Option<&SessionMemoryV2>,
    sources: &[SourceRefV2],
    anchor_resolution: &AnchorResolutionV1,
    canon_candidate: Option<&(DraftArtifactV1, PersistedCanonProposalV1)>,
    canon_write_state: Option<&str>,
) -> String {
    let source_data = sources
        .iter()
        .map(|source| {
            serde_json::json!({
                "sourceId": source.source_id,
                "entity": source.entity,
                "headingPath": source.heading_path,
                "excerpt": source.excerpt,
                "excerptHash": source.excerpt_hash,
                "trust": "untrusted_data"
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "memory": memory.map(|value| serde_json::json!({
            "summary": value.summary,
            "lastTurnId": value.last_turn_id,
            "memoryHash": value.memory_hash
        })),
        "sources": source_data,
        "anchorResolution": anchor_resolution,
        "canonCandidate": canon_candidate.map(|(_, candidate)| serde_json::json!({
            "proposalId": candidate.proposal.proposal_id,
            "targetKind": candidate.proposal.target_kind,
            "displayName": candidate.proposal.display_name,
            "proposalHash": candidate.proposal_hash,
            "state": "prepared_not_committed"
        })),
        "canonWriteState": canon_write_state,
        "anchorGuidance": match anchor_resolution {
            AnchorResolutionV1::ClarificationRequired { visible_reason } => {
                format!("只进行自然澄清，不声称已写入 Canon：{visible_reason}")
            }
            AnchorResolutionV1::Ready { .. } => {
                "Host 已验证本轮 Anchor；当前阶段只向作者自然确认目标，不能声称已写入 Canon。".to_owned()
            }
            AnchorResolutionV1::NoAnchor => {
                "普通 Conversation，不要求生成 Proposal，也不应因无 Proposal 失败。".to_owned()
            }
        }
    })
    .to_string()
}

fn canon_target(kind: AnchorTargetKindV1) -> CanonTargetKindV1 {
    match kind {
        AnchorTargetKindV1::Character => CanonTargetKindV1::Character,
        AnchorTargetKindV1::Setting => CanonTargetKindV1::Setting,
        AnchorTargetKindV1::Material => CanonTargetKindV1::Material,
        AnchorTargetKindV1::Plot => CanonTargetKindV1::Plot,
        AnchorTargetKindV1::Chapter => CanonTargetKindV1::Chapter,
    }
}

fn flatten_unique_sources(
    existing: &[SourceRefV2],
    results: &[ContextQueryResultV2],
) -> Vec<SourceRefV2> {
    let mut known = existing
        .iter()
        .map(|source| source.source_id.clone())
        .collect::<BTreeSet<_>>();
    results
        .iter()
        .flat_map(|result| result.sources.iter())
        .filter(|source| known.insert(source.source_id.clone()))
        .cloned()
        .collect()
}

struct AgentWorkerFailure {
    usage: TurnBudgetUsageV2,
    error: AppError,
    selected_skill_ids: Vec<String>,
    sources: Vec<SourceRefV2>,
    agent_canon_write_count: u16,
}

fn finish_worker_with_error(
    app: &tauri::AppHandle,
    state: &ProjectRegistry,
    context: &AgentWorkerContext,
    sequence: &mut u32,
    failure: AgentWorkerFailure,
) {
    let AgentWorkerFailure {
        usage,
        error,
        selected_skill_ids,
        sources,
        agent_canon_write_count,
    } = failure;
    let cancelled = context.cancellation.is_cancelled()
        || error.code == "provider.cancelled"
        || error.code == "agent.cancelled";
    let (body, terminal_state) = if cancelled {
        ("本次回复已取消。", "cancelled")
    } else {
        ("本次回复未完成，请重试。", "failed")
    };
    let message = append_completed_turn(
        &context.handle,
        CompletedTurnInputV2 {
            turn_id: &context.turn_id,
            user_message_id: Some(&context.current_user_message_id),
            user_content: &context.user_text,
            assistant_content: body,
            selected_skill_ids,
            source_refs: sources,
            terminal_state,
            now_ms: now_ms(),
        },
    )
    .ok()
    .and_then(|snapshot| snapshot.messages.last().cloned());
    let event = if cancelled {
        AgentTurnEventBodyV2::Cancelled {
            message,
            usage,
            agent_canon_write_count,
        }
    } else {
        AgentTurnEventBodyV2::Failed {
            message,
            error,
            usage,
            agent_canon_write_count,
        }
    };
    let _ = emit_turn_event(app, context, sequence, event);
    state.finish_provider_run(&context.run_id);
}

fn emit_turn_event(
    app: &tauri::AppHandle,
    context: &AgentWorkerContext,
    sequence: &mut u32,
    event: AgentTurnEventBodyV2,
) -> Result<(), AppError> {
    *sequence = sequence.saturating_add(1);
    app.emit(
        AGENT_TURN_EVENT_CHANNEL,
        AgentTurnEventV2 {
            schema_version: AGENT_RUNTIME_SCHEMA_VERSION,
            run_id: context.run_id.clone(),
            turn_id: context.turn_id.clone(),
            project_session_id: context.project_session_id.clone(),
            provider_id: context.provider_id,
            model: context.model.clone(),
            sequence: *sequence,
            event,
        },
    )
    .map_err(|_| AppError::runtime_unavailable())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_v2_bundled_complete_and_read_only() {
        let registry = agent_skill_registry_load();
        assert_eq!(registry.schema_version, 2);
        assert_eq!(registry.skills.len(), 9);
        assert_eq!(registry.eligible_skill_ids.len(), 9);
        assert_eq!(registry.runtime_source, "bundled_release_only");
        assert!(registry
            .skills
            .iter()
            .all(|skill| !skill.resources.is_empty()));
    }

    #[test]
    fn source_accumulation_never_loses_a_prior_tool_result() {
        let source = |id: &str, path: &str| SourceRefV2 {
            source_id: id.to_owned(),
            relative_path: path.to_owned(),
            entity: None,
            field_path: None,
            heading_path: Vec::new(),
            content_hash: "a".repeat(64),
            excerpt_hash: "b".repeat(64),
            excerpt: "excerpt".to_owned(),
            truncated: false,
            untrusted: true,
        };
        let first = source("src_12345678", "characters/a.md");
        let duplicate = first.clone();
        let second = source("src_87654321", "world/a.md");
        let results = vec![ContextQueryResultV2 {
            query: novelgainker_agent::v2::ContextQueryV2 {
                kind: novelgainker_agent::v2::ContextQueryKindV2::FullText,
                terms: vec!["规则".to_owned()],
                known_entity_ids: Vec::new(),
                known_source_ids: Vec::new(),
                purpose: "测试累计".to_owned(),
            },
            sources: vec![duplicate, second.clone()],
        }];
        assert_eq!(flatten_unique_sources(&[first], &results), vec![second]);
    }

    #[test]
    fn control_host_state_exposes_completed_empty_queries_to_prevent_repetition() {
        let selection = SkillSelectionV2 {
            schema_version: 2,
            request_id: "req_12345678".to_owned(),
            selected_skill_ids: vec!["novel-story-telling".to_owned()],
            intent_class: SkillIntentClassV2::Composition,
            visible_reason: "继续故事".to_owned(),
        };
        let history = vec![ContextQueryResultV2 {
            query: novelgainker_agent::v2::ContextQueryV2 {
                kind: novelgainker_agent::v2::ContextQueryKindV2::FullText,
                terms: vec!["尚未建立的人物".to_owned()],
                known_entity_ids: Vec::new(),
                known_source_ids: Vec::new(),
                purpose: "核对已有设定".to_owned(),
            },
            sources: Vec::new(),
        }];

        let state: serde_json::Value = serde_json::from_str(&control_host_state(
            &selection,
            &[],
            &history,
            &AnchorResolutionV1::NoAnchor,
        ))
        .expect("host state must be json");
        assert_eq!(state["lastQueryHadNoMatches"], true);
        assert_eq!(
            state["completedContextQueries"][0]["matchedSourceIds"],
            serde_json::json!([])
        );
        assert!(state["nextActionGuidance"]
            .as_str()
            .expect("guidance must be text")
            .contains("start_response"));
    }

    #[test]
    fn skill_selection_input_uses_only_prior_skill_metadata_for_follow_ups() {
        let input = build_skill_selection_input("重试", &["novel-story-telling".to_owned()]);

        assert!(input.contains("priorSelectedSkillIds: [\"novel-story-telling\"]"));
        assert!(input.contains("当前作者消息：\n重试"));
        assert!(!input.contains("流放到岭南的书生"));
    }

    #[test]
    fn turn_events_serialize_exactly_to_the_frontend_camel_case_contract() {
        let event = |event| AgentTurnEventV2 {
            schema_version: AGENT_RUNTIME_SCHEMA_VERSION,
            run_id: "run_10000000000040008000000000000001".to_owned(),
            turn_id: "trn_10000000000040008000000000000001".to_owned(),
            project_session_id: "desktop-session".to_owned(),
            provider_id: ProviderId::Openai,
            model: "gpt-5.6-terra".to_owned(),
            sequence: 1,
            event,
        };

        let state = serde_json::to_value(event(AgentTurnEventBodyV2::TurnState {
            state: "selecting_skills".to_owned(),
            visible_reason: None,
        }))
        .expect("turn state serializes");
        assert_eq!(state["kind"], "turnState");
        assert!(state.get("visibleReason").is_some());
        assert!(state.get("visible_reason").is_none());

        let selection = serde_json::to_value(event(AgentTurnEventBodyV2::SkillSelection {
            selected_skill_ids: vec!["create-character".to_owned()],
            intent_class: SkillIntentClassV2::ProjectDiscussion,
            visible_reason: "读取人物设定".to_owned(),
        }))
        .expect("skill selection serializes");
        assert!(selection.get("selectedSkillIds").is_some());
        assert!(selection.get("intentClass").is_some());
        assert!(selection.get("visibleReason").is_some());
        assert!(selection.get("selected_skill_ids").is_none());

        let cancelled = serde_json::to_value(event(AgentTurnEventBodyV2::Cancelled {
            message: None,
            usage: TurnBudgetUsageV2::default(),
            agent_canon_write_count: 0,
        }))
        .expect("cancelled terminal event serializes");
        assert_eq!(cancelled["kind"], "cancelled");
        assert_eq!(cancelled["agentCanonWriteCount"], 0);
        assert!(cancelled.get("agent_canon_write_count").is_none());
    }

    #[test]
    fn session_marks_are_branch_local_while_project_marks_cross_sessions() {
        let mark = |scope, session_id: &str, branch_id: &str| ContextMarkV1 {
            schema_version: 1,
            mark_id: "mark_12345678".to_owned(),
            project_id: "project-test".to_owned(),
            session_id: session_id.to_owned(),
            branch_id: branch_id.to_owned(),
            scope,
            priority: ContextMarkPriorityV1::Medium,
            creator: ContextMarkCreatorV1::User,
            source_id: "src_12345678".to_owned(),
            relative_path: "world/rule.md".to_owned(),
            field_path: None,
            heading_path: vec!["规则".to_owned()],
            anchor_hash: "a".repeat(64),
            document_hash: "b".repeat(64),
            status: ContextMarkStatusV1::Active,
            created_at_ms: 1,
            updated_at_ms: 1,
            binding_hash: "c".repeat(64),
        };
        let session_mark = mark(
            ContextMarkScopeV1::Session,
            "ses_oldbranch",
            "brn_oldbranch",
        );
        assert!(!context_mark_applies(
            &session_mark,
            "ses_current1",
            "brn_current1"
        ));
        let project_mark = mark(
            ContextMarkScopeV1::Project,
            "ses_oldbranch",
            "brn_oldbranch",
        );
        assert!(context_mark_applies(
            &project_mark,
            "ses_current1",
            "brn_current1"
        ));
    }
}
