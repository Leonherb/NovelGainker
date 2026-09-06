use crate::conversation::ConversationSession;
use crate::project::ProjectHandle;
use crate::tool_runtime::{ProviderToolCall, ToolDefinition, ToolResult, ToolResultStatus};
use crate::workspace::{atomic_replace, is_link_or_reparse};
use crate::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use uuid::Uuid;

const STORE_DIRECTORY: &str = ".novelgainker/tasks";
const STORE_FILE: &str = "session.json";
const SCHEMA_VERSION: u16 = 1;
const MAX_STORE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TASKS: usize = 10_000;

// A UI snapshot must not hold a Windows metadata/read handle while a worker
// replaces this store. Serialize the complete local read/modify/write operation;
// never hold this guard across Provider HTTP or tool execution.
fn store_coordination(handle: &ProjectHandle) -> Result<Arc<Mutex<()>>, AppError> {
    static STORES: OnceLock<Mutex<BTreeMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();
    let mut stores = STORES
        .get_or_init(Mutex::default)
        .lock()
        .map_err(|_| task_error("task.store_unavailable"))?;
    stores.retain(|_, lock| lock.strong_count() != 0);
    let key = handle.root().to_path_buf();
    if let Some(lock) = stores.get(&key).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    stores.insert(key, Arc::downgrade(&lock));
    Ok(lock)
}
const RECOVERY_INTERRUPTED_ERROR: &str = "runtime.interrupted";
const FAILED_TASK_ERRORS: &[&str] = &[
    "publication.metadata_required",
    "publication.no_final_chapters",
    "publication.final_confirmation_required",
    "publication.invalid_chapter",
    "publication.unsupported_markdown",
    "publication.invalid_asset",
    "publication.unsafe_path",
    "publication.limit_exceeded",
    "publication.source_changed",
    "publication.local_validation_failed",
    "publication.store_unavailable",
    "publication.source_required",
    "agent.tool_arguments_invalid",
    "agent.tool_arguments_too_large",
    "agent.tool_definition_invalid",
    "agent.tool_executor_failed",
    "agent.tool_result_invalid",
    "agent.tool_schema_invalid",
    "agent.tool_timeout",
    "agent.tool_unknown",
    "canon.change_invalid",
    "canon.change_budget_exceeded",
    "canon.change_source_invalid",
    "canon.chapter_source_unavailable",
    "canon.change_method_not_read",
    "canon.change_incomplete_read",
    "canon.change_target_invalid",
    "canon.change_source_changed",
    "canon.change_candidate_invalid",
    "canon.change_duplicate_target",
    "canon.change_revision_conflict",
    "canon.change_recovery_required",
    "canon.change_store_full",
    "canon.change_io",
    "canon.change_writer_busy",
];
const CANCELLED_TASK_ERRORS: &[&str] = &["agent.tool_cancelled"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskRecord {
    pub task_id: String,
    pub project_id: String,
    pub conversation_session_id: String,
    pub turn_id: String,
    pub call_id_hash: String,
    pub tool_name: String,
    pub tool_version: u32,
    pub status: TaskStatus,
    pub task_revision: u64,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub display_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    pub arguments_sha256: String,
    pub arguments_bytes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_bytes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskSession {
    schema_version: u16,
    project_id: String,
    tasks: Vec<TaskRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskActivity {
    pub task_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub status: TaskStatus,
    pub display_label: String,
    pub task_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRuntimeView {
    pub schema_version: u16,
    pub conversation: ConversationSession,
    pub task_activities: Vec<TaskActivity>,
}

pub fn runtime_view(
    handle: &ProjectHandle,
    conversation: ConversationSession,
    now_ms: u64,
    recover_interrupted: bool,
) -> Result<ConversationRuntimeView, AppError> {
    runtime_view_for(
        handle,
        conversation,
        now_ms,
        recover_interrupted,
        crate::protocol_profile::ToolProtocolProfile::Nr4,
    )
}

/// The current Host determines store capacity; an old task keeps its original
/// result version even after the project's chapter-capable Host is installed.
pub fn runtime_view_for(
    handle: &ProjectHandle,
    conversation: ConversationSession,
    now_ms: u64,
    recover_interrupted: bool,
    profile: crate::protocol_profile::ToolProtocolProfile,
) -> Result<ConversationRuntimeView, AppError> {
    let coordination = store_coordination(handle)?;
    let _guard = coordination
        .lock()
        .map_err(|_| task_error("task.store_unavailable"))?;
    let mut session = read_or_new(handle)?;
    if recover_interrupted {
        let mut changed = false;
        for task in &mut session.tasks {
            if matches!(task.status, TaskStatus::Queued | TaskStatus::Running) {
                if task.tool_name == crate::canon_change::PROPOSE_CHANGE_TOOL_NAME {
                    recover_canon_task(handle, task, now_ms, profile)?;
                    changed = true;
                    continue;
                }
                if task.tool_name == "novelist.publish" {
                    recover_publication_task(handle, task, now_ms, profile)?;
                    changed = true;
                    continue;
                }
                task.status = TaskStatus::Cancelled;
                task.task_revision = task.task_revision.saturating_add(1);
                task.updated_at_ms = now_ms.max(task.started_at_ms);
                task.error_code = Some(RECOVERY_INTERRUPTED_ERROR.to_owned());
                changed = true;
            }
        }
        if changed {
            validate_session(&session, handle.project_id())?;
            write_session(handle, &session)?;
        }
    }
    Ok(ConversationRuntimeView {
        schema_version: SCHEMA_VERSION,
        task_activities: session
            .tasks
            .iter()
            .filter(|task| task.conversation_session_id == conversation.session_id)
            .map(TaskActivity::from)
            .collect(),
        conversation,
    })
}

fn recover_canon_task(
    handle: &ProjectHandle,
    task: &mut TaskRecord,
    now_ms: u64,
    profile: crate::protocol_profile::ToolProtocolProfile,
) -> Result<(), AppError> {
    let payload = match crate::canon_change_store::task_recovery_result_for(
        handle,
        &task.conversation_session_id,
        &task.turn_id,
        &task.call_id_hash,
        task.tool_version,
        profile,
    ) {
        Ok(result) => {
            result
                .validate()
                .map_err(|_| task_error("task.result_invalid"))?;
            task.status = result_status_to_task_status(result.status);
            task.error_code = result.error_code.clone();
            result.provider_payload()
        }
        Err(_) => {
            task.status = TaskStatus::Failed;
            task.error_code = Some("canon.change_recovery_required".to_owned());
            task.display_label = "写入结果待核对".to_owned();
            serde_json::json!({"status":"failed","errorCode":"canon.change_recovery_required"})
        }
    };
    let bytes = serde_json::to_vec(&payload).map_err(|_| task_error("task.result_invalid"))?;
    task.result_sha256 = Some(sha256(&bytes));
    task.result_bytes =
        Some(u32::try_from(bytes.len()).map_err(|_| task_error("task.result_invalid"))?);
    task.task_revision = task.task_revision.saturating_add(1);
    task.updated_at_ms = now_ms.max(task.started_at_ms);
    Ok(())
}

fn recover_publication_task(
    handle: &ProjectHandle,
    task: &mut TaskRecord,
    now_ms: u64,
    profile: crate::protocol_profile::ToolProtocolProfile,
) -> Result<(), AppError> {
    let result =
        if task.tool_version == 2 && profile == crate::protocol_profile::ToolProtocolProfile::Nr5 {
            crate::publication::task_recovery_result(
                handle,
                &task.conversation_session_id,
                &task.turn_id,
                &task.call_id_hash,
            )
        } else {
            Err(crate::publication::PublicationIssue::StoreUnavailable)
        };
    let payload = match result {
        Ok(result) => {
            result
                .validate()
                .map_err(|_| task_error("task.result_invalid"))?;
            task.status = result_status_to_task_status(result.status);
            task.error_code = result.error_code.clone();
            result.provider_payload()
        }
        Err(_) => {
            task.status = TaskStatus::Failed;
            task.error_code = Some("publication.store_unavailable".to_owned());
            task.display_label = "出版结果待核对".to_owned();
            serde_json::json!({"status":"failed","errorCode":"publication.store_unavailable"})
        }
    };
    let bytes = serde_json::to_vec(&payload).map_err(|_| task_error("task.result_invalid"))?;
    task.result_sha256 = Some(sha256(&bytes));
    task.result_bytes =
        Some(u32::try_from(bytes.len()).map_err(|_| task_error("task.result_invalid"))?);
    task.task_revision = task.task_revision.saturating_add(1);
    task.updated_at_ms = now_ms.max(task.started_at_ms);
    Ok(())
}

impl From<&TaskRecord> for TaskActivity {
    fn from(task: &TaskRecord) -> Self {
        Self {
            task_id: task.task_id.clone(),
            session_id: task.conversation_session_id.clone(),
            turn_id: task.turn_id.clone(),
            status: task.status,
            display_label: task.display_label.clone(),
            task_revision: task.task_revision,
        }
    }
}

pub fn create_queued_task(
    handle: &ProjectHandle,
    conversation_session_id: &str,
    turn_id: &str,
    call: &ProviderToolCall,
    definition: &ToolDefinition,
    now_ms: u64,
) -> Result<TaskRecord, AppError> {
    call.validate_envelope_for(definition.protocol_profile())
        .map_err(|_| task_error("task.invalid"))?;
    if handle
        .conversation_session_id()
        .is_some_and(|id| id != conversation_session_id)
    {
        return Err(task_error("task.invalid"));
    }
    let coordination = store_coordination(handle)?;
    let _guard = coordination
        .lock()
        .map_err(|_| task_error("task.store_unavailable"))?;
    let mut session = read_or_new(handle)?;
    let call_id_hash = sha256(call.call_id.as_bytes());
    if session.tasks.iter().any(|task| {
        task.conversation_session_id == conversation_session_id && task.call_id_hash == call_id_hash
    }) {
        return Err(task_error("agent.tool_duplicate_call"));
    }
    let argument_bytes = call.arguments_json.as_bytes();
    let record = TaskRecord {
        task_id: Uuid::new_v4().to_string(),
        project_id: handle.project_id().to_owned(),
        conversation_session_id: conversation_session_id.to_owned(),
        turn_id: turn_id.to_owned(),
        call_id_hash,
        tool_name: definition.name.clone(),
        tool_version: definition.version,
        status: TaskStatus::Queued,
        task_revision: 1,
        started_at_ms: now_ms,
        updated_at_ms: now_ms,
        display_label: definition.display_label.clone(),
        error_code: None,
        arguments_sha256: sha256(argument_bytes),
        arguments_bytes: u32::try_from(argument_bytes.len())
            .map_err(|_| task_error("task.invalid"))?,
        result_sha256: None,
        result_bytes: None,
    };
    session.tasks.push(record.clone());
    validate_session(&session, handle.project_id())?;
    write_session(handle, &session)?;
    Ok(record)
}

#[allow(clippy::too_many_arguments)]
pub fn transition_task(
    handle: &ProjectHandle,
    task_id: &str,
    expected_revision: u64,
    next_status: TaskStatus,
    result: Option<&ToolResult>,
    error_code: Option<&str>,
    now_ms: u64,
) -> Result<TaskRecord, AppError> {
    let coordination = store_coordination(handle)?;
    let _guard = coordination
        .lock()
        .map_err(|_| task_error("task.store_unavailable"))?;
    let mut session = read_or_new(handle)?;
    let task = session
        .tasks
        .iter_mut()
        .find(|task| task.task_id == task_id)
        .ok_or_else(|| task_error("task.not_found"))?;
    if task.task_revision != expected_revision {
        return Err(task_error("task.revision_conflict"));
    }
    if !valid_transition(task.status, next_status) {
        return Err(task_error("task.transition_invalid"));
    }
    let result_payload = validate_live_transition_payload(task, next_status, result, error_code)?;
    if let Some(payload) = result_payload {
        task.result_sha256 = Some(sha256(&payload));
        task.result_bytes =
            Some(u32::try_from(payload.len()).map_err(|_| task_error("task.result_invalid"))?);
    }
    task.status = next_status;
    task.task_revision = task.task_revision.saturating_add(1);
    task.updated_at_ms = now_ms.max(task.started_at_ms);
    task.error_code = error_code.map(str::to_owned);
    let record = task.clone();
    validate_session(&session, handle.project_id())?;
    write_session(handle, &session)?;
    Ok(record)
}

pub fn find_turn_tasks(handle: &ProjectHandle, turn_id: &str) -> Result<Vec<TaskRecord>, AppError> {
    let coordination = store_coordination(handle)?;
    let _guard = coordination
        .lock()
        .map_err(|_| task_error("task.store_unavailable"))?;
    Ok(read_or_new(handle)?
        .tasks
        .into_iter()
        .filter(|task| task.turn_id == turn_id)
        .collect())
}

fn valid_transition(current: TaskStatus, next: TaskStatus) -> bool {
    matches!(
        (current, next),
        (TaskStatus::Queued, TaskStatus::Running)
            | (TaskStatus::Queued, TaskStatus::Failed)
            | (TaskStatus::Queued, TaskStatus::Cancelled)
            | (TaskStatus::Running, TaskStatus::Succeeded)
            | (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::Cancelled)
    )
}

fn validate_live_transition_payload(
    task: &TaskRecord,
    next_status: TaskStatus,
    result: Option<&ToolResult>,
    error_code: Option<&str>,
) -> Result<Option<Vec<u8>>, AppError> {
    if next_status == TaskStatus::Running {
        return if result.is_none() && error_code.is_none() {
            Ok(None)
        } else {
            Err(task_error("task.result_invalid"))
        };
    }

    let result = result.ok_or_else(|| task_error("task.result_invalid"))?;
    result
        .validate()
        .map_err(|_| task_error("task.result_invalid"))?;
    if sha256(result.call_id.as_bytes()) != task.call_id_hash
        || result.name != task.tool_name
        || result.version != task.tool_version
        || result_status_to_task_status(result.status) != next_status
        || result.error_code.as_deref() != error_code
        || !valid_live_terminal_error(next_status, error_code)
    {
        return Err(task_error("task.result_invalid"));
    }
    serde_json::to_vec(&result.provider_payload())
        .map(Some)
        .map_err(|_| task_error("task.result_invalid"))
}

fn result_status_to_task_status(status: ToolResultStatus) -> TaskStatus {
    match status {
        ToolResultStatus::Succeeded => TaskStatus::Succeeded,
        ToolResultStatus::Failed => TaskStatus::Failed,
        ToolResultStatus::Cancelled => TaskStatus::Cancelled,
    }
}

fn valid_live_terminal_error(status: TaskStatus, error_code: Option<&str>) -> bool {
    match status {
        TaskStatus::Succeeded => error_code.is_none(),
        TaskStatus::Failed => error_code.is_some_and(is_failed_task_error),
        TaskStatus::Cancelled => error_code.is_some_and(is_cancelled_task_error),
        TaskStatus::Queued | TaskStatus::Running => false,
    }
}

fn read_or_new(handle: &ProjectHandle) -> Result<TaskSession, AppError> {
    let path = if handle.session_binding().is_some() {
        crate::conversation_sessions::store_path(
            handle,
            crate::conversation_sessions::StoreKind::Task,
            false,
        )?
    } else {
        store_path(handle)?
    };
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TaskSession {
                schema_version: SCHEMA_VERSION,
                project_id: handle.project_id().to_owned(),
                tasks: Vec::new(),
            });
        }
        Err(_) => return Err(task_error("task.store_unavailable")),
    };
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > MAX_STORE_BYTES {
        return Err(task_error("task.store_invalid"));
    }
    let bytes = fs::read(path).map_err(|_| task_error("task.store_unavailable"))?;
    let session: TaskSession =
        serde_json::from_slice(&bytes).map_err(|_| task_error("task.store_invalid"))?;
    validate_session(&session, handle.project_id())?;
    if handle.conversation_session_id().is_some_and(|id| {
        session
            .tasks
            .iter()
            .any(|task| task.conversation_session_id != id)
    }) {
        return Err(task_error("task.store_invalid"));
    }
    Ok(session)
}

fn validate_session(session: &TaskSession, project_id: &str) -> Result<(), AppError> {
    if session.schema_version != SCHEMA_VERSION
        || session.project_id != project_id
        || session.tasks.len() > MAX_TASKS
    {
        return Err(task_error("task.store_invalid"));
    }
    let mut task_ids = HashSet::new();
    let mut calls = HashSet::new();
    for task in &session.tasks {
        if !task_ids.insert(task.task_id.as_str())
            || !calls.insert((
                task.conversation_session_id.as_str(),
                task.call_id_hash.as_str(),
            ))
            || Uuid::parse_str(&task.task_id).is_err()
            || Uuid::parse_str(&task.project_id).is_err()
            || Uuid::parse_str(&task.conversation_session_id).is_err()
            || Uuid::parse_str(&task.turn_id).is_err()
            || task.project_id != project_id
            || !is_sha256(&task.call_id_hash)
            || !is_sha256(&task.arguments_sha256)
            || task
                .result_sha256
                .as_ref()
                .is_some_and(|hash| !is_sha256(hash))
            || task.tool_name.is_empty()
            || task.tool_name.len() > 64
            || task.tool_version == 0
            || task.task_revision == 0
            || task.started_at_ms == 0
            || task.updated_at_ms < task.started_at_ms
            || task.display_label.trim().is_empty()
            || task.display_label.len() > 160
            || task.arguments_bytes as usize
                > crate::protocol_profile::ToolProtocolProfile::for_definition(
                    &task.tool_name,
                    task.tool_version,
                )
                .call_envelope_bytes()
            || !valid_stored_task_state(task)
        {
            return Err(task_error("task.store_invalid"));
        }
    }
    Ok(())
}

fn store_path(handle: &ProjectHandle) -> Result<PathBuf, AppError> {
    if handle.session_binding().is_some() {
        return crate::conversation_sessions::store_path(
            handle,
            crate::conversation_sessions::StoreKind::Task,
            true,
        );
    }
    let root = handle.root().join(STORE_DIRECTORY);
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
        Ok(_) => return Err(task_error("task.store_boundary")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&root).map_err(|_| task_error("task.store_unavailable"))?;
        }
        Err(_) => return Err(task_error("task.store_unavailable")),
    }
    Ok(root.join(STORE_FILE))
}

fn write_session(handle: &ProjectHandle, session: &TaskSession) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(session).map_err(|_| task_error("task.store_invalid"))?;
    if bytes.len() as u64 > MAX_STORE_BYTES {
        return Err(task_error("task.store_too_large"));
    }
    crate::conversation_sessions::with_store_write(
        handle,
        crate::conversation_sessions::StoreKind::Task,
        bytes.len() as u64 + 1,
        || {
            let target = store_path(handle)?;
            if let Ok(metadata) = fs::symlink_metadata(&target) {
                if !metadata.is_file() || is_link_or_reparse(&metadata) {
                    return Err(task_error("task.store_boundary"));
                }
            }
            let parent = target
                .parent()
                .ok_or_else(|| task_error("task.store_boundary"))?;
            let temporary = parent.join(format!(".session-{}.tmp", Uuid::new_v4()));
            let result = (|| {
                let mut file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temporary)
                    .map_err(|_| task_error("task.store_unavailable"))?;
                file.write_all(&bytes)
                    .and_then(|()| file.write_all(b"\n"))
                    .and_then(|()| file.sync_all())
                    .map_err(|_| task_error("task.store_unavailable"))?;
                drop(file);
                atomic_replace(&temporary, &target)
                    .map_err(|_| task_error("task.store_unavailable"))?;
                sync_directory(parent)
            })();
            if result.is_err() {
                let _ = fs::remove_file(temporary);
            }
            result
        },
    )
}

#[cfg(windows)]
fn sync_directory(_directory: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_directory(directory: &Path) -> Result<(), AppError> {
    std::fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| task_error("task.store_unavailable"))
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_failed_task_error(code: &str) -> bool {
    FAILED_TASK_ERRORS.contains(&code)
}

fn is_cancelled_task_error(code: &str) -> bool {
    CANCELLED_TASK_ERRORS.contains(&code)
}

fn valid_stored_task_state(task: &TaskRecord) -> bool {
    let result_pair = match (&task.result_sha256, task.result_bytes) {
        (None, None) => false,
        (Some(_), Some(bytes)) if bytes > 0 => true,
        _ => return false,
    };
    match task.status {
        TaskStatus::Queued | TaskStatus::Running => !result_pair && task.error_code.is_none(),
        TaskStatus::Succeeded => result_pair && task.error_code.is_none(),
        TaskStatus::Failed => {
            result_pair && task.error_code.as_deref().is_some_and(is_failed_task_error)
        }
        TaskStatus::Cancelled => match (result_pair, task.error_code.as_deref()) {
            (true, Some(code)) => is_cancelled_task_error(code),
            (false, Some(RECOVERY_INTERRUPTED_ERROR)) => true,
            _ => false,
        },
    }
}

fn task_error(code: &'static str) -> AppError {
    AppError::runtime_unavailable().with_diagnostic(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{inspect_project_root, open_project, ProjectScanMode};

    struct Fixture {
        root: PathBuf,
        handle: ProjectHandle,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("novelgainker-nr201-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("project.md"), "# 小说\n").unwrap();
            let inspection = inspect_project_root(&root, ProjectScanMode::Open).unwrap();
            let handle = open_project(&inspection, true).unwrap();
            Self { root, handle }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn definition() -> ToolDefinition {
        ToolDefinition {
            name: "test.echo".to_owned(),
            version: 1,
            description: "回显".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": false
            }),
            display_label: "处理测试内容".to_owned(),
        }
    }

    #[test]
    fn interrupted_tasks_cancel_once_and_terminal_never_regresses() {
        let fixture = Fixture::new();
        let conversation = crate::conversation::load_conversation(&fixture.handle, 1).unwrap();
        let turn_id = Uuid::new_v4().to_string();
        let call = ProviderToolCall {
            call_id: "call_1".to_owned(),
            name: "test.echo".to_owned(),
            arguments_json: r#"{"text":"ok"}"#.to_owned(),
        };
        let queued = create_queued_task(
            &fixture.handle,
            &conversation.session_id,
            &turn_id,
            &call,
            &definition(),
            2,
        )
        .unwrap();
        assert_eq!(queued.task_revision, 1);

        let view = runtime_view(&fixture.handle, conversation, 3, true).unwrap();
        assert_eq!(view.task_activities[0].status, TaskStatus::Cancelled);
        assert_eq!(view.task_activities[0].task_revision, 2);
        assert!(transition_task(
            &fixture.handle,
            &queued.task_id,
            2,
            TaskStatus::Running,
            None,
            None,
            4,
        )
        .is_err());
        let second = runtime_view(&fixture.handle, view.conversation, 5, true).unwrap();
        assert_eq!(second.task_activities[0].task_revision, 2);
    }

    #[test]
    fn duplicate_call_id_is_rejected_without_a_second_task() {
        let fixture = Fixture::new();
        let conversation = crate::conversation::load_conversation(&fixture.handle, 1).unwrap();
        let turn_id = Uuid::new_v4().to_string();
        let call = ProviderToolCall {
            call_id: "call_1".to_owned(),
            name: "test.echo".to_owned(),
            arguments_json: r#"{"text":"ok"}"#.to_owned(),
        };
        create_queued_task(
            &fixture.handle,
            &conversation.session_id,
            &turn_id,
            &call,
            &definition(),
            2,
        )
        .unwrap();
        assert!(create_queued_task(
            &fixture.handle,
            &conversation.session_id,
            &turn_id,
            &call,
            &definition(),
            3,
        )
        .is_err());
        assert_eq!(find_turn_tasks(&fixture.handle, &turn_id).unwrap().len(), 1);
    }

    #[test]
    fn nr506_task_snapshots_and_atomic_updates_are_coordinated() {
        let fixture = Fixture::new();
        let conversation = crate::conversation::load_conversation(&fixture.handle, 1).unwrap();
        let turn = Uuid::new_v4().to_string();
        let start = std::sync::Barrier::new(4);
        std::thread::scope(|scope| {
            for _ in 0..3 {
                let conversation = &conversation;
                let turn = &turn;
                let start = &start;
                let handle = &fixture.handle;
                scope.spawn(move || {
                    start.wait();
                    for _ in 0..100 {
                        runtime_view(handle, conversation.clone(), 5, false).unwrap();
                        find_turn_tasks(handle, turn).unwrap();
                    }
                });
            }
            start.wait();
            for index in 0..100 {
                let call = ProviderToolCall {
                    call_id: format!("nr506_{index}"),
                    name: "test.echo".into(),
                    arguments_json: r#"{"text":"ok"}"#.into(),
                };
                let queued = create_queued_task(
                    &fixture.handle,
                    &conversation.session_id,
                    &turn,
                    &call,
                    &definition(),
                    2,
                )
                .unwrap();
                let running = transition_task(
                    &fixture.handle,
                    &queued.task_id,
                    queued.task_revision,
                    TaskStatus::Running,
                    None,
                    None,
                    3,
                )
                .unwrap();
                let result = ToolResult {
                    call_id: call.call_id,
                    name: "test.echo".into(),
                    version: 1,
                    status: ToolResultStatus::Succeeded,
                    output_json: Some(serde_json::json!({"text":"ok"})),
                    error_code: None,
                };
                transition_task(
                    &fixture.handle,
                    &running.task_id,
                    running.task_revision,
                    TaskStatus::Succeeded,
                    Some(&result),
                    None,
                    4,
                )
                .unwrap();
            }
        });
        let tasks = find_turn_tasks(&fixture.handle, &turn).unwrap();
        assert_eq!(tasks.len(), 100);
        assert!(tasks
            .iter()
            .all(|task| task.status == TaskStatus::Succeeded && task.task_revision == 3));
    }

    #[test]
    fn task_argument_digest_uses_exact_provider_raw_utf8_without_persisting_raw_text() {
        let fixture = Fixture::new();
        let conversation = crate::conversation::load_conversation(&fixture.handle, 1).unwrap();
        let turn_id = Uuid::new_v4().to_string();
        let raw = " { \"text\" : \"malformed-raw-canary\" ";
        let call = ProviderToolCall {
            call_id: "call_raw_digest".to_owned(),
            name: "test.echo".to_owned(),
            arguments_json: raw.to_owned(),
        };
        let queued = create_queued_task(
            &fixture.handle,
            &conversation.session_id,
            &turn_id,
            &call,
            &definition(),
            2,
        )
        .unwrap();
        assert_eq!(queued.arguments_bytes as usize, raw.len());
        assert_eq!(queued.arguments_sha256, sha256(raw.as_bytes()));

        let result = ToolResult {
            call_id: call.call_id,
            name: call.name,
            version: 1,
            status: ToolResultStatus::Failed,
            output_json: None,
            error_code: Some("agent.tool_arguments_invalid".to_owned()),
        };
        transition_task(
            &fixture.handle,
            &queued.task_id,
            queued.task_revision,
            TaskStatus::Failed,
            Some(&result),
            result.error_code.as_deref(),
            3,
        )
        .unwrap();

        let reloaded = find_turn_tasks(&fixture.handle, &turn_id)
            .unwrap()
            .remove(0);
        assert_eq!(reloaded.arguments_bytes as usize, raw.len());
        assert_eq!(reloaded.arguments_sha256, sha256(raw.as_bytes()));
        let stored = fs::read_to_string(store_path(&fixture.handle).unwrap()).unwrap();
        assert!(!stored.contains("malformed-raw-canary"));
        assert!(!stored.contains(raw));
    }

    #[test]
    fn tool_result_identity_status_and_error_are_strict_before_write() {
        let fixture = Fixture::new();
        let conversation = crate::conversation::load_conversation(&fixture.handle, 1).unwrap();
        let turn_id = Uuid::new_v4().to_string();
        let call = ProviderToolCall {
            call_id: "call_exact".to_owned(),
            name: "test.echo".to_owned(),
            arguments_json: r#"{"text":"ok"}"#.to_owned(),
        };
        let queued = create_queued_task(
            &fixture.handle,
            &conversation.session_id,
            &turn_id,
            &call,
            &definition(),
            2,
        )
        .unwrap();
        let running = transition_task(
            &fixture.handle,
            &queued.task_id,
            queued.task_revision,
            TaskStatus::Running,
            None,
            None,
            3,
        )
        .unwrap();
        let before = fs::read(store_path(&fixture.handle).unwrap()).unwrap();
        let success = ToolResult {
            call_id: call.call_id.clone(),
            name: call.name.clone(),
            version: 1,
            status: ToolResultStatus::Succeeded,
            output_json: Some(serde_json::json!({"text": "ok"})),
            error_code: None,
        };

        let mut wrong_call = success.clone();
        wrong_call.call_id = "call_other".to_owned();
        assert!(transition_task(
            &fixture.handle,
            &running.task_id,
            running.task_revision,
            TaskStatus::Succeeded,
            Some(&wrong_call),
            None,
            4,
        )
        .is_err());

        let mut wrong_name = success.clone();
        wrong_name.name = "test.other".to_owned();
        assert!(transition_task(
            &fixture.handle,
            &running.task_id,
            running.task_revision,
            TaskStatus::Succeeded,
            Some(&wrong_name),
            None,
            4,
        )
        .is_err());

        let mut wrong_version = success.clone();
        wrong_version.version = 2;
        assert!(transition_task(
            &fixture.handle,
            &running.task_id,
            running.task_revision,
            TaskStatus::Succeeded,
            Some(&wrong_version),
            None,
            4,
        )
        .is_err());

        let failed = ToolResult {
            call_id: call.call_id.clone(),
            name: call.name.clone(),
            version: 1,
            status: ToolResultStatus::Failed,
            output_json: None,
            error_code: Some("agent.tool_executor_failed".to_owned()),
        };
        assert!(transition_task(
            &fixture.handle,
            &running.task_id,
            running.task_revision,
            TaskStatus::Succeeded,
            Some(&failed),
            failed.error_code.as_deref(),
            4,
        )
        .is_err());

        let mut unknown_error = failed;
        unknown_error.error_code = Some("agent.unapproved_failure".to_owned());
        assert!(transition_task(
            &fixture.handle,
            &running.task_id,
            running.task_revision,
            TaskStatus::Failed,
            Some(&unknown_error),
            unknown_error.error_code.as_deref(),
            4,
        )
        .is_err());

        assert_eq!(
            fs::read(store_path(&fixture.handle).unwrap()).unwrap(),
            before
        );
        assert_eq!(
            find_turn_tasks(&fixture.handle, &turn_id).unwrap()[0].task_revision,
            running.task_revision
        );
        let succeeded = transition_task(
            &fixture.handle,
            &running.task_id,
            running.task_revision,
            TaskStatus::Succeeded,
            Some(&success),
            None,
            5,
        )
        .unwrap();
        assert_eq!(succeeded.status, TaskStatus::Succeeded);
        assert_eq!(succeeded.task_revision, 3);
    }

    #[test]
    fn persisted_task_state_matrix_hashes_and_error_allowlist_are_strict() {
        let fixture = Fixture::new();
        let conversation = crate::conversation::load_conversation(&fixture.handle, 1).unwrap();
        let turn_id = Uuid::new_v4().to_string();
        let call = ProviderToolCall {
            call_id: "call_matrix".to_owned(),
            name: "test.echo".to_owned(),
            arguments_json: r#"{"text":"ok"}"#.to_owned(),
        };
        create_queued_task(
            &fixture.handle,
            &conversation.session_id,
            &turn_id,
            &call,
            &definition(),
            2,
        )
        .unwrap();
        let base = read_or_new(&fixture.handle).unwrap();
        let mut invalid = Vec::new();

        let mut non_hex = base.clone();
        non_hex.tasks[0].call_id_hash = "z".repeat(64);
        invalid.push(non_hex);

        let mut partial_result = base.clone();
        partial_result.tasks[0].result_sha256 = Some("0".repeat(64));
        invalid.push(partial_result);

        let mut queued_error = base.clone();
        queued_error.tasks[0].error_code = Some("agent.tool_cancelled".to_owned());
        invalid.push(queued_error);

        let mut running_result = base.clone();
        running_result.tasks[0].status = TaskStatus::Running;
        running_result.tasks[0].result_sha256 = Some("0".repeat(64));
        running_result.tasks[0].result_bytes = Some(8);
        invalid.push(running_result);

        let mut succeeded_without_result = base.clone();
        succeeded_without_result.tasks[0].status = TaskStatus::Succeeded;
        invalid.push(succeeded_without_result);

        let mut failed_unknown = base.clone();
        failed_unknown.tasks[0].status = TaskStatus::Failed;
        failed_unknown.tasks[0].result_sha256 = Some("0".repeat(64));
        failed_unknown.tasks[0].result_bytes = Some(8);
        failed_unknown.tasks[0].error_code = Some("agent.unapproved_failure".to_owned());
        invalid.push(failed_unknown);

        let mut cancelled_without_recovery_code = base.clone();
        cancelled_without_recovery_code.tasks[0].status = TaskStatus::Cancelled;
        cancelled_without_recovery_code.tasks[0].error_code =
            Some("agent.tool_cancelled".to_owned());
        invalid.push(cancelled_without_recovery_code);

        let mut recovery_with_result = base.clone();
        recovery_with_result.tasks[0].status = TaskStatus::Cancelled;
        recovery_with_result.tasks[0].result_sha256 = Some("0".repeat(64));
        recovery_with_result.tasks[0].result_bytes = Some(8);
        recovery_with_result.tasks[0].error_code = Some(RECOVERY_INTERRUPTED_ERROR.to_owned());
        invalid.push(recovery_with_result);

        for session in invalid {
            assert!(validate_session(&session, fixture.handle.project_id()).is_err());
        }

        let mut succeeded = base.clone();
        succeeded.tasks[0].status = TaskStatus::Succeeded;
        succeeded.tasks[0].result_sha256 = Some("0".repeat(64));
        succeeded.tasks[0].result_bytes = Some(8);
        validate_session(&succeeded, fixture.handle.project_id()).unwrap();

        let mut failed = base.clone();
        failed.tasks[0].status = TaskStatus::Failed;
        failed.tasks[0].result_sha256 = Some("0".repeat(64));
        failed.tasks[0].result_bytes = Some(8);
        failed.tasks[0].error_code = Some("agent.tool_executor_failed".to_owned());
        validate_session(&failed, fixture.handle.project_id()).unwrap();

        let mut live_cancelled = base.clone();
        live_cancelled.tasks[0].status = TaskStatus::Cancelled;
        live_cancelled.tasks[0].result_sha256 = Some("0".repeat(64));
        live_cancelled.tasks[0].result_bytes = Some(8);
        live_cancelled.tasks[0].error_code = Some("agent.tool_cancelled".to_owned());
        validate_session(&live_cancelled, fixture.handle.project_id()).unwrap();

        let mut recovered = base;
        recovered.tasks[0].status = TaskStatus::Cancelled;
        recovered.tasks[0].error_code = Some(RECOVERY_INTERRUPTED_ERROR.to_owned());
        validate_session(&recovered, fixture.handle.project_id()).unwrap();
    }

    #[test]
    fn invalid_store_is_rejected_before_recovery_without_rewrite() {
        let fixture = Fixture::new();
        let conversation = crate::conversation::load_conversation(&fixture.handle, 1).unwrap();
        let turn_id = Uuid::new_v4().to_string();
        let call = ProviderToolCall {
            call_id: "call_corrupt".to_owned(),
            name: "test.echo".to_owned(),
            arguments_json: r#"{"text":"ok"}"#.to_owned(),
        };
        create_queued_task(
            &fixture.handle,
            &conversation.session_id,
            &turn_id,
            &call,
            &definition(),
            2,
        )
        .unwrap();
        let path = store_path(&fixture.handle).unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        document["tasks"][0]["callIdHash"] = serde_json::Value::String("z".repeat(64));
        fs::write(&path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        let corrupt = fs::read(&path).unwrap();

        assert!(runtime_view(&fixture.handle, conversation, 3, true).is_err());
        assert_eq!(fs::read(path).unwrap(), corrupt);
    }
}
