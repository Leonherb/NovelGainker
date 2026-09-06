use super::*;
use crate::canon_context::{
    normalize_deepseek_novelist_read_context_arguments, NovelistReadContextExecutorV1,
};
use crate::continuity::*;
use crate::conversation::{finish_conversation_turn, ConversationTerminalKind};
use crate::tool_runtime::{ToolExecutionContext, ToolExecutor, ToolResultStatus};

fn prepare(kind: CanonEntityKind, manual: bool) -> (Fixture, ChangeReceipt, ChangeEvidence, Value) {
    let f = Fixture::new();
    fs::create_dir(f.coordinator.handle.root().join(kind.directory())).unwrap();
    let source = if manual {
        let markdown = body(&Uuid::new_v4().to_string(), "第一章", "桥上有风。", "draft");
        fs::write(
            f.coordinator
                .handle
                .root()
                .join("chapters/manual-source.md"),
            &markdown,
        )
        .unwrap();
        f.coordinator
            .save_manual(
                "chapters/manual-source.md",
                &markdown,
                &sha256_hex(markdown.as_bytes()),
            )
            .unwrap();
        f.coordinator.receipts().unwrap().remove(0)
    } else {
        let (evidence, method, candidate) = f.candidate();
        let receipt = f
            .coordinator
            .submit_chapter(
                &evidence,
                submit(&candidate, &method),
                "source_chapter",
                &CancellationToken::default(),
            )
            .unwrap();
        finish_conversation_turn(
            &f.coordinator.handle,
            &candidate.scope.turn_id,
            "已保存第一章。",
            ConversationTerminalKind::Completed,
            None,
            2,
        )
        .unwrap();
        receipt
    };
    let started = start_conversation_turn(
        &f.coordinator.handle,
        ProviderId::Deepseek,
        "synthetic",
        "请核对第一章的明确事实并维护资料；推断和冲突先询问。",
        3,
    )
    .unwrap();
    let session = read_conversation(&f.coordinator.handle).unwrap();
    let evidence = Arc::new(Mutex::new(
        ChangeEvidence::from_conversation(&session, &started.turn_id).unwrap(),
    ));
    let coordinator = Arc::new(
        CanonChangeCoordinator::new_nr5(
            f.coordinator.handle.clone(),
            f.coordinator.lease.clone(),
            f.coordinator.recovery.clone(),
        )
        .unwrap(),
    );
    let executor = NovelistReadContextExecutorV1::new(
        f.coordinator.handle.clone(),
        embedded_pack().unwrap(),
        session.session_id.clone(),
        started.turn_id.clone(),
    )
    .with_change_evidence(coordinator, evidence.clone());
    let context = ToolExecutionContext {
        project_id: session.project_id,
        conversation_session_id: session.session_id,
        turn_id: started.turn_id,
        call_id: "read_committed_source".to_owned(),
    };
    let arguments = normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":kind.capability_id(),"canonOperations":[{"kind":"read","relativePath":source.files[0].relative_path,"startLine":1,"maxBytes":32768}]})).unwrap();
    let output = executor.execute(&context, &arguments, &CancellationToken::default());
    assert_eq!(output.status, ToolResultStatus::Succeeded);
    let output = output.output_json.unwrap();
    assert_eq!(output["chapterSources"][0]["status"], "current");
    assert_eq!(
        output["chapterSources"][0]["source"]["transactionId"],
        source.change_id
    );
    let user = &session.turns.last().unwrap().messages[0];
    let request = json!({"schemaVersion":1,"sourceKind":"chapter_commit","risk":"low",
        "sources":[{"messageId":user.message_id,"contentHash":sha256_hex(user.content.as_bytes())}],
        "operations":[{"kind":"create","entityKind":kind,"capabilityId":kind.capability_id(),
            "methodReceiptId":output["changeEvidence"]["methodReceiptId"],"targetId":null,"readReceiptId":null,"expectedSourceHash":null,
            "proposedMarkdown":format!("---\nid: new\ntitle: 章节事实\nkind: {}\nschemaVersion: {}\nwritingLanguage: zh-CN\n---\n\n# 章节事实\n\n## 摘要\n\n桥上有风。\n",kind.metadata_kind(),kind.metadata_version())}]});
    let evidence = evidence.lock().unwrap().clone();
    (f, source, evidence, request)
}

#[test]
fn nr503_actual_chapter_and_manual_sources_maintain_all_five_types_with_bound_receipts() {
    for kind in [
        CanonEntityKind::Character,
        CanonEntityKind::World,
        CanonEntityKind::Material,
        CanonEntityKind::Plot,
        CanonEntityKind::Outline,
    ] {
        for manual in [false, true] {
            let (f, chapter, mut evidence, mut request) = prepare(kind, manual);
            let chapter_bytes = fs::read(
                f.coordinator
                    .handle
                    .root()
                    .join(&chapter.files[0].relative_path),
            )
            .unwrap();
            assert!(
                evidence.validate_request(&request).is_err(),
                "historical NR-4 stays unavailable"
            );
            let receipt = f
                .coordinator
                .submit(
                    &evidence,
                    &request,
                    "maintain",
                    &CancellationToken::default(),
                )
                .unwrap();
            assert_eq!(receipt.status, ChangeStatus::Committed);
            assert!(fs::read_to_string(
                f.coordinator
                    .handle
                    .root()
                    .join(&receipt.files[0].relative_path)
            )
            .unwrap()
            .contains("桥上有风。"));
            let record = f.coordinator.load_record(&receipt.change_id).unwrap();
            assert_eq!(record.source_kind, ChangeSourceKind::ChapterCommit);
            assert_eq!(record.chapter_dependencies.len(), 1);
            assert_eq!(
                record.chapter_dependencies[0].transaction_id,
                chapter.change_id
            );
            let session = read_conversation(&f.coordinator.handle).unwrap();
            let detail = f
                .coordinator
                .detail(&session.session_id, &receipt.change_id)
                .unwrap();
            assert_eq!(
                detail.activity.chapter_source_state,
                Some(ChapterSourceState::Current)
            );
            assert_eq!(
                fs::read(
                    f.coordinator
                        .handle
                        .root()
                        .join(&chapter.files[0].relative_path)
                )
                .unwrap(),
                chapter_bytes
            );
            assert_eq!(
                f.coordinator
                    .submit(
                        &evidence,
                        &request,
                        "maintain",
                        &CancellationToken::default()
                    )
                    .unwrap()
                    .change_id,
                receipt.change_id
            );
            let target = f
                .coordinator
                .handle
                .root()
                .join(&receipt.files[0].relative_path);
            let original = fs::read_to_string(&target).unwrap();
            let (id, read) = evidence
                .record_complete_read(
                    kind,
                    &receipt.files[0].relative_path,
                    &original,
                    &receipt.files[0].after_hash,
                    None,
                )
                .unwrap();
            let operation = &mut request["operations"][0];
            operation["kind"] = json!("upsert");
            operation["targetId"] = json!(id);
            operation["readReceiptId"] = json!(read);
            operation["expectedSourceHash"] = json!(receipt.files[0].after_hash);
            operation["proposedMarkdown"] =
                json!(original.replace("桥上有风。", "章节明确记载：桥上有风。"));
            let updated = f
                .coordinator
                .submit(
                    &evidence,
                    &request,
                    "maintain_update",
                    &CancellationToken::default(),
                )
                .unwrap();
            assert_eq!(updated.status, ChangeStatus::Committed);
            assert_eq!(
                f.coordinator
                    .load_record(&updated.change_id)
                    .unwrap()
                    .chapter_dependencies,
                record.chapter_dependencies
            );
            let after = fs::read_to_string(&target).unwrap();
            let (id, read) = evidence
                .record_complete_read(
                    kind,
                    &receipt.files[0].relative_path,
                    &after,
                    &updated.files[0].after_hash,
                    None,
                )
                .unwrap();
            request["operations"][0]["targetId"] = json!(id);
            request["operations"][0]["readReceiptId"] = json!(read);
            request["operations"][0]["expectedSourceHash"] = json!(updated.files[0].after_hash);
            assert_eq!(
                f.coordinator
                    .submit(
                        &evidence,
                        &request,
                        "maintain_unchanged",
                        &CancellationToken::default()
                    )
                    .unwrap()
                    .status,
                ChangeStatus::NoChange
            );
            f.coordinator
                .decide(
                    &session.session_id,
                    &updated.change_id,
                    updated.revision,
                    ChangeDecision::Undo,
                    &CancellationToken::default(),
                )
                .unwrap();
            assert_eq!(fs::read_to_string(target).unwrap(), original);
            assert!(f
                .coordinator
                .chapter_commit_receipt(&chapter.change_id)
                .is_ok());
            println!("NR503_DATA kind={} manual={} create=committed update=committed same=no_change undo=undone source=current external=0", kind.directory(), manual);
        }
    }
}

#[test]
fn nr503_forged_missing_cross_project_old_revision_and_unknown_source_fields_write_nothing() {
    for fault in 0..6 {
        let (f, _, mut evidence, mut request) = prepare(CanonEntityKind::Character, false);
        let mut source = evidence.chapter_dependencies().remove(0);
        evidence.remove_chapter_source(&source.relative_path);
        match fault {
            0 => {}
            1 => {
                source.transaction_id = Uuid::new_v4().to_string();
                evidence.bind_chapter_source(source).unwrap();
            }
            2 => {
                source.project_id = Uuid::new_v4().to_string();
                assert!(evidence.bind_chapter_source(source).is_err());
            }
            3 => {
                source.revision += 1;
                evidence.bind_chapter_source(source).unwrap();
            }
            4 => {
                source.read_receipt_id = Uuid::new_v4().to_string();
                assert!(evidence.bind_chapter_source(source).is_err());
            }
            _ => {
                evidence.bind_chapter_source(source).unwrap();
                request["chapterReceipt"] = json!({"status":"committed"});
            }
        }
        assert!(f
            .coordinator
            .submit(&evidence, &request, "forged", &CancellationToken::default())
            .is_err());
        assert_eq!(
            fs::read_dir(f.coordinator.handle.root().join("characters"))
                .unwrap()
                .count(),
            0
        );
    }
}

#[test]
fn nr503_undo_marks_dependent_sources_stale_without_cascading_undo_or_replay() {
    let (f, chapter, evidence, request) = prepare(CanonEntityKind::Character, false);
    let receipt = f
        .coordinator
        .submit(
            &evidence,
            &request,
            "dependent",
            &CancellationToken::default(),
        )
        .unwrap();
    let path = f
        .coordinator
        .handle
        .root()
        .join(&receipt.files[0].relative_path);
    let bytes = fs::read(&path).unwrap();
    let session = read_conversation(&f.coordinator.handle).unwrap();
    f.coordinator
        .decide(
            &session.session_id,
            &chapter.change_id,
            chapter.revision,
            ChangeDecision::Undo,
            &CancellationToken::default(),
        )
        .unwrap();
    let detail = f
        .coordinator
        .detail(&session.session_id, &receipt.change_id)
        .unwrap();
    assert_eq!(detail.activity.receipt.status, ChangeStatus::Committed);
    assert_eq!(
        detail.activity.chapter_source_state,
        Some(ChapterSourceState::Stale)
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(f
        .coordinator
        .submit(
            &evidence,
            &request,
            "stale_again",
            &CancellationToken::default()
        )
        .is_err());
    let reopened = CanonChangeCoordinator::new_nr5(
        f.coordinator.handle.clone(),
        f.coordinator.lease.clone(),
        f.coordinator.recovery.clone(),
    )
    .unwrap();
    reopened.recover().unwrap();
    assert_eq!(
        reopened
            .detail(&session.session_id, &receipt.change_id)
            .unwrap()
            .activity
            .chapter_source_state,
        Some(ChapterSourceState::Stale)
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn nr503_external_source_change_before_each_commit_boundary_preserves_external_text() {
    for stage in [
        CommitStage::Prepared,
        CommitStage::BeforeFile(0),
        CommitStage::AfterFile(0),
        CommitStage::BeforeCommit,
    ] {
        let (f, source, evidence, request) = prepare(CanonEntityKind::Character, false);
        let path = f
            .coordinator
            .handle
            .root()
            .join(&source.files[0].relative_path);
        let original = fs::read_to_string(&path).unwrap();
        let external = original.replace("桥上有风。", "外部作者改成了雨。");
        let result = f.coordinator.submit_with_hook(
            &evidence,
            &request,
            "source_race",
            &CancellationToken::default(),
            |at| {
                if at == stage {
                    fs::write(&path, &external).unwrap();
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), external);
        assert_eq!(
            fs::read_dir(f.coordinator.handle.root().join("characters"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            f.coordinator
                .load_record(&source.change_id)
                .unwrap()
                .receipt
                .status,
            ChangeStatus::Committed
        );
        fs::write(&path, &original).unwrap();
        assert!(
            f.coordinator
                .chapter_commit_receipt(&source.change_id)
                .is_err(),
            "observed invalidation never resurrects"
        );
    }
}

#[test]
fn nr503_conflict_and_ambiguity_remain_conversation_only_and_read_only_report_writes_nothing() {
    let (f, _, evidence, mut request) = prepare(CanonEntityKind::Character, false);
    for risk in ["semantic_conflict", "ambiguous_target"] {
        request["risk"] = json!(risk);
        let receipt = f
            .coordinator
            .submit(&evidence, &request, risk, &CancellationToken::default())
            .unwrap();
        assert_eq!(receipt.status, ChangeStatus::Rejected);
        assert!(receipt.files.is_empty());
    }
    let dependency = evidence.chapter_dependencies().remove(0);
    let text =
        fs::read_to_string(f.coordinator.handle.root().join(&dependency.relative_path)).unwrap();
    let line = text.lines().position(|line| line == "桥上有风。").unwrap() as u32 + 1;
    let scope = evidence.chapter_scope();
    let dimensions = [
        ContinuityDimension::Character,
        ContinuityDimension::Timeline,
        ContinuityDimension::WorldRule,
        ContinuityDimension::Clue,
        ContinuityDimension::Macguffin,
        ContinuityDimension::PlotThread,
    ];
    let report = ContinuityReport {
        schema_version: 1,
        project_id: scope.project_id,
        session_id: scope.session_id,
        turn_id: scope.turn_id,
        coverage: dimensions
            .iter()
            .map(|dimension| ContinuityCoverage {
                dimension: *dimension,
                status: CoverageStatus::Partial,
                read_receipt_ids: vec![dependency.read_receipt_id.clone()],
            })
            .collect(),
        findings: dimensions
            .iter()
            .enumerate()
            .map(|(index, dimension)| ContinuityFinding {
                dimension: *dimension,
                kind: [
                    FindingKind::Fact,
                    FindingKind::Inference,
                    FindingKind::Unresolved,
                ][index % 3],
                statement: "此条只验证引用定位，不证明语义判断。".to_owned(),
                citations: vec![SourceCitation {
                    read_receipt_id: dependency.read_receipt_id.clone(),
                    relative_path: dependency.relative_path.clone(),
                    content_hash: dependency.after_hash.clone(),
                    chapter_id: Some(dependency.identity.chapter_id.clone()),
                    start_line: line,
                    end_line: line,
                    quote: "桥上有风。".to_owned(),
                }],
            })
            .collect(),
    };
    report.validate_sources(&evidence).unwrap();
    for fault in 0..6 {
        let mut bad = report.clone();
        match fault {
            0 => bad.findings[0].citations[0].quote = "猜测的引用".to_owned(),
            1 => bad.findings[0].citations[0].content_hash = "f".repeat(64),
            2 => bad.coverage[0].read_receipt_ids.clear(),
            3 => bad.findings[0].citations[0].chapter_id = None,
            4 => bad.session_id = Uuid::new_v4().to_string(),
            _ => bad.coverage[0].dimension = bad.coverage[1].dimension,
        }
        assert!(bad.validate_sources(&evidence).is_err());
    }
    assert_eq!(
        fs::read_dir(f.coordinator.handle.root().join("characters"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(f.coordinator.receipts().unwrap().len(), 1);
}

#[test]
fn nr503_pending_confirmation_rechecks_source_after_undo() {
    let (f, chapter, evidence, mut request) = prepare(CanonEntityKind::Character, false);
    request["risk"] = json!("broad_replacement");
    let pending = f
        .coordinator
        .submit(
            &evidence,
            &request,
            "pending_source",
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(pending.status, ChangeStatus::PendingConfirmation);
    let session = read_conversation(&f.coordinator.handle).unwrap();
    f.coordinator
        .decide(
            &session.session_id,
            &chapter.change_id,
            chapter.revision,
            ChangeDecision::Undo,
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(f
        .coordinator
        .decide(
            &session.session_id,
            &pending.change_id,
            pending.revision,
            ChangeDecision::Confirm,
            &CancellationToken::default()
        )
        .is_err());
    assert_eq!(
        f.coordinator
            .detail(&session.session_id, &pending.change_id)
            .unwrap()
            .activity
            .chapter_source_state,
        Some(ChapterSourceState::Stale)
    );
    assert_eq!(
        fs::read_dir(f.coordinator.handle.root().join("characters"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn nr503_missing_corrupt_recovery_and_truncated_source_proofs_do_not_authorize_writes() {
    for fault in 0..4 {
        let (f, chapter, mut evidence, mut request) = prepare(CanonEntityKind::Character, false);
        let receipt_path = f
            .coordinator
            .directory()
            .unwrap()
            .join(format!("{}.json", chapter.change_id));
        match fault {
            0 => fs::remove_file(receipt_path).unwrap(),
            1 => fs::write(receipt_path, b"{}").unwrap(),
            2 => {
                let mut record = f.coordinator.load_record(&chapter.change_id).unwrap();
                record.receipt.status = ChangeStatus::RecoveryRequired;
                record.receipt.revision += 1;
                f.coordinator.save_record(&record).unwrap();
            }
            _ => {
                let session = read_conversation(&f.coordinator.handle).unwrap();
                let scope = evidence.chapter_scope();
                let next = Arc::new(Mutex::new(
                    ChangeEvidence::from_conversation(&session, &scope.turn_id).unwrap(),
                ));
                let coordinator = Arc::new(
                    CanonChangeCoordinator::new_nr5(
                        f.coordinator.handle.clone(),
                        f.coordinator.lease.clone(),
                        f.coordinator.recovery.clone(),
                    )
                    .unwrap(),
                );
                let executor = NovelistReadContextExecutorV1::new(
                    f.coordinator.handle.clone(),
                    embedded_pack().unwrap(),
                    scope.session_id.clone(),
                    scope.turn_id.clone(),
                )
                .with_change_evidence(coordinator, next.clone());
                let args = normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":"create-character","canonOperations":[{"kind":"read","relativePath":chapter.files[0].relative_path,"startLine":1,"maxBytes":1}]})).unwrap();
                let result = executor.execute(
                    &ToolExecutionContext {
                        project_id: scope.project_id,
                        conversation_session_id: scope.session_id,
                        turn_id: scope.turn_id,
                        call_id: "partial_source".to_owned(),
                    },
                    &args,
                    &CancellationToken::default(),
                );
                assert_eq!(result.status, ToolResultStatus::Succeeded);
                let output = result.output_json.unwrap();
                assert_eq!(output["chapterSources"], json!([]));
                request["operations"][0]["methodReceiptId"] =
                    output["changeEvidence"]["methodReceiptId"].clone();
                evidence = next.lock().unwrap().clone();
            }
        }
        assert!(f
            .coordinator
            .submit(
                &evidence,
                &request,
                "invalid_source",
                &CancellationToken::default()
            )
            .is_err());
        assert_eq!(
            fs::read_dir(f.coordinator.handle.root().join("characters"))
                .unwrap()
                .count(),
            0
        );
        assert!(f
            .coordinator
            .handle
            .root()
            .join(&chapter.files[0].relative_path)
            .exists());
    }
}
