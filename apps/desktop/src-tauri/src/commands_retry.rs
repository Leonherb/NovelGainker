#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationRetryRequest {
    project_session_id: String,
    conversation_session_id: String,
    turn_id: String,
    expected_attempt_id: String,
}

#[tauri::command]
pub fn conversation_turn_retry<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ProjectRegistry>,
    request: ConversationRetryRequest,
) -> Result<ConversationTurnStartResult, AppError> {
    let history_lease = require_session_history_lease(&state, &request.project_session_id)?;
    reconcile_session_run(&state, &request.project_session_id)?;
    let handle = selected_session_handle(
        &state,
        &request.project_session_id,
        Some(&request.conversation_session_id),
    )?;
    let conversation = read_conversation(&handle)?;
    let original = conversation
        .turns
        .last()
        .filter(|turn| turn.turn_id == request.turn_id)
        .ok_or_else(|| session_export_error("conversation.retry_unavailable"))?;
    let provider_id = original.provider_id;
    let model = original.model.clone();
    if model != preset(provider_id).model {
        return Err(session_export_error("conversation.retry_unavailable"));
    }
    let providers = load_provider_state(&state.app_state, state.credential_vault.as_ref())?;
    if !providers
        .providers
        .iter()
        .any(|p| p.provider_id == provider_id && p.configured)
    {
        return Err(AppError::provider_not_ready());
    }
    let cancellation = state.reserve_provider_run(
        &handle.coordination_key(),
        &request.conversation_session_id,
        provider_id,
    )?;
    let reservation = match sessions::reserve_run(&handle, now_ms()?) {
        Ok(value) => value,
        Err(error) => {
            state.release_provider_reservation();
            return Err(error);
        }
    };
    let started = match app_core::conversation::begin_retry(
        &handle,
        &request.turn_id,
        &request.expected_attempt_id,
        now_ms()?,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ = sessions::release_run(&handle, &reservation.reservation_id);
            state.release_provider_reservation();
            return Err(error);
        }
    };
    if let Err(error) =
        sessions::bind_run_turn(&handle, &reservation.reservation_id, &started.turn_id)
            .and_then(|_| state.bind_provider_turn(&started.turn_id))
            .and_then(|_| {
                state.bind_provider_attempt(
                    &started.turn_id,
                    started
                        .retry_state
                        .as_ref()
                        .ok_or_else(AppError::runtime_unavailable)?,
                )
            })
    {
        let _ = finish_conversation_turn(
            &handle,
            &started.turn_id,
            "",
            ConversationTerminalKind::Failed,
            Some(&error.code),
            now_ms().unwrap_or(started.updated_at_ms),
        );
        let _ = sessions::release_run(&handle, &reservation.reservation_id);
        state.finish_provider_run(&started.turn_id);
        state.release_provider_reservation();
        return Err(error);
    }
    let attempt = started
        .retry_state
        .ok_or_else(AppError::runtime_unavailable)?;
    let result = ConversationTurnStartResult {
        session_id: request.project_session_id.clone(),
        conversation_session_id: request.conversation_session_id.clone(),
        turn_id: started.turn_id.clone(),
        provider_id,
        model: model.clone(),
        attempt_id: attempt.attempt_id.clone(),
        attempt_number: 2,
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _history_lease = history_lease;
        run_conversation_provider(
            app,
            handle,
            request.project_session_id,
            request.conversation_session_id,
            reservation.reservation_id,
            started.turn_id,
            attempt,
            provider_id,
            model,
            cancellation,
        );
    });
    Ok(result)
}
