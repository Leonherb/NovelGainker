mod tool_feedback {
    use super::*;
    use crate::canon_change::ChangeStatus;
    use crate::canon_change_store::CanonChangeCoordinator;
    use crate::coordination::try_acquire_project_write_lease;
    use crate::recovery::RecoveryStore;
    use serde_json::{json, Value};

    #[test]
    fn invalid_write_can_be_corrected_and_exhaustion_keeps_chat_without_false_permission_error() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            for second_invalid in [false, true] {
                let fixture = Fixture::new();
                let previous = start_conversation_turn(
                    &fixture.handle,
                    provider,
                    "synthetic",
                    "保存人物设定",
                    1,
                )
                .unwrap();
                crate::conversation::initialize_attempt(&fixture.handle, &previous.turn_id)
                    .unwrap();
                finish_conversation_turn(
                    &fixture.handle,
                    &previous.turn_id,
                    "林溪喜欢观察。已提交，等待 Host 回执。",
                    ConversationTerminalKind::Completed,
                    None,
                    2,
                )
                .unwrap();
                for index in 0..8 {
                    let idle = start_conversation_turn(&fixture.handle, provider, "synthetic", "保存了吗？", 3 + index * 2).unwrap();
                    crate::conversation::initialize_attempt(&fixture.handle, &idle.turn_id).unwrap();
                    finish_conversation_turn(&fixture.handle, &idle.turn_id, "我来读取并核对。", ConversationTerminalKind::Completed, None, 4 + index * 2).unwrap();
                }
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
                    provider,
                    "synthetic",
                    "采用之前林溪喜欢观察的人物建议，并保存。",
                    30,
                )
                .unwrap();
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                listener.set_nonblocking(true).unwrap();
                let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
                let server = thread::spawn(move || {
                    let mut source = Value::Null;
                    let mut historical_source = Value::Null;
                    let mut valid = Value::Null;
                    for index in 0..4 {
                        let mut stream = nr502::accept(&listener);
                        let body = nr502::request_json(&mut stream);
                        let system = if provider == ProviderId::Deepseek {
                            body["messages"][0]["content"].as_str().unwrap()
                        } else {
                            body["instructions"].as_str().unwrap()
                        };
                        assert!(system.contains("参数校验失败也计入工具次数"));
                        assert!(system.contains("不代表环境没有保存能力"));
                        assert!(system.contains("原生 function/tool call"));
                        let facts: Value = serde_json::from_str(
                            system
                                .lines()
                                .find(|line| {
                                    line.starts_with("{\"status\":\"available\"")
                                        && line.contains("recordedCalls")
                                })
                                .unwrap(),
                        )
                        .unwrap();
                        assert_eq!(facts["turns"][0]["toolDispatched"], false);
                        assert_eq!(facts["turns"][0]["recordedCalls"], json!([]));
                        let tool = match index {
                            0 => {
                                let messages = body[if provider == ProviderId::Deepseek { "messages" } else { "input" }].as_array().unwrap();
                                assert!(!messages.iter().any(|message| message["role"] == "assistant"));
                                let reference = messages.iter().find(|message| message["content"].as_str().is_some_and(|text| text.contains("历史对话引用：助手") && text.contains("林溪喜欢观察。"))).unwrap();
                                assert_eq!(reference["role"], "user");
                                let text = reference["content"].as_str().unwrap();
                                assert!(text.contains("本历史轮未调度工具"));
                                historical_source = json!({
                                    "messageId":text.lines().find_map(|line| line.strip_prefix("messageId=")).unwrap(),
                                    "contentHash":text.lines().find_map(|line| line.strip_prefix("contentHash=")).unwrap()
                                });
                                let catalog: Value = serde_json::from_str(
                                    system
                                        .lines()
                                        .find(|line| {
                                            line.starts_with("[{")
                                                && line.contains("\"currentTurn\"")
                                        })
                                        .unwrap(),
                                )
                                .unwrap();
                                assert!(!catalog.as_array().unwrap().iter().any(|row| row["messageId"] == historical_source["messageId"]));
                                source = catalog
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .find(|row| row["currentTurn"] == true && row["role"] == "user")
                                    .unwrap()
                                    .clone();
                                Some((
                                    "novelist_read_context",
                                    nr504::read_arguments("create-character", None),
                                ))
                            }
                            1 => {
                                let result = nr403::last_result(&body, provider);
                                valid = nr504::change_arguments(
                                    false,
                                    &source,
                                    &result["output"]["changeEvidence"]["methodReceiptId"],
                                    false,
                                    0,
                                );
                                valid["sourceKind"] = json!("accepted_suggestion");
                                valid["sources"].as_array_mut().unwrap().push(historical_source.clone());
                                let mut invalid = valid.clone();
                                invalid.as_object_mut().unwrap().remove("chapterIntent");
                                Some(("novelist_propose_change", invalid))
                            }
                            2 => {
                                let result = nr403::last_result(&body, provider);
                                assert_eq!(result["errorCode"], "agent.tool_arguments_invalid");
                                assert_eq!(
                                    result["argumentIssues"],
                                    json!([{"path":"$.chapterIntent","issue":"missing_field"}])
                                );
                                assert!(result["guidance"]
                                    .as_str()
                                    .unwrap()
                                    .contains("显式传 null"));
                                assert!(body["tools"]
                                    .to_string()
                                    .contains("novelist_propose_change"));
                                if second_invalid {
                                    valid["operations"][0]["methodReceiptId"] =
                                        json!("untrusted-method-receipt");
                                }
                                Some(("novelist_propose_change", valid.clone()))
                            }
                            _ => {
                                let result = nr403::last_result(&body, provider);
                                if second_invalid {
                                    assert_eq!(result["errorCode"], "canon.change_method_not_read");
                                    assert!(result["guidance"]
                                        .as_str()
                                        .unwrap()
                                        .contains("methodReceiptId"));
                                } else {
                                    assert_eq!(result["output"]["status"], "committed");
                                }
                                assert!(!body["tools"]
                                    .to_string()
                                    .contains("novelist_propose_change"));
                                assert!(body["tools"]
                                    .to_string()
                                    .contains("novelist_read_context"));
                                None
                            }
                        };
                        nr403::emit(&mut stream, provider, index, tool);
                    }
                });
                let client = ProviderHttpClient::for_test(provider, endpoint).unwrap();
                let terminal = super::super::execute_memory_managed_turn_with_changes(
                    &client,
                    &fixture.handle,
                    &started.turn_id,
                    provider,
                    b"synthetic-loopback-key",
                    &CancellationToken::default(),
                    32,
                    coordinator.clone(),
                    |_| {},
                )
                .unwrap();
                server.join().unwrap();
                assert_eq!(terminal.state, ConversationProviderTerminalState::Completed);
                let receipts = coordinator.receipts().unwrap();
                assert_eq!(receipts.len(), usize::from(!second_invalid));
                assert!(receipts
                    .iter()
                    .all(|receipt| receipt.status == ChangeStatus::Committed));
                assert_eq!(
                    fs::read_dir(fixture.root.join("characters"))
                        .unwrap()
                        .count(),
                    usize::from(!second_invalid)
                );
                let tasks = find_turn_tasks(&fixture.handle, &started.turn_id).unwrap();
                assert_eq!(tasks.len(), 3);
                assert_eq!(
                    tasks
                        .iter()
                        .filter(|task| task.status == TaskStatus::Failed)
                        .count(),
                    if second_invalid { 2 } else { 1 }
                );
                let persisted =
                    fs::read_to_string(fixture.root.join(".novelgainker/tasks/session.json"))
                        .unwrap();
                assert!(!persisted.contains("argumentIssues"));
                assert!(!persisted.contains("proposedMarkdown"));
                assert!(!persisted.contains("untrusted-method-receipt"));
            }
        }
    }
}
