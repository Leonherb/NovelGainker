mod nr505 {
    use super::*;
    use crate::canon_change_store::CanonChangeCoordinator;
    use crate::coordination::try_acquire_project_write_lease;
    use crate::publication::PublicationStatus;
    use crate::recovery::RecoveryStore;
    use serde_json::{json, Value};

    #[test]
    fn nr505_real_publication_eight_agent_one_compactor_through_production_entry() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] { nr504::run(provider, true, 0, true, true); }
    }

    #[test]
    fn nr505_native_publish_success_failure_and_partial_reply_preserve_committed_chapter() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            for mode in 0..4 { run(provider, mode); }
        }
    }

    fn run(provider: ProviderId, mode: u8) {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("chapters")).unwrap();
        fs::write(fixture.root.join("project.md"), if mode == 1 { "# 缺失元数据\n" } else { "---\ntitle: 生产入口合成测试\nauthorDisplayName: 合成作者\nwritingLanguage: zh-CN\nkind: project\nschemaVersion: 1\n---\n" }).unwrap();
        let lease = Arc::new(try_acquire_project_write_lease(&fixture.handle.coordination_key()).unwrap().unwrap());
        let coordinator = Arc::new(CanonChangeCoordinator::new_nr5(fixture.handle.clone(), lease, RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap()).unwrap());
        let started = start_conversation_turn(&fixture.handle, provider, "synthetic", "写并保存第一章，确认这版定稿并构建本地 EPUB。", 1).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap(); listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut source = Value::Null;
            let mut maximum_body = 0;
            for index in 0..5 {
                let mut stream = nr502::accept(&listener);
                let body = nr502::request_json(&mut stream);
                maximum_body = maximum_body.max(serde_json::to_vec(&body).unwrap().len());
                let call = match index {
                    0 => {
                        assert_eq!(body["tools"].as_array().unwrap().len(), 3);
                        let system = if provider == ProviderId::Deepseek { body["messages"][0]["content"].as_str().unwrap() } else { body["instructions"].as_str().unwrap() };
                        let sources: Value = serde_json::from_str(system.lines().find(|line| line.starts_with("[{") && line.contains("\"currentTurn\"")).unwrap()).unwrap();
                        source = sources.as_array().unwrap().iter().find(|source| source["currentTurn"] == true && source["role"] == "user").unwrap().clone();
                        Some(("novelist_read_context", crate::canon_context::normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":"novel-story-telling","canonOperations":[]})).unwrap()))
                    }
                    1 => {
                        let result = nr403::last_result(&body, provider);
                        Some(("novelist_propose_change", json!({"schemaVersion":2,"sourceKind":"user_decision","risk":"low","chapterIntent":"write_and_save","finalize":true,"candidateId":null,
                            "sources":[{"messageId":source["messageId"],"contentHash":source["contentHash"]}],
                            "operations":[{"kind":"create","entityKind":"chapter","capabilityId":"novel-story-telling","methodReceiptId":result["output"]["changeEvidence"]["methodReceiptId"],"targetId":null,"readReceiptId":null,"expectedSourceHash":null,
                                "proposedMarkdown":"---\nid: new\ntitle: 第一章\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 1\nstatus: draft\n---\n\n# 第一章\n\n## Draft\n\n桥上有风。\n"}]})))
                    }
                    2 => {
                        assert_eq!(nr403::last_result(&body, provider)["output"]["status"], "committed");
                        Some(("novelist_read_context", crate::canon_context::normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":"publish-novel","canonOperations":[]})).unwrap()))
                    }
                    3 => {
                        let result = nr403::last_result(&body, provider);
                        Some(("novelist_publish", json!({"schemaVersion":2,"action":"build","methodReceiptId":result["output"]["changeEvidence"]["methodReceiptId"],"source":{"messageId":source["messageId"],"contentHash":source["contentHash"]}})))
                    }
                    _ => {
                        let result = nr403::last_result(&body, provider);
                        assert_eq!(result["status"], if mode == 1 { "failed" } else { "succeeded" });
                        if mode == 1 { assert_eq!(result["errorCode"], "publication.metadata_required"); }
                        else { assert_eq!(result["output"]["status"], "generated"); }
                        assert!(!body["tools"].as_array().unwrap().iter().any(|tool| (if provider == ProviderId::Deepseek { &tool["function"]["name"] } else { &tool["name"] }) == "novelist_publish"));
                        if mode == 2 { write!(stream, "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap(); continue; }
                        if mode == 3 {
                            let delta = if provider == ProviderId::Deepseek { json!({"choices":[{"delta":{"content":"原稿与 EPUB 已保留。"},"finish_reason":null}]}) } else { json!({"type":"response.output_text.delta","delta":"原稿与 EPUB 已保留。","sequence_number":1}) };
                            let wire = format!("data: {delta}\n\n"); write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", wire.len(), wire).unwrap(); continue;
                        }
                        None
                    }
                };
                nr403::emit(&mut stream, provider, index, call);
            }
            println!("NR505_LOOP provider={provider:?} mode={mode} agent=5 compactor=0 requested=20480 maximum_body={maximum_body} publication_fixture=false external=0");
        });
        let client = ProviderHttpClient::for_test(provider, endpoint).unwrap();
        let terminal = super::super::execute_memory_managed_turn_with_changes(&client, &fixture.handle, &started.turn_id, provider, b"synthetic-loopback-key", &CancellationToken::default(), 2, coordinator.clone(), |_| {}).unwrap();
        server.join().unwrap();
        assert_eq!(terminal.state, if mode >= 2 { ConversationProviderTerminalState::Incomplete } else { ConversationProviderTerminalState::Completed });
        let visible = &terminal.session.turns.last().unwrap().messages[1].content;
        if mode <= 1 { assert_eq!(visible, "自然回复完整保留。"); }
        if mode == 3 { assert!(visible.starts_with("原稿与 EPUB 已保留。")); assert!(visible.ends_with(crate::conversation::INCOMPLETE_TEXT)); }
        let changes = coordinator.receipts().unwrap(); assert_eq!(changes.len(), 1); assert_eq!(changes[0].status, crate::canon_change::ChangeStatus::Committed);
        for file in &changes[0].files { assert_eq!(crate::workspace::sha256_hex(&fs::read(fixture.root.join(&file.relative_path)).unwrap()), file.after_hash); }
        let page = coordinator.publication_list(None).unwrap(); assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].status, if mode == 1 { PublicationStatus::ValidationFailed } else { PublicationStatus::Generated });
        if mode != 1 { assert_eq!(Some(crate::workspace::sha256_hex(&fs::read(fixture.root.join(page.items[0].relative_path.as_ref().unwrap())).unwrap())), page.items[0].output_hash); }
        let tasks = find_turn_tasks(&fixture.handle, &started.turn_id).unwrap(); assert_eq!(tasks.len(), 4);
        assert_eq!(tasks.iter().find(|task| task.tool_name == "novelist.publish").unwrap().status, if mode == 1 { TaskStatus::Failed } else { TaskStatus::Succeeded });
    }
}
