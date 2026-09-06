mod nr502 {
    use super::*;
    use crate::canon_change::{ChangeDecision, ChangeListRequest, ChangeStatus};
    use crate::canon_change_store::CanonChangeCoordinator;
    use crate::chapter_contract::{ChapterDocument, NR5_REQUEST_BYTES};
    use crate::coordination::try_acquire_project_write_lease;
    use crate::recovery::RecoveryStore;
    use crate::workspace::{list_project_files, read_project_file, sha256_hex};
    use serde_json::{json, Value};

    #[test]
    fn nr502_zero_tool_chat_and_invalid_write_preserve_full_visible_reply() {
        nr403::zero_tool_chat_for(true);
    }

    pub(super) fn accept(listener: &TcpListener) -> std::net::TcpStream {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                    return stream;
                },
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                Err(error) => panic!("missing synthetic chapter request: {error}"),
            }
        }
    }

    pub(super) fn request_json(stream: &mut std::net::TcpStream) -> Value {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let length = stream.read(&mut chunk).unwrap();
            assert!(length > 0);
            bytes.extend_from_slice(&chunk[..length]);
            assert!(bytes.len() <= NR5_REQUEST_BYTES + 8192);
            let Some(header) = bytes.windows(4).position(|value| value == b"\r\n\r\n") else { continue; };
            assert!(header <= 8192);
            let head = std::str::from_utf8(&bytes[..header]).unwrap();
            let length = head.lines().find_map(|line| { let (key,value) = line.split_once(':')?; key.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().unwrap()) }).unwrap();
            assert!(length <= NR5_REQUEST_BYTES);
            if bytes.len() >= header + 4 + length { return serde_json::from_slice(&bytes[header + 4..header + 4 + length]).unwrap(); }
        }
    }

    fn markdown(id: &str) -> String {
        format!("---\nid: {id}\ntitle: 第一章\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 1\nstatus: draft\n---\n\n# 第一章\n\n## Draft\n\n{}\n", "\"".repeat(29 * 1024))
    }

    #[test]
    fn nr502_both_native_providers_read_commit_reconcile_and_undo_large_escaped_chapters() {
        for provider in [ProviderId::Deepseek, ProviderId::Openai] {
            for existing in [false, true] {
                for final_failure in [false, true] { run_case(provider, existing, final_failure); }
            }
        }
    }

    fn run_case(provider: ProviderId, existing: bool, final_failure: bool) {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("chapters")).unwrap();
        let id = Uuid::new_v4().to_string();
        let original = markdown(&id);
        if existing { fs::write(fixture.root.join("chapters/existing.md"), &original).unwrap(); }
        let lease = Arc::new(try_acquire_project_write_lease(&fixture.handle.coordination_key()).unwrap().unwrap());
        let coordinator = Arc::new(CanonChangeCoordinator::new_nr5(fixture.handle.clone(), lease, RecoveryStore::new(fixture.root.join("fixture-recovery")).unwrap()).unwrap());
        let started = start_conversation_turn(&fixture.handle, provider, "synthetic", if existing { "读取 chapters/existing.md，补充章末后保存，其他内容保持。" } else { "写并保存第一章。" }, 1).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
        let proposed = if existing { format!("{original}补充。\n") } else { markdown("new") };
        let expected = proposed.clone();
        let server = thread::spawn(move || {
            let attempts = if existing { 4 } else { 3 };
            let mut sources = Value::Null;
            let mut maximum = 0usize;
            for index in 0..attempts {
                let mut stream = accept(&listener);
                let body = request_json(&mut stream);
                maximum = maximum.max(serde_json::to_vec(&body).unwrap().len());
                if index == 0 {
                    let system = if provider == ProviderId::Deepseek { body["messages"][0]["content"].as_str().unwrap() } else { body["instructions"].as_str().unwrap() };
                    sources = serde_json::from_str(system.lines().find(|line| line.starts_with("[{") && line.contains("\"currentTurn\"")).unwrap()).unwrap();
                    assert!(system.contains("v2 变更工具支持五类资料及 chapter"));
                    let operation = if existing { json!({"kind":"read","relativePath":"chapters/existing.md","startLine":1,"maxBytes":32768}) } else { json!({"kind":"list","scopes":["chapters"],"limit":64}) };
                    let args = crate::canon_context::normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":"novel-story-telling","canonOperations":[operation]})).unwrap();
                    nr403::emit(&mut stream, provider, index, Some(("novelist_read_context",args)));
                } else if existing && index == 1 {
                    let result = nr403::last_result(&body,provider);
                    assert_eq!(result["status"], "succeeded", "{result}");
                    assert_eq!(result["output"]["canon"][0]["truncated"], true);
                    assert_eq!(result["output"]["changeEvidence"]["completeTargets"], json!([]));
                    let cursor = &result["output"]["canon"][0]["nextCursor"];
                    let args = crate::canon_context::normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":"novel-story-telling","canonOperations":[{"kind":"read","relativePath":"chapters/existing.md","cursor":cursor,"maxBytes":32768}]})).unwrap();
                    nr403::emit(&mut stream,provider,index,Some(("novelist_read_context",args)));
                } else if index + 1 < attempts {
                    let result = nr403::last_result(&body,provider);
                    assert_eq!(result["status"],"succeeded", "{result}");
                    let proof = &result["output"]["changeEvidence"];
                    let target = &proof["completeTargets"][0];
                    if existing { assert!(target["readReceiptId"].is_string()); }
                    let source = sources.as_array().unwrap().iter().find(|source| source["role"] == "user" && source["currentTurn"] == true).unwrap();
                    let args = json!({"schemaVersion":2,"sourceKind":"user_decision","risk":"low","chapterIntent":"write_and_save","finalize":false,"candidateId":null,
                        "sources":[{"messageId":source["messageId"],"contentHash":source["contentHash"]}],
                        "operations":[{"kind":if existing {"update"}else{"create"},"entityKind":"chapter","capabilityId":"novel-story-telling","methodReceiptId":proof["methodReceiptId"],"targetId":target["targetId"],"readReceiptId":target["readReceiptId"],"expectedSourceHash":target["expectedSourceHash"],"proposedMarkdown":proposed}]});
                    assert!(serde_json::to_vec(&args).unwrap().len() > crate::tool_runtime::MAX_TOOL_CONTINUATION_BYTES);
                    nr403::emit(&mut stream,provider,index,Some(("novelist_propose_change",args)));
                } else {
                    let result = nr403::last_result(&body,provider);
                    assert_eq!(result["status"],"succeeded", "{result}");
                    assert_eq!(result["output"]["status"],"committed", "{result}");
                    assert_eq!(body["tool_choice"], "auto", "NR-504 leaves unused tools available; the synthetic model may stop without consuming them");
                    if final_failure { write!(stream,"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap(); }
                    else { nr403::emit(&mut stream,provider,index,None); }
                }
            }
            println!("NR502_LOOP provider={provider:?} existing={existing} final_failure={final_failure} attempts={attempts} maximum_body={maximum} cap={NR5_REQUEST_BYTES} external=0");
        });
        let client = ProviderHttpClient::for_test(provider,endpoint).unwrap();
        let terminal = super::super::execute_memory_managed_turn_with_changes(&client,&fixture.handle,&started.turn_id,provider,b"synthetic-loopback-key",&CancellationToken::default(),2,coordinator.clone(),|_| {}).unwrap();
        server.join().unwrap();
        assert_eq!(terminal.state, if final_failure { ConversationProviderTerminalState::Incomplete } else { ConversationProviderTerminalState::Completed });
        let receipt = coordinator.receipts().unwrap().remove(0);
        assert_eq!(receipt.status,ChangeStatus::Committed);
        let saved = read_project_file(&fixture.handle,&receipt.files[0].relative_path).unwrap();
        let actual = ChapterDocument::parse(&saved.relative_path,saved.content.as_bytes()).unwrap();
        assert_eq!(actual.content_hash,receipt.files[0].after_hash);
        if existing { assert_eq!(saved.content,expected); }
        assert!(!list_project_files(&fixture.handle).unwrap().nodes.is_empty());
        let tasks = find_turn_tasks(&fixture.handle,&started.turn_id).unwrap();
        assert_eq!(tasks.len(), if existing {3}else{2});
        assert!(tasks.iter().all(|task| task.status == TaskStatus::Succeeded && task.tool_version == 2));
        let recovered = crate::canon_change_store::task_recovery_result_for(&fixture.handle,&started.session_id,&started.turn_id,&sha256_hex(format!("change_call_{}",if existing {2}else{1}).as_bytes()),2,crate::protocol_profile::ToolProtocolProfile::Nr5).unwrap();
        assert_eq!(recovered.output_json.unwrap()["status"],"committed");
        let page = coordinator.activities_page(&started.session_id,&ChangeListRequest {project_session_id:Uuid::new_v4().to_string(),turn_id:Some(started.turn_id),cursor:None,limit:100}).unwrap();
        assert_eq!(page.items.len(),1);
        assert!(!coordinator.detail(&started.session_id,&receipt.change_id).unwrap().files[0].lines.is_empty());
        coordinator.decide(&started.session_id,&receipt.change_id,receipt.revision,ChangeDecision::Undo,&CancellationToken::default()).unwrap();
        if existing { assert_eq!(fs::read_to_string(fixture.root.join("chapters/existing.md")).unwrap(),original); }
        else { assert!(!fixture.root.join(&receipt.files[0].relative_path).exists()); }
    }
}
