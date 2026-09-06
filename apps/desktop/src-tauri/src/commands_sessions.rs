// NR-601: closed catalog actions. No Provider call is made by these commands.
use app_core::conversation_sessions::{self as sessions, SessionPage};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionListRequest {
    project_session_id: String,
    archived: bool,
    #[serde(deserialize_with = "session_required_nullable")]
    cursor: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionCreateRequest {
    project_session_id: String,
    request_id: String,
    name: String,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRenameRequest {
    project_session_id: String,
    conversation_session_id: String,
    name: String,
    expected_revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionActivateRequest {
    project_session_id: String,
    conversation_session_id: String,
    expected_revision: u64,
}
fn session_required_nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    <Option<String> as serde::Deserialize>::deserialize(deserializer)
}
fn selected_session_handle(
    state: &ProjectRegistry,
    project_session_id: &str,
    selected: Option<&str>,
) -> Result<ProjectHandle, AppError> {
    let lease = session_history_lease(state, project_session_id)?;
    if lease.is_none() {
        return sessions::bind_existing_session(
            &session_handle(state, project_session_id)?,
            selected,
        );
    }
    sessions::bind_session(
        &session_handle(state, project_session_id)?,
        selected,
        now_ms()?,
    )
}
// Same-process aliases may share the physical lease; another app cannot.
// This permission concerns non-Canon History only, never Canon write authority.
fn session_history_lease(
    state: &ProjectRegistry,
    project_session_id: &str,
) -> Result<Option<Arc<ProjectWriteLease>>, AppError> {
    let registry = lock_registry(state)?;
    let key = registry
        .sessions
        .get(project_session_id)
        .ok_or_else(AppError::project_session_not_found)?
        .handle
        .coordination_key();
    Ok(registry.sessions.values().find_map(|entry| {
        (entry.handle.coordination_key() == key)
            .then(|| entry.write_lease.clone())
            .flatten()
    }))
}
fn require_session_history_lease(
    state: &ProjectRegistry,
    project_session_id: &str,
) -> Result<Arc<ProjectWriteLease>, AppError> {
    session_history_lease(state, project_session_id)?.ok_or_else(AppError::project_writer_required)
}
fn reconcile_session_run(
    state: &ProjectRegistry,
    project_session_id: &str,
) -> Result<(), AppError> {
    let Some(_history_lease) = session_history_lease(state, project_session_id)? else {
        return Ok(());
    };
    let base = session_handle(state, project_session_id)?;
    let project_key = base.coordination_key();
    // Atomically exclude a new Provider reservation, but do not hold the
    // provider mutex across disk recovery (closing must never wait for I/O).
    let active = state
        .provider_run
        .lock()
        .map_err(|_| AppError::runtime_unavailable())?;
    if active
        .as_ref()
        .is_some_and(|run| run.project_key == project_key)
    {
        return Ok(());
    }
    let mut reconciling = state
        .reconciling_sessions
        .lock()
        .map_err(|_| AppError::runtime_unavailable())?;
    if !reconciling.insert(project_key.clone()) {
        return Err(AppError::provider_run_active());
    }
    drop(reconciling);
    drop(active);
    let _permit = SessionReconcilePermit {
        state,
        id: &project_key,
    };
    if let Some(run) = sessions::list_sessions(&base, false, None, now_ms()?)?.active_run {
        let handle = sessions::bind_session(&base, Some(&run.session_id), now_ms()?)?;
        let conversation = load_conversation(&handle, now_ms()?)?;
        runtime_view_for(
            &handle,
            conversation,
            now_ms()?,
            true,
            app_core::protocol_profile::ToolProtocolProfile::Nr5,
        )?;
        sessions::release_run(&base, &run.reservation_id)?;
    }
    Ok(())
}
struct SessionReconcilePermit<'a> {
    state: &'a ProjectRegistry,
    id: &'a str,
}
impl Drop for SessionReconcilePermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut entries) = self.state.reconciling_sessions.lock() {
            entries.remove(self.id);
        }
    }
}
#[tauri::command]
pub fn conversation_session_list(
    state: tauri::State<'_, ProjectRegistry>,
    request: SessionListRequest,
) -> Result<SessionPage, AppError> {
    let lease = session_history_lease(&state, &request.project_session_id)?;
    if lease.is_none() {
        return sessions::list_existing_sessions(
            &session_handle(&state, &request.project_session_id)?,
            request.archived,
            request.cursor.as_deref(),
        );
    }
    reconcile_session_run(&state, &request.project_session_id)?;
    sessions::list_sessions(
        &session_handle(&state, &request.project_session_id)?,
        request.archived,
        request.cursor.as_deref(),
        now_ms()?,
    )
}
#[tauri::command]
pub fn conversation_session_create(
    state: tauri::State<'_, ProjectRegistry>,
    request: SessionCreateRequest,
) -> Result<SessionPage, AppError> {
    let _lease = require_session_history_lease(&state, &request.project_session_id)?;
    sessions::create_session(
        &session_handle(&state, &request.project_session_id)?,
        &request.request_id,
        &request.name,
        request.expected_revision,
        now_ms()?,
    )
}
#[tauri::command]
pub fn conversation_session_rename(
    state: tauri::State<'_, ProjectRegistry>,
    request: SessionRenameRequest,
) -> Result<SessionPage, AppError> {
    let _lease = require_session_history_lease(&state, &request.project_session_id)?;
    sessions::rename_session(
        &session_handle(&state, &request.project_session_id)?,
        &request.conversation_session_id,
        &request.name,
        request.expected_revision,
        now_ms()?,
    )
}
#[tauri::command]
pub fn conversation_session_activate(
    state: tauri::State<'_, ProjectRegistry>,
    request: SessionActivateRequest,
) -> Result<SessionPage, AppError> {
    let _lease = require_session_history_lease(&state, &request.project_session_id)?;
    sessions::activate_session(
        &session_handle(&state, &request.project_session_id)?,
        &request.conversation_session_id,
        request.expected_revision,
        now_ms()?,
    )
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScopedConversationEvent<'a> {
    attempt_id: &'a str,
    attempt_number: u8,
    conversation_session_id: &'a str,
    #[serde(flatten)]
    event: ConversationTurnEvent,
}
