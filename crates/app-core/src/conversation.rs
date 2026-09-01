use crate::credential_vault::ProviderId;
use crate::project::ProjectHandle;
use crate::provider_core::{
    CancellationToken, FinishReason, NormalizedEvent, NormalizedRequest, ProviderError,
    ProviderErrorKind, ProviderHttpClient, MAX_MESSAGE_BYTES as PROVIDER_MAX_MESSAGE_BYTES,
    MAX_RESPONSE_BYTES,
};
use crate::workspace::{atomic_replace, is_link_or_reparse};
use crate::AppError;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const STORE_DIRECTORY: &str = ".novelgainker/conversation";
const STORE_FILE: &str = "session.json";
const SCHEMA_VERSION: u16 = 1;
const MAX_STORE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_VISIBLE_MESSAGE_BYTES: usize = MAX_RESPONSE_BYTES + 128;
const MAX_MESSAGES: usize = 10_000;
pub const NOT_GENERATED_TEXT: &str = "本次回复未生成。";
pub const INCOMPLETE_TEXT: &str = "本次内容未生成完成。";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTurnStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
    NotGenerated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationMessage {
    pub message_id: String,
    pub role: ConversationRole,
    pub content: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationTurn {
    pub turn_id: String,
    pub provider_id: ProviderId,
    pub model: String,
    pub status: ConversationTurnStatus,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub messages: Vec<ConversationMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationSession {
    pub schema_version: u16,
    pub project_id: String,
    pub session_id: String,
    pub turns: Vec<ConversationTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnStarted {
    pub session_id: String,
    pub turn_id: String,
    pub user_message_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationProviderTerminalState {
    Completed,
    Incomplete,
    Cancelled,
}

pub struct ConversationProviderTerminal {
    pub session: ConversationSession,
    pub state: ConversationProviderTerminalState,
    pub finish_reason: Option<FinishReason>,
    pub provider_error: Option<ProviderError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationTerminalKind {
    Completed,
    Cancelled,
    Failed,
}

pub fn load_conversation(
    handle: &ProjectHandle,
    now_ms: u64,
) -> Result<ConversationSession, AppError> {
    let mut session = read_or_new(handle)?;
    let mut changed = false;
    for turn in &mut session.turns {
        if turn.status == ConversationTurnStatus::Running {
            turn.status = ConversationTurnStatus::NotGenerated;
            turn.updated_at_ms = now_ms.max(turn.started_at_ms);
            turn.messages
                .push(assistant_message(NOT_GENERATED_TEXT, now_ms));
            changed = true;
        }
    }
    validate_session(&session, handle.project_id())?;
    if changed {
        write_session(handle, &session)?;
    }
    Ok(session)
}

pub fn read_conversation(handle: &ProjectHandle) -> Result<ConversationSession, AppError> {
    read_or_new(handle)
}

pub fn start_conversation_turn(
    handle: &ProjectHandle,
    provider_id: ProviderId,
    model: &str,
    user_text: &str,
    now_ms: u64,
) -> Result<ConversationTurnStarted, AppError> {
    validate_user_text(user_text)?;
    if model.is_empty() || model.len() > 160 || now_ms == 0 {
        return Err(conversation_error("conversation.turn_invalid"));
    }
    let mut session = load_conversation(handle, now_ms)?;
    if session
        .turns
        .last()
        .is_some_and(|turn| turn.status == ConversationTurnStatus::Running)
    {
        return Err(conversation_error("conversation.turn_active"));
    }
    let turn_id = Uuid::new_v4().to_string();
    let user_message_id = Uuid::new_v4().to_string();
    session.turns.push(ConversationTurn {
        turn_id: turn_id.clone(),
        provider_id,
        model: model.to_owned(),
        status: ConversationTurnStatus::Running,
        started_at_ms: now_ms,
        updated_at_ms: now_ms,
        messages: vec![ConversationMessage {
            message_id: user_message_id.clone(),
            role: ConversationRole::User,
            content: user_text.to_owned(),
            created_at_ms: now_ms,
        }],
        error_code: None,
    });
    validate_session(&session, handle.project_id())?;
    write_session(handle, &session)?;
    Ok(ConversationTurnStarted {
        session_id: session.session_id,
        turn_id,
        user_message_id,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn execute_provider_turn<F>(
    client: &ProviderHttpClient,
    handle: &ProjectHandle,
    turn_id: &str,
    provider_id: ProviderId,
    key: &[u8],
    request: &NormalizedRequest,
    cancellation: &CancellationToken,
    now_ms: u64,
    mut on_event: F,
) -> Result<ConversationProviderTerminal, AppError>
where
    F: FnMut(NormalizedEvent) + Send,
{
    let mut accumulated = String::new();
    let result =
        client.stream_cancellable_blocking(provider_id, key, request, cancellation, &mut |event| {
            if let NormalizedEvent::TextDelta { delta, .. } = &event {
                accumulated.push_str(delta);
            }
            if !matches!(event, NormalizedEvent::Completed { .. }) {
                on_event(event);
            }
            Ok(())
        });
    let (state, kind, finish_reason, provider_error, error_code) = match result {
        Ok(response) if response.finish_reason == FinishReason::Stop => (
            ConversationProviderTerminalState::Completed,
            ConversationTerminalKind::Completed,
            Some(response.finish_reason),
            None,
            None,
        ),
        Ok(response) => (
            ConversationProviderTerminalState::Incomplete,
            ConversationTerminalKind::Failed,
            Some(response.finish_reason),
            None,
            Some("provider.incomplete"),
        ),
        Err(error) if error.kind == ProviderErrorKind::Cancelled => (
            ConversationProviderTerminalState::Cancelled,
            ConversationTerminalKind::Cancelled,
            None,
            Some(error),
            Some(error.code),
        ),
        Err(error) => (
            ConversationProviderTerminalState::Incomplete,
            ConversationTerminalKind::Failed,
            None,
            Some(error),
            Some(error.code),
        ),
    };
    let session =
        finish_conversation_turn(handle, turn_id, &accumulated, kind, error_code, now_ms)?;
    Ok(ConversationProviderTerminal {
        session,
        state,
        finish_reason,
        provider_error,
    })
}

pub fn finish_conversation_turn(
    handle: &ProjectHandle,
    turn_id: &str,
    accumulated_text: &str,
    kind: ConversationTerminalKind,
    error_code: Option<&str>,
    now_ms: u64,
) -> Result<ConversationSession, AppError> {
    let mut session = read_or_new(handle)?;
    let turn = session
        .turns
        .iter_mut()
        .find(|turn| turn.turn_id == turn_id)
        .ok_or_else(|| conversation_error("conversation.turn_not_found"))?;
    if turn.status != ConversationTurnStatus::Running {
        return Ok(session);
    }
    let (status, visible_text) = terminal_text(accumulated_text, kind);
    turn.status = status;
    turn.updated_at_ms = now_ms.max(turn.started_at_ms);
    turn.error_code = sanitize_error_code(error_code);
    turn.messages.push(assistant_message(&visible_text, now_ms));
    validate_session(&session, handle.project_id())?;
    write_session(handle, &session)?;
    Ok(session)
}

pub fn recent_visible_messages(
    session: &ConversationSession,
    maximum_messages: usize,
    maximum_bytes: usize,
) -> Vec<ConversationMessage> {
    if maximum_messages == 0 || maximum_bytes == 0 {
        return Vec::new();
    }
    let mut selected = Vec::new();
    let mut used = 0usize;
    for message in session
        .turns
        .iter()
        .rev()
        .flat_map(|turn| turn.messages.iter().rev())
    {
        if selected.len() >= maximum_messages {
            break;
        }
        let size = message.content.len();
        if size > maximum_bytes.saturating_sub(used) {
            break;
        }
        used += size;
        selected.push(message.clone());
    }
    selected.reverse();
    selected
}

fn terminal_text(
    accumulated_text: &str,
    kind: ConversationTerminalKind,
) -> (ConversationTurnStatus, String) {
    let content = accumulated_text.trim_end();
    if content.is_empty() {
        return (
            ConversationTurnStatus::NotGenerated,
            NOT_GENERATED_TEXT.to_owned(),
        );
    }
    match kind {
        ConversationTerminalKind::Completed => {
            (ConversationTurnStatus::Completed, content.to_owned())
        }
        ConversationTerminalKind::Cancelled => (
            ConversationTurnStatus::Cancelled,
            format!("{content}\n\n{INCOMPLETE_TEXT}"),
        ),
        ConversationTerminalKind::Failed => (
            ConversationTurnStatus::Failed,
            format!("{content}\n\n{INCOMPLETE_TEXT}"),
        ),
    }
}

fn assistant_message(content: &str, now_ms: u64) -> ConversationMessage {
    ConversationMessage {
        message_id: Uuid::new_v4().to_string(),
        role: ConversationRole::Assistant,
        content: content.to_owned(),
        created_at_ms: now_ms,
    }
}

fn read_or_new(handle: &ProjectHandle) -> Result<ConversationSession, AppError> {
    let path = store_path(handle)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(new_session(handle));
        }
        Err(_) => return Err(conversation_error("conversation.store_unavailable")),
    };
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > MAX_STORE_BYTES {
        return Err(conversation_error("conversation.store_invalid"));
    }
    let bytes = fs::read(path).map_err(|_| conversation_error("conversation.store_unavailable"))?;
    let session: ConversationSession = serde_json::from_slice(&bytes)
        .map_err(|_| conversation_error("conversation.store_invalid"))?;
    validate_session(&session, handle.project_id())?;
    Ok(session)
}

fn new_session(handle: &ProjectHandle) -> ConversationSession {
    ConversationSession {
        schema_version: SCHEMA_VERSION,
        project_id: handle.project_id().to_owned(),
        session_id: Uuid::new_v4().to_string(),
        turns: Vec::new(),
    }
}

fn validate_session(session: &ConversationSession, project_id: &str) -> Result<(), AppError> {
    if session.schema_version != SCHEMA_VERSION
        || session.project_id != project_id
        || Uuid::parse_str(&session.session_id).is_err()
        || session
            .turns
            .iter()
            .map(|turn| turn.messages.len())
            .sum::<usize>()
            > MAX_MESSAGES
    {
        return Err(conversation_error("conversation.store_invalid"));
    }
    for turn in &session.turns {
        if Uuid::parse_str(&turn.turn_id).is_err()
            || turn.model.is_empty()
            || turn.model.len() > 160
            || turn.started_at_ms == 0
            || turn.updated_at_ms < turn.started_at_ms
            || turn.messages.is_empty()
            || turn.messages.first().map(|message| message.role) != Some(ConversationRole::User)
            || (turn.status == ConversationTurnStatus::Running && turn.messages.len() != 1)
            || (turn.status != ConversationTurnStatus::Running && turn.messages.len() != 2)
        {
            return Err(conversation_error("conversation.store_invalid"));
        }
        for message in &turn.messages {
            if Uuid::parse_str(&message.message_id).is_err()
                || message.created_at_ms == 0
                || validate_visible_text(&message.content).is_err()
                || (message.role == ConversationRole::User
                    && message.content.len() > PROVIDER_MAX_MESSAGE_BYTES)
            {
                return Err(conversation_error("conversation.store_invalid"));
            }
        }
    }
    Ok(())
}

fn validate_visible_text(text: &str) -> Result<(), AppError> {
    if text.trim().is_empty() || text.len() > MAX_VISIBLE_MESSAGE_BYTES || text.contains('\0') {
        return Err(conversation_error("conversation.message_invalid"));
    }
    Ok(())
}

fn validate_user_text(text: &str) -> Result<(), AppError> {
    validate_visible_text(text)?;
    if text.len() > PROVIDER_MAX_MESSAGE_BYTES {
        return Err(conversation_error("conversation.message_invalid"));
    }
    Ok(())
}

fn sanitize_error_code(value: Option<&str>) -> Option<String> {
    value
        .filter(|code| {
            !code.is_empty()
                && code.len() <= 96
                && code.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        })
        .map(str::to_owned)
}

fn store_path(handle: &ProjectHandle) -> Result<PathBuf, AppError> {
    let root = handle.root().join(STORE_DIRECTORY);
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
        Ok(_) => return Err(conversation_error("conversation.store_boundary")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&root)
                .map_err(|_| conversation_error("conversation.store_unavailable"))?;
        }
        Err(_) => return Err(conversation_error("conversation.store_unavailable")),
    }
    Ok(root.join(STORE_FILE))
}

fn write_session(handle: &ProjectHandle, session: &ConversationSession) -> Result<(), AppError> {
    let target = store_path(handle)?;
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(conversation_error("conversation.store_boundary"));
        }
    }
    let bytes = serde_json::to_vec_pretty(session)
        .map_err(|_| conversation_error("conversation.store_invalid"))?;
    if bytes.len() as u64 > MAX_STORE_BYTES {
        return Err(conversation_error("conversation.store_too_large"));
    }
    let parent = target
        .parent()
        .ok_or_else(|| conversation_error("conversation.store_boundary"))?;
    let temporary = parent.join(format!(".session-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| conversation_error("conversation.store_unavailable"))?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|_| conversation_error("conversation.store_unavailable"))?;
        drop(file);
        atomic_replace(&temporary, &target)
            .map_err(|_| conversation_error("conversation.store_unavailable"))?;
        sync_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(windows)]
fn sync_directory(_directory: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_directory(directory: &Path) -> Result<(), AppError> {
    std::fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| conversation_error("conversation.store_unavailable"))
}

fn conversation_error(code: &'static str) -> AppError {
    AppError::runtime_unavailable().with_diagnostic(code)
}

#[cfg(test)]
mod tests {
    use super::{
        execute_provider_turn, finish_conversation_turn, load_conversation,
        recent_visible_messages, start_conversation_turn, ConversationProviderTerminalState,
        ConversationTerminalKind, ConversationTurnStatus, INCOMPLETE_TEXT, NOT_GENERATED_TEXT,
    };
    use crate::credential_vault::ProviderId;
    use crate::project::{inspect_project_root, open_project, ProjectScanMode};
    use crate::provider_core::{
        preset, CancellationToken, MessageRole, NormalizedEvent, NormalizedMessage,
        NormalizedRequest, ProviderHttpClient,
    };
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    struct Fixture {
        root: PathBuf,
        handle: crate::project::ProjectHandle,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("novelgainker-nr003-{}", Uuid::new_v4()));
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

    #[test]
    fn starts_from_blank_and_persists_user_before_provider_output() {
        let fixture = Fixture::new();
        let blank = load_conversation(&fixture.handle, 1).unwrap();
        assert!(blank.turns.is_empty());
        let started = start_conversation_turn(
            &fixture.handle,
            ProviderId::Deepseek,
            "deepseek-v4-pro",
            "聊聊今天的天气",
            10,
        )
        .unwrap();
        let bytes = fs::read(fixture.root.join(".novelgainker/conversation/session.json")).unwrap();
        let json = String::from_utf8(bytes).unwrap();
        assert!(json.contains(&started.turn_id));
        assert!(json.contains("聊聊今天的天气"));
        assert!(json.contains("\"status\": \"running\""));
        for forbidden in [
            "skill",
            "control",
            "anchor",
            "canon",
            "tool",
            "relativePath",
        ] {
            assert!(!json
                .to_ascii_lowercase()
                .contains(&forbidden.to_ascii_lowercase()));
        }
    }

    #[test]
    fn terminalizes_nonempty_and_empty_results_without_losing_visible_text() {
        let fixture = Fixture::new();
        let first = start_conversation_turn(
            &fixture.handle,
            ProviderId::Openai,
            "gpt-5.6-terra",
            "继续",
            10,
        )
        .unwrap();
        let completed = finish_conversation_turn(
            &fixture.handle,
            &first.turn_id,
            "第一段正文",
            ConversationTerminalKind::Completed,
            None,
            20,
        )
        .unwrap();
        assert_eq!(completed.turns[0].messages[1].content, "第一段正文");

        let second = start_conversation_turn(
            &fixture.handle,
            ProviderId::Openai,
            "gpt-5.6-terra",
            "再来",
            30,
        )
        .unwrap();
        let cancelled = finish_conversation_turn(
            &fixture.handle,
            &second.turn_id,
            "已有半句",
            ConversationTerminalKind::Cancelled,
            Some("provider.cancelled"),
            40,
        )
        .unwrap();
        assert!(cancelled.turns[1].messages[1].content.contains("已有半句"));
        assert!(cancelled.turns[1].messages[1]
            .content
            .contains(INCOMPLETE_TEXT));

        let third = start_conversation_turn(
            &fixture.handle,
            ProviderId::Openai,
            "gpt-5.6-terra",
            "空回复",
            50,
        )
        .unwrap();
        let failed = finish_conversation_turn(
            &fixture.handle,
            &third.turn_id,
            "",
            ConversationTerminalKind::Failed,
            Some("provider.protocol"),
            60,
        )
        .unwrap();
        assert_eq!(failed.turns[2].status, ConversationTurnStatus::NotGenerated);
        assert_eq!(failed.turns[2].messages[1].content, NOT_GENERATED_TEXT);
    }

    #[test]
    fn reopen_terminalizes_an_interrupted_running_turn_without_retry() {
        let fixture = Fixture::new();
        start_conversation_turn(
            &fixture.handle,
            ProviderId::Deepseek,
            "deepseek-v4-pro",
            "继续写",
            10,
        )
        .unwrap();
        let reopened = load_conversation(&fixture.handle, 20).unwrap();
        assert_eq!(
            reopened.turns[0].status,
            ConversationTurnStatus::NotGenerated
        );
        assert_eq!(reopened.turns[0].messages[1].content, NOT_GENERATED_TEXT);
        assert_eq!(recent_visible_messages(&reopened, 10, 1024).len(), 2);
    }

    #[test]
    fn both_provider_loopbacks_preserve_text_for_every_terminal_shape() {
        for provider in ProviderId::ALL {
            let fixture = Fixture::new();
            let completed =
                run_loopback_case(&fixture, provider, LoopbackShape::Completed, "自然回复", 10);
            assert_eq!(
                completed.session.turns.last().unwrap().status,
                ConversationTurnStatus::Completed
            );
            assert_eq!(
                completed.session.turns.last().unwrap().messages[1].content,
                "自然回复"
            );

            let incomplete =
                run_loopback_case(&fixture, provider, LoopbackShape::Length, "已有正文", 30);
            assert_eq!(
                incomplete.session.turns.last().unwrap().status,
                ConversationTurnStatus::Failed
            );
            assert!(incomplete.session.turns.last().unwrap().messages[1]
                .content
                .contains(INCOMPLETE_TEXT));

            let failed = run_loopback_case(
                &fixture,
                provider,
                LoopbackShape::ProtocolError,
                "半句话",
                50,
            );
            assert_eq!(
                failed.session.turns.last().unwrap().status,
                ConversationTurnStatus::Failed
            );
            assert!(failed.session.turns.last().unwrap().messages[1]
                .content
                .starts_with("半句话"));

            let empty = run_loopback_case(&fixture, provider, LoopbackShape::ProtocolError, "", 70);
            assert_eq!(
                empty.session.turns.last().unwrap().status,
                ConversationTurnStatus::NotGenerated
            );
            assert_eq!(
                empty.session.turns.last().unwrap().messages[1].content,
                NOT_GENERATED_TEXT
            );

            assert_manual_cancel_terminalizes_within_two_seconds(&fixture, provider, 90);
        }
    }

    #[derive(Clone, Copy)]
    enum LoopbackShape {
        Completed,
        Length,
        ProtocolError,
        Hold,
    }

    fn run_loopback_case(
        fixture: &Fixture,
        provider: ProviderId,
        shape: LoopbackShape,
        text: &str,
        now_ms: u64,
    ) -> super::ConversationProviderTerminal {
        let endpoint = spawn_loopback(provider, shape, text);
        let client = ProviderHttpClient::for_test(provider, endpoint).unwrap();
        let started = start_conversation_turn(
            &fixture.handle,
            provider,
            preset(provider).model,
            "随便聊聊",
            now_ms,
        )
        .unwrap();
        let request = NormalizedRequest::conversation(
            vec![NormalizedMessage {
                role: MessageRole::User,
                content: "随便聊聊".to_owned(),
            }],
            512,
        )
        .unwrap();
        execute_provider_turn(
            &client,
            &fixture.handle,
            &started.turn_id,
            provider,
            b"synthetic-loopback-key",
            &request,
            &CancellationToken::default(),
            now_ms + 1,
            |_| {},
        )
        .unwrap()
    }

    fn assert_manual_cancel_terminalizes_within_two_seconds(
        fixture: &Fixture,
        provider: ProviderId,
        now_ms: u64,
    ) {
        let endpoint = spawn_loopback(provider, LoopbackShape::Hold, "已生成部分");
        let client = ProviderHttpClient::for_test(provider, endpoint).unwrap();
        let started = start_conversation_turn(
            &fixture.handle,
            provider,
            preset(provider).model,
            "请开始",
            now_ms,
        )
        .unwrap();
        let request = NormalizedRequest::conversation(
            vec![NormalizedMessage {
                role: MessageRole::User,
                content: "请开始".to_owned(),
            }],
            512,
        )
        .unwrap();
        let cancellation = CancellationToken::default();
        let worker_cancellation = cancellation.clone();
        let handle = fixture.handle.clone();
        let turn_id = started.turn_id;
        let (delta_tx, delta_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            execute_provider_turn(
                &client,
                &handle,
                &turn_id,
                provider,
                b"synthetic-loopback-key",
                &request,
                &worker_cancellation,
                now_ms + 1,
                move |event| {
                    if matches!(event, NormalizedEvent::TextDelta { .. }) {
                        let _ = delta_tx.send(());
                    }
                },
            )
            .unwrap()
        });
        delta_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("loopback must emit a visible delta before cancellation");
        let cancel_started = Instant::now();
        cancellation.cancel();
        let terminal = worker.join().unwrap();
        assert!(cancel_started.elapsed() < Duration::from_secs(2));
        assert_eq!(terminal.state, ConversationProviderTerminalState::Cancelled);
        let turn = terminal.session.turns.last().unwrap();
        assert_eq!(turn.status, ConversationTurnStatus::Cancelled);
        assert!(turn.messages[1].content.starts_with("已生成部分"));
        assert!(turn.messages[1].content.contains(INCOMPLETE_TEXT));
    }

    fn spawn_loopback(provider: ProviderId, shape: LoopbackShape, text: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let initial = loopback_initial(provider, text);
        let terminal = loopback_terminal(provider, shape);
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut request = [0u8; 16 * 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
            if !initial.is_empty() {
                stream.write_all(initial.as_bytes()).unwrap();
                stream.flush().unwrap();
            }
            if matches!(shape, LoopbackShape::Hold) {
                thread::sleep(Duration::from_secs(5));
                return;
            }
            stream.write_all(terminal.as_bytes()).unwrap();
            stream.flush().unwrap();
        });
        format!("http://{address}/v1/conversation")
    }

    fn loopback_initial(provider: ProviderId, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        match provider {
            ProviderId::Deepseek => format!(
                "data: {{\"model\":\"deepseek-v4-pro\",\"choices\":[{{\"delta\":{{\"content\":{}}},\"finish_reason\":null}}]}}\n\n",
                serde_json::to_string(text).unwrap()
            ),
            ProviderId::Openai => format!(
                "data: {{\"type\":\"response.output_text.delta\",\"delta\":{},\"sequence_number\":1}}\n\n",
                serde_json::to_string(text).unwrap()
            ),
        }
    }

    fn loopback_terminal(provider: ProviderId, shape: LoopbackShape) -> String {
        match (provider, shape) {
            (ProviderId::Deepseek, LoopbackShape::Completed) => concat!(
                "data: {\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n"
            )
            .to_owned(),
            (ProviderId::Deepseek, LoopbackShape::Length) => concat!(
                "data: {\"model\":\"deepseek-v4-pro\",\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
                "data: [DONE]\n\n"
            )
            .to_owned(),
            (ProviderId::Openai, LoopbackShape::Completed) => concat!(
                "data: {\"type\":\"response.completed\",\"response\":",
                "{\"model\":\"gpt-5.6-terra\"}}\n\n"
            )
            .to_owned(),
            (ProviderId::Openai, LoopbackShape::Length) => {
                "data: {\"type\":\"response.incomplete\"}\n\n".to_owned()
            }
            (_, LoopbackShape::ProtocolError) => "data: {invalid-json}\n\n".to_owned(),
            (_, LoopbackShape::Hold) => String::new(),
        }
    }
}
