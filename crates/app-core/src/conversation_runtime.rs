use crate::conversation::{
    execute_provider_turn, finish_conversation_turn, read_conversation,
    ConversationProviderTerminal, ConversationProviderTerminalState, ConversationTerminalKind,
};
use crate::credential_vault::ProviderId;
use crate::memory::{
    assemble_main_request, build_compactor_request, commit_compaction, load_valid_memory,
    plan_compaction, ContextProfile, NR1_CONTEXT_PROFILE,
};
use crate::project::ProjectHandle;
use crate::provider_core::{
    CancellationToken, FinishReason, NormalizedEvent, ProviderError, ProviderErrorKind,
    ProviderHttpClient,
};
use crate::AppError;

#[allow(clippy::too_many_arguments)]
pub fn execute_memory_managed_turn<F>(
    client: &ProviderHttpClient,
    handle: &ProjectHandle,
    turn_id: &str,
    provider_id: ProviderId,
    key: &[u8],
    cancellation: &CancellationToken,
    timestamp_ms: u64,
    on_event: F,
) -> Result<ConversationProviderTerminal, AppError>
where
    F: FnMut(NormalizedEvent) + Send,
{
    execute_with_profile(
        client,
        handle,
        turn_id,
        provider_id,
        key,
        cancellation,
        timestamp_ms,
        NR1_CONTEXT_PROFILE,
        on_event,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_with_profile<F>(
    client: &ProviderHttpClient,
    handle: &ProjectHandle,
    turn_id: &str,
    provider_id: ProviderId,
    key: &[u8],
    cancellation: &CancellationToken,
    timestamp_ms: u64,
    profile: ContextProfile,
    on_event: F,
) -> Result<ConversationProviderTerminal, AppError>
where
    F: FnMut(NormalizedEvent) + Send,
{
    let session = read_conversation(handle)?;
    let mut active_memory = load_valid_memory(handle, &session, profile);

    if let Ok(Some(plan)) = plan_compaction(&session, active_memory.as_ref(), profile) {
        if let Ok(request) = build_compactor_request(&plan, profile) {
            match client.complete_once_cancellable_blocking(
                provider_id,
                key,
                &request,
                cancellation,
            ) {
                Ok(response) if response.finish_reason == FinishReason::Stop => {
                    if let Ok(committed) = commit_compaction(
                        handle,
                        &session,
                        &plan,
                        &response.text,
                        timestamp_ms,
                        profile,
                    ) {
                        active_memory = Some(committed);
                    }
                }
                Err(error) if error.kind == ProviderErrorKind::Cancelled => {
                    return terminalize_cancelled_turn(handle, turn_id, error, timestamp_ms);
                }
                Ok(_) | Err(_) => {}
            }
        }
    }

    if cancellation.is_cancelled() {
        return terminalize_cancelled_turn(
            handle,
            turn_id,
            ProviderError::cancelled(),
            timestamp_ms,
        );
    }

    let session = read_conversation(handle)?;
    let refreshed_memory = load_valid_memory(handle, &session, profile).or(active_memory);
    let request = assemble_main_request(&session, refreshed_memory.as_ref(), profile)?;
    execute_provider_turn(
        client,
        handle,
        turn_id,
        provider_id,
        key,
        &request,
        cancellation,
        timestamp_ms,
        on_event,
    )
}

fn terminalize_cancelled_turn(
    handle: &ProjectHandle,
    turn_id: &str,
    error: ProviderError,
    timestamp_ms: u64,
) -> Result<ConversationProviderTerminal, AppError> {
    let session = finish_conversation_turn(
        handle,
        turn_id,
        "",
        ConversationTerminalKind::Cancelled,
        Some(error.code),
        timestamp_ms,
    )?;
    Ok(ConversationProviderTerminal {
        session,
        state: ConversationProviderTerminalState::Cancelled,
        finish_reason: None,
        provider_error: Some(error),
    })
}

#[cfg(test)]
mod tests {
    use super::{execute_with_profile, ContextProfile};
    use crate::conversation::{
        finish_conversation_turn, read_conversation, start_conversation_turn,
        ConversationProviderTerminalState, ConversationTerminalKind, ConversationTurnStatus,
    };
    use crate::credential_vault::ProviderId;
    use crate::memory::{commit_compaction, load_valid_memory, plan_compaction};
    use crate::project::{inspect_project_root, open_project, ProjectScanMode};
    use crate::provider_core::{CancellationToken, ProviderHttpClient};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    struct Fixture {
        root: PathBuf,
        handle: crate::project::ProjectHandle,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("novelgainker-nr104-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("project.md"), "# 小说\n").unwrap();
            let inspection = inspect_project_root(&root, ProjectScanMode::Open).unwrap();
            let handle = open_project(&inspection, true).unwrap();
            Self { root, handle }
        }

        fn complete_turn(&self, index: usize) {
            let started = start_conversation_turn(
                &self.handle,
                ProviderId::Openai,
                "gpt-5.6-terra",
                &format!("用户决定 {index} {}", "甲".repeat(40)),
                10 + index as u64 * 10,
            )
            .unwrap();
            finish_conversation_turn(
                &self.handle,
                &started.turn_id,
                &format!("讨论回复 {index} {}", "乙".repeat(40)),
                ConversationTerminalKind::Completed,
                None,
                11 + index as u64 * 10,
            )
            .unwrap();
        }

        fn start_turn(&self, at: u64) -> String {
            start_conversation_turn(
                &self.handle,
                ProviderId::Openai,
                "gpt-5.6-terra",
                "请继续",
                at,
            )
            .unwrap()
            .turn_id
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn profile() -> ContextProfile {
        ContextProfile {
            trigger_bytes: 900,
            protected_tail_bytes: 240,
            segment_content_bytes: 128,
            compactor_input_bytes: 8_192,
            backfill_input_bytes: 256,
            summary_bytes: 1_024,
            compactor_requested_max: 128,
            main_reply_requested_max: 512,
            cumulative_requested_output_max: 640,
        }
    }

    fn summary() -> &'static str {
        "已确认决定\n- 用户已确认继续\n正在创作或讨论\n- 当前故事\n未决问题\n- 无\n用户明确任务\n- 继续\n已产生但未完成的内容\n- 无"
    }

    enum LoopbackReply {
        Completion,
        InvalidCompletion,
        MainStream,
        Unavailable,
    }

    fn spawn_sequence(replies: Vec<LoopbackReply>) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = attempts.clone();
        thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                observed.fetch_add(1, Ordering::SeqCst);
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut request = [0u8; 16 * 1024];
                let _ = stream.read(&mut request);
                match reply {
                    LoopbackReply::Completion => write_json(&mut stream, &completion_json()),
                    LoopbackReply::InvalidCompletion => {
                        write_json(&mut stream, &invalid_completion_json())
                    }
                    LoopbackReply::MainStream => write_main_stream(&mut stream),
                    LoopbackReply::Unavailable => {
                        stream
                            .write_all(
                                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                            .unwrap();
                    }
                }
            }
        });
        (format!("http://{address}/v1/responses"), attempts)
    }

    fn completion_json() -> String {
        serde_json::json!({
            "model": "gpt-5.6-terra",
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [{"type": "output_text", "text": summary()}]
            }],
            "usage": {"input_tokens": 10, "output_tokens": 20, "total_tokens": 30}
        })
        .to_string()
    }

    fn invalid_completion_json() -> String {
        serde_json::json!({
            "model": "gpt-5.6-terra",
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [{"type": "output_text", "text": r#"{"control":"invalid-summary"}"#}]
            }],
            "usage": {"input_tokens": 10, "output_tokens": 20, "total_tokens": 30}
        })
        .to_string()
    }

    fn write_json(stream: &mut std::net::TcpStream, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    }

    fn write_main_stream(stream: &mut std::net::TcpStream) {
        let body = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"主回复\",\"sequence_number\":1}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"model\":\"gpt-5.6-terra\"}}\n\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    }

    #[test]
    fn compactor_success_is_hidden_commits_memory_and_still_streams_main_reply() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fixture.complete_turn(index);
        }
        let turn_id = fixture.start_turn(200);
        let (endpoint, attempts) =
            spawn_sequence(vec![LoopbackReply::Completion, LoopbackReply::MainStream]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let terminal = execute_with_profile(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &CancellationToken::default(),
            201,
            profile(),
            |_| {},
        )
        .unwrap();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        let turn = terminal.session.turns.last().unwrap();
        assert_eq!(turn.messages.len(), 2);
        assert_eq!(turn.messages[1].content, "主回复");
        assert!(!turn.messages[1].content.contains("已确认决定"));
        assert!(load_valid_memory(&fixture.handle, &terminal.session, profile()).is_some());
    }

    #[test]
    fn ordinary_compactor_failure_is_hidden_and_main_reply_still_runs_once() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fixture.complete_turn(index);
        }
        let turn_id = fixture.start_turn(200);
        let (endpoint, attempts) =
            spawn_sequence(vec![LoopbackReply::Unavailable, LoopbackReply::MainStream]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let terminal = execute_with_profile(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &CancellationToken::default(),
            201,
            profile(),
            |_| {},
        )
        .unwrap();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        assert_eq!(
            terminal.session.turns.last().unwrap().messages[1].content,
            "主回复"
        );
        assert!(!fixture
            .root
            .join(".novelgainker/memory/state.json")
            .exists());
    }

    #[test]
    fn invalid_compactor_summary_is_hidden_and_main_reply_still_runs_once() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fixture.complete_turn(index);
        }
        let turn_id = fixture.start_turn(200);
        let (endpoint, attempts) = spawn_sequence(vec![
            LoopbackReply::InvalidCompletion,
            LoopbackReply::MainStream,
        ]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let terminal = execute_with_profile(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &CancellationToken::default(),
            201,
            profile(),
            |_| {},
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        assert_eq!(
            terminal.session.turns.last().unwrap().messages[1].content,
            "主回复"
        );
        assert!(!fixture
            .root
            .join(".novelgainker/memory/state.json")
            .exists());
    }

    #[test]
    fn valid_memory_without_incremental_plan_skips_compactor_and_preserves_bytes() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fixture.complete_turn(index);
        }

        let seed_turn_id = fixture.start_turn(200);
        let seed_session = read_conversation(&fixture.handle).unwrap();
        let seed_plan = plan_compaction(&seed_session, None, profile())
            .unwrap()
            .expect("seed history must produce recent rebuild");
        let seed_memory = commit_compaction(
            &fixture.handle,
            &seed_session,
            &seed_plan,
            summary(),
            201,
            profile(),
        )
        .unwrap();
        finish_conversation_turn(
            &fixture.handle,
            &seed_turn_id,
            "种子回复",
            ConversationTerminalKind::Completed,
            None,
            202,
        )
        .unwrap();

        let turn_id = fixture.start_turn(210);
        let session = read_conversation(&fixture.handle).unwrap();
        let mut no_forward_profile = profile();
        no_forward_profile.trigger_bytes = 1;
        no_forward_profile.protected_tail_bytes = 64 * 1024;
        assert_eq!(
            load_valid_memory(&fixture.handle, &session, no_forward_profile),
            Some(seed_memory.clone())
        );
        assert!(
            plan_compaction(&session, Some(&seed_memory), no_forward_profile)
                .unwrap()
                .is_none()
        );

        let memory_path = fixture.root.join(".novelgainker/memory/state.json");
        let memory_before = fs::read(&memory_path).unwrap();
        let (endpoint, attempts) = spawn_sequence(vec![LoopbackReply::MainStream]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let terminal = execute_with_profile(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &CancellationToken::default(),
            211,
            no_forward_profile,
            |_| {},
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        assert_eq!(
            terminal.session.turns.last().unwrap().messages[1].content,
            "主回复"
        );
        assert_eq!(fs::read(memory_path).unwrap(), memory_before);
    }

    #[test]
    fn cancelling_inflight_compactor_stops_the_turn_without_starting_main() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fixture.complete_turn(index);
        }
        let turn_id = fixture.start_turn(200);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = attempts.clone();
        let (accepted_tx, accepted_rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            observed.fetch_add(1, Ordering::SeqCst);
            let mut request = [0u8; 16 * 1024];
            let _ = stream.read(&mut request);
            let _ = accepted_tx.send(());
            thread::sleep(Duration::from_secs(2));
        });
        let client = ProviderHttpClient::for_test(
            ProviderId::Openai,
            format!("http://{address}/v1/responses"),
        )
        .unwrap();
        let cancellation = CancellationToken::default();
        let worker_cancellation = cancellation.clone();
        let handle = fixture.handle.clone();
        let worker = thread::spawn(move || {
            execute_with_profile(
                &client,
                &handle,
                &turn_id,
                ProviderId::Openai,
                b"synthetic-loopback-key",
                &worker_cancellation,
                201,
                profile(),
                |_| {},
            )
            .unwrap()
        });
        accepted_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("compactor must start before cancellation");
        let cancelled_at = Instant::now();
        cancellation.cancel();
        let terminal = worker.join().unwrap();
        assert!(cancelled_at.elapsed() < Duration::from_secs(2));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Cancelled);
        assert_eq!(
            terminal.session.turns.last().unwrap().status,
            ConversationTurnStatus::NotGenerated
        );
        assert!(!fixture
            .root
            .join(".novelgainker/memory/state.json")
            .exists());
        assert_eq!(
            read_conversation(&fixture.handle).unwrap(),
            terminal.session
        );
    }
}
