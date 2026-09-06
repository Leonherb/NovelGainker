use crate::conversation::{begin_retry, initialize_attempt, retry_state, RetryOutcome};

fn nr603_start(f: &Fixture) -> (String, String) {
    let started = start_conversation_turn(
        &f.handle,
        ProviderId::Openai,
        "gpt-5.6-terra",
        "只读检查",
        10,
    )
    .unwrap();
    let state = initialize_attempt(&f.handle, &started.turn_id).unwrap();
    (started.turn_id, state.attempt_id)
}
fn nr603_reserve(f: &Fixture, turn: &str) {
    assert!(super::reserve_request(&f.handle, turn, super::RequestKind::Agent, 4096).unwrap());
}
#[test]
fn nr603_retry_cas_preserves_user_identity_and_never_resets_budget() {
    let f = Fixture::new();
    let (turn, attempt) = nr603_start(&f);
    nr603_reserve(&f, &turn);
    let terminal = finish_conversation_turn(
        &f.handle,
        &turn,
        "",
        ConversationTerminalKind::Failed,
        Some("provider.unreachable"),
        20,
    )
    .unwrap();
    assert_eq!(
        terminal.turns[0].retry_state.as_ref().unwrap().outcome,
        RetryOutcome::Eligible
    );
    let before = fs::read(f.root.join(".novelgainker/conversation/session.json")).unwrap();
    assert!(begin_retry(&f.handle, &turn, &Uuid::new_v4().to_string(), 21).is_err());
    assert!(begin_retry(&f.handle, &Uuid::new_v4().to_string(), &attempt, 21).is_err());
    assert_eq!(
        fs::read(f.root.join(".novelgainker/conversation/session.json")).unwrap(),
        before
    );
    let retry = begin_retry(&f.handle, &turn, &attempt, 21).unwrap();
    assert_eq!(retry.messages, terminal.turns[0].messages[..1]);
    assert_ne!(retry.retry_state.as_ref().unwrap().attempt_id, attempt);
    assert_eq!(retry.retry_state.as_ref().unwrap().agent_requests, 1);
    assert!(begin_retry(&f.handle, &turn, &attempt, 22).is_err());
    nr603_reserve(&f, &turn);
    let terminal = finish_conversation_turn(
        &f.handle,
        &turn,
        "",
        ConversationTerminalKind::Failed,
        Some("provider.unreachable"),
        23,
    )
    .unwrap();
    let state = terminal.turns[0].retry_state.as_ref().unwrap();
    assert_eq!(state.outcome, RetryOutcome::Blocked);
    assert_eq!(state.agent_requests, 2);
    assert_eq!(state.requested_output_tokens, 8192);
    assert!(begin_retry(&f.handle, &turn, &state.attempt_id, 24).is_err());
}
#[test]
fn nr603_partial_whitespace_tools_authentication_cancel_and_legacy_are_not_replayed() {
    for (text, code, kind, tool) in [
        (
            "正文",
            "provider.timeout",
            ConversationTerminalKind::Failed,
            false,
        ),
        (
            " ",
            "provider.timeout",
            ConversationTerminalKind::Failed,
            false,
        ),
        (
            "",
            "provider.timeout",
            ConversationTerminalKind::Failed,
            true,
        ),
        (
            "",
            "provider.authentication_failed",
            ConversationTerminalKind::Failed,
            false,
        ),
        (
            "",
            "provider.protocol",
            ConversationTerminalKind::Failed,
            false,
        ),
        (
            "",
            "provider.cancelled",
            ConversationTerminalKind::Cancelled,
            false,
        ),
    ] {
        let f = Fixture::new();
        let (turn, attempt) = nr603_start(&f);
        nr603_reserve(&f, &turn);
        if tool {
            super::tool_dispatched(&f.handle, &turn).unwrap();
        }
        finish_conversation_turn(&f.handle, &turn, text, kind, Some(code), 20).unwrap();
        assert_eq!(
            retry_state(&f.handle, &turn).unwrap().unwrap().outcome,
            RetryOutcome::Blocked
        );
        assert!(begin_retry(&f.handle, &turn, &attempt, 21).is_err());
    }
    let f = Fixture::new();
    let turn = start_conversation_turn(
        &f.handle,
        ProviderId::Openai,
        "gpt-5.6-terra",
        "旧版本消息",
        10,
    )
    .unwrap();
    finish_conversation_turn(
        &f.handle,
        &turn.turn_id,
        "",
        ConversationTerminalKind::Failed,
        Some("provider.timeout"),
        20,
    )
    .unwrap();
    assert!(begin_retry(&f.handle, &turn.turn_id, &Uuid::new_v4().to_string(), 21).is_err());
}
#[test]
fn nr603_reopen_interrupted_attempt_is_unknown_and_never_eligible() {
    let f = Fixture::new();
    let (turn, attempt) = nr603_start(&f);
    nr603_reserve(&f, &turn);
    let recovered = load_conversation(&f.handle, 30).unwrap();
    assert_eq!(
        recovered.turns[0].retry_state.as_ref().unwrap().outcome,
        RetryOutcome::Blocked
    );
    assert!(begin_retry(&f.handle, &turn, &attempt, 31).is_err());
    assert_eq!(load_conversation(&f.handle, 32).unwrap(), recovered);
}
#[test]
fn nr603_eight_agent_one_compactor_cap_survives_retry_and_skips_second_compactor() {
    let f = Fixture::new();
    let (turn, attempt) = nr603_start(&f);
    assert!(super::reserve_request(&f.handle, &turn, super::RequestKind::Compactor, 1024).unwrap());
    nr603_reserve(&f, &turn);
    finish_conversation_turn(
        &f.handle,
        &turn,
        "",
        ConversationTerminalKind::Failed,
        Some("provider.rate_limited"),
        20,
    )
    .unwrap();
    begin_retry(&f.handle, &turn, &attempt, 21).unwrap();
    assert!(
        !super::reserve_request(&f.handle, &turn, super::RequestKind::Compactor, 1024).unwrap()
    );
    for _ in 0..7 {
        nr603_reserve(&f, &turn);
    }
    assert!(super::reserve_request(&f.handle, &turn, super::RequestKind::Agent, 4096).is_err());
    let state = retry_state(&f.handle, &turn).unwrap().unwrap();
    assert_eq!(state.requested_output_tokens, 33792);
    assert_eq!(state.agent_requests, 8);
}
#[test]
fn nr603_closed_retry_schema_rejects_unbounded_and_hidden_state_before_use() {
    let f = Fixture::new();
    let (turn, _) = nr603_start(&f);
    let path = f.root.join(".novelgainker/conversation/session.json");
    let base: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for (key, value) in [
        ("agentRequests", serde_json::json!(9)),
        ("compactorRequests", serde_json::json!(2)),
        ("attemptNumber", serde_json::json!(3)),
        ("continuation", serde_json::json!("hidden-canary")),
        ("outcome", serde_json::json!("eligible")),
        ("attemptId", serde_json::json!("invalid")),
    ] {
        let mut invalid = base.clone();
        invalid["turns"][0]["retryState"][key] = value;
        fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(retry_state(&f.handle, &turn).is_err());
        assert!(super::reserve_request(&f.handle, &turn, super::RequestKind::Agent, 4096).is_err());
    }
}
