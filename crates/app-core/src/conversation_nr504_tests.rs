mod nr504 {
    use super::*;
    use crate::capability_pack::embedded_pack;
    use crate::memory::NR1_CONTEXT_PROFILE;
    use crate::tool_runtime::ToolRegistry;
    use crate::canon_change::{ChangeEvidence, ChangeStatus};
    use crate::canon_change_store::CanonChangeCoordinator;
    use crate::coordination::try_acquire_project_write_lease;
    use crate::recovery::RecoveryStore;
    use serde_json::{json, Value};

    fn read_arguments(capability: &str, path: Option<&str>) -> Value {
        let operation = path.map_or_else(|| json!({"kind":"list","scopes":[if capability == "create-character" {"characters"} else {"chapters"}],"limit":64}),
            |path| json!({"kind":"read","relativePath":path,"startLine":1,"maxBytes":32768}));
        crate::canon_context::normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":capability,"canonOperations":[operation]})).unwrap()
    }

    fn native_continuation(provider: ProviderId, call: crate::tool_runtime::ProviderToolCall) -> crate::provider_core::ProviderContinuation {
        use crate::provider_core::{ProviderContinuation, OpenAiContinuationItem};
        match provider {
            ProviderId::Deepseek => ProviderContinuation::Deepseek {assistant_text: String::new(), call},
            ProviderId::Openai => ProviderContinuation::Openai {items:vec![OpenAiContinuationItem::FunctionCall {item_id:format!("fc_{}",call.call_id),call_id:call.call_id,name:call.name,arguments:call.arguments_json,status:"completed".to_owned(),phase:None}]},
        }
    }

    #[test]
    fn nr504_escaped_native_reserve_rejects_before_actual_executor_and_smaller_request_commits() {
        use crate::canon_change::{ChangeTurnBudget, PROPOSE_CHANGE_TOOL_NAME};
        use crate::provider_core::NormalizedRequest;
        use crate::protocol_profile::ToolProtocolProfile;
        use crate::tool_runtime::{ProviderToolCall, ToolExecutionContext, ToolResult, ToolResultStatus};
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            let fixture = Fixture::new();
            fs::create_dir(fixture.root.join("characters")).unwrap();
            let lease = Arc::new(try_acquire_project_write_lease(&fixture.handle.coordination_key()).unwrap().unwrap());
            let coordinator = Arc::new(CanonChangeCoordinator::new_nr5(fixture.handle.clone(),lease,RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap()).unwrap());
            let started = start_conversation_turn(&fixture.handle,provider,"synthetic","记录林溪喜欢观察的人物设定。",1).unwrap();
            let session = read_conversation(&fixture.handle).unwrap();
            let evidence = Arc::new(Mutex::new(ChangeEvidence::from_conversation(&session,&started.turn_id).unwrap()));
            let registry = ToolRegistry::novelist_with_changes(&fixture.handle,embedded_pack().unwrap(),session.session_id.clone(),started.turn_id.clone(),coordinator.clone(),evidence.clone()).unwrap();
            let read_args = read_arguments("create-character",None);
            let read_call = ProviderToolCall {call_id:"source_read".to_owned(),name:crate::canon_context::NOVELIST_READ_CONTEXT_TOOL_NAME.to_owned(),arguments_json:read_args.to_string()};
            let (_,reader) = registry.lookup(&read_call).unwrap();
            let output = reader.execute(&ToolExecutionContext {project_id:session.project_id.clone(),conversation_session_id:session.session_id.clone(),turn_id:started.turn_id.clone(),call_id:read_call.call_id}, &read_args,&CancellationToken::default());
            assert_eq!(output.status,ToolResultStatus::Succeeded);
            let source = serde_json::to_value(&evidence.lock().unwrap().source_catalog()[0]).unwrap();
            let args = change_arguments(false,&source,&output.output_json.unwrap()["changeEvidence"]["methodReceiptId"],false,0);
            for oversized in [true,false] {
                let mut request = NormalizedRequest::user_text("原生字节边界夹具".to_owned(),4096).unwrap().with_tool_definitions(registry.definitions()).unwrap();
                let mut budget = ChangeTurnBudget::for_protocol(ToolProtocolProfile::Nr5);
                budget.consume_compactor().unwrap();
                // Synthetic bounded history exercises exact native double escaping;
                // the actual write authority above still comes from the real read.
                for index in 0..4 {
                    budget.consume_agent(4096).unwrap();
                    budget.consume_read().unwrap();
                    let call = ProviderToolCall {call_id:format!("history_{index}"),name:crate::canon_context::NOVELIST_READ_CONTEXT_TOOL_NAME.to_owned(),
                        arguments_json:json!({"quoted":"\"".repeat(if oversized {16*1024}else{1})}).to_string()};
                    let result = ToolResult {call_id:call.call_id.clone(),name:call.name.clone(),version:2,status:ToolResultStatus::Succeeded,
                        output_json:Some(json!({"quoted":"\"".repeat(if oversized {28*1024}else{1})})),error_code:None};
                    request = request.continue_after_tool(native_continuation(provider,call),result,4096).unwrap().with_tool_definitions(registry.definitions()).unwrap();
                }
                budget.consume_agent(4096).unwrap();
                let call = ProviderToolCall {call_id:if oversized {"blocked_native"}else{"safe_native"}.to_owned(),name:PROPOSE_CHANGE_TOOL_NAME.to_owned(),arguments_json:args.to_string()};
                let (definition,executor) = registry.lookup(&call).unwrap();
                let continuation = native_continuation(provider,call.clone());
                let placeholder = ToolResult {call_id:call.call_id.clone(),name:call.name.clone(),version:2,status:ToolResultStatus::Failed,output_json:None,error_code:Some("canon.change_budget_exceeded".to_owned())};
                let exact = request.continue_after_tool(continuation.clone(),placeholder,4096).unwrap().serialized_request_bytes(provider).unwrap();
                let reserved = exact + 2 * crate::chapter_contract::NR5_TOOL_RESULT_BYTES + 2048;
                assert_eq!(reserved > crate::chapter_contract::NR5_REQUEST_BYTES,oversized);
                let preflight = super::super::changes::reserve_side_effect_reply(&request,continuation,&call,definition,provider,&mut budget).err().map(|_|"canon.change_budget_exceeded");
                assert_eq!(preflight.is_some(),oversized);
                let result = super::super::execute_registered_tool(&fixture.handle,&session,&started.turn_id,provider,&call,definition,executor,&registry,&CancellationToken::default(),2,preflight,&mut |_|{}).unwrap();
                assert_eq!(result.status,if oversized {ToolResultStatus::Failed}else{ToolResultStatus::Succeeded});
                assert_eq!(coordinator.receipts().unwrap().len(),usize::from(!oversized));
                assert_eq!(fs::read_dir(fixture.root.join("characters")).unwrap().count(),usize::from(!oversized));
                println!("NR504_RESERVE provider={provider:?} oversized={oversized} exact={exact} reserved={reserved} executed={} external=0",usize::from(!oversized));
            }
            let tasks = find_turn_tasks(&fixture.handle,&started.turn_id).unwrap();
            assert_eq!(tasks.len(),2);
            assert_eq!(tasks.iter().filter(|t|t.status == TaskStatus::Failed).count(),1);
        }
    }

    fn change_arguments(chapter: bool, source: &Value, method: &Value, from_chapter: bool, mode: u8) -> Value {
        let markdown = if chapter {
            "---\nid: new\ntitle: 第一章\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 1\nstatus: draft\n---\n\n# 第一章\n\n## Draft\n\n林溪喜欢观察。\n"
        } else {
            "---\nid: new\ntitle: 林溪\nkind: character\nschemaVersion: 2\nwritingLanguage: zh-CN\n---\n\n# 林溪\n\n## 摘要\n\n喜欢观察。\n"
        };
        json!({"schemaVersion":2,"sourceKind":if from_chapter {"chapter_commit"} else {"user_decision"},"risk":if mode == 1 {"broad_replacement"} else {"low"},
            "chapterIntent":if chapter {json!("write_and_save")}else{Value::Null},"finalize":if chapter {json!(false)}else{Value::Null},"candidateId":null,
            "sources":[{"messageId":source["messageId"],"contentHash":source["contentHash"]}],
            "operations":[{"kind":"create","entityKind":if chapter {"chapter"}else{"character"},"capabilityId":if chapter {"novel-story-telling"}else{"create-character"},
                "methodReceiptId":method,"targetId":null,"readReceiptId":null,"expectedSourceHash":null,"proposedMarkdown":if mode == 2 {"invalid synthetic candidate"}else{markdown}}]})
    }

    #[test]
    fn nr504_native_production_loop_allows_both_orders_and_independent_terminal_results() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            for chapter_first in [false, true] {
                for mode in 0..4 { run(provider, chapter_first, mode, false, false); }
            }
        }
    }

    #[test]
    fn nr504_native_eight_agent_plus_compactor_uses_real_memory_and_budget_only_publication_fixture() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] { run(provider, true, 0, true, false); }
    }

    pub(super) fn run(provider: ProviderId, chapter_first: bool, mode: u8, maximum: bool, real_publication: bool) {
        let fixture = Fixture::new();
        if real_publication {
            fs::write(fixture.root.join("project.md"), "---\ntitle: 完整原生出版链\nauthorDisplayName: 合成作者\nwritingLanguage: zh-CN\nkind: project\nschemaVersion: 1\n---\n").unwrap();
        }
        if maximum {
            for index in 0..20 {
                let started = start_conversation_turn(&fixture.handle, provider, "synthetic", &format!("已结束的讨论 {index} {}", "甲".repeat(900)), 10 + index * 2).unwrap();
                finish_conversation_turn(&fixture.handle, &started.turn_id, &"乙".repeat(900), ConversationTerminalKind::Completed, None, 11 + index * 2).unwrap();
            }
        }
        fs::create_dir(fixture.root.join("chapters")).unwrap();
        fs::create_dir(fixture.root.join("characters")).unwrap();
        let lease = Arc::new(try_acquire_project_write_lease(&fixture.handle.coordination_key()).unwrap().unwrap());
        let coordinator = Arc::new(CanonChangeCoordinator::new_nr5(fixture.handle.clone(), lease, RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap()).unwrap());
        let started = start_conversation_turn(&fixture.handle, provider, "synthetic", "写并保存第一章，并记录人物林溪喜欢观察的明确设定；顺序按需要决定。", 100).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let attempts = if maximum {8} else {5};
            let mut source = Value::Null;
            let mut next_chapter_path = None;
            let mut requested = 0;
            let mut maximum_body = 0;
            for http_index in 0..attempts + usize::from(maximum) {
                let mut stream = nr502::accept(&listener);
                let body = nr502::request_json(&mut stream);
                maximum_body = maximum_body.max(serde_json::to_vec(&body).unwrap().len());
                requested += body[if provider == ProviderId::Deepseek {"max_tokens"}else{"max_output_tokens"}].as_u64().unwrap();
                if maximum && http_index == 0 {
                    assert_eq!(body["stream"], false);
                    assert!(body.get("tools").is_none());
                    let response = if provider == ProviderId::Openai { completion_json() } else {json!({"model":"deepseek-v4-pro","choices":[{"message":{"content":summary()},"finish_reason":"stop"}]}).to_string()};
                    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
                    continue;
                }
                let index = http_index - usize::from(maximum);
                let tool = match index {
                    0 => {
                        assert_eq!(body["tools"].as_array().unwrap().len(), 3, "NR-505 registers exactly three qualified tools");
                        let system = if provider == ProviderId::Deepseek {body["messages"][0]["content"].as_str().unwrap()} else {body["instructions"].as_str().unwrap()};
                        if maximum {assert!(system.contains(summary()));}
                        let catalog: Value = serde_json::from_str(system.lines().find(|line| line.starts_with("[{") && line.contains("\"currentTurn\"")).unwrap()).unwrap();
                        source = catalog.as_array().unwrap().iter().find(|row| row["currentTurn"] == true && row["role"] == "user").unwrap().clone();
                        Some(("novelist_read_context", read_arguments(if chapter_first {"novel-story-telling"}else{"create-character"}, None)))
                    }
                    1 | 3 => {
                        let result = nr403::last_result(&body, provider);
                        assert_eq!(result["status"], "succeeded", "{result}");
                        let chapter = if index == 1 {chapter_first} else {!chapter_first};
                        let from_chapter = index == 3 && chapter_first && mode != 1 && mode != 2;
                        if from_chapter {assert_eq!(result["output"]["chapterSources"][0]["status"], "current");}
                        let mut args = change_arguments(chapter, &source, &result["output"]["changeEvidence"]["methodReceiptId"], from_chapter, if index == 1 {mode}else{0});
                        if real_publication && chapter { args["finalize"] = json!(true); }
                        Some(("novelist_propose_change", args))
                    }
                    2 => {
                        let result = nr403::last_result(&body, provider);
                        assert_eq!(result["status"], if mode == 2 {"failed"}else{"succeeded"}, "{result}");
                        if mode != 2 {assert_eq!(result["output"]["status"], if mode == 1 {"pending_confirmation"}else{"committed"});}
                        if chapter_first && mode != 1 && mode != 2 {next_chapter_path = Some(result["output"]["files"][0]["relativePath"].as_str().unwrap().to_owned());}
                        assert_eq!(body["tool_choice"], "auto", "other work remains independent");
                        Some(("novelist_read_context", read_arguments(if chapter_first {"create-character"}else{"novel-story-telling"}, next_chapter_path.as_deref())))
                    }
                    4 | 5 if maximum => {
                        let result = nr403::last_result(&body, provider);
                        assert_eq!(result["status"], "succeeded", "{result}");
                        Some(("novelist_read_context", read_arguments(if real_publication && index == 5 { "publish-novel" } else { "analytical-review" }, None)))
                    }
                    6 if maximum => {
                        assert_eq!(body["tools"].as_array().unwrap().len(), 1, "four reads and two changes exhausted");
                        let args = if real_publication {
                            let result = nr403::last_result(&body, provider);
                            json!({"schemaVersion":2,"action":"build","methodReceiptId":result["output"]["changeEvidence"]["methodReceiptId"],"source":{"messageId":source["messageId"],"contentHash":source["contentHash"]}})
                        } else { json!({}) };
                        Some(("novelist_publish", args))
                    }
                    _ => {
                        assert_eq!(nr403::last_result(&body, provider)["status"], "succeeded");
                        if real_publication { assert_eq!(nr403::last_result(&body, provider)["output"]["status"], "generated"); }
                        if maximum {assert_eq!(body["tool_choice"],"none"); assert!(body.get("tools").is_none());}
                        if mode == 3 {
                            let delta = if provider == ProviderId::Deepseek {json!({"choices":[{"delta":{"content":"已完成的写入保留；这是部分回复。"},"finish_reason":null}]})}
                                else {json!({"type":"response.output_text.delta","delta":"已完成的写入保留；这是部分回复。","sequence_number":1})};
                            let wire = format!("data: {delta}\n\n");
                            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",wire.len(),wire).unwrap();
                            continue;
                        }
                        None
                    }
                };
                nr403::emit(&mut stream, provider, index, tool);
            }
            assert_eq!(requested, if maximum {33792} else {20480});
            assert!(maximum_body <= crate::chapter_contract::NR5_REQUEST_BYTES);
            let label = if real_publication { "NR505_MAX" } else { "NR504_LOOP" };
            println!("{label} provider={provider:?} chapter_first={chapter_first} mode={mode} maximum={maximum} agent={attempts} compactor={} requested={requested} maximum_body={maximum_body} publication_fixture={} external=0", usize::from(maximum), maximum && !real_publication);
        });
        let client = ProviderHttpClient::for_test(provider, endpoint).unwrap();
        let publication_calls = Arc::new(AtomicUsize::new(0));
        let terminal = if maximum && !real_publication {
            let session = read_conversation(&fixture.handle).unwrap();
            let evidence = ChangeEvidence::from_conversation(&session, &started.turn_id).unwrap();
            let pack = embedded_pack().unwrap();
            let addendum = format!("{}\n{}", pack.agent_system_addendum_with_chapters(), serde_json::to_string(evidence.source_catalog()).unwrap());
            let registry = ToolRegistry::novelist_with_changes(&fixture.handle, pack, session.session_id, started.turn_id.clone(), coordinator.clone(), Arc::new(Mutex::new(evidence))).unwrap();
            let registry = crate::tool_runtime::tests::with_nr504_publication_fixture(registry, publication_calls.clone());
            super::super::execute_with_profile_and_runtime(&client, &fixture.handle, &started.turn_id, provider, b"synthetic-loopback-key", &CancellationToken::default(), 101, NR1_CONTEXT_PROFILE, &registry, &addendum, crate::canon_change::NR4_LOOP_RESERVE_BYTES, |_| {})
        } else {
            super::super::execute_memory_managed_turn_with_changes(&client, &fixture.handle, &started.turn_id, provider, b"synthetic-loopback-key", &CancellationToken::default(), 101, coordinator.clone(), |_| {})
        }.unwrap();
        server.join().unwrap();
        assert_eq!(terminal.state, if mode == 3 {ConversationProviderTerminalState::Incomplete}else{ConversationProviderTerminalState::Completed});
        if mode != 3 { assert_eq!(terminal.session.turns.last().unwrap().messages[1].content,"自然回复完整保留。"); }
        else {
            let visible = &terminal.session.turns.last().unwrap().messages[1].content;
            assert!(visible.starts_with("已完成的写入保留；这是部分回复。"));
            assert!(visible.ends_with(crate::conversation::INCOMPLETE_TEXT));
        }
        let receipts = coordinator.receipts().unwrap();
        assert_eq!(receipts.iter().filter(|r|r.status == ChangeStatus::Committed).count(), if mode == 1 || mode == 2 {1}else{2});
        assert_eq!(receipts.iter().filter(|r|r.status == ChangeStatus::PendingConfirmation).count(), usize::from(mode == 1));
        for receipt in receipts.iter().filter(|r|r.status == ChangeStatus::Committed) {
            for file in &receipt.files {assert_eq!(crate::workspace::sha256_hex(&fs::read(fixture.root.join(&file.relative_path)).unwrap()), file.after_hash);}
        }
        let tasks = find_turn_tasks(&fixture.handle, &started.turn_id).unwrap();
        assert_eq!(tasks.len(), if maximum {7}else{4});
        assert_eq!(tasks.iter().filter(|task| task.status == TaskStatus::Failed).count(), usize::from(mode == 2));
        assert_eq!(publication_calls.load(Ordering::SeqCst), usize::from(maximum && !real_publication));
        if real_publication {
            let page = coordinator.publication_list(None).unwrap();
            assert_eq!(page.items.len(), 1);
            assert_eq!(page.items[0].status, crate::publication::PublicationStatus::Generated);
            let bytes = fs::read(fixture.root.join(page.items[0].relative_path.as_ref().unwrap())).unwrap();
            assert_eq!(Some(crate::workspace::sha256_hex(&bytes)), page.items[0].output_hash);
        } else { assert!(!fixture.root.join("published").exists(), "unused publication quota creates no artifact"); }
        if maximum {assert_eq!(load_valid_memory(&fixture.handle,&terminal.session,NR1_CONTEXT_PROFILE).unwrap().summary_text, summary());}
    }
}
