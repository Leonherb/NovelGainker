use super::*;
use app_core::app_state::{RouterRunMetadata, RouterRunStatus};
use novelgainker_workflows::router::{
    build_router_prompt, build_router_repair_prompt, parse_router_decision, resolve_local_router,
    router_decision_schema, LocalRouterDecisionV1, LocalRouterOutcome, RouterDecisionV1,
    RouterInputError,
};

pub const ROUTER_RUN_EVENT_CHANNEL: &str = "router-run-event";
const ROUTER_MAX_OUTPUT_TOKENS: u32 = 256;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterRunStarted {
    pub run_id: String,
    pub provider_id: ProviderId,
    pub model: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RouterRunEventBody {
    Completed {
        decision: RouterDecisionV1,
        usage: Option<Usage>,
    },
    Failed {
        error: AppError,
    },
    Cancelled,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterRunEvent {
    pub run_id: String,
    pub provider_id: ProviderId,
    pub model: String,
    #[serde(flatten)]
    pub event: RouterRunEventBody,
}

#[tauri::command]
pub fn router_resolve_local(text: String) -> Result<LocalRouterDecisionV1, AppError> {
    resolve_local_router(&text).map_err(router_input_error)
}

#[tauri::command]
pub fn router_run_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProjectRegistry>,
    text: String,
    expected_provider_id: ProviderId,
) -> Result<RouterRunStarted, AppError> {
    if state.active_provider_id().is_some() {
        return Err(AppError::provider_run_active());
    }
    if resolve_local_router(&text)
        .map_err(router_input_error)?
        .outcome
        != LocalRouterOutcome::NeedsProvider
    {
        return Err(AppError::from_provider_error(
            ProviderError::invalid_request(),
        ));
    }
    if workflow_generation_active(&state)? {
        return Err(AppError::workflow_run_active());
    }
    let snapshot = load_provider_state(&state.app_state, state.credential_vault.as_ref())?;
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
    let prompt = build_router_prompt(&text).map_err(router_input_error)?;
    let request = NormalizedRequest::router_decision_json(
        prompt,
        ROUTER_MAX_OUTPUT_TOKENS,
        router_decision_schema(),
    )
    .map_err(AppError::from_provider_error)?;
    let provider_id = selected.provider_id;
    let run_id = Uuid::new_v4().to_string();
    let cancellation = CancellationToken::default();
    state.begin_provider_run(provider_id, run_id.clone(), cancellation.clone())?;
    let model = preset(provider_id).model.to_owned();
    let worker_run_id = run_id.clone();
    let worker_model = model.clone();
    tauri::async_runtime::spawn_blocking(move || {
        execute_router_run(
            app,
            worker_run_id,
            provider_id,
            worker_model,
            text,
            request,
            cancellation,
        );
    });
    Ok(RouterRunStarted {
        run_id,
        provider_id,
        model,
    })
}

#[tauri::command]
pub fn router_run_cancel(
    state: tauri::State<'_, ProjectRegistry>,
    run_id: String,
) -> Result<ProviderRunCancelResult, AppError> {
    state.cancel_provider_run(&run_id)?;
    Ok(ProviderRunCancelResult {
        cancel_requested: true,
    })
}

fn execute_router_run(
    app: tauri::AppHandle,
    run_id: String,
    provider_id: ProviderId,
    model: String,
    mut original_text: String,
    request: NormalizedRequest,
    cancellation: CancellationToken,
) {
    let state = app.state::<ProjectRegistry>();
    let client = match ProviderHttpClient::production() {
        Ok(client) => client,
        Err(error) => {
            finish_router_run(
                &app,
                &state,
                &run_id,
                provider_id,
                &model,
                None,
                None,
                Err(AppError::from_provider_error(error)),
            );
            wipe_string(&mut original_text);
            return;
        }
    };

    let mut provider_result: Option<Result<(RouterDecisionV1, Option<Usage>), AppError>> = None;
    let vault_result = state.credential_vault.with_secret(provider_id, &mut |key| {
        let first = client.complete_once(provider_id, key, &request, &cancellation);
        provider_result = Some(match first {
            Err(error) => Err(AppError::from_provider_error(error)),
            Ok(mut response) => {
                let first_usage = response.usage;
                let first_decision = parse_router_decision(&response.text);
                wipe_string(&mut response.text);
                match first_decision {
                    Ok(decision) => Ok((decision, first_usage)),
                    Err(output_error) => {
                        let repair_prompt =
                            build_router_repair_prompt(&original_text, output_error)
                                .map_err(router_input_error)?;
                        let repair_request = NormalizedRequest::router_decision_json(
                            repair_prompt,
                            ROUTER_MAX_OUTPUT_TOKENS,
                            router_decision_schema(),
                        )
                        .map_err(AppError::from_provider_error)?;
                        let mut repaired = client
                            .complete_once(provider_id, key, &repair_request, &cancellation)
                            .map_err(AppError::from_provider_error)?;
                        let usage = sum_usage(first_usage, repaired.usage);
                        let decision = parse_router_decision(&repaired.text)
                            .map_err(|_| AppError::invalid_runtime_response());
                        wipe_string(&mut repaired.text);
                        decision.map(|decision| (decision, usage))
                    }
                }
            }
        });
        Ok(())
    });
    wipe_string(&mut original_text);

    let result = match vault_result {
        Err(error) => Err(error),
        Ok(()) => provider_result.unwrap_or_else(|| Err(AppError::provider_vault_unavailable())),
    };
    match result {
        Ok((decision, usage)) => finish_router_run(
            &app,
            &state,
            &run_id,
            provider_id,
            &model,
            Some(decision),
            usage,
            Ok(()),
        ),
        Err(error) => finish_router_run(
            &app,
            &state,
            &run_id,
            provider_id,
            &model,
            None,
            None,
            Err(error),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_router_run(
    app: &tauri::AppHandle,
    state: &ProjectRegistry,
    run_id: &str,
    provider_id: ProviderId,
    model: &str,
    decision: Option<RouterDecisionV1>,
    usage: Option<Usage>,
    result: Result<(), AppError>,
) {
    let cancelled = result
        .as_ref()
        .is_err_and(|error| error.code == "provider.cancelled");
    let status = if decision.is_some() {
        RouterRunStatus::Completed
    } else if cancelled {
        RouterRunStatus::Cancelled
    } else {
        RouterRunStatus::Failed
    };
    let metadata = RouterRunMetadata {
        run_id: run_id.to_owned(),
        provider_id,
        workflow_id: decision.as_ref().map(|value| value.workflow_id.clone()),
        confidence: decision
            .as_ref()
            .map(|value| value.confidence.as_str().to_owned()),
        status,
        usage,
        updated_at: now_unix_seconds(),
    };
    let persisted = state.app_state.save_router_run(&metadata);
    let event = if persisted.is_err() {
        RouterRunEventBody::Failed {
            error: AppError::app_state_unavailable(),
        }
    } else if let Some(decision) = decision {
        RouterRunEventBody::Completed { decision, usage }
    } else if cancelled {
        RouterRunEventBody::Cancelled
    } else {
        RouterRunEventBody::Failed {
            error: result.expect_err("failed Router run must carry an error"),
        }
    };
    let _ = app.emit(
        ROUTER_RUN_EVENT_CHANNEL,
        RouterRunEvent {
            run_id: run_id.to_owned(),
            provider_id,
            model: model.to_owned(),
            event,
        },
    );
    state.finish_provider_run(run_id);
}

fn sum_usage(first: Option<Usage>, second: Option<Usage>) -> Option<Usage> {
    match (first, second) {
        (None, None) => None,
        (first, second) => {
            let first = first.unwrap_or(Usage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            });
            let second = second.unwrap_or(Usage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            });
            Some(Usage {
                input_tokens: first.input_tokens.saturating_add(second.input_tokens),
                output_tokens: first.output_tokens.saturating_add(second.output_tokens),
                total_tokens: first.total_tokens.saturating_add(second.total_tokens),
            })
        }
    }
}

fn router_input_error(_error: RouterInputError) -> AppError {
    AppError::from_provider_error(ProviderError::invalid_request())
}

fn wipe_string(value: &mut String) {
    unsafe {
        value.as_bytes_mut().fill(0);
    }
    value.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_commands_never_require_provider_start() {
        let local = router_resolve_local("创建主剧情".to_owned()).unwrap();
        assert_eq!(local.outcome, LocalRouterOutcome::Direct);
        assert_eq!(local.workflow_id.as_deref(), Some(PLOT_CREATE_WORKFLOW_ID));
    }

    #[test]
    fn usage_aggregation_is_bounded_and_optional() {
        assert_eq!(sum_usage(None, None), None);
        assert_eq!(
            sum_usage(
                Some(Usage {
                    input_tokens: 10,
                    output_tokens: 2,
                    total_tokens: 12,
                }),
                Some(Usage {
                    input_tokens: 6,
                    output_tokens: 1,
                    total_tokens: 7,
                }),
            ),
            Some(Usage {
                input_tokens: 16,
                output_tokens: 3,
                total_tokens: 19,
            })
        );
    }
}
