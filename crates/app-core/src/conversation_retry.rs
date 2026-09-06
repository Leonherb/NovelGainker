//! Bounded Host accounting, never a Provider continuation or server cursor.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryOutcome {
    Running,
    Eligible,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryState {
    pub attempt_id: String,
    pub attempt_number: u8,
    pub agent_requests: u8,
    pub compactor_requests: u8,
    pub requested_output_tokens: u32,
    pub tool_dispatched: bool,
    pub outcome: RetryOutcome,
}
#[derive(Clone, Copy)]
pub(crate) enum RequestKind {
    Agent,
    Compactor,
}

pub fn retry_state(handle: &ProjectHandle, turn_id: &str) -> Result<Option<RetryState>, AppError> {
    let session = read_or_new(handle)?;
    let turn = session
        .turns
        .iter()
        .find(|turn| turn.turn_id == turn_id)
        .ok_or_else(|| conversation_error("conversation.turn_not_found"))?;
    Ok(turn.retry_state.clone())
}

pub fn initialize_attempt(handle: &ProjectHandle, turn_id: &str) -> Result<RetryState, AppError> {
    let mut session = read_or_new(handle)?;
    let turn = session
        .turns
        .last_mut()
        .filter(|turn| {
            turn.turn_id == turn_id
                && turn.status == ConversationTurnStatus::Running
                && turn.retry_state.is_none()
        })
        .ok_or_else(|| conversation_error("conversation.retry_unavailable"))?;
    let state = RetryState {
        attempt_id: Uuid::new_v4().to_string(),
        attempt_number: 1,
        agent_requests: 0,
        compactor_requests: 0,
        requested_output_tokens: 0,
        tool_dispatched: false,
        outcome: RetryOutcome::Running,
    };
    turn.retry_state = Some(state.clone());
    validate_session(&session, handle.project_id())?;
    write_session(handle, &session)?;
    Ok(state)
}

// Caller holds the physical writer lease and the exclusive project run reservation.
// The old attempt ID is the CAS token; a lost acknowledgement cannot launch twice.
pub fn begin_retry(
    handle: &ProjectHandle,
    turn_id: &str,
    expected_attempt_id: &str,
    now_ms: u64,
) -> Result<ConversationTurn, AppError> {
    let mut session = read_or_new(handle)?;
    let turn = session
        .turns
        .last_mut()
        .filter(|turn| turn.turn_id == turn_id)
        .ok_or_else(|| conversation_error("conversation.retry_unavailable"))?;
    let state = turn
        .retry_state
        .as_mut()
        .ok_or_else(|| conversation_error("conversation.retry_unavailable"))?;
    if state.attempt_id != expected_attempt_id
        || state.outcome != RetryOutcome::Eligible
        || state.attempt_number != 1
        || state.tool_dispatched
        || state.agent_requests == 0
        || state.agent_requests >= 8
        || state.requested_output_tokens > 33792 - 4096
        || turn.status != ConversationTurnStatus::NotGenerated
        || turn.messages.len() != 2
        || turn.messages[1].content != NOT_GENERATED_TEXT
        || now_ms == 0
    {
        return Err(conversation_error("conversation.retry_unavailable"));
    }
    state.attempt_id = Uuid::new_v4().to_string();
    state.attempt_number = 2;
    state.outcome = RetryOutcome::Running;
    turn.messages.truncate(1); // Only the verified Host zero-text placeholder.
    turn.status = ConversationTurnStatus::Running;
    turn.error_code = None;
    turn.updated_at_ms = now_ms.max(turn.updated_at_ms);
    let result = turn.clone();
    validate_session(&session, handle.project_id())?;
    write_session(handle, &session)?;
    Ok(result)
}

// A failed reservation never permits HTTP. Legacy core fixtures are deliberately
// untracked; every production start initializes tracking before spawning a worker.
pub(crate) fn reserve_request(
    handle: &ProjectHandle,
    turn_id: &str,
    kind: RequestKind,
    tokens: u32,
) -> Result<bool, ProviderError> {
    let reserve = || -> Result<bool, AppError> {
        let mut session = read_or_new(handle)?;
        let turn = session
            .turns
            .last_mut()
            .filter(|turn| {
                turn.turn_id == turn_id && turn.status == ConversationTurnStatus::Running
            })
            .ok_or_else(|| conversation_error("conversation.retry_unavailable"))?;
        let Some(state) = &mut turn.retry_state else {
            return Ok(true);
        };
        if state.outcome != RetryOutcome::Running || tokens == 0 {
            return Err(conversation_error("conversation.retry_unavailable"));
        }
        match kind {
            RequestKind::Compactor if state.compactor_requests == 1 => return Ok(false),
            RequestKind::Compactor if tokens <= 1024 => state.compactor_requests += 1,
            RequestKind::Agent if state.agent_requests < 8 && tokens <= 4096 => {
                state.agent_requests += 1
            }
            _ => return Err(conversation_error("conversation.retry_budget")),
        }
        state.requested_output_tokens = state
            .requested_output_tokens
            .checked_add(tokens)
            .filter(|count| *count <= 33792)
            .ok_or_else(|| conversation_error("conversation.retry_budget"))?;
        validate_session(&session, handle.project_id())?;
        write_session(handle, &session)?;
        Ok(true)
    };
    reserve().map_err(|_| ProviderError::protocol())
}
pub(crate) fn tool_dispatched(handle: &ProjectHandle, turn_id: &str) -> Result<(), AppError> {
    let mut session = read_or_new(handle)?;
    let turn = session
        .turns
        .last_mut()
        .filter(|turn| turn.turn_id == turn_id && turn.status == ConversationTurnStatus::Running)
        .ok_or_else(|| conversation_error("conversation.retry_unavailable"))?;
    let Some(state) = &mut turn.retry_state else {
        return Ok(());
    };
    if state.outcome != RetryOutcome::Running {
        return Err(conversation_error("conversation.retry_unavailable"));
    }
    state.tool_dispatched = true; // Reserve before Task creation or any executor.
    write_session(handle, &session)
}
pub(super) fn terminalize_retry(
    turn: &mut ConversationTurn,
    text: &str,
    kind: ConversationTerminalKind,
) {
    if let Some(state) = &mut turn.retry_state {
        state.outcome = if kind == ConversationTerminalKind::Failed
            && text.is_empty()
            && state.outcome == RetryOutcome::Running
            && state.attempt_number == 1
            && !state.tool_dispatched
            && state.agent_requests > 0
            && state.agent_requests < 8
            && state.requested_output_tokens <= 33792 - 4096
            && matches!(
                turn.error_code.as_deref(),
                Some("provider.timeout" | "provider.unreachable" | "provider.rate_limited")
            ) {
            RetryOutcome::Eligible
        } else {
            RetryOutcome::Blocked
        };
    }
}
pub(super) fn validate_retry(turn: &ConversationTurn) -> Result<(), AppError> {
    let Some(state) = &turn.retry_state else {
        return Ok(());
    };
    if Uuid::parse_str(&state.attempt_id).is_err()
        || !(1..=2).contains(&state.attempt_number)
        || state.agent_requests > 8
        || state.compactor_requests > 1
        || state.requested_output_tokens
            > u32::from(state.agent_requests) * 4096 + u32::from(state.compactor_requests) * 1024
        || state.requested_output_tokens
            < u32::from(state.agent_requests + state.compactor_requests)
        || (state.outcome == RetryOutcome::Running)
            != (turn.status == ConversationTurnStatus::Running)
        || (state.outcome == RetryOutcome::Eligible
            && (state.attempt_number != 1
                || state.tool_dispatched
                || turn.status != ConversationTurnStatus::NotGenerated
                || state.agent_requests == 0
                || state.agent_requests >= 8
                || state.requested_output_tokens > 33792 - 4096
                || turn
                    .messages
                    .get(1)
                    .is_none_or(|message| message.content != NOT_GENERATED_TEXT)
                || !matches!(
                    turn.error_code.as_deref(),
                    Some("provider.timeout" | "provider.unreachable" | "provider.rate_limited")
                )))
    {
        return Err(conversation_error("conversation.store_invalid"));
    }
    Ok(())
}
