mod tool_stream {
    use super::*;
    use crate::canon_change::ChangeStatus;
    use crate::canon_change_store::CanonChangeCoordinator;
    use crate::coordination::try_acquire_project_write_lease;
    use crate::provider_core::NormalizedEvent;
    use crate::recovery::RecoveryStore;
    use serde_json::{json, Value};

    fn emit_events(stream: &mut std::net::TcpStream, events: Vec<Value>) {
        let body = events
            .into_iter()
            .map(|value| format!("data: {value}\n\n"))
            .collect::<String>()
            + "data: [DONE]\n\n";
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
    }

    fn chunk(delta: Value, finish: Value) -> Value {
        json!({"choices":[{"delta":delta,"finish_reason":finish}]})
    }

    fn fragmented(stream: &mut std::net::TcpStream, name: &str, args: Value, index: usize) {
        let raw = args.to_string();
        let split = raw.char_indices().nth(raw.chars().count() / 2).unwrap().0;
        emit_events(
            stream,
            vec![
                chunk(json!({"tool_calls":[]}), Value::Null),
                chunk(
                    json!({"tool_calls":[{"index":0,"id":format!("fragmented_{index}"),"function":{"name":name,"arguments":&raw[..split]}}]}),
                    Value::Null,
                ),
                chunk(
                    json!({"tool_calls":[{"index":0,"id":"","function":{"name":"","arguments":&raw[split..]}}]}),
                    Value::Null,
                ),
                chunk(json!({"tool_calls":[]}), json!("tool_calls")),
            ],
        );
    }

    fn multiple(stream: &mut std::net::TcpStream, index: usize) {
        emit_events(
            stream,
            vec![
                chunk(json!({"content":"已收到保存请求。"}), Value::Null),
                chunk(
                    json!({"tool_calls":[
                        {"index":0,"id":format!("rejected_{index}_0"),"function":{"name":"novelist_read_context","arguments":"{}"}},
                        {"index":1,"id":format!("rejected_{index}_1"),"function":{"name":"novelist_read_context","arguments":"{}"}}
                    ]}),
                    json!("tool_calls"),
                ),
            ],
        );
    }

    #[test]
    fn fragmented_save_and_multiple_call_repair_keep_budget_and_prior_commits() {
        // Malformed batching both before any tool and after a durable save;
        // repeated batching must stop instead of looping or redoing the write.
        for (after_commit, repeated, prior, cancel) in [
            (false, false, 0, false),
            (true, false, 0, false),
            (false, true, 0, false),
            (true, true, 0, false),
            (false, false, 6, false),
            (false, false, 0, true),
            (true, false, 0, true),
        ] {
            let fixture = Fixture::new();
            fs::create_dir(fixture.root.join("characters")).unwrap();
            let lease = Arc::new(
                try_acquire_project_write_lease(&fixture.handle.coordination_key())
                    .unwrap()
                    .unwrap(),
            );
            let coordinator = Arc::new(
                CanonChangeCoordinator::new_nr5(
                    fixture.handle.clone(),
                    lease,
                    RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap(),
                )
                .unwrap(),
            );
            let started = start_conversation_turn(
                &fixture.handle,
                ProviderId::Deepseek,
                "synthetic",
                "采用已讨论的设定，保存人物资料。",
                1,
            )
            .unwrap();
            crate::conversation::initialize_attempt(&fixture.handle, &started.turn_id).unwrap();
            for _ in 0..prior {
                crate::conversation::reserve_request(
                    &fixture.handle,
                    &started.turn_id,
                    crate::conversation::RequestKind::Agent,
                    4096,
                )
                .unwrap();
            }
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
            let requests = if cancel {
                if after_commit {
                    3
                } else {
                    1
                }
            } else if prior > 0 {
                1
            } else if repeated && !after_commit {
                2
            } else {
                4
            };
            let server = thread::spawn(move || {
                let mut source = Value::Null;
                let mut committed_result = Value::Null;
                for index in 0..requests {
                    let mut stream = nr502::accept(&listener);
                    let body = nr502::request_json(&mut stream);
                    let system = body["messages"][0]["content"].as_str().unwrap();
                    if index == 0 {
                        let catalog: Value = serde_json::from_str(
                            system
                                .lines()
                                .find(|line| {
                                    line.starts_with("[{") && line.contains("\"currentTurn\"")
                                })
                                .unwrap(),
                        )
                        .unwrap();
                        source = catalog
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|row| row["currentTurn"] == true && row["role"] == "user")
                            .unwrap()
                            .clone();
                    }
                    let failed_index = if after_commit { 2 } else { 0 };
                    if index > failed_index {
                        assert!(system.contains("Host 协议纠正"));
                        assert!(!body["messages"].to_string().contains("rejected_"));
                        if after_commit {
                            let result = nr403::last_result(&body, ProviderId::Deepseek);
                            assert_eq!(result["output"]["status"], "committed");
                            assert_eq!(result, committed_result);
                        }
                    }
                    if index == failed_index || (repeated && index == failed_index + 1) {
                        if after_commit && index == failed_index {
                            let result = nr403::last_result(&body, ProviderId::Deepseek);
                            assert_eq!(result["output"]["status"], "committed");
                            committed_result = result;
                        }
                        multiple(&mut stream, index);
                    } else if index == usize::from(!after_commit) {
                        fragmented(
                            &mut stream,
                            "novelist_read_context",
                            nr504::read_arguments("create-character", None),
                            index,
                        );
                    } else if index == 1 + usize::from(!after_commit) {
                        let result = nr403::last_result(&body, ProviderId::Deepseek);
                        let method = result["output"]["changeEvidence"]["methodReceiptId"].clone();
                        assert!(method.is_string());
                        fragmented(
                            &mut stream,
                            "novelist_propose_change",
                            nr504::change_arguments(false, &source, &method, false, 0),
                            index,
                        );
                    } else {
                        let result = nr403::last_result(&body, ProviderId::Deepseek);
                        assert_eq!(result["output"]["status"], "committed");
                        emit_events(
                            &mut stream,
                            vec![chunk(
                                json!({"content":"人物资料已保存。","tool_calls":[]}),
                                json!("stop"),
                            )],
                        );
                    }
                }
                thread::sleep(Duration::from_millis(100));
                assert!(
                    listener.accept().is_err(),
                    "No unbudgeted follow-up request"
                );
            });
            let client = ProviderHttpClient::for_test(ProviderId::Deepseek, endpoint).unwrap();
            let cancellation = CancellationToken::default();
            let terminal=super::super::execute_memory_managed_turn_with_changes(&client,&fixture.handle,&started.turn_id,ProviderId::Deepseek,b"synthetic-loopback-key",&cancellation,2,coordinator.clone(),|event|{
                if cancel && matches!(event, AgentRuntimeEvent::Provider(NormalizedEvent::TextDelta { ref delta, .. }) if delta.contains("已收到保存请求")) { cancellation.cancel(); }
            }).unwrap();
            server.join().unwrap();
            let expected_commits =
                usize::from(after_commit || (!repeated && prior == 0 && !cancel));
            let receipts = coordinator.receipts().unwrap();
            assert_eq!(receipts.len(), expected_commits);
            assert!(receipts.iter().all(|r| r.status == ChangeStatus::Committed));
            assert_eq!(
                fs::read_dir(fixture.root.join("characters"))
                    .unwrap()
                    .count(),
                expected_commits
            );
            let tasks = find_turn_tasks(&fixture.handle, &started.turn_id).unwrap();
            assert_eq!(
                tasks.len(),
                expected_commits * 2,
                "Rejected batch must not dispatch any tool"
            );
            let state = crate::conversation::retry_state(&fixture.handle, &started.turn_id)
                .unwrap()
                .unwrap();
            assert_eq!(usize::from(state.agent_requests), prior + requests);
            assert_eq!(
                state.requested_output_tokens,
                (prior + requests) as u32 * 4096
            );
            assert_eq!(state.attempt_number, 1);
            let session = read_conversation(&fixture.handle).unwrap();
            let turn = session.turns.last().unwrap();
            assert!(turn
                .messages
                .last()
                .unwrap()
                .content
                .contains("已收到保存请求。"));
            if cancel {
                assert_eq!(terminal.state, ConversationProviderTerminalState::Cancelled);
            } else if repeated || prior > 0 {
                assert_eq!(
                    turn.error_code.as_deref(),
                    Some("provider.tool_calls_multiple")
                );
            } else {
                assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
                assert!(turn
                    .messages
                    .last()
                    .unwrap()
                    .content
                    .ends_with("人物资料已保存。"));
            }
        }
    }
    #[test]
    fn candidate_diagnostics_normalization_and_exhausted_tools_still_finish_chat() {
        for mode in ["normalized", "diagnostic", "quota"] {
            let fixture = Fixture::new();
            fs::create_dir(fixture.root.join("characters")).unwrap();
            let lease = Arc::new(
                try_acquire_project_write_lease(&fixture.handle.coordination_key())
                    .unwrap()
                    .unwrap(),
            );
            let coordinator = Arc::new(
                CanonChangeCoordinator::new_nr5(
                    fixture.handle.clone(),
                    lease,
                    RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap(),
                )
                .unwrap(),
            );
            let started = start_conversation_turn(
                &fixture.handle,
                ProviderId::Deepseek,
                "synthetic",
                "保存已确认的人物资料。",
                1,
            )
            .unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
            let requests = match mode {
                "normalized" => 3,
                "diagnostic" => 4,
                _ => 5,
            };
            let server = thread::spawn(move || {
                let mut source = Value::Null;
                let mut valid = Value::Null;
                for index in 0..requests {
                    let mut stream = nr502::accept(&listener);
                    let body = nr502::request_json(&mut stream);
                    if index == 0 {
                        let system = body["messages"][0]["content"].as_str().unwrap();
                        let catalog: Value = serde_json::from_str(
                            system
                                .lines()
                                .find(|line| {
                                    line.starts_with("[{") && line.contains("\"currentTurn\"")
                                })
                                .unwrap(),
                        )
                        .unwrap();
                        source = catalog
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|row| row["currentTurn"] == true && row["role"] == "user")
                            .unwrap()
                            .clone();
                        fragmented(
                            &mut stream,
                            "novelist_read_context",
                            nr504::read_arguments("create-character", None),
                            index,
                        );
                    } else if index == 1 {
                        let result = nr403::last_result(&body, ProviderId::Deepseek);
                        valid = nr504::change_arguments(
                            false,
                            &source,
                            &result["output"]["changeEvidence"]["methodReceiptId"],
                            false,
                            0,
                        );
                        let mut invalid = valid.clone();
                        let markdown = valid["operations"][0]["proposedMarkdown"].as_str().unwrap();
                        invalid["operations"][0]["proposedMarkdown"] = if mode == "normalized" {
                            json!(markdown
                                .replace("schemaVersion: 2", "schemaVersion: 99")
                                .replace("kind: character", "kind: world")
                                .replace("id: new", "id: invented_identity")
                                .replace("writingLanguage: zh-CN", "writingLanguage: en-US")
                                .trim_end_matches('\n')
                                .replace('\n', "\r\n"))
                        } else {
                            json!(
                                markdown
                                    .lines()
                                    .filter(|line| !line.starts_with("title:"))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                                    + "\n"
                            )
                        };
                        fragmented(&mut stream, "novelist_propose_change", invalid, index);
                    } else if index == 2 && mode != "normalized" {
                        let result = nr403::last_result(&body, ProviderId::Deepseek);
                        assert_eq!(result["errorCode"], "canon.change_candidate_invalid");
                        assert!(result["argumentIssues"]
                            .to_string()
                            .contains("frontmatter.required_title"));
                        assert!(result["argumentIssues"]
                            .to_string()
                            .contains("$.operations[0].proposedMarkdown"));
                        let mut next = valid.clone();
                        if mode == "quota" {
                            next["operations"][0]["targetId"] = json!("invented-target");
                        }
                        fragmented(&mut stream, "novelist_propose_change", next, index);
                    } else if index == 3 && mode == "quota" {
                        let result = nr403::last_result(&body, ProviderId::Deepseek);
                        assert_eq!(result["errorCode"], "canon.change_target_invalid");
                        assert_eq!(
                            result["argumentIssues"][0]["path"],
                            "$.operations[0].targetId"
                        );
                        assert!(!body["tools"]
                            .to_string()
                            .contains("novelist_propose_change"));
                        // Reproduce a model ignoring the reduced tool list after two failures.
                        fragmented(&mut stream, "novelist_propose_change", valid.clone(), index);
                    } else {
                        if mode == "quota" {
                            assert!(body.get("tools").is_none());
                            assert!(!body["messages"].to_string().contains("fragmented_3"));
                            emit_events(
                                &mut stream,
                                vec![chunk(
                                    json!({"content":"本轮未保存，候选缺少标题且目标不正确。可以继续讨论设定。"}),
                                    json!("stop"),
                                )],
                            );
                        } else {
                            assert_eq!(
                                nr403::last_result(&body, ProviderId::Deepseek)["output"]["status"],
                                "committed"
                            );
                            emit_events(
                                &mut stream,
                                vec![chunk(json!({"content":"人物资料已保存。"}), json!("stop"))],
                            );
                        }
                    }
                }
                thread::sleep(Duration::from_millis(100));
                assert!(listener.accept().is_err());
            });
            let terminal = super::super::execute_memory_managed_turn_with_changes(
                &ProviderHttpClient::for_test(ProviderId::Deepseek, endpoint).unwrap(),
                &fixture.handle,
                &started.turn_id,
                ProviderId::Deepseek,
                b"synthetic-loopback-key",
                &CancellationToken::default(),
                2,
                coordinator.clone(),
                |_| {},
            )
            .unwrap();
            server.join().unwrap();
            assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
            assert_eq!(
                coordinator.receipts().unwrap().len(),
                usize::from(mode != "quota")
            );
            let tasks = find_turn_tasks(&fixture.handle, &started.turn_id).unwrap();
            assert_eq!(tasks.len(), if mode == "normalized" { 2 } else { 3 });
            if mode == "normalized" {
                let entry = fs::read_dir(fixture.root.join("characters"))
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap();
                let text = fs::read_to_string(entry.path()).unwrap();
                crate::canon_change::validate_candidate(
                    crate::canon_change::CanonEntityKind::Character,
                    &text,
                )
                .unwrap();
                assert!(!text.contains("invented_identity"));
            }
        }
    }
}
