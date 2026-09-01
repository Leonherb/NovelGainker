use app_core::app_state::{AppStateRepository, AppStateSnapshot, UiPreferences};
use app_core::conversation::{
    execute_provider_turn, finish_conversation_turn, load_conversation, read_conversation,
    recent_visible_messages, start_conversation_turn, ConversationProviderTerminalState,
    ConversationSession, ConversationTerminalKind,
};
use app_core::coordination::{try_acquire_project_write_lease, ProjectWriteLease};
use app_core::credential_vault::{CredentialVault, ProviderId, WindowsCredentialVault};
use app_core::project::{
    create_prepared_project, inspect_project_root, open_project, prepare_project_create,
    PreparedProjectCreate, ProjectCreateInputV1, ProjectCreatePreview, ProjectHandle,
    ProjectInspection, ProjectScanMode, ProjectScanPreview,
};
use app_core::provider_core::{
    preset, CancellationToken, FinishReason, MessageRole, NormalizedEvent, NormalizedMessage,
    NormalizedRequest, ProviderError, ProviderHttpClient, Usage, MAX_MESSAGE_BYTES,
    MAX_MESSAGE_COUNT, MAX_OUTPUT_TOKENS, MAX_REQUEST_BYTES,
};
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
use app_core::{health_status, AppError, HealthStatus};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

pub struct ProjectRegistry {
    inner: Mutex<ProjectRegistryInner>,
    recovery_store: RecoveryStore,
    app_state: AppStateRepository,
    credential_vault: Arc<dyn CredentialVault>,
    provider_run: Mutex<Option<ActiveProviderRun>>,
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
    session_id: String,
    turn_id: Option<String>,
    provider_id: ProviderId,
    cancellation: CancellationToken,
}

impl ProjectRegistry {
    pub fn new(app_state_root: PathBuf) -> Result<Self, AppError> {
        Ok(Self {
            inner: Mutex::new(ProjectRegistryInner::default()),
            recovery_store: RecoveryStore::new(app_state_root.clone())?,
            app_state: AppStateRepository::open(&app_state_root),
            credential_vault: Arc::new(WindowsCredentialVault::production()),
            provider_run: Mutex::new(None),
        })
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
        session_id: &str,
        provider_id: ProviderId,
    ) -> Result<CancellationToken, AppError> {
        let mut active = self
            .provider_run
            .lock()
            .map_err(|_| AppError::runtime_unavailable())?;
        if active.is_some() {
            return Err(AppError::provider_run_active());
        }
        let cancellation = CancellationToken::default();
        *active = Some(ActiveProviderRun {
            session_id: session_id.to_owned(),
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

    fn has_active_turn_for_session(&self, session_id: &str) -> bool {
        self.provider_run
            .lock()
            .ok()
            .and_then(|active| {
                active
                    .as_ref()
                    .map(|run| run.session_id == session_id && run.turn_id.is_some())
            })
            .unwrap_or(false)
    }

    fn cancel_provider_turn(&self, session_id: &str, turn_id: &str) -> bool {
        self.provider_run
            .lock()
            .ok()
            .and_then(|active| {
                active.as_ref().map(|run| {
                    let matches =
                        run.session_id == session_id && run.turn_id.as_deref() == Some(turn_id);
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
) -> Result<ConversationSession, AppError> {
    let handle = session_handle(&state, &session_id)?;
    if state.has_active_turn_for_session(&session_id) {
        read_conversation(&handle)
    } else {
        load_conversation(&handle, now_ms()?)
    }
}

#[tauri::command]
pub fn conversation_turn_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    message: String,
) -> Result<ConversationTurnStartResult, AppError> {
    let handle = session_handle(&state, &session_id)?;
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
    let cancellation = state.reserve_provider_run(&session_id, provider_id)?;
    let started =
        match start_conversation_turn(&handle, provider_id, &model, &message, started_at_ms) {
            Ok(started) => started,
            Err(error) => {
                state.release_provider_reservation();
                return Err(error);
            }
        };
    if let Err(error) = state.bind_provider_turn(&started.turn_id) {
        let _ = finish_conversation_turn(
            &handle,
            &started.turn_id,
            "",
            ConversationTerminalKind::Failed,
            Some(&error.code),
            now_ms().unwrap_or(started_at_ms),
        );
        state.release_provider_reservation();
        return Err(error);
    }

    let request = match prepare_conversation_request(&handle) {
        Ok(request) => request,
        Err(app_error) => {
            let _ = finish_conversation_turn(
                &handle,
                &started.turn_id,
                "",
                ConversationTerminalKind::Failed,
                Some(&app_error.code),
                now_ms().unwrap_or(started_at_ms),
            );
            state.finish_provider_run(&started.turn_id);
            return Err(app_error);
        }
    };

    let result = ConversationTurnStartResult {
        session_id: session_id.clone(),
        conversation_session_id: started.session_id,
        turn_id: started.turn_id.clone(),
        provider_id,
        model: model.clone(),
    };
    let worker_session_id = session_id;
    let worker_turn_id = started.turn_id;
    tauri::async_runtime::spawn_blocking(move || {
        run_conversation_provider(
            app,
            handle,
            worker_session_id,
            worker_turn_id,
            provider_id,
            model,
            request,
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
) -> ConversationTurnCancelResult {
    ConversationTurnCancelResult {
        cancel_requested: state.cancel_provider_turn(&session_id, &turn_id),
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
pub async fn provider_connection_test(
    app: tauri::AppHandle,
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
pub async fn project_scan(
    app: tauri::AppHandle,
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
    let requires_write =
        inspection.preview().requires_metadata_confirmation || inspection.requires_clean_slate();
    if write_lease.is_none() && requires_write {
        return Err(AppError::project_writer_required());
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
    if write_lease.is_none() && inspection.requires_clean_slate() {
        return Err(AppError::project_writer_required());
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
    list_project_files(&session_handle(&state, &session_id)?)
}

#[tauri::command]
pub async fn project_read_file(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    relative_path: String,
) -> Result<ProjectFileDocument, AppError> {
    read_project_file(&session_handle(&state, &session_id)?, &relative_path)
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
    let handle = writer_handle(&state, &session_id)?;
    save_project_file_recoverable(
        &handle,
        &relative_path,
        &content,
        &source_hash,
        &state.recovery_store,
    )
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
    search_project_sources(
        &session_handle(&state, &session_id)?,
        &query,
        case_sensitive,
    )
}

#[tauri::command]
pub fn project_close(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<ProjectCloseResult, AppError> {
    state.cancel_active_provider_run();
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
    let recovery = if write_lease.is_some() {
        state.recovery_store.recover_project(&handle)
    } else {
        RecoveryReport::not_checked()
    };
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
fn run_conversation_provider(
    app: tauri::AppHandle,
    handle: ProjectHandle,
    session_id: String,
    turn_id: String,
    provider_id: ProviderId,
    model: String,
    request: NormalizedRequest,
    cancellation: CancellationToken,
) {
    let state = app.state::<ProjectRegistry>();
    let mut last_sequence = 0u32;
    let client = ProviderHttpClient::production().map_err(AppError::from_provider_error);
    let execution = client.and_then(|client| {
        let mut outcome = None;
        state
            .credential_vault
            .with_secret(provider_id, &mut |key| {
                outcome = Some(execute_provider_turn(
                    &client,
                    &handle,
                    &turn_id,
                    provider_id,
                    key,
                    &request,
                    &cancellation,
                    now_ms()?,
                    |event| match event {
                        NormalizedEvent::TextDelta { sequence, delta } => {
                            last_sequence = last_sequence.max(sequence);
                            let _ = app.emit(
                                "conversation-turn-event",
                                ConversationTurnEvent::TextDelta {
                                    session_id: session_id.clone(),
                                    turn_id: turn_id.clone(),
                                    provider_id,
                                    model: model.clone(),
                                    sequence,
                                    delta,
                                },
                            );
                        }
                        NormalizedEvent::Usage { sequence, usage } => {
                            last_sequence = last_sequence.max(sequence);
                            let _ = app.emit(
                                "conversation-turn-event",
                                ConversationTurnEvent::Usage {
                                    session_id: session_id.clone(),
                                    turn_id: turn_id.clone(),
                                    provider_id,
                                    model: model.clone(),
                                    sequence,
                                    usage,
                                },
                            );
                        }
                        NormalizedEvent::Completed { sequence, .. } => {
                            last_sequence = last_sequence.max(sequence);
                        }
                    },
                ));
                Ok(())
            })?;
        outcome.ok_or_else(AppError::runtime_unavailable)?
    });

    state.finish_provider_run(&turn_id);
    let sequence = last_sequence.saturating_add(1);
    match execution {
        Ok(terminal) => match terminal.state {
            ConversationProviderTerminalState::Completed => {
                let _ = app.emit(
                    "conversation-turn-event",
                    ConversationTurnEvent::Completed {
                        session_id,
                        turn_id,
                        provider_id,
                        model,
                        sequence,
                        finish_reason: terminal.finish_reason.unwrap_or(FinishReason::Unknown),
                    },
                );
            }
            ConversationProviderTerminalState::Cancelled => {
                let _ = app.emit(
                    "conversation-turn-event",
                    ConversationTurnEvent::Cancelled {
                        session_id,
                        turn_id,
                        provider_id,
                        model,
                        sequence,
                    },
                );
            }
            ConversationProviderTerminalState::Incomplete => {
                let error = terminal
                    .provider_error
                    .map(AppError::from_provider_error)
                    .unwrap_or_else(|| AppError::from_provider_error(ProviderError::protocol()));
                let _ = app.emit(
                    "conversation-turn-event",
                    ConversationTurnEvent::Failed {
                        session_id,
                        turn_id,
                        provider_id,
                        model,
                        sequence,
                        error,
                    },
                );
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
            let _ = app.emit(
                "conversation-turn-event",
                ConversationTurnEvent::Failed {
                    session_id,
                    turn_id,
                    provider_id,
                    model,
                    sequence,
                    error,
                },
            );
        }
    }
}

fn prepare_conversation_request(handle: &ProjectHandle) -> Result<NormalizedRequest, AppError> {
    let session = read_conversation(handle)?;
    let messages = recent_visible_messages(
        &session,
        MAX_MESSAGE_COUNT.saturating_sub(1),
        MAX_REQUEST_BYTES.saturating_sub(1_024),
    )
    .into_iter()
    .filter(|message| message.content.len() <= MAX_MESSAGE_BYTES)
    .map(|message| NormalizedMessage {
        role: match message.role {
            app_core::conversation::ConversationRole::User => MessageRole::User,
            app_core::conversation::ConversationRole::Assistant => MessageRole::Assistant,
        },
        content: message.content,
    })
    .collect();
    NormalizedRequest::conversation(messages, MAX_OUTPUT_TOKENS)
        .map_err(AppError::from_provider_error)
}

fn now_ms() -> Result<u64, AppError> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::runtime_unavailable())?
        .as_millis();
    u64::try_from(milliseconds).map_err(|_| AppError::runtime_unavailable())
}
