mod nr503 {
    use super::*;
    use crate::canon_change::ChangeStatus;
    use crate::canon_change_store::CanonChangeCoordinator;
    use crate::coordination::try_acquire_project_write_lease;
    use crate::recovery::RecoveryStore;
    use crate::workspace::sha256_hex;
    use serde_json::{json, Value};

    #[test]
    fn nr503_native_providers_maintain_from_manual_chapter_and_isolate_failures() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            for stale in [false, true] {
                for final_failure in [false, true] { run(provider, stale, final_failure); }
            }
        }
    }

    fn run(provider: ProviderId, stale: bool, final_failure: bool) {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("chapters")).unwrap();
        fs::create_dir(fixture.root.join("characters")).unwrap();
        let chapter = format!("---\nid: {}\ntitle: 第一章\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 1\nstatus: draft\n---\n\n# 第一章\n\n## Draft\n\n林溪喜欢观察。\n",Uuid::new_v4());
        let path = fixture.root.join("chapters/source.md");
        fs::write(&path, &chapter).unwrap();
        let lease = Arc::new(try_acquire_project_write_lease(&fixture.handle.coordination_key()).unwrap().unwrap());
        let coordinator = Arc::new(CanonChangeCoordinator::new_nr5(fixture.handle.clone(), lease, RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap()).unwrap());
        coordinator.save_manual("chapters/source.md", &chapter, &sha256_hex(chapter.as_bytes())).unwrap();
        let source_receipt = coordinator.receipts().unwrap().remove(0);
        let started = start_conversation_turn(&fixture.handle, provider, "synthetic", "根据第一章的明确人物事实维护资料；推断与冲突先问我。", 1).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
        let external = chapter.replace("喜欢观察", "不再喜欢观察");
        let expected = if stale {external.clone()} else {chapter.clone()};
        let server = thread::spawn(move || {
            let mut sources = Value::Null;
            let mut maximum_body = 0;
            for index in 0..3 {
                let mut stream = nr502::accept(&listener);
                let body = nr502::request_json(&mut stream);
                maximum_body = maximum_body.max(serde_json::to_vec(&body).unwrap().len());
                if index == 0 {
                    let system = if provider == ProviderId::Deepseek { body["messages"][0]["content"].as_str().unwrap() } else { body["instructions"].as_str().unwrap() };
                    assert!(system.contains("章节来源维护五类资料"));
                    sources = serde_json::from_str(system.lines().find(|line| line.starts_with("[{") && line.contains("\"currentTurn\"")).unwrap()).unwrap();
                    let args = crate::canon_context::normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":"create-character","canonOperations":[{"kind":"read","relativePath":"chapters/source.md","startLine":1,"maxBytes":32768}]})).unwrap();
                    nr403::emit(&mut stream, provider, index, Some(("novelist_read_context", args)));
                } else if index == 1 {
                    let result = nr403::last_result(&body, provider);
                    assert_eq!(result["status"], "succeeded");
                    assert_eq!(result["output"]["chapterSources"][0]["status"], "current");
                    let user = sources.as_array().unwrap().iter().find(|source| source["currentTurn"] == true && source["role"] == "user").unwrap();
                    let args = json!({"schemaVersion":2,"sourceKind":"chapter_commit","risk":"low","chapterIntent":null,"finalize":null,"candidateId":null,
                        "sources":[{"messageId":user["messageId"],"contentHash":user["contentHash"]}],
                        "operations":[{"kind":"create","entityKind":"character","capabilityId":"create-character","methodReceiptId":result["output"]["changeEvidence"]["methodReceiptId"],
                            "targetId":null,"readReceiptId":null,"expectedSourceHash":null,
                            "proposedMarkdown":"---\nid: new\ntitle: 林溪\nkind: character\nschemaVersion: 2\nwritingLanguage: zh-CN\n---\n\n# 林溪\n\n## 摘要\n\n喜欢观察。\n"}]});
                    if stale { fs::write(&path, &external).unwrap(); }
                    nr403::emit(&mut stream, provider, index, Some(("novelist_propose_change", args)));
                } else {
                    let result = nr403::last_result(&body, provider);
                    assert_eq!(result["status"], if stale {"failed"} else {"succeeded"});
                    if !stale { assert_eq!(result["output"]["status"], "committed"); }
                    assert_eq!(body["tool_choice"], "auto", "NR-504 preserves independent remaining work after a failed or successful mutation");
                    if final_failure { write!(stream,"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap(); }
                    else { nr403::emit(&mut stream, provider, index, None); }
                }
            }
            println!("NR503_LOOP provider={provider:?} stale={stale} final_failure={final_failure} attempts=3 maximum_body={maximum_body} external=0");
        });
        let client = ProviderHttpClient::for_test(provider, endpoint).unwrap();
        let terminal = super::super::execute_memory_managed_turn_with_changes(&client, &fixture.handle, &started.turn_id, provider, b"synthetic-loopback-key", &CancellationToken::default(), 2, coordinator.clone(), |_| {}).unwrap();
        server.join().unwrap();
        assert_eq!(terminal.state, if final_failure {ConversationProviderTerminalState::Incomplete} else {ConversationProviderTerminalState::Completed});
        if !final_failure { assert_eq!(terminal.session.turns.last().unwrap().messages[1].content, "自然回复完整保留。"); }
        let records = coordinator.receipts().unwrap();
        assert_eq!(records.iter().find(|receipt| receipt.change_id == source_receipt.change_id).unwrap().status, ChangeStatus::Committed);
        assert_eq!(fs::read_to_string(fixture.root.join("chapters/source.md")).unwrap(), expected);
        assert_eq!(fs::read_dir(fixture.root.join("characters")).unwrap().count(), usize::from(!stale));
        assert_eq!(records.len(), if stale {1} else {2});
        let tasks = find_turn_tasks(&fixture.handle, &started.turn_id).unwrap();
        assert_eq!(tasks.len(), 2);
        let mutation = tasks.iter().find(|task| task.tool_name == crate::canon_change::PROPOSE_CHANGE_TOOL_NAME).unwrap();
        assert_eq!(mutation.status, if stale {TaskStatus::Failed} else {TaskStatus::Succeeded});
    }
}
