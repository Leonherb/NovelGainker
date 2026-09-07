mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::fs;
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Listener;

    mod wire {
        include!("nr506_native_fixture.rs");
    }
    mod nr601 {
        use super::*;
        include!("nr601_desktop_tests.rs");
    }
    mod nr603 {
        use super::*;
        include!("nr603_desktop_tests.rs");
    }

    struct SyntheticVault(Mutex<Vec<ProviderId>>);
    impl CredentialVault for SyntheticVault {
        fn credential_ref(&self, provider: ProviderId) -> &str {
            match provider {
                ProviderId::Deepseek => "fixture:deepseek",
                ProviderId::Openai => "fixture:openai",
            }
        }
        fn put(
            &self,
            provider: ProviderId,
            secret: &app_core::credential_vault::SecretValue,
        ) -> Result<(), AppError> {
            secret.with_bytes(|bytes| assert_eq!(bytes, b"synthetic-loopback-key"));
            self.0.lock().unwrap().push(provider);
            Ok(())
        }
        fn delete(&self, provider: ProviderId) -> Result<(), AppError> {
            self.0.lock().unwrap().retain(|p| *p != provider);
            Ok(())
        }
        fn status(
            &self,
            provider: ProviderId,
        ) -> Result<app_core::credential_vault::VaultSecretStatus, AppError> {
            Ok(if self.0.lock().unwrap().contains(&provider) {
                app_core::credential_vault::VaultSecretStatus::Present
            } else {
                app_core::credential_vault::VaultSecretStatus::Absent
            })
        }
        fn with_secret(
            &self,
            provider: ProviderId,
            operation: &mut dyn FnMut(&[u8]) -> Result<(), AppError>,
        ) -> Result<(), AppError> {
            assert!(self.0.lock().unwrap().contains(&provider));
            operation(b"synthetic-loopback-key")
        }
    }

    struct Fixture {
        root: PathBuf,
        app: tauri::App<tauri::test::MockRuntime>,
        window: tauri::WebviewWindow<tauri::test::MockRuntime>,
        endpoint: Arc<Mutex<Option<String>>>,
        events: Arc<Mutex<Vec<Value>>>,
        provider: ProviderId,
        session: String,
    }
    impl Fixture {
        fn new(provider: ProviderId) -> Self {
            let root = std::env::temp_dir().join(format!("novelgainker-nr506-{}", Uuid::new_v4()));
            fs::create_dir_all(root.join("project")).unwrap();
            let mut registry = ProjectRegistry::new(root.join("app-state")).unwrap();
            registry.credential_vault = Arc::new(SyntheticVault(Mutex::new(Vec::new())));
            let endpoint = Arc::new(Mutex::new(None::<String>));
            let client_endpoint = endpoint.clone();
            registry.test_client_factory = Some(Arc::new(move || {
                ProviderHttpClient::for_test(
                    provider,
                    client_endpoint
                        .lock()
                        .unwrap()
                        .clone()
                        .expect("only a running loopback fixture may provide a client"),
                )
            }));
            let app = builder()
                .manage(registry)
                .build(mock_context(noop_assets()))
                .unwrap();
            let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            let events = Arc::new(Mutex::new(Vec::new()));
            let delivered = events.clone();
            app.listen("conversation-turn-event", move |event| {
                delivered
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(event.payload()).unwrap());
            });
            let mut f = Self {
                root,
                app,
                window,
                endpoint,
                events,
                provider,
                session: String::new(),
            };
            // The sole substituted UI boundary is a file-picker choice of this
            // isolated directory. Preview/create and every later action use IPC.
            let candidate = f.register_candidate(ProjectScanMode::Create);
            let preview = f.ipc("project_create_preview", json!({"candidateId":candidate,"input":{
                "schemaVersion":1,"title":"十二章合成小说","authorDisplayName":"合成作者","genre":null,"premise":null
            }}));
            let opened = f.ipc("project_create", json!({"candidateId":candidate,"previewHash":preview["previewHash"],"confirmed":true}));
            f.session = opened["sessionId"].as_str().unwrap().to_owned();
            let status = f.ipc(
                "provider_key_save",
                json!({"providerId":provider.as_str(),"key":"synthetic-loopback-key"}),
            );
            assert_eq!(status["configured"], true);
            f.ipc("provider_select", json!({"providerId":provider.as_str()}));
            f
        }
        fn register_candidate(&self, mode: ProjectScanMode) -> String {
            let candidate = Uuid::new_v4().to_string();
            self.app
                .state::<ProjectRegistry>()
                .inner
                .lock()
                .unwrap()
                .candidates
                .insert(
                    candidate.clone(),
                    RegisteredProjectCandidate {
                        inspection: inspect_project_root(&self.root.join("project"), mode).unwrap(),
                        prepared_create: None,
                    },
                );
            candidate
        }
        fn ipc(&self, command: &str, body: Value) -> Value {
            tauri::test::get_ipc_response(
                &self.window,
                tauri::webview::InvokeRequest {
                    cmd: command.into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "http://tauri.localhost".parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(body),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            )
            .unwrap_or_else(|error| panic!("{command} failed: {error:?}"))
            .deserialize()
            .unwrap()
        }
        fn turn(&self, message: &str, plan: wire::Plan) -> Value {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            *self.endpoint.lock().unwrap() =
                Some(format!("http://{}/fixture", listener.local_addr().unwrap()));
            let provider = self.provider;
            let server = thread::spawn(move || wire::serve(listener, provider, plan));
            let started = self.ipc(
                "conversation_turn_start",
                json!({"sessionId":self.session,"message":message}),
            );
            let turn_id = started["turnId"].as_str().unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            let terminal = loop {
                assert!(
                    Instant::now() < deadline,
                    "desktop worker did not terminate"
                );
                let view = self.ipc(
                    "conversation_session_load",
                    json!({"sessionId":self.session}),
                );
                if !self.app.state::<ProjectRegistry>().has_active_conversation(
                    &self.session,
                    started["conversationSessionId"].as_str().unwrap(),
                ) && view["conversation"]["turns"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["status"]
                    != "running"
                {
                    break view;
                }
                thread::sleep(Duration::from_millis(10));
            };
            let captured = server.join().unwrap();
            assert!(captured > 0);
            *self.endpoint.lock().unwrap() = None;
            assert!(self
                .events
                .lock()
                .unwrap()
                .iter()
                .any(|e| e["turnId"] == turn_id && e["sessionId"] == self.session));
            terminal
        }
        fn changes(&self) -> Vec<Value> {
            self.ipc("canon_change_list", json!({"request":{"projectSessionId":self.session,"turnId":null,"cursor":null,"limit":100}}))
                ["items"].as_array().unwrap().clone()
        }
        fn snapshot(&self) -> std::collections::BTreeMap<String, String> {
            let files = self.ipc("project_list_files", json!({"sessionId":self.session}));
            let mut result = std::collections::BTreeMap::new();
            let mut pending = files["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .collect::<Vec<_>>();
            while let Some(entry) = pending.pop() {
                if entry["kind"] == "directory" {
                    pending.extend(entry["children"].as_array().unwrap());
                    continue;
                }
                let path = entry["relativePath"].as_str().unwrap();
                if path.starts_with(".novelgainker/") {
                    continue;
                }
                result.insert(
                    path.into(),
                    app_core::workspace::sha256_hex(
                        &fs::read(self.root.join("project").join(path)).unwrap(),
                    ),
                );
            }
            result
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.app
                .state::<ProjectRegistry>()
                .cancel_active_provider_run();
            self.app
                .state::<ProjectRegistry>()
                .inner
                .lock()
                .unwrap()
                .sessions
                .clear();
            if self.root.parent() == Some(std::env::temp_dir().as_path())
                && self
                    .root
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("novelgainker-nr506-")
            {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    fn builder() -> tauri::Builder<tauri::test::MockRuntime> {
        mock_builder().invoke_handler(tauri::generate_handler![
            health_check,
            app_state_load,
            app_state_save_ui,
            conversation_session_load,
            conversation_turn_start,
            conversation_turn_cancel,
            canon_change_list,
            canon_change_detail,
            canon_change_decide,
            canon_editor_state,
            publication_list,
            publication_detail,
            provider_status_load,
            provider_key_save,
            provider_key_delete,
            provider_select,
            provider_connection_test,
            project_scan,
            project_create_preview,
            project_create,
            project_open,
            project_open_recent,
            project_list_files,
            project_read_file,
            app_state_set_current_file,
            project_watch_file,
            project_save_file,
            project_validate_draft,
            project_search,
            project_close,
            conversation_session_list,
            conversation_session_create,
            conversation_session_rename,
            conversation_session_activate,
            conversation_session_manage,
            conversation_export_prepare,
            conversation_export_commit,
            conversation_turn_retry
        ])
    }

    #[test]
    fn repeated_project_open_does_not_create_a_read_only_session() {
        let f = Fixture::new(ProviderId::Deepseek);
        let candidate = f.register_candidate(ProjectScanMode::Open);
        let before = f
            .app
            .state::<ProjectRegistry>()
            .inner
            .lock()
            .unwrap()
            .sessions
            .len();
        let result = tauri::test::get_ipc_response(
            &f.window,
            tauri::webview::InvokeRequest {
                cmd: "project_open".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(
                    json!({"candidateId":candidate,"confirmMetadata":false}),
                ),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
        .unwrap_err();
        assert_eq!(result["code"], "project.already_open");
        assert_eq!(
            f.app
                .state::<ProjectRegistry>()
                .inner
                .lock()
                .unwrap()
                .sessions
                .len(),
            before
        );
        f.ipc("project_close", json!({"sessionId":f.session}));
        let opened = f.ipc(
            "project_open",
            json!({"candidateId":candidate,"confirmMetadata":false}),
        );
        assert_eq!(opened["accessMode"], "writer");
        assert_eq!(opened["capabilities"]["canWrite"], true);
    }

    #[test]
    fn nr506_desktop_ipc_creates_project_and_runs_actual_worker_without_real_vault() {
        for provider in ProviderId::ALL {
            let f = Fixture::new(provider);
            let before = f.snapshot();
            // The UI queries these before the first Provider turn. A blank
            // project must already have a usable Canon coordinator and page.
            let initial = f.ipc("conversation_session_load", json!({"sessionId":f.session}));
            assert!(initial["conversation"]["turns"]
                .as_array()
                .unwrap()
                .is_empty());
            assert!(f.changes().is_empty());
            assert!(f.ipc(
                "publication_list",
                json!({"sessionId":f.session,"cursor":null})
            )["items"]
                .as_array()
                .unwrap()
                .is_empty());
            let registry = f.app.state::<ProjectRegistry>();
            let coordinator = changes_for_session(&registry, &f.session).unwrap();
            assert_eq!(
                coordinator.protocol_profile(),
                app_core::protocol_profile::ToolProtocolProfile::Nr5
            );
            let terminal = f.turn("只讨论十二章小说，不要保存任何资料。", wire::Plan::Chat);
            assert_eq!(terminal["conversation"]["turns"][0]["status"], "completed");
            assert_eq!(f.snapshot(), before);
            assert!(f.changes().is_empty());
        }
    }

    #[test]
    fn nr506_desktop_twelve_chapter_creation_continuity_final_epub_reopen_and_undo_chain() {
        for provider in ProviderId::ALL {
            let mut f = Fixture::new(provider);
            let empty = f.snapshot();
            f.turn("只讨论林舟的十二章故事，不要保存。", wire::Plan::Chat);
            assert_eq!(f.snapshot(), empty);
            for (kind, title, text) in [
                ("character", "林舟", "林舟是钟表修复师，不会游泳。"),
                ("world", "北桥", "北桥在暴雨夜封闭。"),
            ] {
                f.turn(
                    "确定以上人物与设定，请保存对应资料。",
                    wire::Plan::Create {
                        kind,
                        title: title.into(),
                        text: text.into(),
                        chapter: None,
                        chapter_source: None,
                        risk: "low",
                        final_error: false,
                    },
                );
            }
            let mut chapter_changes = Vec::new();
            for chapter in 1..=12 {
                let terminal = f.turn(
                    &format!("请写并保存第{chapter}章，明确确认本次版本定稿。"),
                    wire::Plan::Create {
                        kind: "chapter",
                        title: format!("第{chapter}章"),
                        text: format!("第{chapter}章正文：林舟带着银色怀表来到北桥。"),
                        chapter: Some(chapter),
                        chapter_source: None,
                        risk: "low",
                        final_error: false,
                    },
                );
                assert_eq!(
                    terminal["conversation"]["turns"]
                        .as_array()
                        .unwrap()
                        .last()
                        .unwrap()["status"],
                    "completed"
                );
                let activity = f
                    .changes()
                    .into_iter()
                    .find(|item| {
                        item["turnId"]
                            == terminal["conversation"]["turns"]
                                .as_array()
                                .unwrap()
                                .last()
                                .unwrap()["turnId"]
                    })
                    .unwrap();
                assert_eq!(activity["receipt"]["status"], "committed");
                chapter_changes.push(activity["receipt"].clone());
            }
            let first = &chapter_changes[0];
            let detail = f.ipc(
                "canon_change_detail",
                json!({"sessionId":f.session,"changeId":first["changeId"]}),
            );
            let chapter_path = detail["files"][0]["relativePath"]
                .as_str()
                .unwrap()
                .to_owned();
            f.turn(
                "根据第一章已提交事实，建立银色怀表素材，不推断其他设定。",
                wire::Plan::Create {
                    kind: "material",
                    title: "银色怀表".into(),
                    text: "林舟随身携带银色怀表。".into(),
                    chapter: None,
                    chapter_source: Some(chapter_path.clone()),
                    risk: "low",
                    final_error: false,
                },
            );
            let before_review = f.snapshot();
            let review = f.turn(
                "请显式评审第一章的连续性与行文，先不修改。",
                wire::Plan::Read {
                    capability: "analytical-review",
                    paths: vec![chapter_path],
                },
            );
            assert!(review["conversation"]["turns"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["messages"][1]["content"]
                .as_str()
                .unwrap()
                .contains("作品检查"));
            assert_eq!(f.snapshot(), before_review);
            f.turn(
                "确认现有十二章已定稿，请构建本地 EPUB，不需要强制额外评审。",
                wire::Plan::Publish {
                    failure: false,
                    final_error: false,
                },
            );
            let publications = f.ipc(
                "publication_list",
                json!({"sessionId":f.session,"cursor":null}),
            );
            let receipt = publications["items"][0].clone();
            assert_eq!(receipt["status"], "generated");
            assert_eq!(receipt["chapterCount"], 12);
            assert_eq!(receipt["withoutCover"], true);
            let artifact_path = f
                .root
                .join("project")
                .join(receipt["relativePath"].as_str().unwrap());
            let artifact = fs::read(&artifact_path).unwrap();
            assert_eq!(
                app_core::workspace::sha256_hex(&artifact),
                receipt["outputHash"]
            );
            for chapter in 1..=12 {
                let expected = format!("第{chapter}章正文");
                assert!(artifact
                    .windows(expected.len())
                    .any(|part| part == expected.as_bytes()));
            }
            for excluded in [".novelgainker", "synthetic-loopback-key", "总体判断"] {
                assert!(!artifact
                    .windows(excluded.len())
                    .any(|part| part == excluded.as_bytes()));
            }
            assert_eq!(f.snapshot(), before_review);
            let published = f.ipc(
                "publication_detail",
                json!({"sessionId":f.session,"artifactId":receipt["artifactId"]}),
            );
            assert_eq!(
                published["inputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|input| !input["confirmationId"].is_null())
                    .count(),
                12
            );
            let saved_history = f.ipc("conversation_session_load", json!({"sessionId":f.session}))
                ["conversation"]
                .clone();
            f.ipc("project_close", json!({"sessionId":f.session}));
            let candidate = f.register_candidate(ProjectScanMode::Open);
            let reopened = f.ipc(
                "project_open",
                json!({"candidateId":candidate,"confirmMetadata":false}),
            );
            f.session = reopened["sessionId"].as_str().unwrap().to_owned();
            assert_eq!(
                f.ipc("conversation_session_load", json!({"sessionId":f.session}))["conversation"],
                saved_history
            );
            assert_eq!(f.snapshot(), before_review);
            let last = chapter_changes.last().unwrap();
            f.ipc("canon_change_decide", json!({"request":{"projectSessionId":f.session,"changeId":last["changeId"],"revision":last["revision"],"decision":"undo"}}));
            let stale = f.ipc(
                "publication_detail",
                json!({"sessionId":f.session,"artifactId":receipt["artifactId"]}),
            );
            assert_eq!(stale["receipt"]["status"], "stale");
            assert_eq!(fs::read(artifact_path).unwrap(), artifact);
            println!("NR506_DESKTOP provider={provider:?} chapters=12 publication=1 reopened=true undo_stale=true real_commands=true mock_window=true external=0");
        }
    }

    #[test]
    fn nr506_actual_desktop_commands_compile_in_isolated_ipc_runtime() {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                health_check,
                app_state_load,
                app_state_save_ui,
                conversation_session_load,
                conversation_turn_start,
                conversation_turn_cancel,
                canon_change_list,
                canon_change_detail,
                canon_change_decide,
                canon_editor_state,
                publication_list,
                publication_detail,
                provider_status_load,
                provider_key_save,
                provider_key_delete,
                provider_select,
                provider_connection_test,
                project_scan,
                project_create_preview,
                project_create,
                project_open,
                project_open_recent,
                project_list_files,
                project_read_file,
                app_state_set_current_file,
                project_watch_file,
                project_save_file,
                project_validate_draft,
                project_search,
                project_close,
                conversation_session_list,
                conversation_session_create,
                conversation_session_rename,
                conversation_session_activate,
                conversation_session_manage,
                conversation_export_prepare,
                conversation_export_commit,
                conversation_turn_retry
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let result = tauri::test::get_ipc_response(
            &window,
            tauri::webview::InvokeRequest {
                cmd: "health_check".into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::default(),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
        .unwrap()
        .deserialize::<serde_json::Value>()
        .unwrap();
        assert_eq!(result["application"], "NovelGainker");
    }

    #[test]
    fn nr506_desktop_faults_preserve_text_pending_and_committed_disk_truth() {
        for provider in ProviderId::ALL {
            let f = Fixture::new(provider);
            let baseline = f.snapshot();
            for partial in [false, true] {
                let view = f.turn("继续讨论，不写文件。", wire::Plan::Failure { partial });
                let last = view["conversation"]["turns"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap();
                let text = last["messages"][1]["content"].as_str().unwrap();
                assert_ne!(last["status"], "completed");
                if partial {
                    assert!(text.contains("保留这段已生成内容"));
                    assert!(text.contains("本次内容未生成完成。"));
                } else {
                    assert_eq!(text, "本次回复未生成。");
                }
                assert_eq!(f.snapshot(), baseline);
            }
            let failed_tool = f.turn("只读检查资料，工具失败仍请说明。", wire::Plan::InvalidTool);
            assert_eq!(
                failed_tool["conversation"]["turns"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["status"],
                "completed"
            );
            assert!(failed_tool["taskActivities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|task| task["status"] == "failed"));
            assert_eq!(f.snapshot(), baseline);
            f.turn(
                "这项有高风险，先提出人物修改，等我确认。",
                wire::Plan::Create {
                    kind: "character",
                    title: "待确认人物".into(),
                    text: "仅是待确认提案。".into(),
                    chapter: None,
                    chapter_source: None,
                    risk: "semantic_conflict",
                    final_error: false,
                },
            );
            assert_eq!(f.snapshot(), baseline);
            let pending = f
                .changes()
                .into_iter()
                .find(|v| v["receipt"]["status"] == "pending_confirmation")
                .unwrap()["receipt"]
                .clone();
            f.ipc("canon_change_decide", json!({"request":{"projectSessionId":f.session,"changeId":pending["changeId"],"revision":pending["revision"],"decision":"discard"}}));
            assert_eq!(f.snapshot(), baseline);
            let saved = f.turn(
                "明确保存这个人物资料。",
                wire::Plan::Create {
                    kind: "character",
                    title: "已保存人物".into(),
                    text: "即使回复失败也不能丢失。".into(),
                    chapter: None,
                    chapter_source: None,
                    risk: "low",
                    final_error: true,
                },
            );
            assert_ne!(
                saved["conversation"]["turns"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["status"],
                "completed"
            );
            let committed = f
                .changes()
                .into_iter()
                .find(|v| v["receipt"]["status"] == "committed")
                .unwrap()["receipt"]
                .clone();
            let detail = f.ipc(
                "canon_change_detail",
                json!({"sessionId":f.session,"changeId":committed["changeId"]}),
            );
            let bytes = fs::read(
                f.root
                    .join("project")
                    .join(detail["files"][0]["relativePath"].as_str().unwrap()),
            )
            .unwrap();
            assert!(std::str::from_utf8(&bytes)
                .unwrap()
                .contains("即使回复失败也不能丢失"));
            assert_ne!(f.snapshot(), baseline);
            println!("NR506_FAULT provider={provider:?} zero_text=true partial_text=true invalid_tool=true pending_discard=true committed_after_failure=true external=0");
        }
    }
}
