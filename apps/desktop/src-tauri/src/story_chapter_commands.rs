use super::*;
use crate::commands::story_context_commands::{
    build_registered_story_generation_material, rebuild_registered_story_context,
    StoryContextBuildRequestV2, StoryContextRequestPreviewRequestV1,
};
use crate::commands::story_continuity_commands::load_story_preview_target;
use novelgainker_workflows::chapter::{
    build_chapter_proposal, build_chapter_repair_prompt, parse_chapter_payload,
    ChapterProposalPayloadV1, ChapterProposalV1,
};
use novelgainker_workflows::continuity::ContinuityTargetStateV1;
use novelgainker_workflows::story_context::STORY_MAX_OUTPUT_TOKENS;

pub const STORY_CHAPTER_RUN_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StoryChapterRunStateV1 {
    Generating,
    Repairing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryChapterRunSnapshotV1 {
    pub schema_version: u16,
    pub run_id: String,
    #[serde(skip_serializing)]
    pub session_id: String,
    pub state: StoryChapterRunStateV1,
    pub provider_id: String,
    pub model_id: String,
    pub source_manifest_hash: String,
    pub request_hash: String,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub repair_attempted: bool,
    pub can_cancel: bool,
    pub can_retry: bool,
    pub proposal: Option<ChapterProposalV1>,
    pub invalid_response_preview: Option<String>,
    pub error: Option<AppError>,
    #[serde(skip_serializing)]
    pub(super) generation_target: ContinuityTargetStateV1,
}

impl std::fmt::Debug for StoryChapterRunSnapshotV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoryChapterRunSnapshotV1")
            .field("schema_version", &self.schema_version)
            .field("run_id", &self.run_id)
            .field("session_id", &self.session_id)
            .field("state", &self.state)
            .field("provider_id", &self.provider_id)
            .field("model_id", &self.model_id)
            .field("source_manifest_hash", &self.source_manifest_hash)
            .field("request_hash", &self.request_hash)
            .field("started_at", &self.started_at)
            .field("completed_at", &self.completed_at)
            .field("repair_attempted", &self.repair_attempted)
            .field("proposal", &self.proposal.as_ref().map(|_| "[REDACTED]"))
            .field(
                "invalid_response_preview",
                &self.invalid_response_preview.as_ref().map(|_| "[REDACTED]"),
            )
            .field("error", &self.error)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryChapterStatusSnapshotV1 {
    schema_version: u16,
    run: Option<StoryChapterRunSnapshotV1>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryChapterGenerateRequestV1 {
    schema_version: u16,
    request: StoryContextRequestPreviewRequestV1,
    expected_request_hash: String,
    expected_provider_id: Option<ProviderId>,
    confirmed: bool,
}

enum StoryChapterGenerationOutcome {
    Completed(Box<ChapterProposalV1>),
    Failed {
        error: AppError,
        invalid_response_preview: Option<String>,
        repair_attempted: bool,
    },
    Cancelled,
}

struct StoryChapterCompletionContext {
    provider_id: ProviderId,
    model_id: String,
    snapshot: novelgainker_workflows::story_context::StoryContextRequestSnapshotV1,
    context_input: StoryContextBuildRequestV2,
    started_at: i64,
}

struct StoryChapterGenerationTask {
    app: tauri::AppHandle,
    run_id: String,
    session_id: String,
    request: NormalizedRequest,
    cancellation: CancellationToken,
    completion: StoryChapterCompletionContext,
}

#[tauri::command]
pub fn story_chapter_generate_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryChapterGenerateRequestV1,
) -> Result<StoryChapterRunSnapshotV1, AppError> {
    if input.schema_version != STORY_CHAPTER_RUN_SCHEMA_VERSION {
        return Err(AppError::workflow_input_invalid());
    }
    if !input.confirmed {
        return Err(AppError::workflow_confirmation_required());
    }
    if state.active_provider_id().is_some() {
        return Err(AppError::provider_run_active());
    }
    if workflow_run_active(&state)? {
        return Err(AppError::workflow_run_active());
    }
    let material =
        build_registered_story_generation_material(&state, &session_id, input.request.clone())?;
    if material.snapshot.request_hash != input.expected_request_hash
        || material.snapshot.source_review.status
            != novelgainker_workflows::story_context::StorySourceReviewStatusV1::Current
    {
        return Err(AppError::workflow_context_changed());
    }
    let provider_state = load_provider_state(&state.app_state, state.credential_vault.as_ref())?;
    let generation_target =
        load_story_preview_target(&state, &session_id, &material.snapshot.request_hash);
    let selected = provider_state
        .providers
        .iter()
        .find(|provider| provider.selected)
        .ok_or_else(AppError::provider_not_ready)?;
    if input
        .expected_provider_id
        .is_some_and(|expected| expected != selected.provider_id)
        || !selected.configured
        || selected.connection_status != ProviderConnectionStatus::Connected
    {
        return Err(AppError::provider_not_ready());
    }
    let provider_id = selected.provider_id;
    let model_id = preset(provider_id).model.to_owned();
    let request = NormalizedRequest::chapter_proposal_json(
        material.prompt.messages,
        STORY_MAX_OUTPUT_TOKENS,
        novelgainker_workflows::storytelling_chapter_proposal_output_schema(),
    )
    .map_err(AppError::from_provider_error)?;
    let run_id = Uuid::new_v4().to_string();
    let cancellation = CancellationToken::default();
    state.begin_provider_run(provider_id, run_id.clone(), cancellation.clone())?;
    let snapshot = StoryChapterRunSnapshotV1 {
        schema_version: STORY_CHAPTER_RUN_SCHEMA_VERSION,
        run_id: run_id.clone(),
        session_id: session_id.clone(),
        state: StoryChapterRunStateV1::Generating,
        provider_id: provider_id.as_str().to_owned(),
        model_id: model_id.clone(),
        source_manifest_hash: material.snapshot.manifest.aggregate_hash.clone(),
        request_hash: material.snapshot.request_hash.clone(),
        started_at: now_unix_seconds(),
        completed_at: None,
        repair_attempted: false,
        can_cancel: true,
        can_retry: false,
        proposal: None,
        invalid_response_preview: None,
        error: None,
        generation_target,
    };
    state
        .story_continuity_reports
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    state
        .story_chapter_patches
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    if state
        .story_chapter_runs
        .lock()
        .map(|mut runs| runs.insert(session_id.clone(), snapshot.clone()))
        .is_err()
    {
        state.finish_provider_run(&run_id);
        return Err(AppError::workflow_unavailable());
    }
    tauri::async_runtime::spawn_blocking(move || {
        execute_story_chapter_generation(StoryChapterGenerationTask {
            app,
            run_id,
            session_id,
            request,
            cancellation,
            completion: StoryChapterCompletionContext {
                provider_id,
                model_id,
                snapshot: material.snapshot,
                context_input: input.request.context,
                started_at: snapshot.started_at,
            },
        });
    });
    Ok(snapshot)
}

#[tauri::command]
pub fn story_chapter_status_load(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<StoryChapterStatusSnapshotV1, AppError> {
    session_handle(&state, &session_id)?;
    let run = state
        .story_chapter_runs
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(&session_id)
        .cloned();
    Ok(StoryChapterStatusSnapshotV1 {
        schema_version: STORY_CHAPTER_RUN_SCHEMA_VERSION,
        run,
    })
}

#[tauri::command]
pub fn story_chapter_generate_cancel(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    run_id: String,
) -> Result<StoryChapterRunSnapshotV1, AppError> {
    let current = state
        .story_chapter_runs
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(&session_id)
        .filter(|run| run.run_id == run_id && run.can_cancel)
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    state.cancel_provider_run(&current.run_id)?;
    Ok(current)
}

fn execute_story_chapter_generation(task: StoryChapterGenerationTask) {
    let StoryChapterGenerationTask {
        app,
        run_id,
        session_id,
        request,
        cancellation,
        completion,
    } = task;
    let provider_id = completion.provider_id;
    let state = app.state::<ProjectRegistry>();
    let client = match ProviderHttpClient::production() {
        Ok(client) => client,
        Err(error) => {
            state.finish_provider_run(&run_id);
            finalize_story_chapter_run(
                &state,
                &session_id,
                &run_id,
                StoryChapterGenerationOutcome::Failed {
                    error: AppError::from_provider_error(error),
                    invalid_response_preview: None,
                    repair_attempted: false,
                },
            );
            return;
        }
    };
    let mut generation_outcome = None;
    let vault_result = state.credential_vault.with_secret(provider_id, &mut |key| {
        generation_outcome = Some(
            match client.stream(provider_id, key, &request, &cancellation, &mut |_| Ok(())) {
                Ok(mut response) => {
                    let complete = response.finish_reason == FinishReason::Stop;
                    let first_preview = response_preview(&response.text);
                    match parse_chapter_payload(
                        &response.text,
                        complete,
                        &completion.snapshot.manifest.source_order,
                    ) {
                        Ok(payload) => {
                            wipe_sensitive_string(&mut response.text);
                            complete_story_chapter_proposal(
                                &state,
                                &session_id,
                                &completion,
                                payload,
                            )
                        }
                        Err(_) if !complete => {
                            wipe_sensitive_string(&mut response.text);
                            StoryChapterGenerationOutcome::Failed {
                                error: AppError::workflow_output_invalid(),
                                invalid_response_preview: first_preview,
                                repair_attempted: false,
                            }
                        }
                        Err(first_error) => {
                            mark_story_chapter_repairing(&state, &session_id, &run_id);
                            let repair_prompt =
                                build_chapter_repair_prompt(first_error, response.text.as_str());
                            wipe_sensitive_string(&mut response.text);
                            let repair_request = NormalizedRequest::chapter_proposal_json(
                                vec![repair_prompt],
                                STORY_MAX_OUTPUT_TOKENS,
                                novelgainker_workflows::storytelling_chapter_proposal_output_schema(
                                ),
                            );
                            match repair_request.and_then(|repair_request| {
                                client.stream(
                                    provider_id,
                                    key,
                                    &repair_request,
                                    &cancellation,
                                    &mut |_| Ok(()),
                                )
                            }) {
                                Ok(mut repaired) => {
                                    let parsed = parse_chapter_payload(
                                        &repaired.text,
                                        repaired.finish_reason == FinishReason::Stop,
                                        &completion.snapshot.manifest.source_order,
                                    );
                                    wipe_sensitive_string(&mut repaired.text);
                                    match parsed {
                                        Ok(payload) => complete_story_chapter_proposal(
                                            &state,
                                            &session_id,
                                            &completion,
                                            payload,
                                        ),
                                        Err(_) => StoryChapterGenerationOutcome::Failed {
                                            error: AppError::workflow_repair_failed(),
                                            invalid_response_preview: first_preview,
                                            repair_attempted: true,
                                        },
                                    }
                                }
                                Err(error) if error.kind == ProviderErrorKind::Cancelled => {
                                    StoryChapterGenerationOutcome::Cancelled
                                }
                                Err(error) => StoryChapterGenerationOutcome::Failed {
                                    error: AppError::from_provider_error(error),
                                    invalid_response_preview: first_preview,
                                    repair_attempted: true,
                                },
                            }
                        }
                    }
                }
                Err(error) if error.kind == ProviderErrorKind::Cancelled => {
                    StoryChapterGenerationOutcome::Cancelled
                }
                Err(error) => StoryChapterGenerationOutcome::Failed {
                    error: AppError::from_provider_error(error),
                    invalid_response_preview: None,
                    repair_attempted: false,
                },
            },
        );
        Ok(())
    });
    state.finish_provider_run(&run_id);
    let outcome = match vault_result {
        Err(error) => StoryChapterGenerationOutcome::Failed {
            error,
            invalid_response_preview: None,
            repair_attempted: false,
        },
        Ok(()) => generation_outcome.unwrap_or(StoryChapterGenerationOutcome::Failed {
            error: AppError::provider_vault_unavailable(),
            invalid_response_preview: None,
            repair_attempted: false,
        }),
    };
    finalize_story_chapter_run(&state, &session_id, &run_id, outcome);
}

fn complete_story_chapter_proposal(
    state: &ProjectRegistry,
    session_id: &str,
    completion: &StoryChapterCompletionContext,
    payload: ChapterProposalPayloadV1,
) -> StoryChapterGenerationOutcome {
    let current_manifest =
        match rebuild_registered_story_context(state, session_id, completion.context_input.clone())
        {
            Ok(manifest) => manifest,
            Err(error) => {
                return StoryChapterGenerationOutcome::Failed {
                    error,
                    invalid_response_preview: None,
                    repair_attempted: false,
                }
            }
        };
    match build_chapter_proposal(
        &completion.snapshot,
        payload,
        current_manifest.aggregate_hash,
        completion.provider_id.as_str().to_owned(),
        completion.model_id.clone(),
        completion.started_at.max(0) as u64,
        now_unix_seconds().max(0) as u64,
        "stop".to_owned(),
    ) {
        Ok(proposal) => StoryChapterGenerationOutcome::Completed(Box::new(proposal)),
        Err(_) => StoryChapterGenerationOutcome::Failed {
            error: AppError::workflow_output_invalid(),
            invalid_response_preview: None,
            repair_attempted: false,
        },
    }
}

fn mark_story_chapter_repairing(state: &ProjectRegistry, session_id: &str, run_id: &str) {
    let Ok(mut runs) = state.story_chapter_runs.lock() else {
        return;
    };
    if let Some(run) = runs
        .get_mut(session_id)
        .filter(|run| run.run_id == run_id && run.state == StoryChapterRunStateV1::Generating)
    {
        run.state = StoryChapterRunStateV1::Repairing;
        run.repair_attempted = true;
    }
}

fn finalize_story_chapter_run(
    state: &ProjectRegistry,
    session_id: &str,
    run_id: &str,
    outcome: StoryChapterGenerationOutcome,
) {
    let Ok(mut runs) = state.story_chapter_runs.lock() else {
        return;
    };
    let Some(run) = runs.get_mut(session_id).filter(|run| run.run_id == run_id) else {
        return;
    };
    run.completed_at = Some(now_unix_seconds());
    run.can_cancel = false;
    match outcome {
        StoryChapterGenerationOutcome::Completed(proposal) => {
            run.state = StoryChapterRunStateV1::Completed;
            run.can_retry = false;
            run.proposal = Some(*proposal);
            run.invalid_response_preview = None;
            run.error = None;
        }
        StoryChapterGenerationOutcome::Failed {
            error,
            invalid_response_preview,
            repair_attempted,
        } => {
            run.state = StoryChapterRunStateV1::Failed;
            run.can_retry = true;
            run.repair_attempted |= repair_attempted;
            run.proposal = None;
            run.invalid_response_preview = invalid_response_preview;
            run.error = Some(error);
        }
        StoryChapterGenerationOutcome::Cancelled => {
            run.state = StoryChapterRunStateV1::Cancelled;
            run.can_retry = true;
            run.proposal = None;
            run.invalid_response_preview = None;
            run.error = None;
        }
    }
}

fn response_preview(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    let mut end = value.len().min(4 * 1024);
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    Some(value[..end].to_owned())
}

fn wipe_sensitive_string(value: &mut String) {
    unsafe {
        for byte in value.as_bytes_mut() {
            std::ptr::write_volatile(byte, 0);
        }
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    value.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ng_503_run_debug_redacts_draft_and_invalid_provider_output() {
        let run = StoryChapterRunSnapshotV1 {
            schema_version: 1,
            run_id: "run_fixture".to_owned(),
            session_id: "session_fixture".to_owned(),
            state: StoryChapterRunStateV1::Failed,
            provider_id: "deepseek".to_owned(),
            model_id: "deepseek-v4-pro".to_owned(),
            source_manifest_hash: "a".repeat(64),
            request_hash: "b".repeat(64),
            started_at: 1,
            completed_at: Some(2),
            repair_attempted: true,
            can_cancel: false,
            can_retry: true,
            proposal: None,
            invalid_response_preview: Some("PRIVATE_PROVIDER_CANARY".to_owned()),
            error: Some(AppError::workflow_repair_failed()),
            generation_target: ContinuityTargetStateV1::missing(),
        };
        assert!(!format!("{run:?}").contains("PRIVATE_PROVIDER_CANARY"));
        let serialized = serde_json::to_value(&run).unwrap();
        assert!(serialized.get("sessionId").is_none());
        assert_eq!(serialized["canRetry"], true);
    }
}
