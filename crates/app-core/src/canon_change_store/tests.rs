use super::*;
use crate::capability_pack::embedded_pack;
use crate::conversation::start_conversation_turn;
use crate::coordination::try_acquire_project_write_lease;
use crate::credential_vault::ProviderId;
use crate::project::{inspect_project_root, open_project, ProjectScanMode};
use serde_json::json;

struct Fixture {
    root: PathBuf,
    coordinator: CanonChangeCoordinator,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("novelgainker-nr402-{}", Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("project.md"), "# 合成测试工程\n").unwrap();
        for directory in ["characters", "world", "materials", "plot", "outlines"] {
            fs::create_dir(project.join(directory)).unwrap();
        }
        let handle = open_project(
            &inspect_project_root(&project, ProjectScanMode::Open).unwrap(),
            true,
        )
        .unwrap();
        let lease = Arc::new(
            try_acquire_project_write_lease(&handle.coordination_key())
                .unwrap()
                .unwrap(),
        );
        let recovery = RecoveryStore::new(root.join("state")).unwrap();
        Self {
            root,
            coordinator: CanonChangeCoordinator::new(handle, lease, recovery).unwrap(),
        }
    }

    fn request(&self) -> (ChangeEvidence, Value) {
        start_conversation_turn(
            &self.coordinator.handle,
            ProviderId::Deepseek,
            "fixture",
            "确定角色喜欢观察，请保存。",
            1,
        )
        .unwrap();
        let session = read_conversation(&self.coordinator.handle).unwrap();
        let turn = session.turns.last().unwrap();
        let mut evidence = ChangeEvidence::from_conversation(&session, &turn.turn_id).unwrap();
        let pack = embedded_pack().unwrap();
        let capability = pack.capability("create-character").unwrap();
        let method = evidence
            .record_method(
                &pack,
                "create-character",
                pack.resource_sha256(&capability.instruction_resource)
                    .unwrap(),
            )
            .unwrap();
        let source = &turn.messages[0];
        (
            evidence,
            json!({
                "schemaVersion":1,"sourceKind":"user_decision","risk":"low",
                "sources":[{"messageId":source.message_id,"contentHash":sha256_hex(source.content.as_bytes())}],
                "operations":[{"kind":"create","entityKind":"character","capabilityId":"create-character",
                    "methodReceiptId":method,"targetId":null,"readReceiptId":null,"expectedSourceHash":null,
                    "proposedMarkdown":body("new","人物甲","喜欢观察。")}]
            }),
        )
    }

    fn update(&self) -> (ChangeEvidence, Value, String) {
        let original = body("character-one", "人物甲", "喜欢观察。");
        fs::write(
            self.coordinator.handle.root().join("characters/one.md"),
            &original,
        )
        .unwrap();
        let (mut evidence, mut request) = self.request();
        let hash = sha256_hex(original.as_bytes());
        let (target, receipt) = evidence
            .record_complete_read(
                CanonEntityKind::Character,
                "characters/one.md",
                &original,
                &hash,
                None,
            )
            .unwrap();
        let operation = &mut request["operations"][0];
        operation["kind"] = json!("update");
        operation["targetId"] = json!(target);
        operation["readReceiptId"] = json!(receipt);
        operation["expectedSourceHash"] = json!(hash);
        operation["proposedMarkdown"] = json!(body("character-one", "人物甲", "喜欢观察和写作。"));
        (evidence, request, original)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.root.starts_with(std::env::temp_dir())
            && self
                .root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("novelgainker-nr402-")
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn body(identity: &str, title: &str, text: &str) -> String {
    format!("---\nid: {identity}\ntitle: {title}\nkind: character\nschemaVersion: 2\nwritingLanguage: zh-CN\n---\n\n# {title}\n\n## 摘要\n\n{text}\n")
}

#[test]
fn nr406_all_five_directories_create_update_no_change_and_undo_real_markdown() {
    for kind in [
        CanonEntityKind::Character,
        CanonEntityKind::World,
        CanonEntityKind::Material,
        CanonEntityKind::Plot,
        CanonEntityKind::Outline,
    ] {
        let fixture = Fixture::new();
        let (mut evidence, mut request) = fixture.request();
        let pack = embedded_pack().unwrap();
        let capability = pack.capability(kind.capability_id()).unwrap();
        let method = evidence
            .record_method(
                &pack,
                kind.capability_id(),
                pack.resource_sha256(&capability.instruction_resource)
                    .unwrap(),
            )
            .unwrap();
        request["operations"][0]["entityKind"] = json!(kind);
        request["operations"][0]["capabilityId"] = json!(kind.capability_id());
        request["operations"][0]["methodReceiptId"] = json!(method);
        request["operations"][0]["proposedMarkdown"] = json!(format!(
            "---\nid: new\ntitle: 合成资料\nkind: {}\nschemaVersion: {}\nwritingLanguage: zh-CN\n---\n\n# 合成资料\n\n## 摘要\n\n确定的资料。\n",
            kind.metadata_kind(), kind.metadata_version()));
        let created = fixture
            .coordinator
            .submit(&evidence, &request, "create", &CancellationToken::default())
            .unwrap();
        assert_eq!(created.status, ChangeStatus::Committed);
        let file = &created.files[0];
        assert!(file
            .relative_path
            .starts_with(&format!("{}/", kind.directory())));
        let target = fixture.coordinator.handle.root().join(&file.relative_path);
        let original = fs::read_to_string(&target).unwrap();
        let (id, read) = evidence
            .record_complete_read(kind, &file.relative_path, &original, &file.after_hash, None)
            .unwrap();
        let operation = &mut request["operations"][0];
        operation["kind"] = json!("upsert");
        operation["targetId"] = json!(id);
        operation["readReceiptId"] = json!(read);
        operation["expectedSourceHash"] = json!(file.after_hash);
        operation["proposedMarkdown"] =
            json!(original.replace("确定的资料。", "确定的资料与补充。"));
        let updated = fixture
            .coordinator
            .submit(&evidence, &request, "update", &CancellationToken::default())
            .unwrap();
        assert_eq!(updated.status, ChangeStatus::Committed);
        let record = fixture.coordinator.load_record(&updated.change_id).unwrap();
        let after = fs::read_to_string(&target).unwrap();
        let (id, read) = evidence
            .record_complete_read(
                kind,
                &file.relative_path,
                &after,
                &updated.files[0].after_hash,
                None,
            )
            .unwrap();
        request["operations"][0]["targetId"] = json!(id);
        request["operations"][0]["readReceiptId"] = json!(read);
        request["operations"][0]["expectedSourceHash"] = json!(updated.files[0].after_hash);
        let unchanged = fixture
            .coordinator
            .submit(
                &evidence,
                &request,
                "unchanged",
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(unchanged.status, ChangeStatus::NoChange);
        assert_eq!(
            fs::read_dir(fixture.coordinator.handle.root().join(kind.directory()))
                .unwrap()
                .count(),
            1
        );
        fixture
            .coordinator
            .decide(
                &record.conversation_session_id,
                &updated.change_id,
                updated.revision,
                ChangeDecision::Undo,
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), original);
        fixture
            .coordinator
            .decide(
                &record.conversation_session_id,
                &created.change_id,
                created.revision,
                ChangeDecision::Undo,
                &CancellationToken::default(),
            )
            .unwrap();
        assert!(!target.exists());
        println!("NR406_DIRECTORY kind={} create=committed update=committed same=no_change undo=undone external=0", kind.directory());
    }
}

#[test]
fn nr406_over_budget_mutation_rejected_before_record_or_canon() {
    let fixture = Fixture::new();
    let (evidence, mut request) = fixture.request();
    request["operations"][0]["proposedMarkdown"] = json!(body("new", "超限", &"界".repeat(3000)));
    assert_eq!(
        fixture
            .coordinator
            .submit(
                &evidence,
                &request,
                "overflow",
                &CancellationToken::default()
            )
            .unwrap_err(),
        ChangeError::BudgetExceeded
    );
    assert!(fixture.coordinator.receipts().unwrap().is_empty());
    assert_eq!(
        fs::read_dir(fixture.coordinator.handle.root().join("characters"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn nr406_windows_case_alias_cannot_bypass_manual_edit_protection() {
    let fixture = Fixture::new();
    let (evidence, request, original) = fixture.update();
    fixture
        .coordinator
        .record_editor_buffer("characters/ONE.md", 1, true)
        .unwrap();
    let receipt = fixture
        .coordinator
        .submit(
            &evidence,
            &request,
            "case_alias",
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(receipt.status, ChangeStatus::PendingConfirmation);
    assert_eq!(
        fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md")).unwrap(),
        original
    );
    assert_eq!(
        fixture
            .coordinator
            .record_editor_buffer("characters/one.md", 1, false)
            .unwrap_err(),
        ChangeError::RevisionConflict
    );
}

#[test]
fn nr403_actual_read_executor_issues_complete_receipt_only_after_all_fragments() {
    use crate::canon_context::{
        normalize_deepseek_novelist_read_context_arguments, NovelistReadContextExecutorV1,
    };
    use crate::tool_runtime::{ToolExecutionContext, ToolExecutor, ToolResultStatus};
    let fixture = Fixture::new();
    let original = body("character-one", "人物甲", &"观察。\n".repeat(550));
    fs::write(
        fixture.coordinator.handle.root().join("characters/one.md"),
        &original,
    )
    .unwrap();
    let (evidence, mut request) = fixture.request();
    let evidence = Arc::new(Mutex::new(evidence));
    let coordinator = Arc::new(
        CanonChangeCoordinator::new(
            fixture.coordinator.handle.clone(),
            fixture.coordinator.lease.clone(),
            fixture.coordinator.recovery.clone(),
        )
        .unwrap(),
    );
    let session = read_conversation(&fixture.coordinator.handle).unwrap();
    let context = ToolExecutionContext {
        project_id: session.project_id.clone(),
        conversation_session_id: session.session_id.clone(),
        turn_id: session.turns.last().unwrap().turn_id.clone(),
        call_id: "read_one".to_owned(),
    };
    let executor = NovelistReadContextExecutorV1::new(
        fixture.coordinator.handle.clone(),
        embedded_pack().unwrap(),
        session.session_id,
        context.turn_id.clone(),
    )
    .with_change_evidence(coordinator, evidence.clone());
    let mut cursor = Value::Null;
    for round in 0..2 {
        let arguments = normalize_deepseek_novelist_read_context_arguments(json!({
            "capabilityId":"create-character", "capabilityResources":[],
            "canonOperations":[{"kind":"read","relativePath":"characters/one.md",
                "startLine":if round == 0 { json!(1) } else { Value::Null },"maxBytes":4096,"cursor":cursor}]
        }))
        .unwrap();
        let output = executor.execute(&context, &arguments, &CancellationToken::default());
        assert_eq!(
            output.status,
            ToolResultStatus::Succeeded,
            "{:?}",
            output.error_code
        );
        let output = output.output_json.unwrap();
        let targets = output["changeEvidence"]["completeTargets"]
            .as_array()
            .unwrap();
        if round == 0 {
            assert!(targets.is_empty());
            assert_eq!(output["canon"][0]["truncated"], true);
            cursor = output["canon"][0]["nextCursor"].clone();
        } else {
            assert_eq!(targets.len(), 1);
            assert_eq!(
                targets[0]["expectedSourceHash"],
                sha256_hex(original.as_bytes())
            );
            let operation = &mut request["operations"][0];
            operation["kind"] = json!("update");
            operation["targetId"] = targets[0]["targetId"].clone();
            operation["readReceiptId"] = targets[0]["readReceiptId"].clone();
            operation["expectedSourceHash"] = targets[0]["expectedSourceHash"].clone();
            operation["methodReceiptId"] = output["changeEvidence"]["methodReceiptId"].clone();
            operation["proposedMarkdown"] =
                json!(original.replace("## 摘要", "## 摘要\n\n新的用户决定。"));
            evidence.lock().unwrap().validate_request(&request).unwrap();
        }
    }
    assert_eq!(
        fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md")).unwrap(),
        original
    );
}

#[test]
fn nr403_truncated_method_or_result_cannot_issue_write_evidence() {
    let fixture = Fixture::new();
    let (mut evidence, _) = fixture.request();
    let pack = embedded_pack().unwrap();
    let capability = pack.capability("create-character").unwrap();
    let mut bundle = json!({
        "capability":{"id":"create-character",
            "instruction":pack.resource(&capability.instruction_resource).unwrap()},
        "canon":[]
    });
    let mut delivered = crate::canon_change_read::DeliveredCanonReads::default();
    let original = bundle.clone();
    bundle["capability"]["instruction"] = json!("截断的方法");
    assert_eq!(
        delivered.annotate(&mut evidence, &pack, &mut bundle),
        Err(ChangeError::MethodNotRead)
    );
    assert!(bundle.get("changeEvidence").is_none());
    bundle = original;
    bundle["padding"] = json!("x".repeat(crate::tool_runtime::MAX_TOOL_RESULT_BYTES));
    assert_eq!(
        delivered.annotate(&mut evidence, &pack, &mut bundle),
        Err(ChangeError::BudgetExceeded)
    );
    assert!(bundle.get("changeEvidence").is_none());
    bundle.as_object_mut().unwrap().remove("padding");
    delivered
        .annotate(&mut evidence, &pack, &mut bundle)
        .unwrap();
    assert!(bundle["changeEvidence"]["methodReceiptId"].is_string());
    let catalog = evidence.source_catalog();
    assert!(catalog
        .iter()
        .any(|source| source["role"] == "user" && source["currentTurn"] == true));
}

#[test]
fn creates_exactly_once_with_host_identity_and_durable_receipt() {
    let fixture = Fixture::new();
    let (evidence, request) = fixture.request();
    let token = CancellationToken::default();
    let receipt = fixture
        .coordinator
        .submit(&evidence, &request, "call_1", &token)
        .unwrap();
    assert_eq!(receipt.status, ChangeStatus::Committed);
    let saved =
        read_project_file(&fixture.coordinator.handle, &receipt.files[0].relative_path).unwrap();
    assert_ne!(metadata_field(&saved.content, "id").as_deref(), Some("new"));
    assert_eq!(saved.content_hash, receipt.files[0].after_hash);
    let repeated = fixture
        .coordinator
        .submit(&evidence, &request, "call_1", &token)
        .unwrap();
    assert_eq!(repeated.change_id, receipt.change_id);
    assert_eq!(fixture.coordinator.records().unwrap().len(), 1);
    let mut changed = request.clone();
    changed["risk"] = json!("broad_replacement");
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &changed, "call_1", &token)
            .unwrap_err(),
        ChangeError::RevisionConflict
    );
}

fn undo_batch(fixture: &Fixture) -> (ChangeEvidence, Value, String, ChangeReceipt) {
    let (evidence, mut request, original) = fixture.update();
    let mut create = request["operations"][0].clone();
    create["kind"] = json!("create");
    create["targetId"] = Value::Null;
    create["readReceiptId"] = Value::Null;
    create["expectedSourceHash"] = Value::Null;
    create["proposedMarkdown"] = json!(body("new", "人物乙", "新的资料。"));
    request["operations"].as_array_mut().unwrap().push(create);
    let receipt = fixture
        .coordinator
        .submit(
            &evidence,
            &request,
            "call_undo",
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(receipt.status, ChangeStatus::Committed);
    (evidence, request, original, receipt)
}

#[test]
fn nr405_mixed_batch_undo_is_exact_local_and_idempotent() {
    let fixture = Fixture::new();
    let (_, _, original, receipt) = undo_batch(&fixture);
    let record = fixture.coordinator.load_record(&receipt.change_id).unwrap();
    let changed_file = fixture
        .coordinator
        .handle
        .root()
        .join(&receipt.files[1].relative_path);
    let undo = fixture
        .coordinator
        .decide(
            &record.conversation_session_id,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Undo,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(undo.status, ChangeStatus::Undone);
    assert!(!changed_file.exists());
    assert_eq!(
        fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md")).unwrap(),
        original
    );
    assert_eq!(
        fixture
            .coordinator
            .decide(
                &record.conversation_session_id,
                &receipt.change_id,
                receipt.revision,
                ChangeDecision::Undo,
                &CancellationToken::default()
            )
            .unwrap()
            .revision,
        undo.revision
    );
    assert_eq!(
        fixture.coordinator.recover().unwrap()[0].status,
        ChangeStatus::Undone
    );
    assert_eq!(
        fixture
            .coordinator
            .load_record(&receipt.change_id)
            .unwrap()
            .snapshots[0]
            .before,
        Some(original)
    );
}

#[test]
fn nr405_undo_preflight_checks_every_hash_revision_and_dirty_buffer_before_writes() {
    let fixture = Fixture::new();
    let (_, _, _, receipt) = undo_batch(&fixture);
    let record = fixture.coordinator.load_record(&receipt.change_id).unwrap();
    let first = fixture
        .coordinator
        .handle
        .root()
        .join(&receipt.files[0].relative_path);
    let second = fixture
        .coordinator
        .handle
        .root()
        .join(&receipt.files[1].relative_path);
    fs::write(&second, "外部编辑不能覆盖").unwrap();
    assert_eq!(
        fixture
            .coordinator
            .decide(
                &record.conversation_session_id,
                &receipt.change_id,
                receipt.revision,
                ChangeDecision::Undo,
                &CancellationToken::default()
            )
            .unwrap_err(),
        ChangeError::SourceChanged
    );
    assert_eq!(
        fs::read_to_string(&first).unwrap(),
        record.snapshots[0].after
    );
    assert_eq!(fs::read_to_string(&second).unwrap(), "外部编辑不能覆盖");
    fs::write(&second, &record.snapshots[1].after).unwrap();
    fixture
        .coordinator
        .record_editor_buffer(&receipt.files[0].relative_path, 1, true)
        .unwrap();
    assert_eq!(
        fixture
            .coordinator
            .decide(
                &record.conversation_session_id,
                &receipt.change_id,
                receipt.revision,
                ChangeDecision::Undo,
                &CancellationToken::default()
            )
            .unwrap_err(),
        ChangeError::RevisionConflict
    );
    fixture
        .coordinator
        .record_editor_buffer(&receipt.files[0].relative_path, 2, false)
        .unwrap();
    assert_eq!(
        fixture
            .coordinator
            .decide(
                &record.conversation_session_id,
                &receipt.change_id,
                receipt.revision - 1,
                ChangeDecision::Undo,
                &CancellationToken::default()
            )
            .unwrap_err(),
        ChangeError::RevisionConflict
    );
    assert_eq!(
        fixture
            .coordinator
            .load_record(&receipt.change_id)
            .unwrap()
            .receipt
            .revision,
        receipt.revision
    );
}

#[test]
fn nr405_undo_each_failure_and_cancel_preserves_prior_commit_or_durable_undo() {
    for fail in [
        CommitStage::Prepared,
        CommitStage::BeforeFile(1),
        CommitStage::AfterFile(1),
        CommitStage::BeforeFile(0),
        CommitStage::AfterFile(0),
        CommitStage::BeforeCommit,
        CommitStage::Committed,
    ] {
        let fixture = Fixture::new();
        let (_, _, original, receipt) = undo_batch(&fixture);
        let record = fixture.coordinator.load_record(&receipt.change_id).unwrap();
        let result = fixture.coordinator.decide_with_hook(
            &record.conversation_session_id,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Undo,
            &CancellationToken::default(),
            |stage| {
                if stage == fail {
                    Err(ChangeError::Io)
                } else {
                    Ok(())
                }
            },
        );
        let recovered = fixture.coordinator.recover().unwrap().remove(0);
        if fail == CommitStage::Committed {
            assert_eq!(result.unwrap().status, ChangeStatus::Undone);
            assert_eq!(recovered.status, ChangeStatus::Undone);
            assert_eq!(
                fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md"))
                    .unwrap(),
                original
            );
        } else {
            assert!(result.is_err(), "{fail:?}");
            assert_eq!(recovered.status, ChangeStatus::Committed, "{fail:?}");
            for snapshot in &record.snapshots {
                assert_eq!(
                    fs::read_to_string(
                        fixture
                            .coordinator
                            .handle
                            .root()
                            .join(&snapshot.relative_path)
                    )
                    .unwrap(),
                    snapshot.after,
                    "{fail:?}"
                );
            }
        }
    }
    let fixture = Fixture::new();
    let (_, _, _, receipt) = undo_batch(&fixture);
    let record = fixture.coordinator.load_record(&receipt.change_id).unwrap();
    let cancellation = CancellationToken::default();
    let result = fixture.coordinator.decide_with_hook(
        &record.conversation_session_id,
        &receipt.change_id,
        receipt.revision,
        ChangeDecision::Undo,
        &cancellation,
        |stage| {
            if stage == CommitStage::AfterFile(1) {
                cancellation.cancel();
            }
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), ChangeError::Cancelled);
    assert_eq!(
        fixture.coordinator.recover().unwrap()[0].status,
        ChangeStatus::Committed
    );
}

#[test]
fn nr405_reopen_interrupted_undo_restores_committed_after_images_without_provider() {
    let fixture = Fixture::new();
    let (_, _, _, receipt) = undo_batch(&fixture);
    let mut record = fixture.coordinator.load_record(&receipt.change_id).unwrap();
    record.undo_revision = Some(receipt.revision);
    record.receipt.revision += 1;
    record.receipt.status = ChangeStatus::Applying;
    fixture.coordinator.save_record(&record).unwrap();
    fs::remove_file(
        fixture
            .coordinator
            .handle
            .root()
            .join(&record.snapshots[1].relative_path),
    )
    .unwrap();
    let reopened = CanonChangeCoordinator::new(
        fixture.coordinator.handle.clone(),
        fixture.coordinator.lease.clone(),
        fixture.coordinator.recovery.clone(),
    )
    .unwrap();
    assert_eq!(
        reopened.recover().unwrap()[0].status,
        ChangeStatus::Committed
    );
    for snapshot in &record.snapshots {
        assert_eq!(
            fs::read_to_string(
                fixture
                    .coordinator
                    .handle
                    .root()
                    .join(&snapshot.relative_path)
            )
            .unwrap(),
            snapshot.after
        );
    }
    assert!(reopened
        .load_record(&receipt.change_id)
        .unwrap()
        .undo_revision
        .is_none());
}

#[test]
fn nr405_external_edit_during_undo_keeps_evidence_blocks_writes_and_allows_safe_retry() {
    let fixture = Fixture::new();
    let (evidence, request, _, receipt) = undo_batch(&fixture);
    let record = fixture.coordinator.load_record(&receipt.change_id).unwrap();
    let target = fixture
        .coordinator
        .handle
        .root()
        .join(&receipt.files[1].relative_path);
    let result = fixture.coordinator.decide_with_hook(
        &record.conversation_session_id,
        &receipt.change_id,
        receipt.revision,
        ChangeDecision::Undo,
        &CancellationToken::default(),
        |stage| {
            if stage == CommitStage::AfterFile(1) {
                fs::write(&target, "外部新稿").unwrap();
                return Err(ChangeError::Io);
            }
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), ChangeError::RecoveryRequired);
    let interrupted = fixture.coordinator.load_record(&receipt.change_id).unwrap();
    assert_eq!(interrupted.receipt.status, ChangeStatus::RecoveryRequired);
    assert_eq!(fs::read_to_string(&target).unwrap(), "外部新稿");
    assert_eq!(
        fixture
            .coordinator
            .submit(
                &evidence,
                &request,
                "another",
                &CancellationToken::default()
            )
            .unwrap_err(),
        ChangeError::RecoveryRequired
    );
    assert!(!fixture.coordinator.read_snapshot(Ok).unwrap());
    assert!(fixture
        .coordinator
        .decide(
            &record.conversation_session_id,
            &receipt.change_id,
            interrupted.receipt.revision,
            ChangeDecision::RetryRecovery,
            &CancellationToken::default()
        )
        .is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "外部新稿");
    // Simulate an explicit external resolution restoring the known committed bytes.
    fs::write(&target, &record.snapshots[1].after).unwrap();
    let revision = fixture
        .coordinator
        .load_record(&receipt.change_id)
        .unwrap()
        .receipt
        .revision;
    let restored = fixture
        .coordinator
        .decide(
            &record.conversation_session_id,
            &receipt.change_id,
            revision,
            ChangeDecision::RetryRecovery,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(restored.status, ChangeStatus::Committed);
    assert!(fixture.coordinator.read_snapshot(Ok).unwrap());
}

#[test]
fn nr405_orphan_bytes_and_missing_snapshots_never_delete_canon_or_claim_undo() {
    let fixture = Fixture::new();
    let (evidence, request) = fixture.request();
    let orphan = fixture
        .coordinator
        .directory()
        .unwrap()
        .join(".record-interrupted.tmp");
    let file = fs::File::create(&orphan).unwrap();
    file.set_len(MAX_CHANGE_STORE_BYTES - MAX_RECORD_BYTES)
        .unwrap();
    drop(file);
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &request, "full", &CancellationToken::default())
            .unwrap_err(),
        ChangeError::StoreFull
    );
    assert_eq!(
        fs::read_dir(fixture.coordinator.handle.root().join("characters"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        fs::metadata(&orphan).unwrap().len(),
        MAX_CHANGE_STORE_BYTES - MAX_RECORD_BYTES
    );

    let fixture = Fixture::new();
    let (_, _, _, receipt) = undo_batch(&fixture);
    let record = fixture.coordinator.load_record(&receipt.change_id).unwrap();
    fs::remove_file(
        fixture
            .coordinator
            .directory()
            .unwrap()
            .join(format!("{}.json", receipt.change_id)),
    )
    .unwrap();
    assert!(fixture
        .coordinator
        .decide(
            &record.conversation_session_id,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Undo,
            &CancellationToken::default()
        )
        .is_err());
    for snapshot in &record.snapshots {
        assert_eq!(
            fs::read_to_string(
                fixture
                    .coordinator
                    .handle
                    .root()
                    .join(&snapshot.relative_path)
            )
            .unwrap(),
            snapshot.after
        );
    }
}

#[test]
fn nr404_pending_diff_confirmation_and_discard_use_local_cas_without_provider() {
    for discard in [false, true] {
        let fixture = Fixture::new();
        let (evidence, mut request, before) = fixture.update();
        request["risk"] = json!("semantic_conflict");
        let pending = fixture
            .coordinator
            .submit(
                &evidence,
                &request,
                "pending_call",
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(pending.status, ChangeStatus::PendingConfirmation);
        let session = read_conversation(&fixture.coordinator.handle).unwrap();
        let detail = fixture
            .coordinator
            .detail(&session.session_id, &pending.change_id)
            .unwrap();
        let reconstructed_before: String = detail.files[0]
            .lines
            .iter()
            .filter(|line| !matches!(line.kind, DiffLineKind::Added))
            .map(|line| line.text.as_str())
            .collect();
        let reconstructed_after: String = detail.files[0]
            .lines
            .iter()
            .filter(|line| !matches!(line.kind, DiffLineKind::Removed))
            .map(|line| line.text.as_str())
            .collect();
        assert_eq!(reconstructed_before, before);
        assert_eq!(
            reconstructed_after,
            request["operations"][0]["proposedMarkdown"]
                .as_str()
                .unwrap()
        );
        assert_eq!(
            fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md"))
                .unwrap(),
            before
        );
        let decision = if discard {
            ChangeDecision::Discard
        } else {
            ChangeDecision::Confirm
        };
        assert_eq!(
            fixture
                .coordinator
                .decide(
                    &session.session_id,
                    &pending.change_id,
                    99,
                    decision,
                    &CancellationToken::default()
                )
                .unwrap_err(),
            ChangeError::RevisionConflict
        );
        let result = fixture
            .coordinator
            .decide(
                &session.session_id,
                &pending.change_id,
                pending.revision,
                decision,
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(
            result.status,
            if discard {
                ChangeStatus::Rejected
            } else {
                ChangeStatus::Committed
            }
        );
        let repeated = fixture
            .coordinator
            .decide(
                &session.session_id,
                &pending.change_id,
                pending.revision,
                decision,
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(repeated.revision, result.revision);
        if discard {
            assert!(fixture
                .coordinator
                .decide(
                    &session.session_id,
                    &pending.change_id,
                    result.revision,
                    ChangeDecision::Confirm,
                    &CancellationToken::default()
                )
                .is_err());
            assert_eq!(
                fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md"))
                    .unwrap(),
                before
            );
        }
    }
}

#[test]
fn nr404_pending_revalidates_disk_buffers_sources_and_session() {
    for mode in ["disk", "buffer", "session", "source"] {
        let fixture = Fixture::new();
        let (evidence, mut request, before) = fixture.update();
        request["risk"] = json!("semantic_conflict");
        let pending = fixture
            .coordinator
            .submit(
                &evidence,
                &request,
                "pending_call",
                &CancellationToken::default(),
            )
            .unwrap();
        let mut session = read_conversation(&fixture.coordinator.handle).unwrap();
        let expected = match mode {
            "disk" => {
                fs::write(
                    fixture.coordinator.handle.root().join("characters/one.md"),
                    body("character-one", "人物甲", "作者的新内容"),
                )
                .unwrap();
                ChangeError::SourceChanged
            }
            "buffer" => {
                fixture
                    .coordinator
                    .record_editor_buffer("characters/one.md", 1, true)
                    .unwrap();
                ChangeError::RevisionConflict
            }
            "source" => {
                // Rewrite only the synthetic stored source, preserving a valid Conversation shape.
                let record_path = fixture
                    .coordinator
                    .directory()
                    .unwrap()
                    .join(format!("{}.json", pending.change_id));
                let mut record = fixture.coordinator.load_record(&pending.change_id).unwrap();
                record.sources[0].content_hash = "a".repeat(64);
                fs::write(record_path, serde_json::to_vec(&record).unwrap()).unwrap();
                ChangeError::SourceInvalid
            }
            _ => {
                session.session_id = Uuid::new_v4().to_string();
                ChangeError::SourceInvalid
            }
        };
        assert_eq!(
            fixture
                .coordinator
                .decide(
                    &session.session_id,
                    &pending.change_id,
                    pending.revision,
                    ChangeDecision::Confirm,
                    &CancellationToken::default()
                )
                .unwrap_err(),
            expected
        );
        if mode != "disk" {
            assert_eq!(
                fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md"))
                    .unwrap(),
                before
            );
        }
        assert_eq!(
            fixture
                .coordinator
                .load_record(&pending.change_id)
                .unwrap()
                .receipt
                .status,
            ChangeStatus::PendingConfirmation
        );
    }
}

#[test]
fn updates_with_immutable_before_image_and_rechecks_disk_hash() {
    let fixture = Fixture::new();
    let (evidence, request, original) = fixture.update();
    let receipt = fixture
        .coordinator
        .submit(&evidence, &request, "call_1", &CancellationToken::default())
        .unwrap();
    let record = fixture.coordinator.load_record(&receipt.change_id).unwrap();
    assert_eq!(
        record.snapshots[0].before.as_deref(),
        Some(original.as_str())
    );
    let target = fixture.coordinator.handle.root().join("characters/one.md");
    fs::write(&target, body("character-one", "人物甲", "作者后续编辑。")).unwrap();
    assert_eq!(
        fixture
            .coordinator
            .load_record(&receipt.change_id)
            .unwrap()
            .snapshots[0]
            .before
            .as_deref(),
        Some(original.as_str())
    );
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &request, "call_2", &CancellationToken::default())
            .unwrap_err(),
        ChangeError::SourceChanged
    );
}

#[test]
fn suggestions_ambiguity_and_dirty_buffers_do_not_write_canon() {
    let fixture = Fixture::new();
    let (evidence, mut request, original) = fixture.update();
    request["sourceKind"] = json!("ai_suggestion");
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &request, "call_1", &CancellationToken::default())
            .unwrap()
            .status,
        ChangeStatus::Rejected
    );
    request["sourceKind"] = json!("user_decision");
    request["risk"] = json!("ambiguous_target");
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &request, "call_2", &CancellationToken::default())
            .unwrap_err(),
        ChangeError::TargetInvalid
    );
    request["risk"] = json!("low");
    fixture
        .coordinator
        .record_editor_buffer("characters/one.md", 1, true)
        .unwrap();
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &request, "call_3", &CancellationToken::default())
            .unwrap()
            .status,
        ChangeStatus::PendingConfirmation
    );
    assert_eq!(
        fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md")).unwrap(),
        original
    );
}

#[test]
fn failure_at_each_commit_stage_preserves_original_or_real_commit() {
    for fail in [
        CommitStage::Prepared,
        CommitStage::BeforeFile(0),
        CommitStage::AfterFile(0),
        CommitStage::BeforeCommit,
        CommitStage::Committed,
    ] {
        let fixture = Fixture::new();
        let (evidence, request, original) = fixture.update();
        let result = fixture.coordinator.submit_with_hook(
            &evidence,
            &request,
            "call_1",
            &CancellationToken::default(),
            |stage| {
                if stage == fail {
                    Err(ChangeError::Io)
                } else {
                    Ok(())
                }
            },
        );
        if fail == CommitStage::Committed {
            assert_eq!(result.unwrap().status, ChangeStatus::Committed);
        } else {
            assert!(result.is_err(), "{fail:?}");
            fixture.coordinator.recover().unwrap();
            assert_eq!(
                fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md"))
                    .unwrap(),
                original,
                "{fail:?}"
            );
            assert_eq!(
                fixture.coordinator.records().unwrap()[0].receipt.status,
                ChangeStatus::Cancelled
            );
        }
    }
}

#[test]
fn batch_failure_rolls_back_only_its_own_created_files() {
    let fixture = Fixture::new();
    let (evidence, mut request) = fixture.request();
    let mut second = request["operations"][0].clone();
    second["proposedMarkdown"] = json!(body("new", "人物乙", "喜欢阅读。"));
    request["operations"].as_array_mut().unwrap().push(second);
    let result = fixture.coordinator.submit_with_hook(
        &evidence,
        &request,
        "call_1",
        &CancellationToken::default(),
        |stage| {
            if stage == CommitStage::BeforeFile(1) {
                Err(ChangeError::Io)
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(result.unwrap_err(), ChangeError::Io);
    assert_eq!(
        fs::read_dir(fixture.coordinator.handle.root().join("characters"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        fixture.coordinator.records().unwrap()[0].receipt.status,
        ChangeStatus::Cancelled
    );
}

#[test]
fn external_edit_during_failure_is_preserved_and_requires_recovery() {
    let fixture = Fixture::new();
    let (evidence, request, _) = fixture.update();
    let target = fixture.coordinator.handle.root().join("characters/one.md");
    let external = body("character-one", "人物甲", "外部编辑器内容。");
    let result = fixture.coordinator.submit_with_hook(
        &evidence,
        &request,
        "call_1",
        &CancellationToken::default(),
        |stage| {
            if stage == CommitStage::AfterFile(0) {
                fs::write(&target, &external).unwrap();
                return Err(ChangeError::Io);
            }
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), ChangeError::RecoveryRequired);
    assert_eq!(fs::read_to_string(&target).unwrap(), external);
    assert_eq!(
        fixture.coordinator.records().unwrap()[0].receipt.status,
        ChangeStatus::RecoveryRequired
    );
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &request, "call_2", &CancellationToken::default())
            .unwrap_err(),
        ChangeError::RecoveryRequired
    );
}

#[test]
fn cancelled_before_submit_and_after_replace_do_not_lie_about_disk() {
    let fixture = Fixture::new();
    let (evidence, request, original) = fixture.update();
    let token = CancellationToken::default();
    token.cancel();
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &request, "call_1", &token)
            .unwrap_err(),
        ChangeError::Cancelled
    );
    let token = CancellationToken::default();
    let result =
        fixture
            .coordinator
            .submit_with_hook(&evidence, &request, "call_2", &token, |stage| {
                if stage == CommitStage::AfterFile(0) {
                    token.cancel();
                }
                Ok(())
            });
    assert_eq!(result.unwrap_err(), ChangeError::Cancelled);
    assert_eq!(
        fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md")).unwrap(),
        original
    );
}

#[test]
fn duplicate_identity_invalid_store_and_wrong_lease_are_rejected() {
    let fixture = Fixture::new();
    let (evidence, request) = fixture.request();
    fs::write(
        fixture
            .coordinator
            .handle
            .root()
            .join("characters/existing.md"),
        body("existing", "人物甲", "既有角色。"),
    )
    .unwrap();
    assert_eq!(
        fixture
            .coordinator
            .submit(&evidence, &request, "call_1", &CancellationToken::default())
            .unwrap_err(),
        ChangeError::DuplicateTarget
    );
    let other = Fixture::new();
    assert!(CanonChangeCoordinator::new(
        fixture.coordinator.handle.clone(),
        other.coordinator.lease.clone(),
        fixture.coordinator.recovery.clone()
    )
    .is_err());
    fs::write(
        fixture
            .coordinator
            .directory()
            .unwrap()
            .join(format!("{}.json", Uuid::new_v4())),
        "{}",
    )
    .unwrap();
    assert_eq!(
        fixture.coordinator.receipts().unwrap_err(),
        ChangeError::RecoveryRequired
    );
}

#[test]
fn restart_rolls_back_applying_snapshot_without_replaying_provider() {
    let fixture = Fixture::new();
    let (evidence, request, original) = fixture.update();
    let result = fixture.coordinator.submit_with_hook(
        &evidence,
        &request,
        "call_1",
        &CancellationToken::default(),
        |stage| {
            if stage == CommitStage::Prepared {
                Err(ChangeError::Io)
            } else {
                Ok(())
            }
        },
    );
    assert!(result.is_err());
    let mut record = fixture.coordinator.records().unwrap().remove(0);
    record.receipt.status = ChangeStatus::Applying;
    record.receipt.revision += 1;
    fixture.coordinator.save_record(&record).unwrap();
    fs::write(
        fixture.coordinator.handle.root().join("characters/one.md"),
        &record.snapshots[0].after,
    )
    .unwrap();
    let reopened = CanonChangeCoordinator::new(
        fixture.coordinator.handle.clone(),
        fixture.coordinator.lease.clone(),
        fixture.coordinator.recovery.clone(),
    )
    .unwrap();
    let receipts = reopened.recover().unwrap();
    assert_eq!(receipts[0].status, ChangeStatus::Cancelled);
    assert_eq!(
        fs::read_to_string(fixture.coordinator.handle.root().join("characters/one.md")).unwrap(),
        original
    );
}

#[test]
fn concrete_executor_is_qualified_and_task_restart_uses_durable_commit() {
    use crate::task_store::{create_queued_task, runtime_view, transition_task, TaskStatus};
    use crate::tool_runtime::{
        ProviderToolCall, ToolExecutionContext, ToolExecutor, ToolRegistry, ToolResultStatus,
    };
    let fixture = Fixture::new();
    let (evidence, request) = fixture.request();
    let session = read_conversation(&fixture.coordinator.handle).unwrap();
    let turn_id = session.turns.last().unwrap().turn_id.clone();
    let call = ProviderToolCall {
        call_id: "call_1".to_owned(),
        name: PROPOSE_CHANGE_TOOL_NAME.to_owned(),
        arguments_json: serde_json::to_string(&request).unwrap(),
    };
    let queued = create_queued_task(
        &fixture.coordinator.handle,
        &session.session_id,
        &turn_id,
        &call,
        &propose_change_definition(),
        2,
    )
    .unwrap();
    transition_task(
        &fixture.coordinator.handle,
        &queued.task_id,
        1,
        TaskStatus::Running,
        None,
        None,
        3,
    )
    .unwrap();
    let coordinator = Arc::new(
        CanonChangeCoordinator::new(
            fixture.coordinator.handle.clone(),
            fixture.coordinator.lease.clone(),
            fixture.coordinator.recovery.clone(),
        )
        .unwrap(),
    );
    let executor =
        NovelistProposeChangeExecutorV1::new(coordinator, Arc::new(Mutex::new(evidence)));
    let context = ToolExecutionContext {
        project_id: session.project_id.clone(),
        conversation_session_id: session.session_id.clone(),
        turn_id: turn_id.clone(),
        call_id: "call_1".to_owned(),
    };
    let output = executor.execute(&context, &request, &CancellationToken::default());
    assert_eq!(output.status, ToolResultStatus::Succeeded);
    let registry = ToolRegistry::novelist_production(
        &fixture.coordinator.handle,
        embedded_pack().unwrap(),
        session.session_id.clone(),
        turn_id,
    )
    .unwrap()
    .with_canon_mutation(executor)
    .unwrap();
    assert_eq!(registry.definitions().len(), 2);
    // Simulate crash after durable commit but before the Task success write.
    let recovered = runtime_view(&fixture.coordinator.handle, session, 4, true).unwrap();
    assert_eq!(recovered.task_activities[0].status, TaskStatus::Succeeded);
    assert_eq!(recovered.task_activities[0].task_revision, 3);
    assert_eq!(
        runtime_view(&fixture.coordinator.handle, recovered.conversation, 5, true)
            .unwrap()
            .task_activities[0]
            .task_revision,
        3
    );
}

#[test]
fn pending_quota_and_state_boundary_fail_before_canon_write() {
    let fixture = Fixture::new();
    let (evidence, mut request) = fixture.request();
    request["risk"] = json!("broad_replacement");
    for index in 0..MAX_PENDING_CHANGES {
        fixture
            .coordinator
            .submit(
                &evidence,
                &request,
                &format!("call_{index}"),
                &CancellationToken::default(),
            )
            .unwrap();
    }
    assert_eq!(
        fixture
            .coordinator
            .submit(
                &evidence,
                &request,
                "call_overflow",
                &CancellationToken::default()
            )
            .unwrap_err(),
        ChangeError::StoreFull
    );
    assert_eq!(
        fs::read_dir(fixture.coordinator.handle.root().join("characters"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn multiple_host_handles_share_transaction_and_dirty_buffer_state() {
    let fixture = Fixture::new();
    let other = CanonChangeCoordinator::new(
        fixture.coordinator.handle.clone(),
        fixture.coordinator.lease.clone(),
        fixture.coordinator.recovery.clone(),
    )
    .unwrap();
    fixture
        .coordinator
        .record_editor_buffer("characters/one.md", 1, true)
        .unwrap();
    assert_eq!(
        other.state.lock().unwrap().buffers.get("characters/one.md"),
        Some(&(1, true))
    );
    let held = fixture.coordinator.state.lock().unwrap();
    assert_eq!(other.receipts().unwrap_err(), ChangeError::WriterBusy);
    drop(held);
    assert!(other.receipts().unwrap().is_empty());
}

#[cfg(windows)]
#[test]
fn actual_state_junction_is_rejected_without_writing_outside_project() {
    use std::os::windows::fs::symlink_dir;
    let fixture = Fixture::new();
    let (evidence, request) = fixture.request();
    let outside = fixture.root.join("outside");
    fs::create_dir(&outside).unwrap();
    // Developer mode may be unavailable; creating a junction needs no symlink privilege.
    let link = fixture
        .coordinator
        .handle
        .root()
        .join(".novelgainker/changes");
    if symlink_dir(&outside, &link).is_err() {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(status.status.success());
    }
    assert!(fixture
        .coordinator
        .submit(&evidence, &request, "call_1", &CancellationToken::default())
        .is_err());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
    fs::remove_dir(link).unwrap();
}
