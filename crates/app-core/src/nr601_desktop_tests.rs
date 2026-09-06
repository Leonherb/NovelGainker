fn catalog(f: &Fixture) -> Value {
    f.ipc(
        "conversation_session_list",
        json!({"request":{"projectSessionId":f.session,"archived":false,"cursor":null}}),
    )
}
#[test]
fn nr602_branch_inherits_no_change_authority_and_archived_original_keeps_hash_guarded_undo() {
    let f = Fixture::new(ProviderId::Deepseek);
    let before = f.snapshot();
    let terminal = f.turn(
        "请创建并保存人物林舟，职业是修表匠。",
        wire::Plan::Create {
            kind: "character",
            title: "林舟".into(),
            text: "林舟是修表匠。".into(),
            chapter: None,
            chapter_source: None,
            risk: "low",
            final_error: false,
        },
    );
    let page = catalog(&f);
    let original = page["activeSessionId"].as_str().unwrap();
    let changes = f.changes();
    assert_eq!(changes.len(), 1);
    let change = changes[0].clone();
    let branch = f.ipc("conversation_session_manage", json!({"request":{"projectSessionId":f.session,"operation":{
        "action":"branch","conversationSessionId":original,"requestId":Uuid::new_v4().to_string(),
        "boundaryTurnId":terminal["conversation"]["turns"][0]["turnId"],"name":"分支讨论","expectedRevision":page["revision"]}}}));
    assert!(f.changes().is_empty());
    assert_ne!(f.snapshot(), before);
    let fresh = f.ipc(
        "conversation_session_load",
        json!({"sessionId":f.session,"conversationSessionId":branch["targetSessionId"]}),
    );
    assert!(fresh["taskActivities"].as_array().unwrap().is_empty());
    f.ipc("conversation_session_manage", json!({"request":{"projectSessionId":f.session,"operation":{
        "action":"archive","conversationSessionId":original,"expectedRevision":branch["catalog"]["revision"]}}}));
    let undone = f.ipc("canon_change_decide", json!({"conversationSessionId":original,"request":{
        "projectSessionId":f.session,"changeId":change["receipt"]["changeId"],"revision":change["receipt"]["revision"],"decision":"undo"}}));
    assert_eq!(undone["status"], "undone");
    assert_eq!(f.snapshot(), before);
    assert_eq!(catalog(&f)["activeSessionId"], branch["targetSessionId"]);
    println!(
        "NR602_BRANCH original_undo=true inherited_tasks=0 inherited_changes=0 replay=0 external=0"
    );
}
#[test]
fn nr602_desktop_manage_archive_clear_restore_and_export_token_lifecycle() {
    let f = Fixture::new(ProviderId::Deepseek);
    let page = catalog(&f);
    let id = page["activeSessionId"].as_str().unwrap();
    let state = f.app.state::<ProjectRegistry>();
    let base = session_handle(&state, &f.session).unwrap();
    let original = sessions::bind_session(&base, Some(id), now_ms().unwrap()).unwrap();
    let made = start_conversation_turn(
        &original,
        ProviderId::Deepseek,
        "deepseek-v4-pro",
        "原始可见消息",
        now_ms().unwrap(),
    )
    .unwrap();
    finish_conversation_turn(
        &original,
        &made.turn_id,
        "原始可见回复",
        ConversationTerminalKind::Completed,
        None,
        now_ms().unwrap(),
    )
    .unwrap();
    let cleared = f.ipc("conversation_session_manage", json!({"request":{"projectSessionId":f.session,"operation":{
        "action":"clear","conversationSessionId":id,"requestId":Uuid::new_v4().to_string(),"name":"新空会话","expectedRevision":page["revision"]}}}));
    assert_ne!(cleared["targetSessionId"], id);
    assert_eq!(cleared["catalog"]["nonArchivedCount"], 1);
    assert!(sessions::is_archived(&original).unwrap());
    assert!(sessions::reserve_run(&original, now_ms().unwrap()).is_err());
    let old = f.ipc(
        "conversation_session_load",
        json!({"sessionId":f.session,"conversationSessionId":id}),
    );
    assert_eq!(
        old["conversation"]["turns"][0]["messages"][1]["content"],
        "原始可见回复"
    );
    let restored = f.ipc("conversation_session_manage", json!({"request":{"projectSessionId":f.session,"operation":{
        "action":"restore","conversationSessionId":id,"expectedRevision":cleared["catalog"]["revision"]}}}));
    assert_eq!(restored["targetSessionId"], id);
    assert!(!sessions::is_archived(&original).unwrap());
    let token = Uuid::new_v4().to_string();
    let make_pending = |expires_at_ms| PendingConversationExport {
        in_flight: false,
        token: token.clone(),
        project_session_id: f.session.clone(),
        expires_at_ms,
        prepared: Some(
            sessions::prepare_export(&base, id, ConversationExportFormat::Json).unwrap(),
        ),
        target: Some(f.root.join("visible.json")),
    };
    // Only the native file picker is substituted; commit uses the actual IPC and filesystem.
    *state.conversation_export.lock().unwrap() = Some(make_pending(now_ms().unwrap() + 300000));
    let wrong = ConversationExportCommitRequest {
        project_session_id: Uuid::new_v4().to_string(),
        token: token.clone(),
    };
    assert!(take_conversation_export(&state, &wrong, now_ms().unwrap()).is_err());
    assert!(state.conversation_export.lock().unwrap().is_some());
    let request = ConversationExportCommitRequest {
        project_session_id: f.session.clone(),
        token: token.clone(),
    };
    let claimed = take_conversation_export(&state, &request, now_ms().unwrap()).unwrap();
    assert!(
        state
            .conversation_export
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .in_flight
    );
    assert!(take_conversation_export(&state, &request, u64::MAX).is_err());
    assert!(
        state.conversation_export.lock().unwrap().is_some(),
        "in-flight capacity cannot expire"
    );
    drop(claimed);
    {
        let _released = ExportPreparePermit {
            state: &state,
            token: token.clone(),
            ready: false,
        };
    }
    *state.conversation_export.lock().unwrap() = Some(make_pending(now_ms().unwrap() + 300000));
    let receipt = f.ipc(
        "conversation_export_commit",
        json!({"request":{"projectSessionId":f.session,"token":token}}),
    );
    assert_eq!(receipt["fileName"], "visible.json");
    assert!(f.root.join("visible.json").exists());
    assert!(take_conversation_export(&state, &request, now_ms().unwrap()).is_err());
    *state.conversation_export.lock().unwrap() = Some(make_pending(1));
    assert!(take_conversation_export(&state, &request, 2).is_err());
    assert!(state.conversation_export.lock().unwrap().is_none());
    *state.conversation_export.lock().unwrap() = Some(make_pending(now_ms().unwrap() + 300000));
    {
        let _cancelled = ExportPreparePermit {
            state: &state,
            token: token.clone(),
            ready: false,
        };
    }
    assert!(state.conversation_export.lock().unwrap().is_none());
    *state.conversation_export.lock().unwrap() = Some(make_pending(now_ms().unwrap() + 300000));
    f.ipc("project_close", json!({"sessionId":f.session}));
    assert!(state.conversation_export.lock().unwrap().is_none());
    assert!(f.events.lock().unwrap().is_empty());
}

#[test]
fn nr602_desktop_export_requests_reject_paths_bytes_unknown_formats_and_extra_fields() {
    for request in [
        json!({"projectSessionId":"id","conversationSessionId":"id","format":"zip"}),
        json!({"projectSessionId":"id","conversationSessionId":"id","format":"json","path":"D:/unauthorized.json"}),
        json!({"projectSessionId":"id","conversationSessionId":"id","format":"json","bytes":"replace"}),
    ] {
        assert!(serde_json::from_value::<ConversationExportPrepareRequest>(request).is_err());
    }
    assert!(serde_json::from_value::<ConversationExportCommitRequest>(
        json!({"projectSessionId":"id","token":"id","path":"D:/unauthorized.json"})
    )
    .is_err());
    assert!(serde_json::from_value::<SessionManageRequest>(json!({"projectSessionId":"id","operation":{"action":"delete","conversationSessionId":"id","expectedRevision":1}})).is_err());
}

#[test]
fn nr602_separate_registry_readonly_does_not_recover_or_mutate_another_writer() {
    let writer = Fixture::new(ProviderId::Deepseek);
    let page = catalog(&writer);
    let original = page["activeSessionId"].as_str().unwrap();
    let writer_state = writer.app.state::<ProjectRegistry>();
    let base = session_handle(&writer_state, &writer.session).unwrap();
    let bound = sessions::bind_session(&base, Some(original), now_ms().unwrap()).unwrap();
    let reservation = sessions::reserve_run(&bound, now_ms().unwrap()).unwrap();
    let started = start_conversation_turn(
        &bound,
        ProviderId::Deepseek,
        "deepseek-v4-pro",
        "另一实例正在生成",
        now_ms().unwrap(),
    )
    .unwrap();
    sessions::bind_run_turn(&bound, &reservation.reservation_id, &started.turn_id).unwrap();
    let before =
        std::fs::read(base.root().join(".novelgainker/conversation/session.json")).unwrap();
    let catalog_before =
        std::fs::read(base.root().join(".novelgainker/conversation/catalog.json")).unwrap();
    let reader = Fixture::new(ProviderId::Deepseek);
    let reader_state = reader.app.state::<ProjectRegistry>();
    assert!(try_acquire_project_write_lease(&base.coordination_key())
        .unwrap()
        .is_none());
    let alias = register_session(&reader_state, None, base.clone(), None).unwrap();
    assert!(session_history_lease(&reader_state, &alias.session_id)
        .unwrap()
        .is_none());
    let listed = reader.ipc(
        "conversation_session_list",
        json!({"request":{
        "projectSessionId":alias.session_id,"archived":false,"cursor":null}}),
    );
    assert_eq!(listed["activeRun"]["turnId"], started.turn_id);
    let viewed = reader.ipc(
        "conversation_session_load",
        json!({
        "sessionId":alias.session_id,"conversationSessionId":original}),
    );
    assert_eq!(viewed["conversation"]["turns"][0]["status"], "running");
    for (cmd, body) in [
        (
            "conversation_turn_start",
            json!({"sessionId":alias.session_id,"conversationSessionId":original,"message":"不允许写入"}),
        ),
        (
            "conversation_session_create",
            json!({"request":{"projectSessionId":alias.session_id,"requestId":Uuid::new_v4().to_string(),"name":"不可创建","expectedRevision":listed["revision"]}}),
        ),
        (
            "conversation_session_rename",
            json!({"request":{"projectSessionId":alias.session_id,"conversationSessionId":original,"name":"不可重命名","expectedRevision":listed["revision"]}}),
        ),
        (
            "conversation_session_activate",
            json!({"request":{"projectSessionId":alias.session_id,"conversationSessionId":original,"expectedRevision":listed["revision"]}}),
        ),
    ] {
        let result = tauri::test::get_ipc_response(
            &reader.window,
            tauri::webview::InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        );
        assert!(result.is_err(), "{cmd} must reject read-only writes");
    }
    assert_eq!(
        before,
        std::fs::read(base.root().join(".novelgainker/conversation/session.json")).unwrap()
    );
    assert_eq!(
        catalog_before,
        std::fs::read(base.root().join(".novelgainker/conversation/catalog.json")).unwrap()
    );
    // Neither registry has a Provider endpoint. A replay would fail this fixture.
    assert!(reader.events.lock().unwrap().is_empty());
    finish_conversation_turn(
        &bound,
        &started.turn_id,
        "",
        ConversationTerminalKind::Cancelled,
        None,
        now_ms().unwrap(),
    )
    .unwrap();
    sessions::release_run(&bound, &reservation.reservation_id).unwrap();
}

#[test]
fn nr601_reopened_project_alias_does_not_recover_live_run_and_cancels_only_same_project() {
    let f = Fixture::new(ProviderId::Deepseek);
    let page = catalog(&f);
    let original = page["activeSessionId"].as_str().unwrap();
    let state = f.app.state::<ProjectRegistry>();
    let base = session_handle(&state, &f.session).unwrap();
    let bound = sessions::bind_session(&base, Some(original), now_ms().unwrap()).unwrap();
    let cancellation = state
        .reserve_provider_run(&bound.coordination_key(), original, ProviderId::Deepseek)
        .unwrap();
    let reservation = sessions::reserve_run(&bound, now_ms().unwrap()).unwrap();
    let started = start_conversation_turn(
        &bound,
        ProviderId::Deepseek,
        "deepseek-v4-pro",
        "活动合成消息",
        now_ms().unwrap(),
    )
    .unwrap();
    sessions::bind_run_turn(&bound, &reservation.reservation_id, &started.turn_id).unwrap();
    state.bind_provider_turn(&started.turn_id).unwrap();
    let alias = register_session(&state, None, base, None).unwrap();
    let live = f.ipc(
        "conversation_session_list",
        json!({"request":{"projectSessionId":alias.session_id,"archived":false,"cursor":null}}),
    );
    assert_eq!(live["activeRun"]["turnId"], started.turn_id);
    let history = f.ipc(
        "conversation_session_load",
        json!({"sessionId":alias.session_id,"conversationSessionId":original}),
    );
    assert_eq!(history["conversation"]["turns"][0]["status"], "running");
    let duplicate = tauri::test::get_ipc_response(
        &f.window,
        tauri::webview::InvokeRequest {
            cmd: "conversation_turn_start".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(
                json!({"sessionId":alias.session_id,"conversationSessionId":original,"message":"不可插入第二次生成"}),
            ),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    );
    assert!(duplicate.is_err());
    assert_eq!(read_conversation(&bound).unwrap().turns.len(), 1);
    let foreign = Fixture::new(ProviderId::Deepseek);
    let foreign_handle =
        session_handle(&foreign.app.state::<ProjectRegistry>(), &foreign.session).unwrap();
    let wrong = register_session(&state, None, foreign_handle, None).unwrap();
    assert_eq!(
        f.ipc(
            "conversation_turn_cancel",
            json!({"sessionId":wrong.session_id,"turnId":started.turn_id})
        )["cancelRequested"],
        false
    );
    assert!(!cancellation.is_cancelled());
    assert_eq!(
        f.ipc(
            "conversation_turn_cancel",
            json!({"sessionId":alias.session_id,"turnId":started.turn_id})
        )["cancelRequested"],
        true
    );
    assert!(cancellation.is_cancelled());
    finish_conversation_turn(
        &bound,
        &started.turn_id,
        "",
        app_core::conversation::ConversationTerminalKind::Cancelled,
        None,
        now_ms().unwrap(),
    )
    .unwrap();
    sessions::release_run(&bound, &reservation.reservation_id).unwrap();
    state.finish_provider_run(&started.turn_id);
}
fn create(f: &Fixture, name: &str, page: &Value) -> Value {
    f.ipc("conversation_session_create", json!({"request":{"projectSessionId":f.session,"requestId":Uuid::new_v4().to_string(),"name":name,"expectedRevision":page["revision"]}}))
}
#[test]
fn nr601_desktop_switch_during_actual_native_tools_keeps_captured_session_and_shared_canon() {
    for provider in ProviderId::ALL {
        let f = Fixture::new(provider);
        let before = f.snapshot();
        let first_page = create(&f, "工具原始会话", &catalog(&f));
        let original = first_page["activeSessionId"].as_str().unwrap().to_owned();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        *f.endpoint.lock().unwrap() =
            Some(format!("http://{}/fixture", listener.local_addr().unwrap()));
        let (go, wait) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            wait.recv_timeout(Duration::from_secs(10)).unwrap();
            wire::serve(
                listener,
                provider,
                wire::Plan::Create {
                    kind: "character",
                    title: "林舟".into(),
                    text: "林舟是修表匠。".into(),
                    chapter: None,
                    chapter_source: None,
                    risk: "low",
                    final_error: false,
                },
            );
        });
        let started = f.ipc("conversation_turn_start", json!({"sessionId":f.session,"conversationSessionId":original,"message":"请创建并保存人物林舟，职业是修表匠。"}));
        let page = catalog(&f);
        assert_eq!(page["activeRun"]["sessionId"], original);
        let new_page = create(&f, "另一段讨论", &page);
        let selected = new_page["activeSessionId"].as_str().unwrap().to_owned();
        assert_ne!(selected, original);
        assert_eq!(new_page["activeRun"]["sessionId"], original);
        let blank = f.ipc(
            "conversation_session_load",
            json!({"sessionId":f.session,"conversationSessionId":selected}),
        );
        assert!(blank["conversation"]["turns"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(blank["taskActivities"].as_array().unwrap().is_empty());
        assert_eq!(f.snapshot(), before);
        go.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(Instant::now() < deadline);
            if !f
                .app
                .state::<ProjectRegistry>()
                .has_active_conversation(&f.session, &original)
            {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        server.join().unwrap();
        let stored = f.ipc(
            "conversation_session_load",
            json!({"sessionId":f.session,"conversationSessionId":original}),
        );
        assert_eq!(stored["conversation"]["turns"][0]["status"], "completed");
        assert!(!stored["taskActivities"].as_array().unwrap().is_empty());
        for task in stored["taskActivities"].as_array().unwrap() {
            assert_eq!(task["sessionId"], original);
        }
        let blank = f.ipc(
            "conversation_session_load",
            json!({"sessionId":f.session,"conversationSessionId":selected}),
        );
        assert!(blank["conversation"]["turns"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(blank["taskActivities"].as_array().unwrap().is_empty());
        assert_ne!(f.snapshot(), before);
        assert!(
            f.changes().is_empty(),
            "selected conversation owns no changes"
        );
        let original_changes = f.ipc(
            "canon_change_list",
            json!({"conversationSessionId":original,
            "request":{"projectSessionId":f.session,"turnId":null,"cursor":null,"limit":100}}),
        );
        let change = &original_changes["items"][0];
        assert_eq!(change["conversationSessionId"], original);
        assert_eq!(change["receipt"]["status"], "committed");
        let detail = f.ipc("canon_change_detail", json!({"sessionId":f.session,"conversationSessionId":original,"changeId":change["receipt"]["changeId"]}));
        assert!(!detail.is_null());
        let undone = f.ipc("canon_change_decide", json!({"conversationSessionId":original,"request":{
            "projectSessionId":f.session,"changeId":change["receipt"]["changeId"],"revision":change["receipt"]["revision"],"decision":"undo"}}));
        assert_eq!(undone["status"], "undone");
        assert_eq!(f.snapshot(), before);
        assert_eq!(catalog(&f)["activeSessionId"], selected);
        assert!(catalog(&f)["activeRun"].is_null());
        let events = f.events.lock().unwrap();
        assert!(!events.is_empty());
        for event in events.iter() {
            assert_eq!(event["conversationSessionId"], original);
            assert_eq!(event["turnId"], started["turnId"]);
        }
        println!("NR601_SWITCH provider={provider:?} captured_history=true captured_tasks=true captured_events=true shared_canon=true external=0");
    }
}

#[test]
fn nr601_desktop_recovery_repairs_original_running_session_without_provider_replay() {
    let f = Fixture::new(ProviderId::Deepseek);
    let before = f.snapshot();
    let page = catalog(&f);
    let original = page["activeSessionId"].as_str().unwrap().to_owned();
    let base = session_handle(&f.app.state::<ProjectRegistry>(), &f.session).unwrap();
    let bound = sessions::bind_session(&base, Some(&original), now_ms().unwrap()).unwrap();
    let reservation = sessions::reserve_run(&bound, now_ms().unwrap()).unwrap();
    let started = start_conversation_turn(
        &bound,
        ProviderId::Deepseek,
        "deepseek-v4-pro",
        "合成的中断轮次",
        now_ms().unwrap(),
    )
    .unwrap();
    sessions::bind_run_turn(&bound, &reservation.reservation_id, &started.turn_id).unwrap();
    // No live worker exists (as after restart). The client factory has no endpoint,
    // so any accidental Provider replay panics rather than silently passing.
    let page = catalog(&f);
    assert!(page["activeRun"].is_null());
    let result = f.ipc(
        "conversation_session_load",
        json!({"sessionId":f.session,"conversationSessionId":original}),
    );
    assert_eq!(
        result["conversation"]["turns"][0]["status"],
        "not_generated"
    );
    assert_eq!(result["conversation"]["turns"].as_array().unwrap().len(), 1);
    let new_page = create(&f, "恢复后新会话", &page);
    assert_ne!(new_page["activeSessionId"], original);
    assert_eq!(f.snapshot(), before);
    assert!(f.events.lock().unwrap().is_empty());
}
