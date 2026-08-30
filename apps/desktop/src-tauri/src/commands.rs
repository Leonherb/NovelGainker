use app_core::agent_transaction::{recover_incomplete_agent_transactions, AgentUndoStore};
use app_core::app_state::{AppStateRepository, AppStateSnapshot, UiPreferences};
#[cfg(test)]
use app_core::canon_apply::apply_canon_file_recoverable;
use app_core::coordination::{try_acquire_project_write_lease, ProjectWriteLease};
use app_core::credential_vault::{CredentialVault, ProviderId, WindowsCredentialVault};
use app_core::project::{
    create_prepared_project, inspect_project_root, open_project, prepare_project_create,
    PreparedProjectCreate, ProjectCreateInputV1, ProjectCreatePreview, ProjectHandle,
    ProjectInspection, ProjectScanMode, ProjectScanPreview,
};
use app_core::provider_core::CancellationToken;
#[cfg(test)]
use app_core::provider_core::{
    preset, FinishReason, MessageRole, NormalizedEvent, NormalizedMessage, NormalizedRequest,
    ProviderError, ProviderErrorKind, ProviderHttpClient, Usage, MAX_OUTPUT_TOKENS,
};
#[cfg(test)]
use app_core::provider_state::ProviderConnectionStatus;
use app_core::provider_state::{
    delete_provider_key, load_provider_state, save_provider_key, select_provider,
    test_provider_connection, ProviderStateSnapshot, ProviderStatus,
};
use app_core::recovery::{RecoveryReport, RecoveryStatus, RecoveryStore};
use app_core::search::{search_project_sources, SearchReport};
use app_core::workspace::{
    check_project_file, list_project_files, read_project_file, save_project_file_recoverable,
    validate_project_draft, ProjectFileDocument, ProjectFileIndex, ProjectFileSaveResult,
    ProjectFileWatchStatus, ValidationReport,
};
#[cfg(test)]
use app_core::workspace::{ProjectFileKind, ProjectFileNode, ProjectFileWatchState};
use app_core::{health_status, AppError, HealthStatus};
#[cfg(test)]
use novelgainker_workflows::context::{
    prepare_character_context, prepare_material_context, prepare_plot_context,
    prepare_setting_context, CharacterBriefModeV2, CharacterBriefV2, ContextBuildError,
    ContextManifestV1, ContextSourceInput, MaterialBriefV1, PlotBriefV1,
    PrepareCharacterContextInput, PrepareMaterialContextInput, PreparePlotContextInput,
    PrepareSettingContextInput, SettingBriefV1,
};
use novelgainker_workflows::manager::{WorkflowManager, WorkflowRunState};
#[cfg(test)]
use novelgainker_workflows::manager::{WorkflowManagerError, WorkflowRunRecord};
#[cfg(test)]
use novelgainker_workflows::proposal::CanonProposalViewV1;
use novelgainker_workflows::registry as embedded_workflow_registry;
#[cfg(test)]
use novelgainker_workflows::{
    CHARACTER_CREATE_WORKFLOW_ID, MATERIAL_CREATE_WORKFLOW_ID, PLOT_CREATE_WORKFLOW_ID,
    SETTING_CREATE_WORKFLOW_ID,
};
#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(test)]
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

#[path = "agent_commands.rs"]
pub(crate) mod agent_commands;
#[cfg(test)]
#[path = "router_commands.rs"]
pub mod router_commands;
#[cfg(test)]
#[path = "story_chapter_commands.rs"]
pub mod story_chapter_commands;
#[cfg(test)]
#[path = "story_chapter_patch_commands.rs"]
pub mod story_chapter_patch_commands;
#[cfg(test)]
#[path = "story_context_commands.rs"]
pub mod story_context_commands;
#[cfg(test)]
#[path = "story_continuity_commands.rs"]
pub mod story_continuity_commands;
#[cfg(test)]
#[path = "story_lifecycle_commands.rs"]
pub mod story_lifecycle_commands;
#[cfg(test)]
#[path = "workflow_commands.rs"]
pub mod workflow_commands;
#[cfg(test)]
#[path = "workflow_proposal_commands.rs"]
pub mod workflow_proposal_commands;

#[cfg(test)]
pub(crate) fn install_legacy_story_ledger_fixture(project_root: &std::path::Path) {
    const CONTENT: &str = "---\ntitle: 故事状态账本\nwritingLanguage: zh-CN\nstatus: active\n---\n\n# 故事状态账本\n\n在这里记录时间线、角色状态、线索、承诺与回收等连续性事实。\n";
    std::fs::write(
        project_root.join("materials/000.story-ledger.md"),
        CONTENT.as_bytes(),
    )
    .expect("legacy Story Ledger kernel fixture must be installed explicitly");
}

pub struct ProjectRegistry {
    inner: Mutex<ProjectRegistryInner>,
    recovery_store: RecoveryStore,
    app_state: AppStateRepository,
    agent_undo_store: AgentUndoStore,
    credential_vault: Arc<dyn CredentialVault>,
    provider_run: Mutex<Option<ActiveProviderRun>>,
    agent_canon_runtime: Mutex<agent_commands::AgentCanonRuntimeV2>,
    workflow_manager: Mutex<Option<WorkflowManager>>,
    #[cfg(test)]
    workflow_sessions: Mutex<HashMap<String, String>>,
    #[cfg(test)]
    story_chapter_runs: Mutex<HashMap<String, story_chapter_commands::StoryChapterRunSnapshotV1>>,
    #[cfg(test)]
    story_continuity_reports:
        Mutex<HashMap<String, novelgainker_workflows::continuity::StoryContinuityReportV1>>,
    #[cfg(test)]
    story_target_previews: Mutex<HashMap<String, story_continuity_commands::StoryTargetPreviewV1>>,
    #[cfg(test)]
    story_chapter_patches:
        Mutex<HashMap<String, story_chapter_patch_commands::StoredStoryChapterPatchV1>>,
    #[cfg(test)]
    story_lifecycle:
        Mutex<HashMap<String, story_lifecycle_commands::StoredStoryChapterLifecycleV1>>,
}

#[derive(Default)]
struct ProjectRegistryInner {
    candidates: HashMap<String, RegisteredProjectCandidate>,
    sessions: HashMap<String, RegisteredProjectSession>,
}

struct RegisteredProjectCandidate {
    inspection: ProjectInspection,
    prepared_create: Option<PreparedProjectCreate>,
}

struct RegisteredProjectSession {
    handle: ProjectHandle,
    write_lease: Option<ProjectWriteLease>,
    recovery: RecoveryReport,
}

struct ActiveProviderRun {
    run_id: String,
    provider_id: ProviderId,
    cancellation: CancellationToken,
}

impl ProjectRegistry {
    pub fn new(app_state_root: PathBuf) -> Result<Self, AppError> {
        let recovery_store = RecoveryStore::new(app_state_root.clone())?;
        let agent_undo_store = AgentUndoStore::new(app_state_root.clone())?;
        let app_state = AppStateRepository::open(&app_state_root);
        let workflow_manager = restore_workflow_manager(&app_state);
        Ok(Self {
            inner: Mutex::new(ProjectRegistryInner::default()),
            recovery_store,
            app_state,
            agent_undo_store,
            credential_vault: Arc::new(WindowsCredentialVault::production()),
            provider_run: Mutex::new(None),
            agent_canon_runtime: Mutex::new(agent_commands::AgentCanonRuntimeV2::default()),
            workflow_manager: Mutex::new(workflow_manager),
            #[cfg(test)]
            workflow_sessions: Mutex::new(HashMap::new()),
            #[cfg(test)]
            story_chapter_runs: Mutex::new(HashMap::new()),
            #[cfg(test)]
            story_continuity_reports: Mutex::new(HashMap::new()),
            #[cfg(test)]
            story_target_previews: Mutex::new(HashMap::new()),
            #[cfg(test)]
            story_chapter_patches: Mutex::new(HashMap::new()),
            #[cfg(test)]
            story_lifecycle: Mutex::new(HashMap::new()),
        })
    }

    fn begin_provider_run(
        &self,
        provider_id: ProviderId,
        run_id: String,
        cancellation: CancellationToken,
    ) -> Result<(), AppError> {
        let mut active = self
            .provider_run
            .lock()
            .map_err(|_| AppError::runtime_unavailable())?;
        if active.is_some() {
            return Err(AppError::provider_run_active());
        }
        *active = Some(ActiveProviderRun {
            run_id,
            provider_id,
            cancellation,
        });
        Ok(())
    }

    fn finish_provider_run(&self, run_id: &str) {
        let Ok(mut active) = self.provider_run.lock() else {
            return;
        };
        if active
            .as_ref()
            .is_some_and(|candidate| candidate.run_id == run_id)
        {
            *active = None;
        }
    }

    fn cancel_provider_run(&self, run_id: &str) -> Result<(), AppError> {
        let active = self
            .provider_run
            .lock()
            .map_err(|_| AppError::runtime_unavailable())?;
        let run = active
            .as_ref()
            .filter(|run| run.run_id == run_id)
            .ok_or_else(AppError::provider_run_not_found)?;
        run.cancellation.cancel();
        Ok(())
    }

    pub fn cancel_active_provider_run(&self) {
        if let Ok(active) = self.provider_run.lock() {
            if let Some(run) = active.as_ref() {
                run.cancellation.cancel();
            }
        }
    }

    pub fn active_provider_id(&self) -> Option<ProviderId> {
        self.provider_run
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|run| run.provider_id))
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunStatusSnapshot {
    pub schema_version: u16,
    pub available: bool,
    pub has_active_run: bool,
    pub run: Option<WorkflowRunStatus>,
    pub context_manifest: Option<ContextManifestV1>,
    pub proposal_view: Option<CanonProposalViewV1>,
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunStatus {
    pub run_id: String,
    pub project_id: String,
    pub workflow_id: String,
    pub workflow_version: u16,
    pub state: String,
    pub sequence: u64,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub updated_at: i64,
    pub can_cancel: bool,
    pub can_abandon: bool,
    pub recovery_required: bool,
}

#[cfg(test)]
impl From<WorkflowRunRecord> for WorkflowRunStatus {
    fn from(record: WorkflowRunRecord) -> Self {
        Self {
            run_id: record.run_id,
            project_id: record.project_id,
            workflow_id: record.workflow_id,
            workflow_version: record.workflow_version,
            state: record.state.as_str().to_owned(),
            sequence: record.sequence,
            provider_id: record.provider_id,
            model: record.model,
            updated_at: record.updated_at,
            can_cancel: record.state.can_cancel(),
            can_abandon: record.state.can_abandon(),
            recovery_required: record.state == WorkflowRunState::RecoveryRequired,
        }
    }
}

fn restore_workflow_manager(app_state: &AppStateRepository) -> Option<WorkflowManager> {
    let records = app_state
        .workflow_runs()
        .ok()?
        .into_iter()
        .filter(|record| {
            record.pack_hash == embedded_workflow_registry().pack_hash
                && embedded_workflow_registry()
                    .workflows
                    .iter()
                    .any(|workflow| {
                        workflow.id == record.workflow_id
                            && workflow.version == record.workflow_version
                    })
        })
        .collect();
    let (manager, recovered) = WorkflowManager::restore(records, now_unix_seconds()).ok()?;
    for record in recovered {
        app_state.save_workflow_run(&record).ok()?;
    }
    Some(manager)
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
fn workflow_manager_error(error: WorkflowManagerError) -> AppError {
    match error {
        WorkflowManagerError::RunConflict => AppError::workflow_run_active(),
        WorkflowManagerError::ConfirmationRequired => AppError::workflow_confirmation_required(),
        WorkflowManagerError::ManifestMismatch => AppError::workflow_context_changed(),
        WorkflowManagerError::InvalidRecord => AppError::workflow_unavailable(),
        WorkflowManagerError::ProposalMismatch => AppError::workflow_proposal_expired(),
        WorkflowManagerError::RunNotFound
        | WorkflowManagerError::InvalidState
        | WorkflowManagerError::StaleEvent
        | WorkflowManagerError::TerminalEvent => AppError::workflow_invalid_state(),
    }
}

#[cfg(test)]
fn workflow_status_snapshot(
    manager: Option<&WorkflowManager>,
    project_id: &str,
) -> WorkflowRunStatusSnapshot {
    let Some(manager) = manager else {
        return WorkflowRunStatusSnapshot {
            schema_version: 3,
            available: false,
            has_active_run: false,
            run: None,
            context_manifest: None,
            proposal_view: None,
        };
    };
    WorkflowRunStatusSnapshot {
        schema_version: 3,
        available: true,
        has_active_run: manager.has_active_run(),
        run: manager
            .status_for_project(project_id)
            .map(WorkflowRunStatus::from),
        context_manifest: manager.prepared_manifest_for_project(project_id),
        proposal_view: manager.proposal_view_for_project(project_id),
    }
}

#[cfg(test)]
fn workflow_generation_active(state: &ProjectRegistry) -> Result<bool, AppError> {
    let manager = state
        .workflow_manager
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    Ok(manager
        .as_ref()
        .is_some_and(|manager| manager.active_state() == Some(WorkflowRunState::Generating)))
}

#[cfg(test)]
fn retire_orphaned_legacy_workflow_generation(state: &ProjectRegistry) -> Result<bool, AppError> {
    if state.active_provider_id().is_some() {
        return Ok(false);
    }

    let mut manager_guard = state
        .workflow_manager
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let Some(manager) = manager_guard.as_mut() else {
        return Ok(false);
    };
    if manager.active_state() != Some(WorkflowRunState::Generating) {
        return Ok(false);
    }
    let current = manager
        .active_record()
        .ok_or_else(AppError::workflow_invalid_state)?;

    let mut candidate = manager.clone();
    let retired = candidate
        .abandon(&current.run_id, true, now_unix_seconds())
        .map_err(workflow_manager_error)?;
    state
        .app_state
        .save_workflow_run(&retired)
        .map_err(|_| AppError::workflow_unavailable())?;
    *manager = candidate;
    if let Ok(mut sessions) = state.workflow_sessions.lock() {
        sessions.remove(&current.run_id);
    }
    Ok(true)
}

fn legacy_workflow_blocks_unified_runtime(state: &ProjectRegistry) -> Result<bool, AppError> {
    let manager = state
        .workflow_manager
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let current = manager.as_ref().and_then(WorkflowManager::active_record);
    Ok(current.is_some_and(|record| record.state != WorkflowRunState::RecoveryRequired))
}

#[cfg(test)]
fn workflow_run_active(state: &ProjectRegistry) -> Result<bool, AppError> {
    let manager = state
        .workflow_manager
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    Ok(manager
        .as_ref()
        .is_some_and(WorkflowManager::has_active_run))
}

#[cfg(test)]
fn context_build_error(error: ContextBuildError) -> AppError {
    match error {
        ContextBuildError::InvalidBrief => AppError::workflow_input_invalid(),
        ContextBuildError::SourceNotAllowed => AppError::workflow_source_not_allowed(),
        ContextBuildError::SourceLimitExceeded => AppError::workflow_source_limit_exceeded(),
        _ => AppError::workflow_unavailable(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCandidate {
    pub candidate_id: String,
    #[serde(flatten)]
    pub preview: ProjectScanPreview,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSession {
    pub session_id: String,
    pub project_id: String,
    pub display_name: String,
    pub schema_version: u16,
    pub writing_language: String,
    pub markdown_file_count: u32,
    pub access_mode: ProjectAccessMode,
    pub recovery_status: RecoveryStatus,
    pub recovered_transaction_count: u32,
    pub required_transaction_count: u32,
    pub restored_current_file: Option<String>,
    pub capabilities: ProjectCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectAccessMode {
    Writer,
    ReadOnly,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCapabilities {
    pub can_read: bool,
    pub can_write: bool,
    pub can_browse_files: bool,
    pub can_save_files: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCloseResult {
    pub closed: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateWriteResult {
    pub saved: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCatalogEntry {
    pub id: &'static str,
    pub version: u16,
    pub display_name: &'static str,
    pub description: &'static str,
    pub mode: &'static str,
    pub execution_available: bool,
    pub routable: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRegistrySnapshot {
    pub schema_version: u16,
    pub workflows: Vec<WorkflowCatalogEntry>,
}

#[cfg(test)]
const PROVIDER_RUN_EVENT_CHANNEL: &str = "provider-run-event";

#[cfg(test)]
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderChatRole {
    User,
    Assistant,
}

#[cfg(test)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderChatMessageInput {
    pub role: ProviderChatRole,
    pub content: String,
}

#[cfg(test)]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRunStarted {
    pub run_id: String,
    pub provider_id: ProviderId,
    pub model: String,
}

#[cfg(test)]
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRunCancelResult {
    pub cancel_requested: bool,
}

#[cfg(test)]
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProviderRunEventBody {
    TextDelta {
        delta: String,
    },
    Usage {
        usage: Usage,
    },
    Completed {
        #[serde(rename = "finishReason")]
        finish_reason: FinishReason,
    },
    Failed {
        error: AppError,
    },
    Cancelled,
}

#[cfg(test)]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRunEvent {
    pub run_id: String,
    pub provider_id: ProviderId,
    pub model: String,
    pub sequence: u32,
    #[serde(flatten)]
    pub event: ProviderRunEventBody,
}

#[tauri::command]
pub fn health_check() -> Result<HealthStatus, AppError> {
    Ok(health_status(
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
    ))
}

#[tauri::command]
pub fn app_state_load(state: tauri::State<'_, ProjectRegistry>) -> AppStateSnapshot {
    state.app_state.snapshot()
}

#[cfg(test)]
pub fn workflow_registry_load() -> WorkflowRegistrySnapshot {
    let registry = embedded_workflow_registry();
    WorkflowRegistrySnapshot {
        schema_version: registry.schema_version,
        workflows: registry
            .workflows
            .iter()
            .map(|workflow| WorkflowCatalogEntry {
                id: workflow.id,
                version: workflow.version,
                display_name: workflow.display_name,
                description: workflow.description,
                mode: workflow.mode,
                execution_available: workflow.execution_available,
                routable: workflow.routable,
            })
            .collect(),
    }
}

#[cfg(test)]
pub fn workflow_run_status_load(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<WorkflowRunStatusSnapshot, AppError> {
    load_registered_workflow_status(&state, &session_id)
}

#[cfg(test)]
pub fn workflow_run_cancel(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<WorkflowRunStatusSnapshot, AppError> {
    mutate_registered_workflow_run(&state, &session_id, WorkflowRunAction::Cancel)
}

#[cfg(test)]
pub fn workflow_run_abandon(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    confirmed: bool,
) -> Result<WorkflowRunStatusSnapshot, AppError> {
    mutate_registered_workflow_run(
        &state,
        &session_id,
        WorkflowRunAction::Abandon { confirmed },
    )
}

#[cfg(test)]
fn load_registered_workflow_status(
    state: &ProjectRegistry,
    session_id: &str,
) -> Result<WorkflowRunStatusSnapshot, AppError> {
    let handle = session_handle(state, session_id)?;
    let manager = state
        .workflow_manager
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    Ok(workflow_status_snapshot(
        manager.as_ref(),
        handle.project_id(),
    ))
}

#[cfg(test)]
enum WorkflowRunAction {
    Cancel,
    Abandon { confirmed: bool },
}

#[cfg(test)]
fn mutate_registered_workflow_run(
    state: &ProjectRegistry,
    session_id: &str,
    action: WorkflowRunAction,
) -> Result<WorkflowRunStatusSnapshot, AppError> {
    let handle = session_handle(state, session_id)?;
    let mut manager_guard = state
        .workflow_manager
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let manager = manager_guard
        .as_mut()
        .ok_or_else(AppError::workflow_unavailable)?;
    let current = manager
        .status_for_project(handle.project_id())
        .ok_or_else(AppError::workflow_invalid_state)?;

    if current.state == WorkflowRunState::Generating {
        if let Ok(active) = state.provider_run.lock() {
            if let Some(run) = active.as_ref().filter(|run| run.run_id == current.run_id) {
                run.cancellation.cancel();
            }
        }
    }

    let mut candidate = manager.clone();
    let updated_at = now_unix_seconds();
    let updated = match action {
        WorkflowRunAction::Cancel => candidate.cancel(&current.run_id, updated_at),
        WorkflowRunAction::Abandon { confirmed } => {
            candidate.abandon(&current.run_id, confirmed, updated_at)
        }
    }
    .map_err(workflow_manager_error)?;
    state
        .app_state
        .save_workflow_run(&updated)
        .map_err(|_| AppError::workflow_unavailable())?;
    *manager = candidate;
    if updated.state.is_terminal() {
        state
            .workflow_sessions
            .lock()
            .map_err(|_| AppError::workflow_unavailable())?
            .remove(&current.run_id);
    }
    Ok(workflow_status_snapshot(Some(manager), handle.project_id()))
}

#[tauri::command]
pub fn app_state_save_ui(
    state: tauri::State<'_, ProjectRegistry>,
    preferences: UiPreferences,
) -> Result<AppStateWriteResult, AppError> {
    state.app_state.save_ui(preferences)?;
    Ok(AppStateWriteResult { saved: true })
}

#[tauri::command]
pub fn provider_status_load(
    state: tauri::State<'_, ProjectRegistry>,
) -> Result<ProviderStateSnapshot, AppError> {
    load_provider_state(&state.app_state, state.credential_vault.as_ref())
}

#[tauri::command]
pub fn provider_key_save(
    state: tauri::State<'_, ProjectRegistry>,
    provider_id: String,
    key: String,
) -> Result<ProviderStatus, AppError> {
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    save_provider_key(
        &state.app_state,
        state.credential_vault.as_ref(),
        &provider_id,
        key,
    )
}

#[tauri::command]
pub fn provider_key_delete(
    state: tauri::State<'_, ProjectRegistry>,
    provider_id: String,
    confirmed: bool,
) -> Result<ProviderStatus, AppError> {
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    delete_provider_key(
        &state.app_state,
        state.credential_vault.as_ref(),
        &provider_id,
        confirmed,
    )
}

#[tauri::command]
pub fn provider_select(
    state: tauri::State<'_, ProjectRegistry>,
    provider_id: String,
) -> Result<ProviderStatus, AppError> {
    select_registered_provider(&state, &provider_id)
}

fn select_registered_provider(
    state: &ProjectRegistry,
    provider_id: &str,
) -> Result<ProviderStatus, AppError> {
    if state.active_provider_id().is_some() {
        return Err(AppError::provider_run_active());
    }
    if legacy_workflow_blocks_unified_runtime(state)? {
        return Err(AppError::workflow_run_active());
    }
    select_provider(
        &state.app_state,
        state.credential_vault.as_ref(),
        provider_id,
    )
}

#[tauri::command]
pub async fn provider_connection_test(
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<ProviderStatus, AppError> {
    let state = app.state::<ProjectRegistry>();
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ProjectRegistry>();
        test_provider_connection(
            &state.app_state,
            state.credential_vault.as_ref(),
            &provider_id,
        )
    })
    .await
    .map_err(|_| AppError::runtime_unavailable())?
}

#[cfg(test)]
pub fn provider_chat_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProjectRegistry>,
    messages: Vec<ProviderChatMessageInput>,
    expected_provider_id: ProviderId,
) -> Result<ProviderRunStarted, AppError> {
    if state.active_provider_id().is_some() {
        return Err(AppError::provider_run_active());
    }
    let snapshot = load_provider_state(&state.app_state, state.credential_vault.as_ref())?;
    if workflow_generation_active(&state)? {
        return Err(AppError::workflow_run_active());
    }
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
    let provider_id = selected.provider_id;
    let normalized_messages = messages
        .into_iter()
        .map(|message| NormalizedMessage {
            role: match message.role {
                ProviderChatRole::User => MessageRole::User,
                ProviderChatRole::Assistant => MessageRole::Assistant,
            },
            content: message.content,
        })
        .collect();
    let request = NormalizedRequest::conversation(normalized_messages, MAX_OUTPUT_TOKENS)
        .map_err(AppError::from_provider_error)?;
    let run_id = Uuid::new_v4().to_string();
    let cancellation = CancellationToken::default();
    state.begin_provider_run(provider_id, run_id.clone(), cancellation.clone())?;
    let model = preset(provider_id).model.to_owned();
    let worker_run_id = run_id.clone();
    let worker_model = model.clone();
    tauri::async_runtime::spawn_blocking(move || {
        execute_provider_chat(
            app,
            worker_run_id,
            provider_id,
            worker_model,
            request,
            cancellation,
        );
    });
    Ok(ProviderRunStarted {
        run_id,
        provider_id,
        model,
    })
}

#[cfg(test)]
pub fn provider_chat_cancel(
    state: tauri::State<'_, ProjectRegistry>,
    run_id: String,
) -> Result<ProviderRunCancelResult, AppError> {
    state.cancel_provider_run(&run_id)?;
    Ok(ProviderRunCancelResult {
        cancel_requested: true,
    })
}

#[cfg(test)]
fn execute_provider_chat(
    app: tauri::AppHandle,
    run_id: String,
    provider_id: ProviderId,
    model: String,
    request: NormalizedRequest,
    cancellation: CancellationToken,
) {
    let state = app.state::<ProjectRegistry>();
    let client = match ProviderHttpClient::production() {
        Ok(client) => client,
        Err(error) => {
            let _ = emit_provider_event(
                &app,
                &run_id,
                provider_id,
                &model,
                1,
                ProviderRunEventBody::Failed {
                    error: AppError::from_provider_error(error),
                },
            );
            state.finish_provider_run(&run_id);
            return;
        }
    };
    let mut sequence = 0u32;
    let mut provider_result = None;
    let vault_result = state.credential_vault.with_secret(provider_id, &mut |key| {
        provider_result =
            Some(
                client.stream(provider_id, key, &request, &cancellation, &mut |event| {
                    let (event_sequence, body) = match event {
                        NormalizedEvent::TextDelta { sequence, delta } => {
                            (sequence, ProviderRunEventBody::TextDelta { delta })
                        }
                        NormalizedEvent::Usage { sequence, usage } => {
                            (sequence, ProviderRunEventBody::Usage { usage })
                        }
                        NormalizedEvent::Completed {
                            sequence,
                            finish_reason,
                        } => (sequence, ProviderRunEventBody::Completed { finish_reason }),
                    };
                    sequence = event_sequence;
                    emit_provider_event(&app, &run_id, provider_id, &model, event_sequence, body)
                }),
            );
        Ok(())
    });

    let terminal_sequence = sequence.saturating_add(1);
    if let Err(error) = vault_result {
        let _ = emit_provider_event(
            &app,
            &run_id,
            provider_id,
            &model,
            terminal_sequence,
            ProviderRunEventBody::Failed { error },
        );
    } else {
        match provider_result {
            Some(Err(error)) => {
                let event = if error.kind == ProviderErrorKind::Cancelled {
                    ProviderRunEventBody::Cancelled
                } else {
                    ProviderRunEventBody::Failed {
                        error: AppError::from_provider_error(error),
                    }
                };
                let _ = emit_provider_event(
                    &app,
                    &run_id,
                    provider_id,
                    &model,
                    terminal_sequence,
                    event,
                );
            }
            None => {
                let _ = emit_provider_event(
                    &app,
                    &run_id,
                    provider_id,
                    &model,
                    terminal_sequence,
                    ProviderRunEventBody::Failed {
                        error: AppError::provider_vault_unavailable(),
                    },
                );
            }
            Some(Ok(_)) => {}
        }
    }
    state.finish_provider_run(&run_id);
}

#[cfg(test)]
fn emit_provider_event(
    app: &tauri::AppHandle,
    run_id: &str,
    provider_id: ProviderId,
    model: &str,
    sequence: u32,
    event: ProviderRunEventBody,
) -> Result<(), ProviderError> {
    app.emit(
        PROVIDER_RUN_EVENT_CHANNEL,
        ProviderRunEvent {
            run_id: run_id.to_owned(),
            provider_id,
            model: model.to_owned(),
            sequence,
            event,
        },
    )
    .map_err(|_| ProviderError::cancelled())
}

#[tauri::command]
pub fn app_state_set_current_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
) -> Result<AppStateWriteResult, AppError> {
    set_registered_current_file(&state, &session_id, &relative_path)
}

#[tauri::command]
pub async fn project_scan(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProjectRegistry>,
    mode: ProjectScanMode,
) -> Result<Option<ProjectCandidate>, AppError> {
    let selected = app.dialog().file().blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let selected_root = selected
        .into_path()
        .map_err(|_| AppError::project_invalid_root())?;
    let inspection = inspect_project_root(&selected_root, mode)?;
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    let candidate_id = Uuid::new_v4().to_string();
    let preview = inspection.preview().clone();

    let mut registry = lock_registry(&state)?;
    registry.candidates.clear();
    registry.candidates.insert(
        candidate_id.clone(),
        RegisteredProjectCandidate {
            inspection,
            prepared_create: None,
        },
    );

    Ok(Some(ProjectCandidate {
        candidate_id,
        preview,
    }))
}

#[tauri::command]
pub async fn project_create_preview(
    state: tauri::State<'_, ProjectRegistry>,
    candidate_id: String,
    input: ProjectCreateInputV1,
) -> Result<ProjectCreatePreview, AppError> {
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    let inspection = candidate_inspection(&state, &candidate_id)?;
    let prepared = prepare_project_create(&inspection, input)?;
    let preview = prepared.preview().clone();

    let mut registry = lock_registry(&state)?;
    let candidate = registry
        .candidates
        .get_mut(&candidate_id)
        .ok_or_else(AppError::project_candidate_changed)?;
    if candidate.inspection.coordination_key() != prepared.coordination_key() {
        return Err(AppError::project_candidate_changed());
    }
    candidate.prepared_create = Some(prepared);
    Ok(preview)
}

#[tauri::command]
pub async fn project_create(
    state: tauri::State<'_, ProjectRegistry>,
    candidate_id: String,
    preview_hash: String,
    confirmed: bool,
) -> Result<ProjectSession, AppError> {
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    if !confirmed {
        return Err(AppError::project_confirmation_required());
    }
    let prepared = candidate_prepared_create(&state, &candidate_id)?;
    let write_lease = try_acquire_project_write_lease(&prepared.coordination_key())?
        .ok_or_else(AppError::project_writer_required)?;
    let handle = create_prepared_project(&prepared, &preview_hash)?;
    register_session(&state, Some(candidate_id), handle, Some(write_lease))
}

#[tauri::command]
pub async fn project_open(
    state: tauri::State<'_, ProjectRegistry>,
    candidate_id: String,
    confirm_metadata: bool,
) -> Result<ProjectSession, AppError> {
    let inspection = candidate_inspection(&state, &candidate_id)?;
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    let write_lease = try_acquire_project_write_lease(&inspection.coordination_key())?;
    if write_lease.is_none() && inspection.preview().requires_metadata_confirmation {
        return Err(AppError::project_writer_required());
    }
    let handle = open_project(&inspection, write_lease.is_some() && confirm_metadata)?;
    register_session(&state, Some(candidate_id), handle, write_lease)
}

#[tauri::command]
pub async fn project_open_recent(
    state: tauri::State<'_, ProjectRegistry>,
    project_id: String,
) -> Result<ProjectSession, AppError> {
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    open_registered_recent_project(&state, &project_id)
}

#[tauri::command]
pub async fn project_list_files(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<ProjectFileIndex, AppError> {
    let handle = session_handle(&state, &session_id)?;
    list_project_files(&handle)
}

#[tauri::command]
pub async fn project_read_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
) -> Result<ProjectFileDocument, AppError> {
    let handle = session_handle(&state, &session_id)?;
    read_project_file(&handle, &relative_path)
}

#[tauri::command]
pub async fn project_watch_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
    expected_hash: String,
) -> Result<ProjectFileWatchStatus, AppError> {
    let handle = session_handle(&state, &session_id)?;
    check_project_file(&handle, &relative_path, &expected_hash)
}
#[tauri::command]
pub async fn project_save_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
    content: String,
    source_hash: String,
) -> Result<ProjectFileSaveResult, AppError> {
    save_registered_project_file(&state, &session_id, &relative_path, &content, &source_hash)
}

#[tauri::command]
pub async fn project_validate_draft(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
    content: String,
) -> Result<ValidationReport, AppError> {
    let handle = session_handle(&state, &session_id)?;
    validate_project_draft(&handle, &relative_path, &content)
}

#[tauri::command]
pub async fn project_search(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    query: String,
    case_sensitive: bool,
) -> Result<SearchReport, AppError> {
    let handle = session_handle(&state, &session_id)?;
    search_project_sources(&handle, &query, case_sensitive)
}

#[tauri::command]
pub fn project_close(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<ProjectCloseResult, AppError> {
    if legacy_workflow_blocks_unified_runtime(&state)? {
        return Err(AppError::workflow_run_active());
    }
    state.cancel_active_provider_run();
    let project_handle = {
        let registry = lock_registry(&state)?;
        registry
            .sessions
            .get(&session_id)
            .map(|session| session.handle.clone())
    };
    if let Some(handle) = project_handle.as_ref() {
        agent_commands::clear_project_agent_runtime(&state, handle)?;
    }
    #[cfg(test)]
    {
        state
            .story_chapter_runs
            .lock()
            .map_err(|_| AppError::workflow_unavailable())?
            .remove(&session_id);
    }
    #[cfg(test)]
    state
        .story_continuity_reports
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    #[cfg(test)]
    state
        .story_target_previews
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    #[cfg(test)]
    state
        .story_chapter_patches
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    #[cfg(test)]
    state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    let closed = lock_registry(&state)?
        .sessions
        .remove(&session_id)
        .is_some();
    Ok(ProjectCloseResult { closed })
}

fn set_registered_current_file(
    state: &ProjectRegistry,
    session_id: &str,
    relative_path: &str,
) -> Result<AppStateWriteResult, AppError> {
    let handle = session_handle(state, session_id)?;
    let document = read_project_file(&handle, relative_path)?;
    state
        .app_state
        .record_current_file(handle.project_id(), &document.relative_path)?;
    Ok(AppStateWriteResult { saved: true })
}

fn open_registered_recent_project(
    state: &ProjectRegistry,
    project_id: &str,
) -> Result<ProjectSession, AppError> {
    Uuid::parse_str(project_id).map_err(|_| AppError::app_state_unavailable())?;
    let root = state
        .app_state
        .recent_project_root(project_id)
        .ok_or_else(AppError::app_state_unavailable)?;
    let inspection = match inspect_project_root(&root, ProjectScanMode::Open) {
        Ok(inspection) => inspection,
        Err(error) => {
            state.app_state.forget_project(project_id);
            return Err(error);
        }
    };
    if inspection.preview().requires_metadata_confirmation {
        state.app_state.forget_project(project_id);
        return Err(AppError::project_confirmation_required());
    }
    let write_lease = try_acquire_project_write_lease(&inspection.coordination_key())?;
    let handle = open_project(&inspection, false)?;
    register_session(state, None, handle, write_lease)
}

fn save_registered_project_file(
    state: &ProjectRegistry,
    session_id: &str,
    relative_path: &str,
    content: &str,
    source_hash: &str,
) -> Result<ProjectFileSaveResult, AppError> {
    let handle = {
        let registry = lock_registry(state)?;
        let session = registry
            .sessions
            .get(session_id)
            .ok_or_else(AppError::project_session_not_found)?;
        if session.write_lease.is_none() {
            return Err(AppError::project_writer_required());
        }
        if session.recovery.status == RecoveryStatus::Required {
            return Err(AppError::project_recovery_required());
        }
        session.handle.clone()
    };
    save_project_file_recoverable(
        &handle,
        relative_path,
        content,
        source_hash,
        &state.recovery_store,
    )
}
fn candidate_inspection(
    state: &ProjectRegistry,
    candidate_id: &str,
) -> Result<ProjectInspection, AppError> {
    lock_registry(state)?
        .candidates
        .get(candidate_id)
        .map(|candidate| candidate.inspection.clone())
        .ok_or_else(AppError::project_candidate_changed)
}

fn candidate_prepared_create(
    state: &ProjectRegistry,
    candidate_id: &str,
) -> Result<PreparedProjectCreate, AppError> {
    lock_registry(state)?
        .candidates
        .get(candidate_id)
        .and_then(|candidate| candidate.prepared_create.clone())
        .ok_or_else(AppError::project_candidate_changed)
}

fn session_handle(state: &ProjectRegistry, session_id: &str) -> Result<ProjectHandle, AppError> {
    lock_registry(state)?
        .sessions
        .get(session_id)
        .map(|session| session.handle.clone())
        .ok_or_else(AppError::project_session_not_found)
}

fn session_writer_handle(
    state: &ProjectRegistry,
    session_id: &str,
) -> Result<ProjectHandle, AppError> {
    let registry = lock_registry(state)?;
    let session = registry
        .sessions
        .get(session_id)
        .ok_or_else(AppError::project_session_not_found)?;
    if session.write_lease.is_none() {
        return Err(AppError::project_writer_required());
    }
    if session.recovery.status == RecoveryStatus::Required {
        return Err(AppError::project_recovery_required());
    }
    Ok(session.handle.clone())
}

fn register_session(
    state: &ProjectRegistry,
    candidate_id: Option<String>,
    handle: ProjectHandle,
    write_lease: Option<ProjectWriteLease>,
) -> Result<ProjectSession, AppError> {
    debug_assert!(handle.is_bound_to_root());
    let session_id = Uuid::new_v4().to_string();
    let access_mode = if write_lease.is_some() {
        ProjectAccessMode::Writer
    } else {
        ProjectAccessMode::ReadOnly
    };
    let mut recovery = if write_lease.is_some() {
        state.recovery_store.recover_project(&handle)
    } else {
        RecoveryReport::not_checked()
    };
    if write_lease.is_some() && recovery.status != RecoveryStatus::Required {
        match state.app_state.agent_undo_records(handle.project_id()) {
            Ok(records) => {
                let agent_recovery = recover_incomplete_agent_transactions(
                    &handle,
                    &records,
                    &state.recovery_store,
                    &state.agent_undo_store,
                );
                recovery.recovered_transactions = recovery
                    .recovered_transactions
                    .saturating_add(u32::from(agent_recovery.recovered_transactions));
                recovery.required_transactions = recovery
                    .required_transactions
                    .saturating_add(u32::from(agent_recovery.recovery_required_transactions));
                if agent_recovery.recovery_required_transactions > 0 {
                    recovery.status = RecoveryStatus::Required;
                } else if agent_recovery.recovered_transactions > 0 {
                    recovery.status = RecoveryStatus::Recovered;
                }
            }
            Err(_) => recovery.status = RecoveryStatus::Required,
        }
    }
    let can_write = write_lease.is_some() && recovery.status != RecoveryStatus::Required;
    let restored_current_file = state
        .app_state
        .current_file(handle.project_id())
        .filter(|relative_path| read_project_file(&handle, relative_path).is_ok());
    let _ = state.app_state.record_project(&handle);
    let session = ProjectSession {
        session_id: session_id.clone(),
        project_id: handle.project_id().to_owned(),
        display_name: handle.display_name().to_owned(),
        schema_version: handle.schema_version(),
        writing_language: handle.writing_language().to_owned(),
        markdown_file_count: handle.markdown_file_count(),
        access_mode,
        recovery_status: recovery.status,
        recovered_transaction_count: recovery.recovered_transactions,
        required_transaction_count: recovery.required_transactions,
        restored_current_file,
        capabilities: ProjectCapabilities {
            can_read: true,
            can_write,
            can_browse_files: true,
            can_save_files: can_write,
        },
    };

    let mut registry = lock_registry(state)?;
    if let Some(candidate_id) = candidate_id {
        registry.candidates.remove(&candidate_id);
    }
    registry.sessions.insert(
        session_id,
        RegisteredProjectSession {
            handle,
            write_lease,
            recovery,
        },
    );
    Ok(session)
}

fn lock_registry(
    state: &ProjectRegistry,
) -> Result<MutexGuard<'_, ProjectRegistryInner>, AppError> {
    state
        .inner
        .lock()
        .map_err(|_| AppError::project_io().with_diagnostic("project-registry-unavailable"))
}

#[cfg(test)]
mod tests {
    use super::{
        health_check, legacy_workflow_blocks_unified_runtime, load_registered_workflow_status,
        mutate_registered_workflow_run, open_registered_recent_project, register_session,
        retire_orphaned_legacy_workflow_generation, save_registered_project_file,
        select_registered_provider, set_registered_current_file, workflow_generation_active,
        workflow_registry_load, ProjectRegistry, RegisteredProjectSession, WorkflowRunAction,
    };
    use crate::command_manifest::APP_COMMANDS;
    use app_core::app_state::{UiPreferences, APP_STATE_DATABASE_NAME};
    use app_core::coordination::try_acquire_project_write_lease;
    use app_core::credential_vault::ProviderId;
    use app_core::project::{create_project, inspect_project_root, ProjectScanMode};
    use app_core::provider_core::CancellationToken;
    use app_core::recovery::{RecoveryReport, RecoveryStatus};
    use app_core::workspace::read_project_file;
    use app_core::{HealthState, HEALTH_CONTRACT_VERSION};
    use novelgainker_workflows::manager::{WorkflowRunRecord, WorkflowRunState};
    use novelgainker_workflows::{registry, CHARACTER_CREATE_WORKFLOW_ID};
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn health_check_returns_the_public_runtime_contract() {
        let result = health_check().expect("health check must be read-only and infallible");

        assert_eq!(result.application, "NovelGainker");
        assert_eq!(result.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(result.platform, std::env::consts::OS);
        assert_eq!(result.status, HealthState::Ready);
        assert_eq!(result.contract_version, HEALTH_CONTRACT_VERSION);
    }

    #[test]
    fn ng_104_save_adapter_requires_a_writer_lease_and_uses_recovery_journaling() {
        let root =
            std::env::temp_dir().join(format!("novelgainker-ng103-tauri-save-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("test root must create");
        let inspection =
            inspect_project_root(&root, ProjectScanMode::Create).expect("test project must scan");
        let handle = create_project(&inspection).expect("test project must create");
        fs::write(root.join("chapters/command.md"), b"old").expect("fixture must write");
        let document =
            read_project_file(&handle, "chapters/command.md").expect("fixture must read");
        let app_state = root.with_extension("app-state");
        let write_lease = try_acquire_project_write_lease(&handle.coordination_key())
            .expect("writer lease must resolve")
            .expect("writer lease must be available");
        let registry = ProjectRegistry::new(app_state.clone()).expect("registry must create");
        registry
            .inner
            .lock()
            .expect("registry must lock")
            .sessions
            .insert(
                "test-session".to_owned(),
                RegisteredProjectSession {
                    handle,
                    write_lease: Some(write_lease),
                    recovery: RecoveryReport::ready(),
                },
            );

        let result = save_registered_project_file(
            &registry,
            "test-session",
            "chapters/command.md",
            "new",
            &document.content_hash,
        )
        .expect("registered save must succeed");

        assert_ne!(result.content_hash, document.content_hash);
        assert_eq!(fs::read(root.join("chapters/command.md")).unwrap(), b"new");
        drop(registry);
        fs::remove_dir_all(&app_state).expect("verified app state must remove");
        fs::remove_dir_all(&root).expect("verified test root must remove");
    }
    #[test]
    fn ng_104_save_adapter_blocks_read_only_and_recovery_required_sessions() {
        let root =
            std::env::temp_dir().join(format!("novelgainker-ng104-tauri-block-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("test root must create");
        let inspection =
            inspect_project_root(&root, ProjectScanMode::Create).expect("test project must scan");
        let handle = create_project(&inspection).expect("test project must create");
        fs::write(root.join("chapters/blocked.md"), b"old").expect("fixture must write");
        let document =
            read_project_file(&handle, "chapters/blocked.md").expect("fixture must read");
        let app_state = root.with_extension("app-state");
        let registry = ProjectRegistry::new(app_state.clone()).expect("registry must create");

        registry
            .inner
            .lock()
            .expect("registry must lock")
            .sessions
            .insert(
                "read-only".to_owned(),
                RegisteredProjectSession {
                    handle: handle.clone(),
                    write_lease: None,
                    recovery: RecoveryReport::not_checked(),
                },
            );
        let read_only_error = save_registered_project_file(
            &registry,
            "read-only",
            "chapters/blocked.md",
            "new",
            &document.content_hash,
        )
        .expect_err("read-only session must not save");
        assert_eq!(read_only_error.code, "project.writer_required");
        assert_eq!(fs::read(root.join("chapters/blocked.md")).unwrap(), b"old");

        let write_lease = try_acquire_project_write_lease(&handle.coordination_key())
            .expect("writer lease must resolve")
            .expect("writer lease must be available");
        registry
            .inner
            .lock()
            .expect("registry must lock")
            .sessions
            .insert(
                "recovery-required".to_owned(),
                RegisteredProjectSession {
                    handle,
                    write_lease: Some(write_lease),
                    recovery: RecoveryReport {
                        status: RecoveryStatus::Required,
                        recovered_transactions: 0,
                        required_transactions: 1,
                    },
                },
            );
        let recovery_error = save_registered_project_file(
            &registry,
            "recovery-required",
            "chapters/blocked.md",
            "new",
            &document.content_hash,
        )
        .expect_err("unresolved recovery must block every new save");
        assert_eq!(recovery_error.code, "project.recovery_required");
        assert_eq!(fs::read(root.join("chapters/blocked.md")).unwrap(), b"old");

        drop(registry);
        fs::remove_dir_all(&app_state).expect("verified app state must remove");
        fs::remove_dir_all(&root).expect("verified test root must remove");
    }

    #[test]
    fn ng_106_reopens_recent_project_and_restores_only_a_relative_current_file() {
        let root =
            std::env::temp_dir().join(format!("novelgainker-ng106-recent-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("test root must create");
        let inspection =
            inspect_project_root(&root, ProjectScanMode::Create).expect("project must scan");
        let handle = create_project(&inspection).expect("project must create");
        fs::write(root.join("chapters/current.md"), b"current").expect("fixture must write");
        let project_id = handle.project_id().to_owned();
        let app_state = root.with_extension("app-state");
        let registry = ProjectRegistry::new(app_state.clone()).expect("registry must create");
        let write_lease = try_acquire_project_write_lease(&handle.coordination_key())
            .expect("writer lease must resolve")
            .expect("writer lease must be available");
        let opened = register_session(&registry, None, handle, Some(write_lease))
            .expect("session must open");
        set_registered_current_file(&registry, &opened.session_id, "chapters/current.md")
            .expect("current file must persist");
        drop(registry);

        let restarted = ProjectRegistry::new(app_state.clone()).expect("registry must restart");
        let restored = open_registered_recent_project(&restarted, &project_id)
            .expect("recent project must reopen");
        assert_eq!(
            restored.restored_current_file.as_deref(),
            Some("chapters/current.md")
        );
        assert_eq!(restarted.app_state.snapshot().recent_projects.len(), 1);

        drop(restarted);
        fs::remove_dir_all(app_state).expect("verified app state must remove");
        fs::remove_dir_all(root).expect("verified project must remove");
    }

    #[test]
    fn ng_106_preference_failure_does_not_block_manuscript_save() {
        let root =
            std::env::temp_dir().join(format!("novelgainker-ng106-isolation-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("test root must create");
        let inspection =
            inspect_project_root(&root, ProjectScanMode::Create).expect("project must scan");
        let handle = create_project(&inspection).expect("project must create");
        fs::write(root.join("chapters/isolation.md"), b"old").expect("fixture must write");
        let document = read_project_file(&handle, "chapters/isolation.md").unwrap();
        let app_state = root.with_extension("app-state");
        fs::create_dir_all(&app_state).expect("app state root must create");
        fs::write(app_state.join(APP_STATE_DATABASE_NAME), b"not a database")
            .expect("corrupt state fixture must write");
        let registry = ProjectRegistry::new(app_state.clone()).expect("registry must create");
        let write_lease = try_acquire_project_write_lease(&handle.coordination_key())
            .expect("writer lease must resolve")
            .expect("writer lease must be available");
        registry
            .inner
            .lock()
            .expect("registry must lock")
            .sessions
            .insert(
                "isolation".to_owned(),
                RegisteredProjectSession {
                    handle,
                    write_lease: Some(write_lease),
                    recovery: RecoveryReport::ready(),
                },
            );
        assert_eq!(
            registry
                .app_state
                .save_ui(UiPreferences::default())
                .unwrap_err()
                .code,
            "app_state.unavailable"
        );

        let saved = save_registered_project_file(
            &registry,
            "isolation",
            "chapters/isolation.md",
            "new",
            &document.content_hash,
        )
        .expect("canon save must remain independent");
        assert_ne!(saved.content_hash, document.content_hash);
        assert_eq!(
            fs::read(root.join("chapters/isolation.md")).unwrap(),
            b"new"
        );

        drop(registry);
        fs::remove_dir_all(app_state).expect("verified app state must remove");
        fs::remove_dir_all(root).expect("verified project must remove");
    }

    #[test]
    fn ng_205_allows_only_one_cancellable_provider_run() {
        let app_state = std::env::temp_dir().join(format!(
            "novelgainker-ng205-provider-run-{}",
            Uuid::new_v4()
        ));
        let registry = ProjectRegistry::new(app_state.clone()).expect("registry must create");
        let cancellation = CancellationToken::default();
        registry
            .begin_provider_run(
                ProviderId::Openai,
                "10000000-0000-4000-8000-000000000001".to_owned(),
                cancellation.clone(),
            )
            .expect("first run must start");
        let switch_error = select_registered_provider(&registry, "deepseek")
            .expect_err("active run must block Provider switching");
        assert_eq!(switch_error.code, "provider.run_active");
        assert_eq!(registry.active_provider_id(), Some(ProviderId::Openai));
        let duplicate = registry
            .begin_provider_run(
                ProviderId::Deepseek,
                "20000000-0000-4000-8000-000000000002".to_owned(),
                CancellationToken::default(),
            )
            .expect_err("second run must be rejected");
        assert_eq!(duplicate.code, "provider.run_active");
        registry
            .cancel_provider_run("10000000-0000-4000-8000-000000000001")
            .expect("active run must cancel");
        assert!(cancellation.is_cancelled());
        registry.finish_provider_run("10000000-0000-4000-8000-000000000001");
        assert_eq!(registry.active_provider_id(), None);
        drop(registry);
        if app_state.exists() {
            fs::remove_dir_all(app_state).expect("verified app state must remove");
        }
    }
    #[test]
    fn ng_506_exposes_five_routable_workflows_and_one_executable_non_routable_storytelling_workflow(
    ) {
        let snapshot = workflow_registry_load();
        let serialized = serde_json::to_value(snapshot).expect("registry must serialize");
        assert_eq!(
            serialized,
            serde_json::json!({
                "schemaVersion": 1,
                "workflows": [{
                    "id": "novelgainker.character-create",
                    "version": 2,
                    "displayName": "创建/完善人物",
                    "description": "审查显式选择的 Canon 来源，创建或完善单个人物 Markdown，并保留稳定实体引用。",
                    "mode": "interactive-write",
                    "routable": true,
                    "executionAvailable": true
                }, {
                    "id": "novelgainker.project-create",
                    "version": 1,
                    "displayName": "创建项目",
                    "description": "离线填写项目元数据，安全审查完整脚手架后创建本地 Markdown 小说项目。",
                    "mode": "deterministic-write",
                    "routable": true,
                    "executionAvailable": true
                }, {
                    "id": "novelgainker.setting-create",
                    "version": 1,
                    "displayName": "创建设定",
                    "description": "审查显式选择的世界资料，生成结构化设定提案，经确认后写入一个 world Markdown。",
                    "mode": "interactive-write",
                    "routable": true,
                    "executionAvailable": true
                }, {
                    "id": "novelgainker.material-create",
                    "version": 1,
                    "displayName": "创建素材",
                    "description": "审查显式选择的 Canon 来源，生成结构化素材提案，经确认后写入一个普通 materials Markdown。",
                    "mode": "interactive-write",
                    "routable": true,
                    "executionAvailable": true
                }, {
                    "id": "novelgainker.plot-create",
                    "version": 1,
                    "displayName": "创建主剧情",
                    "description": "审查显式选择的人物、设定和普通素材，生成结构化主剧情提案，经确认后写入固定 plot/main.md。",
                    "mode": "interactive-write",
                    "routable": true,
                    "executionAvailable": true
                }, {
                    "id": "novelgainker.storytelling",
                    "version": 1,
                    "displayName": "小说续写与章节创作",
                    "description": "审查确定性上下文、章节提案与连续性；经独立差异确认可恢复地写入章节、迁移 accepted/final 状态，并单独更新固定 Story Ledger。",
                    "mode": "interactive-write",
                    "routable": false,
                    "executionAvailable": true
                }]
            })
        );
        let serialized = serialized.to_string();
        assert!(!serialized.contains("source.create_character"));
        assert!(!serialized.contains("allowedTools"));
        assert!(!serialized.contains("resource"));
        assert!(!serialized.contains("contentHash"));
        assert!(!serialized.contains("packHash"));
    }

    #[test]
    fn ng_302_restored_recovery_is_inert_and_can_still_be_explicitly_abandoned() {
        let root =
            std::env::temp_dir().join(format!("novelgainker-ng302-recovery-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("test root must create");
        let inspection =
            inspect_project_root(&root, ProjectScanMode::Create).expect("project must scan");
        let handle = create_project(&inspection).expect("project must create");
        let project_id = handle.project_id().to_owned();
        let characters_before = fs::read_dir(root.join("characters")).unwrap().count();
        let app_state = root.with_extension("app-state");
        let project_registry =
            ProjectRegistry::new(app_state.clone()).expect("registry must create");
        let checkpoint = WorkflowRunRecord {
            run_id: Uuid::new_v4().to_string(),
            project_id: project_id.clone(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: registry().pack_hash.to_owned(),
            state: WorkflowRunState::Generating,
            sequence: 4,
            provider_id: Some("deepseek".to_owned()),
            preset_version: Some(1),
            model: Some("deepseek-v4-pro".to_owned()),
            manifest_hash: Some("a".repeat(64)),
            proposal_hash: None,
            target_relative_path: Some("characters/example.md".to_owned()),
            target_source_hash: None,
            confirmed_at: None,
            updated_at: 4,
        };
        project_registry
            .app_state
            .save_workflow_run(&checkpoint)
            .unwrap();
        drop(project_registry);

        let restarted = ProjectRegistry::new(app_state.clone()).expect("registry must restart");
        assert_eq!(restarted.active_provider_id(), None);
        assert!(!workflow_generation_active(&restarted).unwrap());
        assert!(!legacy_workflow_blocks_unified_runtime(&restarted).unwrap());
        let write_lease = try_acquire_project_write_lease(&handle.coordination_key())
            .expect("writer lease must resolve")
            .expect("writer lease must be available");
        let session = register_session(&restarted, None, handle, Some(write_lease)).unwrap();
        let status = load_registered_workflow_status(&restarted, &session.session_id).unwrap();
        let recovered = status.run.expect("recovered run must be visible");
        assert!(status.available);
        assert!(status.has_active_run);
        assert_eq!(recovered.state, "recovery_required");
        assert!(recovered.recovery_required);
        assert!(!recovered.can_cancel);
        assert!(recovered.can_abandon);

        let confirmation = mutate_registered_workflow_run(
            &restarted,
            &session.session_id,
            WorkflowRunAction::Abandon { confirmed: false },
        )
        .unwrap_err();
        assert_eq!(confirmation.code, "workflow.confirmation_required");
        let abandoned = mutate_registered_workflow_run(
            &restarted,
            &session.session_id,
            WorkflowRunAction::Abandon { confirmed: true },
        )
        .unwrap();
        assert!(!abandoned.has_active_run);
        assert_eq!(abandoned.run.unwrap().state, "cancelled");
        assert_eq!(
            fs::read_dir(root.join("characters")).unwrap().count(),
            characters_before
        );
        assert_eq!(restarted.app_state.provider_settings().unwrap().len(), 2);

        drop(restarted);
        fs::remove_dir_all(app_state).expect("verified app state must remove");
        fs::remove_dir_all(root).expect("verified project must remove");
    }

    #[test]
    fn ng_302_generating_cancel_shares_the_provider_lock_and_cannot_revive() {
        let root =
            std::env::temp_dir().join(format!("novelgainker-ng302-cancel-{}", Uuid::new_v4()));
        fs::create_dir(&root).expect("test root must create");
        let inspection =
            inspect_project_root(&root, ProjectScanMode::Create).expect("project must scan");
        let handle = create_project(&inspection).expect("project must create");
        let app_state = root.with_extension("app-state");
        let registry_state = ProjectRegistry::new(app_state.clone()).expect("registry must create");
        let write_lease = try_acquire_project_write_lease(&handle.coordination_key())
            .expect("writer lease must resolve")
            .expect("writer lease must be available");
        let session =
            register_session(&registry_state, None, handle.clone(), Some(write_lease)).unwrap();
        let initial = WorkflowRunRecord {
            run_id: Uuid::new_v4().to_string(),
            project_id: handle.project_id().to_owned(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("deepseek".to_owned()),
            preset_version: Some(1),
            model: Some("deepseek-v4-pro".to_owned()),
            manifest_hash: None,
            proposal_hash: None,
            target_relative_path: None,
            target_source_hash: None,
            confirmed_at: None,
            updated_at: 1,
        };
        registry_state
            .app_state
            .save_workflow_run(&initial)
            .unwrap();
        {
            let mut guard = registry_state.workflow_manager.lock().unwrap();
            let manager = guard.as_mut().expect("manager must be available");
            manager.create(initial.clone()).unwrap();
            for (index, next) in [
                WorkflowRunState::Routed,
                WorkflowRunState::AwaitingInput,
                WorkflowRunState::ContextReady,
                WorkflowRunState::Generating,
            ]
            .into_iter()
            .enumerate()
            {
                let updated = manager
                    .transition(&initial.run_id, index as u64 + 1, next, index as i64 + 2)
                    .unwrap();
                registry_state
                    .app_state
                    .save_workflow_run(&updated)
                    .unwrap();
            }
        }

        let cancellation = CancellationToken::default();
        registry_state
            .begin_provider_run(
                ProviderId::Deepseek,
                initial.run_id.clone(),
                cancellation.clone(),
            )
            .unwrap();
        assert!(workflow_generation_active(&registry_state).unwrap());
        let parallel_chat = registry_state
            .begin_provider_run(
                ProviderId::Openai,
                Uuid::new_v4().to_string(),
                CancellationToken::default(),
            )
            .unwrap_err();
        assert_eq!(parallel_chat.code, "provider.run_active");

        let cancelled = mutate_registered_workflow_run(
            &registry_state,
            &session.session_id,
            WorkflowRunAction::Cancel,
        )
        .unwrap();
        assert!(cancellation.is_cancelled());
        assert_eq!(cancelled.run.unwrap().state, "cancelled");
        assert!(!cancelled.has_active_run);
        let duplicate = mutate_registered_workflow_run(
            &registry_state,
            &session.session_id,
            WorkflowRunAction::Cancel,
        )
        .unwrap_err();
        assert_eq!(duplicate.code, "workflow.invalid_state");
        registry_state.finish_provider_run(&initial.run_id);

        drop(registry_state);
        fs::remove_dir_all(app_state).expect("verified app state must remove");
        fs::remove_dir_all(root).expect("verified project must remove");
    }

    #[test]
    fn ga2_retires_only_orphaned_legacy_generation_before_agent_use() {
        let app_state = std::env::temp_dir().join(format!(
            "novelgainker-ga2-orphaned-workflow-{}",
            Uuid::new_v4()
        ));
        let registry_state = ProjectRegistry::new(app_state.clone()).expect("registry must create");
        let initial = WorkflowRunRecord {
            run_id: Uuid::new_v4().to_string(),
            project_id: Uuid::new_v4().to_string(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("deepseek".to_owned()),
            preset_version: Some(1),
            model: Some("deepseek-v4-pro".to_owned()),
            manifest_hash: None,
            proposal_hash: None,
            target_relative_path: None,
            target_source_hash: None,
            confirmed_at: None,
            updated_at: 1,
        };
        let generating = {
            let mut guard = registry_state.workflow_manager.lock().unwrap();
            let manager = guard.as_mut().expect("manager must be available");
            manager.create(initial.clone()).unwrap();
            let mut current = initial.clone();
            for next in [
                WorkflowRunState::Routed,
                WorkflowRunState::AwaitingInput,
                WorkflowRunState::ContextReady,
                WorkflowRunState::Generating,
            ] {
                current = manager
                    .transition(
                        &initial.run_id,
                        current.sequence + 1,
                        next,
                        current.updated_at + 1,
                    )
                    .unwrap();
            }
            current
        };
        registry_state
            .app_state
            .save_workflow_run(&generating)
            .unwrap();
        registry_state
            .workflow_sessions
            .lock()
            .unwrap()
            .insert(initial.run_id.clone(), "legacy-session".to_owned());

        assert_eq!(registry_state.active_provider_id(), None);
        assert!(workflow_generation_active(&registry_state).unwrap());
        assert!(retire_orphaned_legacy_workflow_generation(&registry_state).unwrap());
        assert!(!workflow_generation_active(&registry_state).unwrap());
        assert!(!registry_state
            .workflow_manager
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .has_active_run());
        assert!(!registry_state
            .workflow_sessions
            .lock()
            .unwrap()
            .contains_key(&initial.run_id));
        let stored = registry_state
            .app_state
            .workflow_runs()
            .unwrap()
            .into_iter()
            .find(|record| record.run_id == initial.run_id)
            .expect("retired record must persist");
        assert_eq!(stored.state, WorkflowRunState::Cancelled);
        assert!(!retire_orphaned_legacy_workflow_generation(&registry_state).unwrap());

        drop(registry_state);
        fs::remove_dir_all(app_state).expect("verified app state must remove");
    }
    #[test]
    fn registers_only_the_approved_commands() {
        assert_eq!(
            APP_COMMANDS,
            [
                "health_check",
                "app_state_load",
                "app_state_save_ui",
                "agent_skill_registry_load",
                "agent_session_load",
                "agent_turn_start",
                "agent_turn_cancel",
                "agent_session_clear",
                "agent_memory_compact",
                "agent_history_branch_resolve",
                "agent_write_policy_set",
                "agent_context_mark_upsert",
                "agent_context_mark_remove",
                "agent_conflict_resolution_submit",
                "agent_write_confirmation_submit",
                "agent_write_plan_reject",
                "agent_write_undo_apply",
                "provider_status_load",
                "provider_key_save",
                "provider_key_delete",
                "provider_select",
                "provider_connection_test",
                "project_scan",
                "project_create_preview",
                "project_create",
                "project_open",
                "project_open_recent",
                "project_list_files",
                "project_read_file",
                "app_state_set_current_file",
                "project_watch_file",
                "project_save_file",
                "project_validate_draft",
                "project_search",
                "project_close"
            ]
        );
        assert!(!APP_COMMANDS.contains(&"shell"));
        assert!(!APP_COMMANDS.contains(&"process"));
        assert!(APP_COMMANDS.contains(&"agent_skill_registry_load"));
        assert!(APP_COMMANDS.contains(&"agent_session_load"));
        assert!(APP_COMMANDS.contains(&"agent_turn_start"));
        assert!(APP_COMMANDS.contains(&"agent_turn_cancel"));
        assert!(APP_COMMANDS.contains(&"agent_session_clear"));
        assert!(APP_COMMANDS.contains(&"agent_memory_compact"));
        assert!(APP_COMMANDS.contains(&"agent_history_branch_resolve"));
        assert!(APP_COMMANDS.contains(&"agent_write_policy_set"));
        assert!(APP_COMMANDS.contains(&"agent_context_mark_upsert"));
        assert!(APP_COMMANDS.contains(&"agent_context_mark_remove"));
        assert!(APP_COMMANDS.contains(&"agent_conflict_resolution_submit"));
        assert!(APP_COMMANDS.contains(&"agent_write_confirmation_submit"));
        assert!(APP_COMMANDS.contains(&"agent_write_plan_reject"));
        assert!(APP_COMMANDS.contains(&"agent_write_undo_apply"));
        assert!(APP_COMMANDS.contains(&"provider_select"));
        assert!(APP_COMMANDS.contains(&"provider_connection_test"));
        assert!(APP_COMMANDS.contains(&"project_read_file"));
        assert!(APP_COMMANDS.contains(&"project_create_preview"));
        assert!(!APP_COMMANDS.contains(&"source.create_character"));
        assert!(APP_COMMANDS.contains(&"project_watch_file"));
        assert!(APP_COMMANDS.contains(&"project_save_file"));
        assert!(APP_COMMANDS.contains(&"project_validate_draft"));
        assert!(APP_COMMANDS.contains(&"project_search"));
        assert!(APP_COMMANDS.contains(&"provider_status_load"));
        assert!(APP_COMMANDS.contains(&"provider_key_save"));
        assert!(APP_COMMANDS.contains(&"provider_key_delete"));
        assert_eq!(APP_COMMANDS.len(), 35);
        assert_eq!(
            APP_COMMANDS
                .iter()
                .filter(|command| command.starts_with("agent_"))
                .count(),
            14
        );
        for removed in [
            "agent_policy_set",
            "agent_write_confirm",
            "agent_write_reject",
            "agent_undo_apply",
            "provider_chat_start",
            "provider_chat_cancel",
            "router_resolve_local",
            "router_run_start",
            "router_run_cancel",
            "workflow_registry_load",
            "workflow_run_status_load",
            "workflow_source_catalog",
            "workflow_run_prepare",
            "workflow_run_generate",
            "workflow_proposal_apply",
            "workflow_run_cancel",
            "workflow_run_abandon",
            "story_context_catalog",
            "story_context_build",
            "story_context_request_preview",
            "story_chapter_generate_start",
            "story_chapter_status_load",
            "story_chapter_generate_cancel",
            "story_continuity_check",
            "story_continuity_status_load",
            "story_continuity_warning_confirm",
            "story_chapter_patch_preview",
            "story_chapter_patch_status_load",
            "story_chapter_patch_apply",
            "story_chapter_state_preview",
            "story_chapter_lifecycle_status_load",
            "story_chapter_state_apply",
            "story_ledger_update_preview",
            "story_ledger_update_apply",
        ] {
            assert!(
                !APP_COMMANDS.contains(&removed),
                "{removed} must be retired"
            );
        }
    }
}
