#[test]
fn nr603_late_cancel_cannot_stop_a_new_attempt_but_matching_cancel_is_prompt() {
    use std::io::Read;
    for provider in [ProviderId::Deepseek, ProviderId::Openai] {
        let f = Fixture::new(provider);
        let failed = f.turn("只读检查", wire::Plan::Failure { partial: false });
        let turn = &failed["conversation"]["turns"][0];
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        *f.endpoint.lock().unwrap() =
            Some(format!("http://{}/fixture", listener.local_addr().unwrap()));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0; 4096];
            let _ = socket.read(&mut bytes).unwrap();
            ready_tx.send(()).unwrap();
            thread::sleep(Duration::from_millis(500));
        });
        let started = f.ipc("conversation_turn_retry", json!({"request":{"projectSessionId":f.session,
            "conversationSessionId":failed["conversation"]["sessionId"],"turnId":turn["turnId"],"expectedAttemptId":turn["retryState"]["attemptId"]}}));
        ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let mut cancel = json!({"sessionId":f.session,"conversationSessionId":started["conversationSessionId"],"turnId":started["turnId"],"attemptId":turn["retryState"]["attemptId"]});
        assert_eq!(
            f.ipc("conversation_turn_cancel", cancel.clone())["cancelRequested"],
            false
        );
        cancel["attemptId"] = Value::Null;
        assert_eq!(
            f.ipc("conversation_turn_cancel", cancel.clone())["cancelRequested"],
            false
        );
        cancel["attemptId"] = started["attemptId"].clone();
        let now = Instant::now();
        assert_eq!(
            f.ipc("conversation_turn_cancel", cancel)["cancelRequested"],
            true
        );
        loop {
            assert!(now.elapsed() < Duration::from_secs(2));
            if !f.app.state::<ProjectRegistry>().has_active_conversation(
                &f.session,
                started["conversationSessionId"].as_str().unwrap(),
            ) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        server.join().unwrap();
        *f.endpoint.lock().unwrap() = None;
        let final_view = f.ipc("conversation_session_load", json!({"sessionId":f.session}));
        assert_eq!(
            final_view["conversation"]["turns"][0]["retryState"]["outcome"],
            "blocked"
        );
        assert!(f.changes().is_empty());
    }
}
fn reject(f: &Fixture, body: Value) {
    let result = tauri::test::get_ipc_response(
        &f.window,
        tauri::webview::InvokeRequest {
            cmd: "conversation_turn_retry".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    );
    assert!(result.is_err());
}
#[test]
fn nr603_actual_dual_provider_retry_is_explicit_once_and_uses_same_user_and_original_budget() {
    for provider in [ProviderId::Deepseek, ProviderId::Openai] {
        let f = Fixture::new(provider);
        let failed = f.turn(
            "请只读检查人物资料。",
            wire::Plan::Failure { partial: false },
        );
        let turn = &failed["conversation"]["turns"][0];
        assert_eq!(turn["retryState"]["outcome"], "eligible");
        let body = json!({"request":{"projectSessionId":f.session,"conversationSessionId":failed["conversation"]["sessionId"],
            "turnId":turn["turnId"],"expectedAttemptId":turn["retryState"]["attemptId"]}});
        let mut invalid = body.clone();
        invalid["request"]["expectedAttemptId"] = json!(Uuid::new_v4().to_string());
        reject(&f, invalid);
        let mut invalid = body.clone();
        invalid["request"]["providerCursor"] = json!("must-not-be-read");
        reject(&f, invalid);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        *f.endpoint.lock().unwrap() =
            Some(format!("http://{}/fixture", listener.local_addr().unwrap()));
        let server = thread::spawn(move || wire::serve(listener, provider, wire::Plan::Chat));
        let started = f.ipc("conversation_turn_retry", body.clone());
        assert_eq!(started["turnId"], turn["turnId"]);
        assert_eq!(started["attemptNumber"], 2);
        assert_ne!(started["attemptId"], turn["retryState"]["attemptId"]);
        reject(&f, body.clone());
        let deadline = Instant::now() + Duration::from_secs(10);
        let terminal = loop {
            assert!(Instant::now() < deadline);
            let view = f.ipc("conversation_session_load", json!({"sessionId":f.session}));
            if !f.app.state::<ProjectRegistry>().has_active_conversation(
                &f.session,
                started["conversationSessionId"].as_str().unwrap(),
            ) {
                break view;
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(server.join().unwrap(), 1);
        *f.endpoint.lock().unwrap() = None;
        assert_eq!(
            terminal["conversation"]["turns"].as_array().unwrap().len(),
            1
        );
        let retried = &terminal["conversation"]["turns"][0];
        assert_eq!(retried["messages"][0], turn["messages"][0]);
        assert_eq!(retried["status"], "completed");
        assert_eq!(retried["retryState"]["agentRequests"], 2);
        assert_eq!(retried["retryState"]["requestedOutputTokens"], 8192);
        reject(&f, body);
        assert!(f.changes().is_empty());
        let events = f.events.lock().unwrap();
        assert!(events.iter().any(|event| event["attemptNumber"] == 1));
        assert!(events.iter().any(|event| event["attemptNumber"] == 2));
        assert!(events.iter().all(|event| event["attemptId"].is_string()));
        println!("NR603_RETRY provider={provider:?} requests=2 users=1 duplicate=0 external=0");
    }
}
#[test]
fn nr603_actual_partial_and_tool_dispatched_failures_cannot_retry_or_replay_canon() {
    for provider in [ProviderId::Deepseek, ProviderId::Openai] {
        for plan in [
            wire::Plan::Failure { partial: true },
            wire::Plan::Create {
                kind: "character",
                title: "林舟".into(),
                text: "林舟是修表匠。".into(),
                chapter: None,
                chapter_source: None,
                risk: "low",
                final_error: true,
            },
        ] {
            let f = Fixture::new(provider);
            let failed = f.turn("请创建并保存人物林舟，职业是修表匠。", plan);
            let turn = &failed["conversation"]["turns"][0];
            let before = f.snapshot();
            assert_eq!(turn["retryState"]["outcome"], "blocked");
            reject(
                &f,
                json!({"request":{"projectSessionId":f.session,"conversationSessionId":failed["conversation"]["sessionId"],
                "turnId":turn["turnId"],"expectedAttemptId":turn["retryState"]["attemptId"]}}),
            );
            assert_eq!(f.snapshot(), before);
        }
    }
}
