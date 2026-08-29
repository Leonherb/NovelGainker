use crate::contracts::{
    AgentTurnV1, ContentHashV1, ContractError, EpochMillisV1, ProjectAgentSessionV1,
    SessionStatusV1, SessionTerminationReasonV1, TurnFailureCodeV1, TurnFailureV1, TurnStateV1,
    WriterLeaseV1,
};

pub fn is_transition_allowed(from: TurnStateV1, to: TurnStateV1) -> bool {
    use TurnStateV1::{
        Acting, Applying, AwaitingHighRiskConfirmation, Cancelled, Completed, Failed, Idle,
        Planning, Reading, RecoveryRequired, Validating,
    };
    matches!(
        (from, to),
        (Idle, Planning | Cancelled)
            | (Planning, Reading | Acting | Completed | Cancelled | Failed)
            | (Reading, Acting | Completed | Cancelled | Failed)
            | (
                Acting,
                AwaitingHighRiskConfirmation | Applying | Completed | Cancelled | Failed
            )
            | (AwaitingHighRiskConfirmation, Applying | Cancelled | Failed)
            | (Applying, Validating | RecoveryRequired)
            | (Validating, Completed | RecoveryRequired)
    )
}

pub fn begin_turn(
    session: &mut ProjectAgentSessionV1,
    turn: &AgentTurnV1,
    now_ms: EpochMillisV1,
) -> Result<(), ContractError> {
    session.validate()?;
    turn.validate()?;
    ensure_binding(session, turn)?;
    if session.status != SessionStatusV1::Active {
        return Err(ContractError::SessionNotActive);
    }
    if turn.state != TurnStateV1::Idle || turn.state.is_terminal() {
        return Err(ContractError::InvalidTransition);
    }
    if session.active_turn_id.is_some() {
        return Err(ContractError::TurnAlreadyActive);
    }
    if !matches!(session.writer_lease, WriterLeaseV1::Released) {
        return Err(ContractError::WriterUnavailable);
    }
    if snapshot_drift_code(session, turn).is_some() {
        return Err(ContractError::SnapshotDrift);
    }
    session.active_turn_id = Some(turn.turn_id.clone());
    session.updated_at_ms = now_ms;
    session.validate()
}

pub fn transition_turn(
    session: &mut ProjectAgentSessionV1,
    turn: &mut AgentTurnV1,
    to: TurnStateV1,
    now_ms: EpochMillisV1,
) -> Result<(), ContractError> {
    ensure_active_binding(session, turn)?;
    if turn.state.is_terminal() {
        return Err(ContractError::TerminalStateReuse);
    }
    if let Some(code) = snapshot_drift_code(session, turn) {
        force_failure(session, turn, code, now_ms);
        return Err(ContractError::SnapshotDrift);
    }
    if !is_transition_allowed(turn.state, to) {
        return Err(ContractError::InvalidTransition);
    }

    match to {
        TurnStateV1::Applying => {
            if !matches!(session.writer_lease, WriterLeaseV1::Released) {
                return Err(ContractError::WriterUnavailable);
            }
            turn.write_transaction_started = true;
            session.writer_lease = WriterLeaseV1::Held {
                turn_id: turn.turn_id.clone(),
            };
        }
        TurnStateV1::Validating => {
            if !matches!(
                &session.writer_lease,
                WriterLeaseV1::Held { turn_id } if turn_id == &turn.turn_id
            ) {
                return Err(ContractError::WriterUnavailable);
            }
        }
        TurnStateV1::Completed => finish_turn(session, turn, now_ms),
        TurnStateV1::Cancelled => {
            if turn.write_transaction_started {
                return Err(ContractError::InvalidTransition);
            }
            turn.cancellation_requested = true;
            finish_turn(session, turn, now_ms);
        }
        TurnStateV1::Failed => {
            if turn.write_transaction_started {
                return Err(ContractError::InvalidTransition);
            }
            turn.failure = Some(failure(TurnFailureCodeV1::ContractViolation));
            finish_turn(session, turn, now_ms);
        }
        TurnStateV1::RecoveryRequired => enter_recovery(session, turn, now_ms),
        _ => {}
    }

    turn.state = to;
    turn.updated_at_ms = now_ms;
    session.updated_at_ms = now_ms;
    turn.validate()?;
    session.validate()
}

pub fn request_cancel(
    session: &mut ProjectAgentSessionV1,
    turn: &mut AgentTurnV1,
    now_ms: EpochMillisV1,
) -> Result<(), ContractError> {
    ensure_active_binding(session, turn)?;
    if turn.state.is_terminal() {
        return Err(ContractError::TerminalStateReuse);
    }
    turn.cancellation_requested = true;
    if turn.write_transaction_started {
        enter_recovery(session, turn, now_ms);
        turn.state = TurnStateV1::RecoveryRequired;
    } else {
        finish_turn(session, turn, now_ms);
        turn.state = TurnStateV1::Cancelled;
    }
    turn.updated_at_ms = now_ms;
    session.updated_at_ms = now_ms;
    turn.validate()?;
    session.validate()
}

pub fn resolve_recovery(
    session: &mut ProjectAgentSessionV1,
    turn: &AgentTurnV1,
    now_ms: EpochMillisV1,
) -> Result<(), ContractError> {
    ensure_active_binding(session, turn)?;
    if session.status != SessionStatusV1::RecoveryRequired
        || turn.state != TurnStateV1::RecoveryRequired
        || !matches!(
            &session.writer_lease,
            WriterLeaseV1::RecoveryRequired { turn_id } if turn_id == &turn.turn_id
        )
    {
        return Err(ContractError::RecoveryNotRequired);
    }
    session.status = SessionStatusV1::Active;
    session.active_turn_id = None;
    session.writer_lease = WriterLeaseV1::Released;
    session.updated_at_ms = now_ms;
    session.validate()
}

pub fn terminate_session(
    session: &mut ProjectAgentSessionV1,
    reason: SessionTerminationReasonV1,
    now_ms: EpochMillisV1,
) -> Result<(), ContractError> {
    if session.status != SessionStatusV1::Active {
        return Err(ContractError::SessionNotActive);
    }
    if session.active_turn_id.is_some() {
        return Err(ContractError::TurnAlreadyActive);
    }
    if !matches!(session.writer_lease, WriterLeaseV1::Released) {
        return Err(ContractError::WriterUnavailable);
    }
    session.status = SessionStatusV1::Terminated;
    session.termination_reason = Some(reason);
    session.updated_at_ms = now_ms;
    session.validate()
}

fn ensure_binding(
    session: &ProjectAgentSessionV1,
    turn: &AgentTurnV1,
) -> Result<(), ContractError> {
    if session.project_id != turn.project_id || session.session_id != turn.session_id {
        return Err(ContractError::SessionMismatch);
    }
    Ok(())
}

fn ensure_active_binding(
    session: &ProjectAgentSessionV1,
    turn: &AgentTurnV1,
) -> Result<(), ContractError> {
    ensure_binding(session, turn)?;
    if session.active_turn_id.as_ref() != Some(&turn.turn_id) {
        return Err(ContractError::TurnNotActive);
    }
    Ok(())
}

fn snapshot_drift_code(
    session: &ProjectAgentSessionV1,
    turn: &AgentTurnV1,
) -> Option<TurnFailureCodeV1> {
    if session.provider != turn.frozen.provider {
        Some(TurnFailureCodeV1::ProviderSnapshotDrift)
    } else if session.skill_pack != turn.frozen.skill_pack {
        Some(TurnFailureCodeV1::SkillPackSnapshotDrift)
    } else if session.authorization_policy != turn.frozen.authorization_policy {
        Some(TurnFailureCodeV1::AuthorizationPolicySnapshotDrift)
    } else {
        None
    }
}

fn force_failure(
    session: &mut ProjectAgentSessionV1,
    turn: &mut AgentTurnV1,
    code: TurnFailureCodeV1,
    now_ms: EpochMillisV1,
) {
    turn.failure = Some(failure(code));
    if turn.write_transaction_started {
        enter_recovery(session, turn, now_ms);
        turn.state = TurnStateV1::RecoveryRequired;
    } else {
        finish_turn(session, turn, now_ms);
        turn.state = TurnStateV1::Failed;
    }
    turn.updated_at_ms = now_ms;
    session.updated_at_ms = now_ms;
}

fn finish_turn(session: &mut ProjectAgentSessionV1, turn: &AgentTurnV1, _now_ms: EpochMillisV1) {
    if session.active_turn_id.as_ref() == Some(&turn.turn_id) {
        session.active_turn_id = None;
    }
    session.writer_lease = WriterLeaseV1::Released;
}

fn enter_recovery(
    session: &mut ProjectAgentSessionV1,
    turn: &mut AgentTurnV1,
    _now_ms: EpochMillisV1,
) {
    turn.write_transaction_started = true;
    if turn.failure.is_none() {
        turn.failure = Some(failure(TurnFailureCodeV1::WriteRecoveryRequired));
    }
    session.status = SessionStatusV1::RecoveryRequired;
    session.active_turn_id = Some(turn.turn_id.clone());
    session.writer_lease = WriterLeaseV1::RecoveryRequired {
        turn_id: turn.turn_id.clone(),
    };
}

fn failure(code: TurnFailureCodeV1) -> TurnFailureV1 {
    let detail_hash = ContentHashV1::digest(format!("{code:?}").as_bytes());
    TurnFailureV1 { code, detail_hash }
}
