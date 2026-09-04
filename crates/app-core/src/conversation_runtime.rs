use crate::capability_pack::{
    embedded_pack, CapabilityPackError, ValidatedCapabilityPack, NOVELIST_AGENT_FALLBACK_ADDENDUM,
};
use crate::conversation::{
    execute_provider_turn, finish_conversation_turn, read_conversation,
    ConversationProviderTerminal, ConversationProviderTerminalState, ConversationTerminalKind,
};
use crate::credential_vault::ProviderId;
use crate::memory::{
    assemble_main_request_with_addendum, build_compactor_request, commit_compaction,
    load_valid_memory, plan_compaction, ContextProfile, NR1_CONTEXT_PROFILE,
};
use crate::project::ProjectHandle;
use crate::provider_core::{
    CancellationToken, FinishReason, NormalizedEvent, NormalizedRequest, ProviderError,
    ProviderErrorKind, ProviderHttpClient,
};
use crate::task_store::{create_queued_task, transition_task, TaskActivity, TaskStatus};
use crate::tool_runtime::{ToolExecutionContext, ToolRegistry, ToolResult, ToolResultStatus};
use crate::AppError;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const TOOL_EXECUTION_DEADLINE: Duration = Duration::from_secs(30);
pub const TOOL_CANCELLATION_ACK_DEADLINE: Duration = Duration::from_secs(2);
pub const NR2_COMPACTOR_OUTPUT_BUDGET: u32 = 1_024;
pub const NR2_AGENT_ATTEMPT_OUTPUT_BUDGET: u32 = 4_096;
pub const NR2_CUMULATIVE_OUTPUT_BUDGET: u32 = 9_216;
pub const NR3_AGENT_LOOP_RESERVE_BYTES: usize = 40 * 1024;
const FALLBACK_REQUEST_RESERVE_BYTES: usize = 2 * 1024;

struct AgentRuntimeBinding {
    registry: ToolRegistry,
    system_addendum: String,
    request_reserve_bytes: usize,
}

fn bind_novelist_runtime(
    handle: &ProjectHandle,
    session_id: &str,
    turn_id: &str,
    pack: Result<Arc<ValidatedCapabilityPack>, CapabilityPackError>,
) -> AgentRuntimeBinding {
    if let Ok(pack) = pack {
        let system_addendum = pack.agent_system_addendum();
        if let Ok(registry) = ToolRegistry::novelist_production(
            handle,
            pack,
            session_id.to_owned(),
            turn_id.to_owned(),
        ) {
            return AgentRuntimeBinding {
                registry,
                system_addendum,
                request_reserve_bytes: NR3_AGENT_LOOP_RESERVE_BYTES,
            };
        }
    }
    AgentRuntimeBinding {
        registry: ToolRegistry::empty_production(),
        system_addendum: NOVELIST_AGENT_FALLBACK_ADDENDUM.to_owned(),
        request_reserve_bytes: FALLBACK_REQUEST_RESERVE_BYTES,
    }
}

pub enum AgentRuntimeEvent {
    Provider(NormalizedEvent),
    Task(TaskActivity),
}

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
    F: FnMut(AgentRuntimeEvent) + Send,
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
    F: FnMut(AgentRuntimeEvent) + Send,
{
    let identity = read_conversation(handle)?;
    let binding = bind_novelist_runtime(handle, &identity.session_id, turn_id, embedded_pack());
    execute_with_profile_and_runtime(
        client,
        handle,
        turn_id,
        provider_id,
        key,
        cancellation,
        timestamp_ms,
        profile,
        &binding.registry,
        &binding.system_addendum,
        binding.request_reserve_bytes,
        on_event,
    )
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
fn execute_with_profile_and_registry<F>(
    client: &ProviderHttpClient,
    handle: &ProjectHandle,
    turn_id: &str,
    provider_id: ProviderId,
    key: &[u8],
    cancellation: &CancellationToken,
    timestamp_ms: u64,
    profile: ContextProfile,
    registry: &ToolRegistry,
    on_event: F,
) -> Result<ConversationProviderTerminal, AppError>
where
    F: FnMut(AgentRuntimeEvent) + Send,
{
    execute_with_profile_and_runtime(
        client,
        handle,
        turn_id,
        provider_id,
        key,
        cancellation,
        timestamp_ms,
        profile,
        registry,
        "",
        2 * 1024,
        on_event,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_with_profile_and_runtime<F>(
    client: &ProviderHttpClient,
    handle: &ProjectHandle,
    turn_id: &str,
    provider_id: ProviderId,
    key: &[u8],
    cancellation: &CancellationToken,
    timestamp_ms: u64,
    profile: ContextProfile,
    registry: &ToolRegistry,
    system_addendum: &str,
    request_reserve_bytes: usize,
    on_event: F,
) -> Result<ConversationProviderTerminal, AppError>
where
    F: FnMut(AgentRuntimeEvent) + Send,
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
    let request = assemble_main_request_with_addendum(
        &session,
        refreshed_memory.as_ref(),
        profile,
        system_addendum,
        request_reserve_bytes,
    )?;
    execute_agent_turn(
        client,
        handle,
        turn_id,
        provider_id,
        key,
        &request,
        registry,
        cancellation,
        timestamp_ms,
        on_event,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_agent_turn<F>(
    client: &ProviderHttpClient,
    handle: &ProjectHandle,
    turn_id: &str,
    provider_id: ProviderId,
    key: &[u8],
    request: &NormalizedRequest,
    registry: &ToolRegistry,
    cancellation: &CancellationToken,
    timestamp_ms: u64,
    mut on_event: F,
) -> Result<ConversationProviderTerminal, AppError>
where
    F: FnMut(AgentRuntimeEvent) + Send,
{
    if registry.is_empty() {
        return execute_provider_turn(
            client,
            handle,
            turn_id,
            provider_id,
            key,
            request,
            cancellation,
            timestamp_ms,
            |event| on_event(AgentRuntimeEvent::Provider(event)),
        );
    }

    let initial = request
        .clone()
        .with_tool_definitions(registry.definitions())
        .map_err(AppError::from_provider_error)?;
    let mut visible_text = String::new();
    let attempt_one = stream_agent_attempt(
        client,
        provider_id,
        key,
        &initial,
        cancellation,
        &mut visible_text,
        &mut on_event,
    );
    let response = match attempt_one {
        Ok(response) => response,
        Err(error) => {
            return finish_agent_error(handle, turn_id, &visible_text, error, timestamp_ms)
        }
    };
    if response.tool_call.is_none() {
        return finish_agent_response(
            handle,
            turn_id,
            &visible_text,
            response.finish_reason,
            timestamp_ms,
        );
    }
    if response.finish_reason != FinishReason::ToolCall {
        return finish_agent_error(
            handle,
            turn_id,
            &visible_text,
            ProviderError::protocol(),
            timestamp_ms,
        );
    }
    let call = response
        .tool_call
        .ok_or_else(AppError::runtime_unavailable)?;
    let continuation = response
        .continuation
        .ok_or_else(AppError::runtime_unavailable)?;
    if cancellation.is_cancelled() {
        return terminalize_cancelled_turn_with_text(
            handle,
            turn_id,
            &visible_text,
            ProviderError::cancelled(),
            timestamp_ms,
        );
    }
    let session = read_conversation(handle)?;
    let (definition, executor) = match registry.lookup(&call) {
        Ok(resolved) => resolved,
        Err(_) => {
            return finish_agent_error(
                handle,
                turn_id,
                &visible_text,
                ProviderError::agent_tool_unknown(),
                timestamp_ms,
            )
        }
    };
    let queued = create_queued_task(
        handle,
        &session.session_id,
        turn_id,
        &call,
        definition,
        timestamp_ms,
    )?;
    on_event(AgentRuntimeEvent::Task(TaskActivity::from(&queued)));
    let arguments = registry
        .decode_arguments(&call, definition)
        .map_err(|error| error.code())
        .and_then(|arguments| executor.validate_arguments(&arguments).map(|_| arguments));
    let result = match arguments {
        Err(error_code) => {
            let result = ToolResult {
                call_id: call.call_id.clone(),
                name: definition.name.clone(),
                version: definition.version,
                status: ToolResultStatus::Failed,
                output_json: None,
                error_code: Some(error_code.to_owned()),
            };
            let failed = transition_task(
                handle,
                &queued.task_id,
                queued.task_revision,
                TaskStatus::Failed,
                Some(&result),
                result.error_code.as_deref(),
                timestamp_ms.saturating_add(1),
            )?;
            on_event(AgentRuntimeEvent::Task(TaskActivity::from(&failed)));
            result
        }
        Ok(arguments) => {
            let running = transition_task(
                handle,
                &queued.task_id,
                queued.task_revision,
                TaskStatus::Running,
                None,
                None,
                timestamp_ms.saturating_add(1),
            )?;
            on_event(AgentRuntimeEvent::Task(TaskActivity::from(&running)));

            let context = ToolExecutionContext {
                project_id: handle.project_id().to_owned(),
                conversation_session_id: session.session_id,
                turn_id: turn_id.to_owned(),
            };
            let output = execute_bounded_tool(executor, &context, &arguments, cancellation);
            let mut result = ToolResult {
                call_id: call.call_id.clone(),
                name: definition.name.clone(),
                version: definition.version,
                status: output.status,
                output_json: output.output_json,
                error_code: output.error_code.map(str::to_owned),
            };
            if result.validate().is_err() {
                result = ToolResult {
                    call_id: result.call_id,
                    name: result.name,
                    version: result.version,
                    status: ToolResultStatus::Failed,
                    output_json: None,
                    error_code: Some("agent.tool_result_invalid".to_owned()),
                };
            }
            let next_status = match result.status {
                ToolResultStatus::Succeeded => TaskStatus::Succeeded,
                ToolResultStatus::Failed => TaskStatus::Failed,
                ToolResultStatus::Cancelled => TaskStatus::Cancelled,
            };
            let terminal_task = transition_task(
                handle,
                &running.task_id,
                running.task_revision,
                next_status,
                Some(&result),
                result.error_code.as_deref(),
                timestamp_ms.saturating_add(2),
            )?;
            on_event(AgentRuntimeEvent::Task(TaskActivity::from(&terminal_task)));
            result
        }
    };

    if cancellation.is_cancelled() {
        return terminalize_cancelled_turn_with_text(
            handle,
            turn_id,
            &visible_text,
            ProviderError::cancelled(),
            timestamp_ms.saturating_add(3),
        );
    }
    let continuation_request = initial
        .continue_after_tool(continuation, result, NR2_AGENT_ATTEMPT_OUTPUT_BUDGET)
        .map_err(AppError::from_provider_error)?;
    let attempt_two = stream_agent_attempt(
        client,
        provider_id,
        key,
        &continuation_request,
        cancellation,
        &mut visible_text,
        &mut on_event,
    );
    match attempt_two {
        Ok(response) if response.tool_call.is_some() => finish_agent_error(
            handle,
            turn_id,
            &visible_text,
            ProviderError::agent_tool_round_limit(),
            timestamp_ms.saturating_add(3),
        ),
        Ok(response) => finish_agent_response(
            handle,
            turn_id,
            &visible_text,
            response.finish_reason,
            timestamp_ms.saturating_add(3),
        ),
        Err(error) => finish_agent_error(
            handle,
            turn_id,
            &visible_text,
            error,
            timestamp_ms.saturating_add(3),
        ),
    }
}

fn execute_bounded_tool(
    executor: &dyn crate::tool_runtime::ToolExecutor,
    context: &ToolExecutionContext,
    arguments: &serde_json::Value,
    turn_cancellation: &CancellationToken,
) -> crate::tool_runtime::ToolExecutionOutput {
    let tool_cancellation = CancellationToken::default();
    let started = Instant::now();
    std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::sync_channel(1);
        let executor_cancellation = tool_cancellation.clone();
        scope.spawn(move || {
            let output = executor.execute(context, arguments, &executor_cancellation);
            let _ = sender.send(output);
        });
        loop {
            if turn_cancellation.is_cancelled() {
                tool_cancellation.cancel();
                return receiver
                    .recv_timeout(TOOL_CANCELLATION_ACK_DEADLINE)
                    .unwrap_or(crate::tool_runtime::ToolExecutionOutput {
                        status: ToolResultStatus::Cancelled,
                        output_json: None,
                        error_code: Some("agent.tool_cancelled"),
                    });
            }
            let elapsed = started.elapsed();
            if elapsed >= TOOL_EXECUTION_DEADLINE {
                tool_cancellation.cancel();
                let _ = receiver.recv_timeout(TOOL_CANCELLATION_ACK_DEADLINE);
                return crate::tool_runtime::ToolExecutionOutput {
                    status: ToolResultStatus::Failed,
                    output_json: None,
                    error_code: Some("agent.tool_timeout"),
                };
            }
            let wait = (TOOL_EXECUTION_DEADLINE - elapsed).min(Duration::from_millis(20));
            match receiver.recv_timeout(wait) {
                Ok(output) => return output,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return crate::tool_runtime::ToolExecutionOutput {
                        status: ToolResultStatus::Failed,
                        output_json: None,
                        error_code: Some("agent.tool_executor_failed"),
                    }
                }
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn stream_agent_attempt<F>(
    client: &ProviderHttpClient,
    provider_id: ProviderId,
    key: &[u8],
    request: &NormalizedRequest,
    cancellation: &CancellationToken,
    visible_text: &mut String,
    on_event: &mut F,
) -> Result<crate::provider_core::NormalizedResponse, ProviderError>
where
    F: FnMut(AgentRuntimeEvent) + Send,
{
    client.stream_once_cancellable_blocking(provider_id, key, request, cancellation, &mut |event| {
        if let NormalizedEvent::TextDelta { delta, .. } = &event {
            visible_text.push_str(delta);
        }
        if !matches!(event, NormalizedEvent::Completed { .. }) {
            on_event(AgentRuntimeEvent::Provider(event));
        }
        Ok(())
    })
}

fn finish_agent_response(
    handle: &ProjectHandle,
    turn_id: &str,
    visible_text: &str,
    finish_reason: FinishReason,
    timestamp_ms: u64,
) -> Result<ConversationProviderTerminal, AppError> {
    let completed = finish_reason == FinishReason::Stop;
    let session = finish_conversation_turn(
        handle,
        turn_id,
        visible_text,
        if completed {
            ConversationTerminalKind::Completed
        } else {
            ConversationTerminalKind::Failed
        },
        (!completed).then_some("provider.incomplete"),
        timestamp_ms,
    )?;
    Ok(ConversationProviderTerminal {
        session,
        state: if completed {
            ConversationProviderTerminalState::Completed
        } else {
            ConversationProviderTerminalState::Incomplete
        },
        finish_reason: Some(finish_reason),
        provider_error: None,
    })
}

fn finish_agent_error(
    handle: &ProjectHandle,
    turn_id: &str,
    visible_text: &str,
    error: ProviderError,
    timestamp_ms: u64,
) -> Result<ConversationProviderTerminal, AppError> {
    if error.kind == ProviderErrorKind::Cancelled {
        return terminalize_cancelled_turn_with_text(
            handle,
            turn_id,
            visible_text,
            error,
            timestamp_ms,
        );
    }
    let session = finish_conversation_turn(
        handle,
        turn_id,
        visible_text,
        ConversationTerminalKind::Failed,
        Some(error.code),
        timestamp_ms,
    )?;
    Ok(ConversationProviderTerminal {
        session,
        state: ConversationProviderTerminalState::Incomplete,
        finish_reason: None,
        provider_error: Some(error),
    })
}

fn terminalize_cancelled_turn(
    handle: &ProjectHandle,
    turn_id: &str,
    error: ProviderError,
    timestamp_ms: u64,
) -> Result<ConversationProviderTerminal, AppError> {
    terminalize_cancelled_turn_with_text(handle, turn_id, "", error, timestamp_ms)
}

fn terminalize_cancelled_turn_with_text(
    handle: &ProjectHandle,
    turn_id: &str,
    visible_text: &str,
    error: ProviderError,
    timestamp_ms: u64,
) -> Result<ConversationProviderTerminal, AppError> {
    let session = finish_conversation_turn(
        handle,
        turn_id,
        visible_text,
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
    use super::{
        bind_novelist_runtime, execute_agent_turn, execute_with_profile,
        execute_with_profile_and_registry, AgentRuntimeEvent, ContextProfile,
        NR2_AGENT_ATTEMPT_OUTPUT_BUDGET, NR2_COMPACTOR_OUTPUT_BUDGET, NR2_CUMULATIVE_OUTPUT_BUDGET,
        TOOL_CANCELLATION_ACK_DEADLINE,
    };
    use crate::conversation::{
        finish_conversation_turn, read_conversation, start_conversation_turn,
        ConversationProviderTerminalState, ConversationTerminalKind, ConversationTurnStatus,
    };
    use crate::credential_vault::ProviderId;
    use crate::memory::{commit_compaction, load_valid_memory, plan_compaction};
    use crate::project::{inspect_project_root, open_project, ProjectScanMode};
    use crate::provider_core::{CancellationToken, ProviderHttpClient};
    use crate::task_store::{find_turn_tasks, TaskStatus};
    use crate::tool_runtime::tests::{
        cooperative_cancellation_registry, deterministic_failure_registry as failing_registry,
        echo_registry as test_registry,
    };
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
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
        ToolStream,
        ToolStreamInvalidArguments,
        ToolStreamMalformedArguments,
        ToolStreamNonObjectArguments,
        ToolStreamOversizedArguments,
        ToolStreamUnknownName,
        ToolStreamInvalidCallId,
        ToolStreamInterrupted,
        NovelistToolStream,
        NovelistToolStreamInvalidArguments,
        Unavailable,
    }

    fn spawn_sequence(replies: Vec<LoopbackReply>) -> (String, Arc<AtomicUsize>) {
        let (endpoint, attempts, _) = spawn_sequence_with_capture(replies);
        (endpoint, attempts)
    }

    fn spawn_sequence_with_capture(
        replies: Vec<LoopbackReply>,
    ) -> (String, Arc<AtomicUsize>, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = attempts.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                observed.fetch_add(1, Ordering::SeqCst);
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                captured
                    .lock()
                    .unwrap()
                    .push(read_http_request(&mut stream));
                match reply {
                    LoopbackReply::Completion => write_json(&mut stream, &completion_json()),
                    LoopbackReply::InvalidCompletion => {
                        write_json(&mut stream, &invalid_completion_json())
                    }
                    LoopbackReply::MainStream => write_main_stream(&mut stream),
                    LoopbackReply::ToolStream => write_tool_stream(&mut stream),
                    LoopbackReply::ToolStreamInvalidArguments => {
                        write_tool_stream_with_arguments(&mut stream, r#"{"other":"no"}"#)
                    }
                    LoopbackReply::ToolStreamMalformedArguments => {
                        write_tool_stream_with_arguments(&mut stream, malformed_arguments())
                    }
                    LoopbackReply::ToolStreamNonObjectArguments => {
                        write_tool_stream_with_arguments(&mut stream, r#"["not-an-object"]"#)
                    }
                    LoopbackReply::ToolStreamOversizedArguments => {
                        write_tool_stream_with_arguments(&mut stream, &oversized_arguments())
                    }
                    LoopbackReply::ToolStreamUnknownName => write_named_tool_stream(
                        &mut stream,
                        "call_1",
                        "unknown.tool",
                        r#"{"text":"ok"}"#,
                    ),
                    LoopbackReply::ToolStreamInvalidCallId => write_named_tool_stream(
                        &mut stream,
                        "invalid call id",
                        "test.echo",
                        r#"{"text":"ok"}"#,
                    ),
                    LoopbackReply::ToolStreamInterrupted => {
                        write_interrupted_tool_stream(&mut stream)
                    }
                    LoopbackReply::NovelistToolStream => write_novelist_tool_stream(
                        &mut stream,
                        r#"{"capabilityId":"novel-story-telling","capabilityResources":[],"canonOperations":[{"kind":"read","scopes":[],"query":null,"caseSensitive":null,"limit":null,"relativePath":"characters/lu-chen.md","startLine":1,"maxBytes":4096,"cursor":null}]}"#,
                    ),
                    LoopbackReply::NovelistToolStreamInvalidArguments => {
                        write_novelist_tool_stream(
                            &mut stream,
                            r#"{"capabilityId":"novel-story-telling","capabilityResources":[{"resourceId":"references/storytelling/drama.md","cursor":null},{"resourceId":"references/storytelling/mystery.md","cursor":null},{"resourceId":"references/storytelling/romantic-entanglement.md","cursor":null},{"resourceId":"references/storytelling/science-fiction.md","cursor":null},{"resourceId":"references/storytelling/story-flow.md","cursor":null},{"resourceId":"references/storytelling/time-travel-and-loop.md","cursor":null},{"resourceId":"references/storytelling/wuxia.md","cursor":null},{"resourceId":"references/storytelling/drama.md","cursor":null},{"resourceId":"references/storytelling/mystery.md","cursor":null}],"canonOperations":[]}"#,
                        )
                    }
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
        (format!("http://{address}/v1/responses"), attempts, requests)
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut chunk = [0u8; 4 * 1024];
        loop {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            assert!(request.len() <= crate::provider_core::MAX_REQUEST_BYTES + 8 * 1024);
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let body_start = header_end + 4;
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .expect("loopback request must contain Content-Length");
                if request.len() >= body_start + content_length {
                    break;
                }
            }
        }
        String::from_utf8(request).unwrap()
    }

    fn captured_json(request: &str) -> serde_json::Value {
        let (_, body) = request
            .split_once("\r\n\r\n")
            .expect("captured HTTP request must contain a body");
        serde_json::from_str(body).unwrap()
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

    fn write_tool_stream(stream: &mut std::net::TcpStream) {
        write_tool_stream_with_arguments(stream, r#"{"text":"ok"}"#);
    }

    fn malformed_arguments() -> &'static str {
        r#"{"text":"malformed-canary""#
    }

    fn oversized_arguments() -> String {
        format!(
            r#"{{"text":"{}"}}"#,
            "x".repeat(crate::tool_runtime::MAX_TOOL_ARGUMENTS_BYTES)
        )
    }

    fn write_tool_stream_with_arguments(stream: &mut std::net::TcpStream, arguments_json: &str) {
        write_named_tool_stream(stream, "call_1", "test.echo", arguments_json);
    }

    fn write_named_tool_stream(
        stream: &mut std::net::TcpStream,
        call_id: &str,
        name: &str,
        arguments_json: &str,
    ) {
        let completed = serde_json::json!({
            "type": "response.completed",
            "response": {
                "model": "gpt-5.6-terra",
                "output": [
                    {
                        "type": "message",
                        "id": "msg_1",
                        "status": "completed",
                        "role": "assistant",
                        "phase": "commentary",
                        "content": [{"type": "output_text", "text": "我先处理。"}]
                    },
                    {
                        "type": "function_call",
                        "id": "fc_1",
                        "call_id": call_id,
                        "name": name,
                        "arguments": arguments_json,
                        "status": "completed",
                        "phase": "commentary"
                    }
                ],
                "usage": {"input_tokens": 10, "output_tokens": 8, "total_tokens": 18}
            }
        });
        let body = format!(
            "data: {{\"type\":\"response.output_text.delta\",\"delta\":\"我先处理。\",\"sequence_number\":1}}\n\ndata: {completed}\n\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    }

    fn write_interrupted_tool_stream(stream: &mut std::net::TcpStream) {
        let body = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"我先处理。\",\"sequence_number\":1}\n\n",
            "data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{\\\"text\\\":\",\"sequence_number\":2}\n\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    }

    fn write_novelist_tool_stream(stream: &mut std::net::TcpStream, arguments_json: &str) {
        let completed = serde_json::json!({
            "type": "response.completed",
            "response": {
                "model": "gpt-5.6-terra",
                "output": [
                    {
                        "type": "message",
                        "id": "msg_nr3",
                        "status": "completed",
                        "role": "assistant",
                        "phase": "commentary",
                        "content": [{"type": "output_text", "text": "我先核对已有设定。"}]
                    },
                    {
                        "type": "function_call",
                        "id": "fc_nr3",
                        "call_id": "call_nr3",
                        "name": "novelist_read_context",
                        "arguments": arguments_json,
                        "status": "completed",
                        "phase": "commentary"
                    }
                ],
                "usage": {"input_tokens": 10, "output_tokens": 8, "total_tokens": 18}
            }
        });
        let body = format!(
            "data: {{\"type\":\"response.output_text.delta\",\"delta\":\"我先核对已有设定。\",\"sequence_number\":1}}\n\ndata: {completed}\n\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    }

    #[test]
    fn production_runtime_exposes_catalog_and_one_tool_without_keyword_routing() {
        let fixture = Fixture::new();
        let turn_id = fixture.start_turn(100);
        let (endpoint, attempts, requests) =
            spawn_sequence_with_capture(vec![LoopbackReply::MainStream]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();

        let terminal = execute_with_profile(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &CancellationToken::default(),
            101,
            profile(),
            |_| {},
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        let turn = terminal.session.turns.last().unwrap();
        assert_eq!(turn.status, ConversationTurnStatus::Completed);
        assert_eq!(turn.messages[1].content, "主回复");
        assert!(!turn.messages[1]
            .content
            .contains(crate::conversation::NOT_GENERATED_TEXT));
        let captured = requests.lock().unwrap();
        let body = captured_json(&captured[0]);
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert_eq!(body["tools"][0]["name"], "novelist_read_context");
        let instructions = body["instructions"].as_str().unwrap();
        assert!(instructions.contains("常驻的 Novelist Agent"));
        assert!(instructions.contains("create-character"));
        assert!(instructions.contains("novel-story-telling"));
        assert!(!body.to_string().contains("selectedCapability"));
        assert!(body.to_string().contains("请继续"));
    }

    #[test]
    fn invalid_pack_atomically_removes_catalog_resources_and_registry_but_keeps_identity() {
        let fixture = Fixture::new();
        let binding = bind_novelist_runtime(
            &fixture.handle,
            "session-fallback",
            "turn-fallback",
            Err(crate::capability_pack::CapabilityPackError::IntegrityInvalid),
        );
        assert!(binding.registry.is_empty());
        assert_eq!(binding.request_reserve_bytes, 2 * 1024);
        assert_eq!(
            binding.system_addendum,
            crate::capability_pack::NOVELIST_AGENT_FALLBACK_ADDENDUM
        );
        assert!(binding.system_addendum.contains("Novelist Agent"));
        assert!(!binding.system_addendum.contains("可用能力目录"));
        assert!(!binding.system_addendum.contains("novelist.read_context"));
    }

    #[test]
    fn production_novelist_tool_reads_canon_then_completes_the_same_conversation() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.root.join("characters")).unwrap();
        let character_path = fixture.root.join("characters/lu-chen.md");
        fs::write(&character_path, "# 陆沉\n他在雾港守着灯塔。\n").unwrap();
        let before = fs::read(&character_path).unwrap();
        let turn_id = fixture.start_turn(100);
        let (endpoint, attempts, requests) = spawn_sequence_with_capture(vec![
            LoopbackReply::NovelistToolStream,
            LoopbackReply::MainStream,
        ]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let mut task_statuses = Vec::new();

        let terminal = execute_with_profile(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &CancellationToken::default(),
            101,
            profile(),
            |event| {
                if let AgentRuntimeEvent::Task(activity) = event {
                    task_statuses.push(activity.status);
                }
            },
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(
            task_statuses,
            vec![
                TaskStatus::Queued,
                TaskStatus::Running,
                TaskStatus::Succeeded
            ]
        );
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        assert_eq!(
            terminal.session.turns.last().unwrap().messages[1].content,
            "我先核对已有设定。主回复"
        );
        assert_eq!(fs::read(&character_path).unwrap(), before);
        let captured = requests.lock().unwrap();
        let continuation = captured_json(&captured[1]);
        assert_eq!(continuation["tool_choice"], "none");
        assert!(continuation.to_string().contains("陆沉"));
        let tasks = find_turn_tasks(&fixture.handle, &turn_id).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Succeeded);
    }

    #[test]
    fn production_tool_semantic_failure_is_a_task_failure_not_a_chat_failure() {
        let fixture = Fixture::new();
        let turn_id = fixture.start_turn(100);
        let (endpoint, attempts) = spawn_sequence(vec![
            LoopbackReply::NovelistToolStreamInvalidArguments,
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
            101,
            profile(),
            |_| {},
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        assert_eq!(
            terminal.session.turns.last().unwrap().status,
            ConversationTurnStatus::Completed
        );
        let task = find_turn_tasks(&fixture.handle, &turn_id)
            .unwrap()
            .remove(0);
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(
            task.error_code.as_deref(),
            Some("agent.tool_arguments_invalid")
        );
    }

    #[test]
    fn one_tool_round_executes_once_and_keeps_task_terminal_independent() {
        let fixture = Fixture::new();
        let turn_id = fixture.start_turn(100);
        let request =
            crate::provider_core::NormalizedRequest::user_text("执行测试".to_owned(), 512).unwrap();
        let (endpoint, attempts) =
            spawn_sequence(vec![LoopbackReply::ToolStream, LoopbackReply::MainStream]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = test_registry(executions.clone());
        let mut task_statuses = Vec::new();

        let terminal = execute_agent_turn(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &request,
            &registry,
            &CancellationToken::default(),
            101,
            |event| {
                if let AgentRuntimeEvent::Task(activity) = event {
                    task_statuses.push(activity.status);
                }
            },
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert_eq!(
            task_statuses,
            vec![
                TaskStatus::Queued,
                TaskStatus::Running,
                TaskStatus::Succeeded
            ]
        );
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        assert_eq!(
            terminal.session.turns.last().unwrap().messages[1].content,
            "我先处理。主回复"
        );
        let tasks = find_turn_tasks(&fixture.handle, &turn_id).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Succeeded);
        assert_eq!(tasks[0].task_revision, 3);
        let stored =
            fs::read_to_string(fixture.root.join(".novelgainker/tasks/session.json")).unwrap();
        assert!(!stored.contains(r#""text":"ok""#));
        assert!(!stored.contains("synthetic-loopback-key"));
    }

    #[test]
    fn second_tool_call_fails_with_the_stable_round_limit_without_reexecution() {
        let fixture = Fixture::new();
        let turn_id = fixture.start_turn(100);
        let request =
            crate::provider_core::NormalizedRequest::user_text("执行测试".to_owned(), 512).unwrap();
        let (endpoint, attempts) =
            spawn_sequence(vec![LoopbackReply::ToolStream, LoopbackReply::ToolStream]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = test_registry(executions.clone());

        let terminal = execute_agent_turn(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &request,
            &registry,
            &CancellationToken::default(),
            101,
            |_| {},
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert_eq!(
            terminal.provider_error.unwrap().code,
            "agent.tool_round_limit"
        );
        assert_eq!(
            terminal.session.turns.last().unwrap().status,
            ConversationTurnStatus::Failed
        );
        let tasks = find_turn_tasks(&fixture.handle, &turn_id).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Succeeded);
    }

    #[test]
    fn schema_rejection_records_a_failed_task_and_continues_without_execution() {
        let fixture = Fixture::new();
        let turn_id = fixture.start_turn(100);
        let request =
            crate::provider_core::NormalizedRequest::user_text("执行测试".to_owned(), 512).unwrap();
        let (endpoint, attempts) = spawn_sequence(vec![
            LoopbackReply::ToolStreamInvalidArguments,
            LoopbackReply::MainStream,
        ]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = test_registry(executions.clone());
        let mut task_statuses = Vec::new();

        let terminal = execute_agent_turn(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &request,
            &registry,
            &CancellationToken::default(),
            101,
            |event| {
                if let AgentRuntimeEvent::Task(activity) = event {
                    task_statuses.push(activity.status);
                }
            },
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert_eq!(task_statuses, vec![TaskStatus::Queued, TaskStatus::Failed]);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        let task = find_turn_tasks(&fixture.handle, &turn_id)
            .unwrap()
            .remove(0);
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.task_revision, 2);
        assert_eq!(
            task.error_code.as_deref(),
            Some("agent.tool_arguments_invalid")
        );
    }

    #[test]
    fn pairable_malformed_non_object_and_oversized_arguments_fail_task_then_continue_chat() {
        for (reply, expected_error, raw) in [
            (
                LoopbackReply::ToolStreamMalformedArguments,
                "agent.tool_arguments_invalid",
                malformed_arguments().to_owned(),
            ),
            (
                LoopbackReply::ToolStreamNonObjectArguments,
                "agent.tool_arguments_invalid",
                r#"["not-an-object"]"#.to_owned(),
            ),
            (
                LoopbackReply::ToolStreamOversizedArguments,
                "agent.tool_arguments_too_large",
                oversized_arguments(),
            ),
        ] {
            let fixture = Fixture::new();
            let turn_id = fixture.start_turn(100);
            let request =
                crate::provider_core::NormalizedRequest::user_text("执行测试".to_owned(), 512)
                    .unwrap();
            let (endpoint, attempts) = spawn_sequence(vec![reply, LoopbackReply::MainStream]);
            let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
            let executions = Arc::new(AtomicUsize::new(0));
            let registry = test_registry(executions.clone());
            let mut task_statuses = Vec::new();

            let terminal = execute_agent_turn(
                &client,
                &fixture.handle,
                &turn_id,
                ProviderId::Openai,
                b"synthetic-loopback-key",
                &request,
                &registry,
                &CancellationToken::default(),
                101,
                |event| {
                    if let AgentRuntimeEvent::Task(activity) = event {
                        task_statuses.push(activity.status);
                    }
                },
            )
            .unwrap();

            assert_eq!(attempts.load(Ordering::SeqCst), 2);
            assert_eq!(executions.load(Ordering::SeqCst), 0);
            assert_eq!(task_statuses, vec![TaskStatus::Queued, TaskStatus::Failed]);
            assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
            let assistant = &terminal.session.turns.last().unwrap().messages[1].content;
            assert_eq!(assistant, "我先处理。主回复");
            assert!(!assistant.contains(crate::conversation::NOT_GENERATED_TEXT));
            assert!(!assistant.contains(crate::conversation::INCOMPLETE_TEXT));

            let task = find_turn_tasks(&fixture.handle, &turn_id)
                .unwrap()
                .remove(0);
            assert_eq!(task.status, TaskStatus::Failed);
            assert_eq!(task.task_revision, 2);
            assert_eq!(task.error_code.as_deref(), Some(expected_error));
            assert_eq!(task.arguments_bytes as usize, raw.len());
            let store =
                fs::read_to_string(fixture.root.join(".novelgainker/tasks/session.json")).unwrap();
            assert!(!store.contains("malformed-canary"));
            assert!(!store.contains(&raw));
        }
    }

    #[test]
    fn unpairable_unknown_name_bad_call_id_and_interrupted_stream_create_no_task() {
        for reply in [
            LoopbackReply::ToolStreamUnknownName,
            LoopbackReply::ToolStreamInvalidCallId,
            LoopbackReply::ToolStreamInterrupted,
        ] {
            let fixture = Fixture::new();
            let turn_id = fixture.start_turn(100);
            let request =
                crate::provider_core::NormalizedRequest::user_text("执行测试".to_owned(), 512)
                    .unwrap();
            let (endpoint, attempts) = spawn_sequence(vec![reply]);
            let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
            let executions = Arc::new(AtomicUsize::new(0));
            let registry = test_registry(executions.clone());

            let terminal = execute_agent_turn(
                &client,
                &fixture.handle,
                &turn_id,
                ProviderId::Openai,
                b"synthetic-loopback-key",
                &request,
                &registry,
                &CancellationToken::default(),
                101,
                |_| {},
            )
            .unwrap();

            assert_eq!(attempts.load(Ordering::SeqCst), 1);
            assert_eq!(executions.load(Ordering::SeqCst), 0);
            assert_eq!(
                terminal.state,
                ConversationProviderTerminalState::Incomplete
            );
            assert!(find_turn_tasks(&fixture.handle, &turn_id)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn tool_failure_is_terminal_for_task_but_attempt_two_can_complete_conversation() {
        let fixture = Fixture::new();
        let turn_id = fixture.start_turn(100);
        let request =
            crate::provider_core::NormalizedRequest::user_text("执行测试".to_owned(), 512).unwrap();
        let (endpoint, attempts) =
            spawn_sequence(vec![LoopbackReply::ToolStream, LoopbackReply::MainStream]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = failing_registry(executions.clone());
        let mut task_statuses = Vec::new();

        let terminal = execute_agent_turn(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &request,
            &registry,
            &CancellationToken::default(),
            101,
            |event| {
                if let AgentRuntimeEvent::Task(activity) = event {
                    task_statuses.push(activity.status);
                }
            },
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert_eq!(
            task_statuses,
            vec![TaskStatus::Queued, TaskStatus::Running, TaskStatus::Failed]
        );
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        let task = find_turn_tasks(&fixture.handle, &turn_id)
            .unwrap()
            .remove(0);
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.task_revision, 3);
        assert_eq!(
            task.error_code.as_deref(),
            Some("agent.tool_executor_failed")
        );
    }

    #[test]
    fn nr2_requested_output_budget_is_mechanically_bounded() {
        assert_eq!(NR2_COMPACTOR_OUTPUT_BUDGET, 1_024);
        assert_eq!(NR2_AGENT_ATTEMPT_OUTPUT_BUDGET, 4_096);
        assert_eq!(
            NR2_COMPACTOR_OUTPUT_BUDGET + 2 * NR2_AGENT_ATTEMPT_OUTPUT_BUDGET,
            NR2_CUMULATIVE_OUTPUT_BUDGET
        );
    }

    #[test]
    fn nr3_agent_loop_reserve_is_frozen_at_forty_kibibytes() {
        assert_eq!(super::NR3_AGENT_LOOP_RESERVE_BYTES, 40 * 1024);
        assert_eq!(crate::provider_core::MAX_REQUEST_BYTES, 96 * 1024);
        assert_eq!(crate::tool_runtime::MAX_TOOL_RESULT_BYTES, 24 * 1024);
    }

    #[test]
    fn cancelling_a_running_tool_acknowledges_within_two_seconds_and_skips_attempt_two() {
        let fixture = Fixture::new();
        let turn_id = fixture.start_turn(100);
        let request =
            crate::provider_core::NormalizedRequest::user_text("执行测试".to_owned(), 512).unwrap();
        let (endpoint, attempts) = spawn_sequence(vec![LoopbackReply::ToolStream]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let cancellation = CancellationToken::default();
        let worker_cancellation = cancellation.clone();
        let handle = fixture.handle.clone();
        let worker_turn_id = turn_id.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let exits = Arc::new(AtomicUsize::new(0));
        let worker_exits = exits.clone();
        let registry = cooperative_cancellation_registry(started_tx, worker_exits);
        let worker = thread::spawn(move || {
            execute_agent_turn(
                &client,
                &handle,
                &worker_turn_id,
                ProviderId::Openai,
                b"synthetic-loopback-key",
                &request,
                &registry,
                &worker_cancellation,
                101,
                |_| {},
            )
            .unwrap()
        });
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("tool executor must enter running");
        let cancelled_at = Instant::now();
        cancellation.cancel();
        let terminal = worker.join().unwrap();

        assert!(cancelled_at.elapsed() < TOOL_CANCELLATION_ACK_DEADLINE);
        assert_eq!(exits.load(Ordering::SeqCst), 1);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(terminal.state, ConversationProviderTerminalState::Cancelled);
        let task = find_turn_tasks(&fixture.handle, &turn_id)
            .unwrap()
            .remove(0);
        assert_eq!(task.status, TaskStatus::Cancelled);
        assert_eq!(task.task_revision, 3);
        assert_eq!(
            terminal.session.turns.last().unwrap().status,
            ConversationTurnStatus::Cancelled
        );
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
    fn compactor_tool_turn_uses_committed_memory_and_stays_within_three_provider_attempts() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fixture.complete_turn(index);
        }
        let turn_id = fixture.start_turn(200);
        let (endpoint, attempts, requests) = spawn_sequence_with_capture(vec![
            LoopbackReply::Completion,
            LoopbackReply::ToolStream,
            LoopbackReply::MainStream,
        ]);
        let client = ProviderHttpClient::for_test(ProviderId::Openai, endpoint).unwrap();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = test_registry(executions.clone());
        let mut task_statuses = Vec::new();

        let terminal = execute_with_profile_and_registry(
            &client,
            &fixture.handle,
            &turn_id,
            ProviderId::Openai,
            b"synthetic-loopback-key",
            &CancellationToken::default(),
            201,
            profile(),
            &registry,
            |event| {
                if let AgentRuntimeEvent::Task(activity) = event {
                    task_statuses.push(activity.status);
                }
            },
        )
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert_eq!(
            task_statuses,
            vec![
                TaskStatus::Queued,
                TaskStatus::Running,
                TaskStatus::Succeeded
            ]
        );
        assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
        let turn = terminal.session.turns.last().unwrap();
        assert_eq!(turn.messages.len(), 2);
        assert_eq!(turn.messages[1].content, "我先处理。主回复");
        assert!(!turn.messages[1].content.contains("已确认决定"));

        let memory = load_valid_memory(&fixture.handle, &terminal.session, profile())
            .expect("compactor must commit a valid Memory");
        assert_eq!(memory.summary_text, summary());
        assert_eq!(memory.updated_at_ms, 201);

        let captured = requests.lock().unwrap();
        assert_eq!(captured.len(), 3);
        let compactor = captured_json(&captured[0]);
        let initial = captured_json(&captured[1]);
        let continuation = captured_json(&captured[2]);
        assert_eq!(
            compactor["instructions"],
            crate::memory::COMPACTOR_SYSTEM_INSTRUCTION
        );
        let initial_instructions = initial["instructions"].as_str().unwrap();
        assert!(initial_instructions.contains(summary()));
        assert_eq!(initial_instructions.matches(summary()).count(), 1);
        assert!(!initial.to_string().contains("用户决定 0"));
        assert_eq!(initial["tools"].as_array().unwrap().len(), 1);
        assert_eq!(continuation["instructions"], initial["instructions"]);
        assert_eq!(continuation["tool_choice"], "none");
        assert!(continuation["input"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["type"] == "function_call_output"));

        let tasks = find_turn_tasks(&fixture.handle, &turn_id).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Succeeded);
        assert_eq!(tasks[0].task_revision, 3);
        assert_eq!(
            read_conversation(&fixture.handle).unwrap(),
            terminal.session
        );
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
