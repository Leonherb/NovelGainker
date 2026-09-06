mod nr403 {
    use super::*;
    use crate::canon_change::{ChangeStatus, NR4_REQUEST_BYTES};
    use crate::canon_change_store::CanonChangeCoordinator;
    use crate::coordination::try_acquire_project_write_lease;
    use crate::memory::NR1_CONTEXT_PROFILE;
    use crate::recovery::RecoveryStore;
    use serde_json::{json, Value};

    pub(super) fn emit(
        stream: &mut std::net::TcpStream,
        provider: ProviderId,
        index: usize,
        tool: Option<(&str, Value)>,
    ) {
        let body = match provider {
            ProviderId::Deepseek => {
                let delta = if let Some((name, arguments)) = tool {
                    json!({"tool_calls":[{"index":0,"id":format!("change_call_{index}"),"type":"function",
                        "function":{"name":name,"arguments":arguments.to_string()}}]})
                } else {
                    json!({"content":"自然回复完整保留。"})
                };
                let reason = if delta.get("tool_calls").is_some() {
                    "tool_calls"
                } else {
                    "stop"
                };
                format!(
                    "data: {}\n\ndata: [DONE]\n\n",
                    json!({"choices":[{"delta":delta,"finish_reason":reason}]})
                )
            }
            ProviderId::Openai => {
                if let Some((name, arguments)) = tool {
                    format!(
                        "data: {}\n\n",
                        json!({"type":"response.completed","response":{
                        "model":"gpt-5.6-terra","status":"completed","output":[{
                            "type":"function_call","id":format!("fc_{index}"),"call_id":format!("change_call_{index}"),
                            "name":name,"arguments":arguments.to_string(),"status":"completed"
                        }]}})
                    )
                } else {
                    format!(
                        "data: {}\n\ndata: {}\n\n",
                        json!({"type":"response.output_text.delta","delta":"自然回复完整保留。","sequence_number":1}),
                        json!({"type":"response.completed","response":{"model":"gpt-5.6-terra","status":"completed"}})
                    )
                }
            }
        };
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    }

    pub(super) fn last_result(body: &Value, provider: ProviderId) -> Value {
        let text = match provider {
            ProviderId::Deepseek => body["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap(),
            ProviderId::Openai => body["input"].as_array().unwrap().last().unwrap()["output"]
                .as_str()
                .unwrap(),
        };
        serde_json::from_str(text).unwrap()
    }

    fn candidate(id: &str) -> String {
        format!("---\nid: {id}\ntitle: 林溪\nkind: character\nschemaVersion: 2\nwritingLanguage: zh-CN\n---\n\n# 林溪\n\n## 摘要\n\n喜欢观察，也喜欢写作。\n")
    }

    #[test]
    fn nr406_zero_tool_chat_and_invalid_write_reply_preserve_conversation_without_canon() {
        zero_tool_chat_for(false);
    }

    pub(super) fn zero_tool_chat_for(nr5: bool) {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            for invalid in [false, true] {
                let fixture = Fixture::new();
                fs::create_dir(fixture.root.join("characters")).unwrap();
                let lease = Arc::new(
                    try_acquire_project_write_lease(&fixture.handle.coordination_key())
                        .unwrap()
                        .unwrap(),
                );
                let coordinator = Arc::new(
                    (if nr5 { CanonChangeCoordinator::new_nr5 } else { CanonChangeCoordinator::new })(
                        fixture.handle.clone(),
                        lease,
                        RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap(),
                    )
                    .unwrap(),
                );
                let turn = fixture.start_turn(100);
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                listener.set_nonblocking(true).unwrap();
                let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
                let server = thread::spawn(move || {
                    for index in 0..if invalid { 2 } else { 1 } {
                        let deadline = Instant::now() + Duration::from_secs(5);
                        let mut stream = loop {
                            match listener.accept() {
                                Ok((stream, _)) => break stream,
                                Err(error)
                                    if error.kind() == std::io::ErrorKind::WouldBlock
                                        && Instant::now() < deadline =>
                                {
                                    thread::sleep(Duration::from_millis(5))
                                }
                                Err(error) => panic!("missing offline attempt: {error}"),
                            }
                        };
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let body = captured_json(&read_http_request(&mut stream));
                        if invalid && index == 0 {
                            emit(
                                &mut stream,
                                provider,
                                index,
                                Some(("novelist_propose_change", json!({"schemaVersion":if nr5 { 2 } else { 1 }}))),
                            );
                        } else {
                            if invalid {
                                assert_eq!(last_result(&body, provider)["status"], "failed");
                                assert_eq!(body["tool_choice"], if nr5 { "auto" } else { "none" });
                            }
                            emit(&mut stream, provider, index, None);
                        }
                    }
                });
                let client = ProviderHttpClient::for_test(provider, endpoint).unwrap();
                let terminal = super::super::execute_memory_managed_turn_with_changes(
                    &client,
                    &fixture.handle,
                    &turn,
                    provider,
                    b"synthetic-loopback-key",
                    &CancellationToken::default(),
                    101,
                    coordinator.clone(),
                    |_| {},
                )
                .unwrap();
                server.join().unwrap();
                assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
                assert_eq!(
                    terminal.session.turns.last().unwrap().messages[1].content,
                    "自然回复完整保留。"
                );
                assert!(coordinator.receipts().unwrap().is_empty());
                assert_eq!(
                    fs::read_dir(fixture.root.join("characters"))
                        .unwrap()
                        .count(),
                    0
                );
                if !invalid {
                    assert!(find_turn_tasks(&fixture.handle, &turn).unwrap().is_empty());
                }
            }
        }
    }

    #[test]
    fn nr403_core_loop_reads_then_creates_or_updates_using_real_delivered_receipts() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            for existing in [false, true] {
                run_case(provider, existing, false, false);
            }
        }
    }

    #[test]
    fn nr406_both_providers_capture_four_agent_plus_one_compactor_with_real_budget() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            run_case(provider, true, true, false);
        }
    }

    #[test]
    fn nr406_post_commit_provider_failure_preserves_saved_truth() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            run_case(provider, false, false, true);
        }
    }

    fn run_case(provider: ProviderId, existing: bool, compacted: bool, fail_final: bool) {
        let fixture = Fixture::new();
        if compacted {
            for index in 0..20 {
                let turn = start_conversation_turn(
                    &fixture.handle,
                    provider,
                    "fixture",
                    &format!("已结束的合成讨论 {index} {}", "甲".repeat(900)),
                    10 + index * 2,
                )
                .unwrap();
                finish_conversation_turn(
                    &fixture.handle,
                    &turn.turn_id,
                    &"乙".repeat(900),
                    ConversationTerminalKind::Completed,
                    None,
                    11 + index * 2,
                )
                .unwrap();
            }
        }
        fs::create_dir(fixture.root.join("characters")).unwrap();
        if existing {
            fs::write(
                fixture.root.join("characters/lin.md"),
                candidate("lin-xi").replace("，也喜欢写作", ""),
            )
            .unwrap();
        }
        let lease = Arc::new(
            try_acquire_project_write_lease(&fixture.handle.coordination_key())
                .unwrap()
                .unwrap(),
        );
        let coordinator = Arc::new(
            CanonChangeCoordinator::new(
                fixture.handle.clone(),
                lease,
                RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap(),
            )
            .unwrap(),
        );
        let started = start_conversation_turn(
            &fixture.handle,
            provider,
            "fixture",
            "就这样定下来：林溪爱观察，也爱写作，记下这个人物设定。",
            1,
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let agent_attempts = if existing { 4 } else { 3 };
            let attempts = agent_attempts + usize::from(compacted);
            let mut sources = Value::Null;
            let mut requested = 0u64;
            let mut maximum_body = 0usize;
            for http_index in 0..attempts {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("missing offline attempt {http_index}: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let raw = read_http_request(&mut stream);
                let exact_bytes = raw.split_once("\r\n\r\n").unwrap().1.len();
                maximum_body = maximum_body.max(exact_bytes);
                assert!(exact_bytes <= NR4_REQUEST_BYTES);
                let body = captured_json(&raw);
                requested += body[if provider == ProviderId::Deepseek {
                    "max_tokens"
                } else {
                    "max_output_tokens"
                }]
                .as_u64()
                .unwrap();
                if compacted && http_index == 0 {
                    assert_eq!(body["stream"], false);
                    assert!(body.get("tools").is_none());
                    let response = if provider == ProviderId::Openai {
                        completion_json()
                    } else {
                        json!({"model":"deepseek-v4-pro","choices":[{"message":{"content":summary()},"finish_reason":"stop"}]}).to_string()
                    };
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
                    continue;
                }
                let index = http_index - usize::from(compacted);
                if index == 0 {
                    assert_eq!(body["tools"].as_array().unwrap().len(), 2);
                    let system = if provider == ProviderId::Deepseek {
                        body["messages"][0]["content"].as_str().unwrap()
                    } else {
                        body["instructions"].as_str().unwrap()
                    };
                    sources = serde_json::from_str(
                        system
                            .lines()
                            .find(|line| line.starts_with("[{") && line.contains("\"currentTurn\""))
                            .unwrap(),
                    )
                    .unwrap();
                    if compacted {
                        assert!(system.contains(summary()));
                    }
                    let arguments = crate::canon_context::normalize_deepseek_novelist_read_context_arguments(json!({
                                "capabilityId":"create-character","canonOperations":[{"kind":"list","scopes":["characters"],"limit":64}]
                            })).unwrap();
                    emit(
                        &mut stream,
                        provider,
                        index,
                        Some(("novelist_read_context", arguments)),
                    );
                } else if existing && index == 1 {
                    assert_eq!(
                        last_result(&body, provider)["output"]["canon"][0]["paths"],
                        json!(["characters/lin.md"])
                    );
                    let arguments = crate::canon_context::normalize_deepseek_novelist_read_context_arguments(json!({
                                "capabilityId":"create-character","canonOperations":[{"kind":"read","relativePath":"characters/lin.md","startLine":1,"maxBytes":4096}]
                            })).unwrap();
                    emit(
                        &mut stream,
                        provider,
                        index,
                        Some(("novelist_read_context", arguments)),
                    );
                } else if index + 1 < agent_attempts {
                    let read = last_result(&body, provider);
                    assert_eq!(read["status"], "succeeded");
                    let proof = &read["output"]["changeEvidence"];
                    let target = &proof["completeTargets"][0];
                    let source = sources
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|source| source["role"] == "user" && source["currentTurn"] == true)
                        .unwrap();
                    let arguments = json!({
                        "schemaVersion":1,"sourceKind":"user_decision","risk":"low",
                        "sources":[{"messageId":source["messageId"],"contentHash":source["contentHash"]}],
                        "operations":[{"kind":if existing {"update"}else{"create"},
                            "entityKind":"character","capabilityId":"create-character","methodReceiptId":proof["methodReceiptId"],
                            "targetId":target["targetId"],"readReceiptId":target["readReceiptId"],"expectedSourceHash":target["expectedSourceHash"],
                            "proposedMarkdown":candidate(if existing {"lin-xi"}else{"new"})}]
                    });
                    emit(
                        &mut stream,
                        provider,
                        index,
                        Some(("novelist_propose_change", arguments)),
                    );
                } else {
                    assert_eq!(body["tool_choice"], "none");
                    assert!(body.get("tools").is_none());
                    let result = last_result(&body, provider);
                    assert_eq!(result["status"], "succeeded", "{result}");
                    assert_eq!(result["output"]["status"], "committed", "{result}");
                    if fail_final {
                        write!(stream, "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    } else {
                        emit(&mut stream, provider, index, None);
                    }
                }
            }
            assert_eq!(
                requested,
                agent_attempts as u64 * 4096 + if compacted { 1024 } else { 0 }
            );
            if compacted {
                assert_eq!(attempts, 5);
                assert_eq!(requested, 17408);
            }
            println!("NR406_LOOP provider={provider:?} attempts={attempts} agent={agent_attempts} compactor={} requested={requested} maximum_body={maximum_body} cap={NR4_REQUEST_BYTES} external=0", usize::from(compacted));
        });
        let client = ProviderHttpClient::for_test(provider, endpoint).unwrap();
        let terminal = super::super::execute_memory_managed_turn_with_changes(
            &client,
            &fixture.handle,
            &started.turn_id,
            provider,
            b"synthetic-loopback-key",
            &CancellationToken::default(),
            2,
            coordinator.clone(),
            |_| {},
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(
            terminal.state,
            if fail_final {
                ConversationProviderTerminalState::Incomplete
            } else {
                ConversationProviderTerminalState::Completed
            }
        );
        if compacted {
            assert_eq!(
                load_valid_memory(&fixture.handle, &terminal.session, NR1_CONTEXT_PROFILE)
                    .unwrap()
                    .summary_text,
                summary()
            );
        }
        let receipts = coordinator.receipts().unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, ChangeStatus::Committed);
        let saved =
            fs::read_to_string(fixture.root.join(&receipts[0].files[0].relative_path)).unwrap();
        assert!(saved.contains("也喜欢写作"));
        if !fail_final {
            assert!(terminal
                .session
                .turns
                .last()
                .unwrap()
                .messages
                .iter()
                .any(|message| message.content == "自然回复完整保留。"));
        }
    }
}
