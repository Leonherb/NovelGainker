use app_core::app_state::{AppStateRepository, AppStateSnapshot, UiPreferences};
use app_core::canon_change::{
    ChangeDecisionRequest, ChangeDetail, ChangeError, ChangeListRequest, ChangePage, ChangeReceipt,
    EditorBufferRevision,
};
use app_core::canon_change_store::CanonChangeCoordinator;
use app_core::conversation::{
    finish_conversation_turn, load_conversation, read_conversation, start_conversation_turn,
    ConversationProviderTerminalState, ConversationTerminalKind,
};
use app_core::conversation_runtime::{
    execute_memory_managed_turn, execute_memory_managed_turn_with_changes, AgentRuntimeEvent,
};
use app_core::coordination::{try_acquire_project_write_lease, ProjectWriteLease};
use app_core::credential_vault::{CredentialVault, ProviderId, WindowsCredentialVault};

use app_core::project::{
    create_prepared_project, inspect_project_root, open_project, prepare_project_create,
    PreparedProjectCreate, ProjectCreateInputV1, ProjectCreatePreview, ProjectHandle,
    ProjectInspection, ProjectScanMode, ProjectScanPreview,
};
use app_core::provider_core::{
    preset, CancellationToken, FinishReason, NormalizedEvent, ProviderError, ProviderHttpClient,
    Usage,
};
use app_core::provider_state::{
    delete_provider_key, load_provider_state, save_provider_key, select_provider,
    test_provider_connection, ProviderStateSnapshot, ProviderStatus,
};
use app_core::recovery::{RecoveryReport, RecoveryStatus, RecoveryStore};
use app_core::search::{search_project_sources, SearchReport};
use app_core::task_store::{runtime_view_for, ConversationRuntimeView, TaskActivity};
use app_core::workspace::{
    check_project_file, list_project_files, read_project_file, validate_project_draft,
    ProjectFileDocument, ProjectFileIndex, ProjectFileSaveResult, ProjectFileWatchStatus,
    ValidationReport,
};
use app_core::{health_status, AppError, HealthStatus};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
include!("commands_sessions.rs");
include!("commands_session_management.rs");
include!("commands_retry.rs");

pub struct ProjectRegistry {
    pub writer_window: Option<isize>,
    inner: Mutex<ProjectRegistryInner>,
    recovery_store: RecoveryStore,
    app_state: AppStateRepository,
    credential_vault: Arc<dyn CredentialVault>,
    provider_run: Mutex<Option<ActiveProviderRun>>,
    reconciling_sessions: Mutex<HashSet<String>>,
    conversation_export: Mutex<Option<PendingConversationExport>>,
    #[cfg(test)]
    test_client_factory: Option<TestClientFactory>,
}

#[cfg(test)]
type TestClientFactory = Arc<dyn Fn() -> Result<ProviderHttpClient, ProviderError> + Send + Sync>;

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
    _window_registration: Option<app_core::coordination::ProjectWindowRegistration>,
    handle: ProjectHandle,
    write_lease: Option<Arc<ProjectWriteLease>>,
    recovery: RecoveryReport,
    changes: Option<Arc<CanonChangeCoordinator>>,
    editor_files: HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorDocument {
    #[serde(flatten)]
    document: ProjectFileDocument,
    file_id: String,
    editor_revision: u64,
}

struct ActiveProviderRun {
    attempt_id: Option<String>,
    attempt_number: u8,
    project_key: String,
    conversation_session_id: String,
    turn_id: Option<String>,
    provider_id: ProviderId,
    cancellation: CancellationToken,
}

impl ProjectRegistry {
    pub fn new(app_state_root: PathBuf) -> Result<Self, AppError> {
        Ok(Self {
            writer_window: None,
            inner: Mutex::new(ProjectRegistryInner::default()),
            recovery_store: RecoveryStore::new(app_state_root.clone())?,
            app_state: AppStateRepository::open(&app_state_root),
            credential_vault: Arc::new(WindowsCredentialVault::production()),
            provider_run: Mutex::new(None),
            reconciling_sessions: Mutex::new(HashSet::new()),
            conversation_export: Mutex::new(None),
            #[cfg(test)]
            test_client_factory: None,
        })
    }

    fn provider_client(&self) -> Result<ProviderHttpClient, ProviderError> {
        #[cfg(test)]
        if let Some(factory) = &self.test_client_factory {
            return factory();
        }
        ProviderHttpClient::production()
    }

    pub fn cancel_active_provider_run(&self) {
        if let Ok(active) = self.provider_run.lock() {
            if let Some(run) = active.as_ref() {
                run.cancellation.cancel();
            }
        }
    }

    fn active_provider_id(&self) -> Option<ProviderId> {
        self.provider_run
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|run| run.provider_id))
    }

    fn reserve_provider_run(
        &self,
        project_key: &str,
        conversation_session_id: &str,
        provider_id: ProviderId,
    ) -> Result<CancellationToken, AppError> {
        let mut active = self
            .provider_run
            .lock()
            .map_err(|_| AppError::runtime_unavailable())?;
        if active.is_some()
            || self
                .reconciling_sessions
                .lock()
                .map_err(|_| AppError::runtime_unavailable())?
                .contains(project_key)
        {
            return Err(AppError::provider_run_active());
        }
        let cancellation = CancellationToken::default();
        *active = Some(ActiveProviderRun {
            attempt_id: None,
            attempt_number: 0,
            project_key: project_key.to_owned(),
            conversation_session_id: conversation_session_id.to_owned(),
            turn_id: None,
            provider_id,
            cancellation: cancellation.clone(),
        });
        Ok(cancellation)
    }

    fn bind_provider_turn(&self, turn_id: &str) -> Result<(), AppError> {
        let mut active = self
            .provider_run
            .lock()
            .map_err(|_| AppError::runtime_unavailable())?;
        let run = active
            .as_mut()
            .ok_or_else(AppError::provider_run_not_found)?;
        run.turn_id = Some(turn_id.to_owned());
        Ok(())
    }

    fn bind_provider_attempt(
        &self,
        turn_id: &str,
        attempt: &app_core::conversation::RetryState,
    ) -> Result<(), AppError> {
        let mut active = self
            .provider_run
            .lock()
            .map_err(|_| AppError::runtime_unavailable())?;
        let run = active
            .as_mut()
            .filter(|run| run.turn_id.as_deref() == Some(turn_id))
            .ok_or_else(AppError::provider_run_not_found)?;
        run.attempt_id = Some(attempt.attempt_id.clone());
        run.attempt_number = attempt.attempt_number;
        Ok(())
    }
    fn finish_provider_run(&self, turn_id: &str) {
        if let Ok(mut active) = self.provider_run.lock() {
            if active.as_ref().and_then(|run| run.turn_id.as_deref()) == Some(turn_id) {
                *active = None;
            }
        }
    }

    fn release_provider_reservation(&self) {
        if let Ok(mut active) = self.provider_run.lock() {
            if active.as_ref().is_some_and(|run| run.turn_id.is_none()) {
                *active = None;
            }
        }
    }

    fn has_active_conversation(&self, session_id: &str, conversation_id: &str) -> bool {
        let Ok(handle) = session_handle(self, session_id) else {
            return false;
        };
        let project_key = handle.coordination_key();
        self.provider_run.lock().ok().is_some_and(|active| {
            active.as_ref().is_some_and(|run| {
                run.project_key == project_key && run.conversation_session_id == conversation_id
            })
        })
    }

    fn cancel_provider_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        conversation_id: Option<&str>,
        attempt_id: Option<&str>,
    ) -> bool {
        let Ok(handle) = session_handle(self, session_id) else {
            return false;
        };
        let project_key = handle.coordination_key();
        self.provider_run
            .lock()
            .ok()
            .and_then(|active| {
                active.as_ref().map(|run| {
                    let matches = run.project_key == project_key
                        && run.turn_id.as_deref() == Some(turn_id)
                        && conversation_id.is_none_or(|id| run.conversation_session_id == id)
                        && attempt_id.map_or(run.attempt_number <= 1, |id| {
                            run.attempt_id.as_deref() == Some(id)
                        });
                    if matches {
                        run.cancellation.cancel();
                    }
                    matches
                })
            })
            .unwrap_or(false)
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnStartResult {
    pub attempt_id: String,
    pub attempt_number: u8,
    pub session_id: String,
    pub conversation_session_id: String,
    pub turn_id: String,
    pub provider_id: ProviderId,
    pub model: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnCancelResult {
    pub cancel_requested: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ConversationTurnEvent {
    TextDelta {
        session_id: String,
        turn_id: String,
        provider_id: ProviderId,
        model: String,
        sequence: u32,
        delta: String,
    },
    Usage {
        session_id: String,
        turn_id: String,
        provider_id: ProviderId,
        model: String,
        sequence: u32,
        usage: Usage,
    },
    TaskActivity {
        session_id: String,
        turn_id: String,
        provider_id: ProviderId,
        model: String,
        sequence: u32,
        task_activity: TaskActivity,
    },
    Completed {
        session_id: String,
        turn_id: String,
        provider_id: ProviderId,
        model: String,
        sequence: u32,
        finish_reason: FinishReason,
    },
    Failed {
        session_id: String,
        turn_id: String,
        provider_id: ProviderId,
        model: String,
        sequence: u32,
        error: AppError,
    },
    Cancelled {
        session_id: String,
        turn_id: String,
        provider_id: ProviderId,
        model: String,
        sequence: u32,
    },
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

#[tauri::command]
pub fn app_state_save_ui(
    state: tauri::State<'_, ProjectRegistry>,
    preferences: UiPreferences,
) -> Result<AppStateWriteResult, AppError> {
    state.app_state.save_ui(preferences)?;
    Ok(AppStateWriteResult { saved: true })
}

#[tauri::command]
pub fn conversation_session_load(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    conversation_session_id: Option<String>,
) -> Result<ConversationRuntimeView, AppError> {
    let history_lease = session_history_lease(&state, &session_id)?;
    reconcile_session_run(&state, &session_id)?;
    let handle = selected_session_handle(&state, &session_id, conversation_session_id.as_deref())?;
    let active = state.has_active_conversation(
        &session_id,
        handle
            .conversation_session_id()
            .ok_or_else(AppError::runtime_unavailable)?,
    );
    let conversation = if active || history_lease.is_none() {
        read_conversation(&handle)?
    } else {
        load_conversation(&handle, now_ms()?)?
    };
    runtime_view_for(
        &handle,
        conversation,
        now_ms()?,
        !active && history_lease.is_some(),
        app_core::protocol_profile::ToolProtocolProfile::Nr5,
    )
}

#[tauri::command]
pub fn conversation_turn_start<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    message: String,
    conversation_session_id: Option<String>,
) -> Result<ConversationTurnStartResult, AppError> {
    let history_lease = require_session_history_lease(&state, &session_id)?;
    reconcile_session_run(&state, &session_id)?;
    let handle = selected_session_handle(&state, &session_id, conversation_session_id.as_deref())?;
    let provider_state = load_provider_state(&state.app_state, state.credential_vault.as_ref())?;
    let provider_id = provider_state.selected_provider_id;
    if !provider_state
        .providers
        .iter()
        .any(|provider| provider.provider_id == provider_id && provider.configured)
    {
        return Err(AppError::provider_not_ready());
    }
    let model = preset(provider_id).model.to_owned();
    let started_at_ms = now_ms()?;
    let cancellation = state.reserve_provider_run(
        &handle.coordination_key(),
        handle
            .conversation_session_id()
            .ok_or_else(AppError::runtime_unavailable)?,
        provider_id,
    )?;
    let reservation = match sessions::reserve_run(&handle, started_at_ms) {
        Ok(value) => value,
        Err(error) => {
            state.release_provider_reservation();
            return Err(error);
        }
    };
    let started =
        match start_conversation_turn(&handle, provider_id, &model, &message, started_at_ms) {
            Ok(started) => started,
            Err(error) => {
                let _ = sessions::release_run(&handle, &reservation.reservation_id);
                state.release_provider_reservation();
                return Err(error);
            }
        };
    let attempt =
        match sessions::bind_run_turn(&handle, &reservation.reservation_id, &started.turn_id)
            .and_then(|_| state.bind_provider_turn(&started.turn_id))
            .and_then(|_| app_core::conversation::initialize_attempt(&handle, &started.turn_id))
            .and_then(|attempt| {
                state
                    .bind_provider_attempt(&started.turn_id, &attempt)
                    .map(|()| attempt)
            }) {
            Ok(value) => value,
            Err(error) => {
                let _ = finish_conversation_turn(
                    &handle,
                    &started.turn_id,
                    "",
                    ConversationTerminalKind::Failed,
                    Some(&error.code),
                    now_ms().unwrap_or(started_at_ms),
                );
                let _ = sessions::release_run(&handle, &reservation.reservation_id);
                state.finish_provider_run(&started.turn_id);
                state.release_provider_reservation();
                return Err(error);
            }
        };

    let result = ConversationTurnStartResult {
        attempt_id: attempt.attempt_id.clone(),
        attempt_number: attempt.attempt_number,
        session_id: session_id.clone(),
        conversation_session_id: started.session_id.clone(),
        turn_id: started.turn_id.clone(),
        provider_id,
        model: model.clone(),
    };
    let worker_session_id = session_id;
    let worker_conversation_session_id = started.session_id;
    let worker_turn_id = started.turn_id;
    tauri::async_runtime::spawn_blocking(move || {
        // Keep physical writer exclusion after the registry row is closed.
        let _history_lease = history_lease;
        run_conversation_provider(
            app,
            handle,
            worker_session_id,
            worker_conversation_session_id,
            reservation.reservation_id,
            worker_turn_id,
            attempt,
            provider_id,
            model,
            cancellation,
        );
    });
    Ok(result)
}

#[tauri::command]
pub fn conversation_turn_cancel(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    turn_id: String,
    conversation_session_id: Option<String>,
    attempt_id: Option<String>,
) -> ConversationTurnCancelResult {
    ConversationTurnCancelResult {
        cancel_requested: state.cancel_provider_turn(
            &session_id,
            &turn_id,
            conversation_session_id.as_deref(),
            attempt_id.as_deref(),
        ),
    }
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
    if state.active_provider_id().is_some() {
        return Err(AppError::provider_run_active());
    }
    select_provider(
        &state.app_state,
        state.credential_vault.as_ref(),
        &provider_id,
    )
}

#[tauri::command]
pub async fn provider_connection_test<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    provider_id: String,
) -> Result<ProviderStatus, AppError> {
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

#[tauri::command]
pub fn app_state_set_current_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
) -> Result<AppStateWriteResult, AppError> {
    let handle = session_handle(&state, &session_id)?;
    let document = read_project_file(&handle, &relative_path)?;
    state
        .app_state
        .record_current_file(handle.project_id(), &document.relative_path)?;
    Ok(AppStateWriteResult { saved: true })
}

#[tauri::command]
pub async fn project_scan<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ProjectRegistry>,
    mode: ProjectScanMode,
) -> Result<Option<ProjectCandidate>, AppError> {
    let Some(selected) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let selected_root = selected
        .into_path()
        .map_err(|_| AppError::project_invalid_root())?;
    let inspection = inspect_project_root(&selected_root, mode)?;
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
    let write_lease = try_acquire_project_write_lease(&inspection.coordination_key())?;
    if write_lease.is_none() {
        return Err(existing_project_window(&inspection.coordination_key()));
    }
    let allow_metadata_write =
        write_lease.is_some() && (confirm_metadata || inspection.requires_clean_slate());
    let handle = open_project(&inspection, allow_metadata_write)?;
    register_session(&state, Some(candidate_id), handle, write_lease)
}

#[tauri::command]
pub async fn project_open_recent(
    state: tauri::State<'_, ProjectRegistry>,
    project_id: String,
) -> Result<ProjectSession, AppError> {
    Uuid::parse_str(&project_id).map_err(|_| AppError::app_state_unavailable())?;
    let root = state
        .app_state
        .recent_project_root(&project_id)
        .ok_or_else(AppError::app_state_unavailable)?;
    let inspection = inspect_project_root(&root, ProjectScanMode::Open).inspect_err(|_| {
        state.app_state.forget_project(&project_id);
    })?;
    if inspection.preview().requires_metadata_confirmation {
        state.app_state.forget_project(&project_id);
        return Err(AppError::project_confirmation_required());
    }
    let write_lease = try_acquire_project_write_lease(&inspection.coordination_key())?;
    if write_lease.is_none() {
        return Err(existing_project_window(&inspection.coordination_key()));
    }
    let handle = open_project(
        &inspection,
        write_lease.is_some() && inspection.requires_clean_slate(),
    )?;
    register_session(&state, None, handle, write_lease)
}

#[tauri::command]
pub async fn project_list_files(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<ProjectFileIndex, AppError> {
    let handle = session_handle(&state, &session_id)?;
    if let Ok(changes) = changes_for_session(&state, &session_id) {
        return changes
            .read_consistent(|| list_project_files(&handle).map_err(|_| ChangeError::Io))
            .map_err(change_app_error);
    }
    list_project_files(&handle)
}

#[tauri::command]
pub async fn project_read_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
) -> Result<EditorDocument, AppError> {
    let mut registry = lock_registry(&state)?;
    let session = registry
        .sessions
        .get_mut(&session_id)
        .ok_or_else(AppError::project_session_not_found)?;
    let (document, editor_revision) = if let Some(changes) = &session.changes {
        changes
            .open_editor(&relative_path)
            .map_err(change_app_error)?
    } else {
        (read_project_file(&session.handle, &relative_path)?, 0)
    };
    let file_id = if let Some((id, _)) = session
        .editor_files
        .iter()
        .find(|(_, path)| **path == relative_path)
    {
        id.clone()
    } else {
        if session.editor_files.len() >= 64 {
            return Err(change_app_error(ChangeError::BudgetExceeded));
        }
        let id = Uuid::new_v4().to_string();
        session.editor_files.insert(id.clone(), relative_path);
        id
    };
    Ok(EditorDocument {
        document,
        file_id,
        editor_revision,
    })
}

#[tauri::command]
pub async fn project_watch_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
    expected_hash: String,
) -> Result<ProjectFileWatchStatus, AppError> {
    check_project_file(
        &session_handle(&state, &session_id)?,
        &relative_path,
        &expected_hash,
    )
}

#[tauri::command]
pub async fn project_save_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
    content: String,
    source_hash: String,
) -> Result<ProjectFileSaveResult, AppError> {
    writer_handle(&state, &session_id)?;
    changes_for_session(&state, &session_id)?
        .save_manual(&relative_path, &content, &source_hash)
        .map_err(|error| {
            if error == ChangeError::SourceChanged {
                AppError::file_source_changed()
            } else {
                change_app_error(error)
            }
        })
}

fn change_app_error(error: ChangeError) -> AppError {
    AppError {
        code: error.code().to_owned(),
        message_key: "errors.fileSaveFailed".to_owned(),
        diagnostic: None,
    }
}

fn publication_app_error(error: app_core::publication::PublicationIssue) -> AppError {
    AppError {
        code: error.code().to_owned(),
        message_key: "errors.publicationUnavailable".to_owned(),
        diagnostic: None,
    }
}

#[tauri::command]
pub async fn publication_list(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    cursor: Option<String>,
) -> Result<app_core::publication::PublicationPage, AppError> {
    let changes = changes_for_session(&state, &session_id)?;
    if cursor.as_ref().is_some_and(|value| value.len() > 72) {
        return Err(publication_app_error(
            app_core::publication::PublicationIssue::SourceRequired,
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        changes
            .publication_list(cursor.as_deref())
            .map_err(publication_app_error)
    })
    .await
    .map_err(|_| publication_app_error(app_core::publication::PublicationIssue::StoreUnavailable))?
}

#[tauri::command]
pub async fn publication_detail(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    artifact_id: String,
) -> Result<app_core::publication::PublicationDetail, AppError> {
    let changes = changes_for_session(&state, &session_id)?;
    if uuid::Uuid::parse_str(&artifact_id).is_err() || artifact_id.len() != 36 {
        return Err(publication_app_error(
            app_core::publication::PublicationIssue::SourceRequired,
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        changes
            .publication_detail(&artifact_id)
            .map_err(publication_app_error)
    })
    .await
    .map_err(|_| publication_app_error(app_core::publication::PublicationIssue::StoreUnavailable))?
}

fn changes_for_session(
    state: &ProjectRegistry,
    session_id: &str,
) -> Result<Arc<CanonChangeCoordinator>, AppError> {
    let handle = selected_session_handle(state, session_id, None)?;
    changes_for_handle(state, session_id, handle)
}

fn existing_project_window(key: &str) -> AppError {
    if app_core::coordination::activate_project_window(key) {
        AppError {
            code: "project.window_activated".into(),
            message_key: "errors.projectWindowActivated".into(),
            diagnostic: None,
        }
    } else {
        AppError {
            code: "project.already_open".into(),
            message_key: "errors.projectAlreadyOpen".into(),
            diagnostic: None,
        }
    }
}
fn changes_for_handle(
    state: &ProjectRegistry,
    session_id: &str,
    handle: ProjectHandle,
) -> Result<Arc<CanonChangeCoordinator>, AppError> {
    let coordinator = lock_registry(state)?
        .sessions
        .get(session_id)
        .ok_or_else(AppError::project_session_not_found)?
        .changes
        .clone()
        .ok_or_else(AppError::project_writer_required)?;
    coordinator
        .for_conversation(handle)
        .map(Arc::new)
        .map_err(change_app_error)
}

#[tauri::command]
pub async fn canon_change_list(
    state: tauri::State<'_, ProjectRegistry>,
    request: ChangeListRequest,
    conversation_session_id: Option<String>,
) -> Result<ChangePage, AppError> {
    request.validate().map_err(change_app_error)?;
    let handle = selected_session_handle(
        &state,
        &request.project_session_id,
        conversation_session_id.as_deref(),
    )?;
    let conversation = read_conversation(&handle)?;
    match changes_for_handle(&state, &request.project_session_id, handle) {
        Ok(changes) => changes
            .activities_page(&conversation.session_id, &request)
            .map_err(change_app_error),
        Err(error) if error.code == "project.writer_required" => Ok(ChangePage {
            schema_version: 1,
            items: Vec::new(),
            next_cursor: None,
            candidates: Vec::new(),
        }),
        Err(error) => Err(error),
    }
}

#[tauri::command]
pub async fn canon_change_detail(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    change_id: String,
    conversation_session_id: Option<String>,
) -> Result<ChangeDetail, AppError> {
    let handle = selected_session_handle(&state, &session_id, conversation_session_id.as_deref())?;
    let conversation = read_conversation(&handle)?;
    changes_for_handle(&state, &session_id, handle)?
        .detail(&conversation.session_id, &change_id)
        .map_err(change_app_error)
}

#[tauri::command]
pub async fn canon_change_decide(
    state: tauri::State<'_, ProjectRegistry>,
    request: ChangeDecisionRequest,
    conversation_session_id: Option<String>,
) -> Result<ChangeReceipt, AppError> {
    request.validate().map_err(change_app_error)?;
    let handle = selected_session_handle(
        &state,
        &request.project_session_id,
        conversation_session_id.as_deref(),
    )?;
    let conversation = read_conversation(&handle)?;
    if request.decision == app_core::canon_change::ChangeDecision::Confirm
        && sessions::is_archived(&handle)?
    {
        return Err(session_export_error("sessions.archived"));
    }
    changes_for_handle(&state, &request.project_session_id, handle)?
        .decide(
            &conversation.session_id,
            &request.change_id,
            request.revision,
            request.decision,
            &CancellationToken::default(),
        )
        .map_err(change_app_error)
}

#[tauri::command]
pub async fn canon_editor_state(
    state: tauri::State<'_, ProjectRegistry>,
    request: EditorBufferRevision,
) -> Result<u64, AppError> {
    let registry = lock_registry(&state)?;
    let session = registry
        .sessions
        .get(&request.project_session_id)
        .ok_or_else(AppError::project_session_not_found)?;
    let path = session
        .editor_files
        .get(&request.file_id)
        .ok_or_else(|| change_app_error(ChangeError::TargetInvalid))?;
    session
        .changes
        .as_ref()
        .ok_or_else(AppError::project_writer_required)?
        .record_editor_buffer(path, request.revision, request.dirty)
        .map_err(change_app_error)?;
    Ok(request.revision)
}

#[tauri::command]
pub async fn project_validate_draft(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
    content: String,
) -> Result<ValidationReport, AppError> {
    validate_project_draft(
        &session_handle(&state, &session_id)?,
        &relative_path,
        &content,
    )
}

#[tauri::command]
pub async fn project_search(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    query: String,
    case_sensitive: bool,
) -> Result<SearchReport, AppError> {
    let handle = session_handle(&state, &session_id)?;
    if let Ok(changes) = changes_for_session(&state, &session_id) {
        return changes
            .read_consistent(|| {
                search_project_sources(&handle, &query, case_sensitive).map_err(|_| ChangeError::Io)
            })
            .map_err(change_app_error);
    }
    search_project_sources(&handle, &query, case_sensitive)
}

#[tauri::command]
pub fn project_close(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<ProjectCloseResult, AppError> {
    state.cancel_active_provider_run();
    if let Ok(mut pending) = state.conversation_export.lock() {
        if pending
            .as_ref()
            .is_some_and(|entry| entry.project_session_id == session_id && !entry.in_flight)
        {
            *pending = None;
        }
    }
    let closed = lock_registry(&state)?
        .sessions
        .remove(&session_id)
        .is_some();
    Ok(ProjectCloseResult { closed })
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

fn writer_handle(state: &ProjectRegistry, session_id: &str) -> Result<ProjectHandle, AppError> {
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
    let write_lease = write_lease.map(Arc::new);
    let mut recovery = if write_lease.is_some() {
        state.recovery_store.recover_project(&handle)
    } else {
        RecoveryReport::not_checked()
    };
    let changes = write_lease
        .as_ref()
        .map(|lease| {
            CanonChangeCoordinator::new_nr5(
                handle.clone(),
                lease.clone(),
                state.recovery_store.clone(),
            )
            .map(Arc::new)
        })
        .transpose()
        .map_err(change_app_error)?;
    if let Some(changes) = &changes {
        if changes.recover().is_err() {
            recovery.status = RecoveryStatus::Required;
            recovery.required_transactions = recovery.required_transactions.saturating_add(1);
        }
    }
    let session_id = Uuid::new_v4().to_string();
    let can_write = write_lease.is_some() && recovery.status != RecoveryStatus::Required;
    let restored_current_file = state
        .app_state
        .current_file(handle.project_id())
        .filter(|relative_path| read_project_file(&handle, relative_path).is_ok());
    let _ = state.app_state.record_project(&handle);
    let result = ProjectSession {
        session_id: session_id.clone(),
        project_id: handle.project_id().to_owned(),
        display_name: handle.display_name().to_owned(),
        schema_version: handle.schema_version(),
        writing_language: handle.writing_language().to_owned(),
        markdown_file_count: handle.markdown_file_count(),
        access_mode: if write_lease.is_some() {
            ProjectAccessMode::Writer
        } else {
            ProjectAccessMode::ReadOnly
        },
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
    let window_registration = match (write_lease.as_ref(), state.writer_window) {
        (Some(lease), Some(hwnd)) => Some(
            app_core::coordination::ProjectWindowRegistration::attach(lease, hwnd)?,
        ),
        _ => None,
    };
    if let Some(candidate_id) = candidate_id {
        registry.candidates.remove(&candidate_id);
    }
    registry.sessions.insert(
        session_id,
        RegisteredProjectSession {
            _window_registration: window_registration,
            handle,
            write_lease,
            recovery,
            changes,
            editor_files: HashMap::new(),
        },
    );
    Ok(result)
}

fn lock_registry(
    state: &ProjectRegistry,
) -> Result<MutexGuard<'_, ProjectRegistryInner>, AppError> {
    state
        .inner
        .lock()
        .map_err(|_| AppError::project_io().with_diagnostic("project-registry-unavailable"))
}

#[allow(clippy::too_many_arguments)]
fn run_conversation_provider<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    handle: ProjectHandle,
    session_id: String,
    conversation_session_id: String,
    reservation_id: String,
    turn_id: String,
    attempt: app_core::conversation::RetryState,
    provider_id: ProviderId,
    model: String,
    cancellation: CancellationToken,
) {
    let state = app.state::<ProjectRegistry>();
    let emit_event = |event| {
        app.emit(
            "conversation-turn-event",
            ScopedConversationEvent {
                attempt_id: &attempt.attempt_id,
                attempt_number: attempt.attempt_number,
                conversation_session_id: &conversation_session_id,
                event,
            },
        )
    };
    let mut last_sequence = 0u32;
    let client = state
        .provider_client()
        .map_err(AppError::from_provider_error);
    let execution = client.and_then(|client| {
        let mut outcome = None;
        state
            .credential_vault
            .with_secret(provider_id, &mut |key| {
                let on_event = |event| match event {
                    AgentRuntimeEvent::Provider(event) => match event {
                        NormalizedEvent::TextDelta { delta, .. } => {
                            last_sequence = last_sequence.saturating_add(1);
                            let _ = emit_event(ConversationTurnEvent::TextDelta {
                                session_id: session_id.clone(),
                                turn_id: turn_id.clone(),
                                provider_id,
                                model: model.clone(),
                                sequence: last_sequence,
                                delta,
                            });
                        }
                        NormalizedEvent::Usage { usage, .. } => {
                            last_sequence = last_sequence.saturating_add(1);
                            let _ = emit_event(ConversationTurnEvent::Usage {
                                session_id: session_id.clone(),
                                turn_id: turn_id.clone(),
                                provider_id,
                                model: model.clone(),
                                sequence: last_sequence,
                                usage,
                            });
                        }
                        NormalizedEvent::Completed { .. } => {}
                    },
                    AgentRuntimeEvent::Task(task_activity) => {
                        last_sequence = last_sequence.saturating_add(1);
                        let _ = emit_event(ConversationTurnEvent::TaskActivity {
                            session_id: session_id.clone(),
                            turn_id: turn_id.clone(),
                            provider_id,
                            model: model.clone(),
                            sequence: last_sequence,
                            task_activity,
                        });
                    }
                };
                let coordinator = changes_for_handle(&state, &session_id, handle.clone()).ok();
                outcome = Some(if let Some(coordinator) = coordinator {
                    execute_memory_managed_turn_with_changes(
                        &client,
                        &handle,
                        &turn_id,
                        provider_id,
                        key,
                        &cancellation,
                        now_ms()?,
                        coordinator,
                        on_event,
                    )
                } else {
                    execute_memory_managed_turn(
                        &client,
                        &handle,
                        &turn_id,
                        provider_id,
                        key,
                        &cancellation,
                        now_ms()?,
                        on_event,
                    )
                });
                Ok(())
            })?;
        outcome.ok_or_else(AppError::runtime_unavailable)?
    });

    let completed_turn_id = turn_id.clone();
    let sequence = last_sequence.saturating_add(1);
    match execution {
        Ok(terminal) => match terminal.state {
            ConversationProviderTerminalState::Completed => {
                let _ = emit_event(ConversationTurnEvent::Completed {
                    session_id,
                    turn_id,
                    provider_id,
                    model,
                    sequence,
                    finish_reason: terminal.finish_reason.unwrap_or(FinishReason::Unknown),
                });
            }
            ConversationProviderTerminalState::Cancelled => {
                let _ = emit_event(ConversationTurnEvent::Cancelled {
                    session_id,
                    turn_id,
                    provider_id,
                    model,
                    sequence,
                });
            }
            ConversationProviderTerminalState::Incomplete => {
                let error = terminal
                    .provider_error
                    .map(AppError::from_provider_error)
                    .unwrap_or_else(|| AppError::from_provider_error(ProviderError::protocol()));
                let _ = emit_event(ConversationTurnEvent::Failed {
                    session_id,
                    turn_id,
                    provider_id,
                    model,
                    sequence,
                    error,
                });
            }
        },
        Err(error) => {
            let _ = finish_conversation_turn(
                &handle,
                &turn_id,
                "",
                ConversationTerminalKind::Failed,
                Some(&error.code),
                now_ms().unwrap_or(1),
            );
            let _ = emit_event(ConversationTurnEvent::Failed {
                session_id,
                turn_id,
                provider_id,
                model,
                sequence,
                error,
            });
        }
    }
    let _ = sessions::release_run(&handle, &reservation_id);
    state.finish_provider_run(&completed_turn_id);
}

fn now_ms() -> Result<u64, AppError> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::runtime_unavailable())?
        .as_millis();
    u64::try_from(milliseconds).map_err(|_| AppError::runtime_unavailable())
}
