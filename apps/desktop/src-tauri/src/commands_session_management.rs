// NR-602 closed structural actions and native-dialog-scoped visible export.
use sessions::{
    ConversationExportFormat, ConversationExportReceipt, PreparedConversationExport,
    SessionManagement, SessionManagementResult,
};
fn session_export_error(code: &'static str) -> AppError {
    AppError {
        code: code.into(),
        message_key: "errors.runtimeUnavailable".into(),
        diagnostic: None,
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionManageRequest {
    project_session_id: String,
    operation: SessionManagement,
}
#[tauri::command]
pub fn conversation_session_manage(
    state: tauri::State<'_, ProjectRegistry>,
    request: SessionManageRequest,
) -> Result<SessionManagementResult, AppError> {
    let lease = require_session_history_lease(&state, &request.project_session_id)?;
    let handle = session_handle(&state, &request.project_session_id)?;
    sessions::manage_session(&handle, &lease, &request.operation, now_ms()?)
}

struct PendingConversationExport {
    token: String,
    project_session_id: String,
    expires_at_ms: u64,
    in_flight: bool,
    prepared: Option<PreparedConversationExport>,
    target: Option<PathBuf>,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationExportPrepareRequest {
    project_session_id: String,
    conversation_session_id: String,
    format: ConversationExportFormat,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationExportPrepared {
    schema_version: u16,
    token: String,
    expires_at_ms: u64,
    bytes: usize,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationExportCommitRequest {
    project_session_id: String,
    token: String,
}
struct ExportPreparePermit<'a> {
    state: &'a ProjectRegistry,
    token: String,
    ready: bool,
}
impl Drop for ExportPreparePermit<'_> {
    fn drop(&mut self) {
        if !self.ready {
            if let Ok(mut pending) = self.state.conversation_export.lock() {
                if pending
                    .as_ref()
                    .is_some_and(|entry| entry.token == self.token)
                {
                    *pending = None;
                }
            }
        }
    }
}
#[tauri::command]
pub async fn conversation_export_prepare<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ProjectRegistry>,
    request: ConversationExportPrepareRequest,
) -> Result<Option<ConversationExportPrepared>, AppError> {
    let handle = session_handle(&state, &request.project_session_id)?;
    let now = now_ms()?;
    let token = Uuid::new_v4().to_string();
    {
        let mut slot = state
            .conversation_export
            .lock()
            .map_err(|_| AppError::runtime_unavailable())?;
        if slot
            .as_ref()
            .is_some_and(|entry| entry.in_flight || entry.expires_at_ms > now)
        {
            return Err(session_export_error("sessions.export_busy"));
        }
        *slot = Some(PendingConversationExport {
            token: token.clone(),
            project_session_id: request.project_session_id.clone(),
            expires_at_ms: now + 300_000,
            in_flight: true,
            prepared: None,
            target: None,
        });
    }
    let mut permit = ExportPreparePermit {
        state: &state,
        token: token.clone(),
        ready: false,
    };
    let format = request.format;
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        sessions::prepare_export(&handle, &request.conversation_session_id, format)
    })
    .await
    .map_err(|_| AppError::runtime_unavailable())??;
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("可见聊天记录", &[format.extension()])
        .set_file_name(format!("聊天记录.{}", format.extension()))
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let target = selected
        .into_path()
        .map_err(|_| AppError::project_invalid_root())?;
    session_handle(&state, &request.project_session_id)?;
    let mut slot = state
        .conversation_export
        .lock()
        .map_err(|_| AppError::runtime_unavailable())?;
    let entry = slot
        .as_mut()
        .filter(|entry| entry.token == token && entry.expires_at_ms > now_ms().unwrap_or(u64::MAX))
        .ok_or_else(|| session_export_error("sessions.export_expired"))?;
    let response = ConversationExportPrepared {
        schema_version: 1,
        token,
        expires_at_ms: entry.expires_at_ms,
        bytes: prepared.bytes_len(),
    };
    entry.target = Some(target);
    entry.prepared = Some(prepared);
    entry.in_flight = false;
    permit.ready = true;
    Ok(Some(response))
}

fn take_conversation_export(
    state: &ProjectRegistry,
    request: &ConversationExportCommitRequest,
    now: u64,
) -> Result<PendingConversationExport, AppError> {
    let mut slot = state
        .conversation_export
        .lock()
        .map_err(|_| AppError::runtime_unavailable())?;
    let entry = slot
        .as_ref()
        .filter(|entry| {
            entry.token == request.token && entry.project_session_id == request.project_session_id
        })
        .ok_or_else(|| session_export_error("sessions.export_expired"))?;
    if entry.in_flight {
        return Err(session_export_error("sessions.export_busy"));
    }
    if entry.expires_at_ms <= now {
        *slot = None;
        return Err(session_export_error("sessions.export_expired"));
    }
    if entry.prepared.is_none() || entry.target.is_none() {
        return Err(session_export_error("sessions.export_busy"));
    }
    let pending = slot.take().ok_or_else(AppError::runtime_unavailable)?;
    *slot = Some(PendingConversationExport {
        token: pending.token.clone(),
        project_session_id: pending.project_session_id.clone(),
        expires_at_ms: pending.expires_at_ms,
        in_flight: true,
        prepared: None,
        target: None,
    });
    Ok(pending)
}
#[tauri::command]
pub async fn conversation_export_commit(
    state: tauri::State<'_, ProjectRegistry>,
    request: ConversationExportCommitRequest,
) -> Result<ConversationExportReceipt, AppError> {
    session_handle(&state, &request.project_session_id)?;
    let pending = take_conversation_export(&state, &request, now_ms()?)?;
    let _permit = ExportPreparePermit {
        state: &state,
        token: request.token,
        ready: false,
    };
    let roots = lock_registry(&state)?
        .sessions
        .values()
        .map(|entry| entry.handle.clone())
        .collect::<Vec<_>>();
    tauri::async_runtime::spawn_blocking(move || {
        sessions::commit_export(
            pending.prepared.ok_or_else(AppError::runtime_unavailable)?,
            &pending.target.ok_or_else(AppError::runtime_unavailable)?,
            &roots,
        )
    })
    .await
    .map_err(|_| AppError::runtime_unavailable())?
}
