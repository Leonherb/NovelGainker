use super::*;
use crate::canon_change_store::publication::PublicationCheckpoint;
use crate::canon_context::{
    normalize_deepseek_novelist_read_context_arguments, NovelistReadContextExecutorV1,
};
use crate::conversation::{finish_conversation_turn, ConversationTerminalKind};
use crate::publication::{
    store, PublicationAction, PublicationIssue, PublicationRequest, PublicationStatus,
};
use crate::tool_runtime::{ToolExecutionContext, ToolExecutor, ToolResultStatus};

fn seed(finalize: bool) -> (Fixture, ChangeReceipt) {
    let f = Fixture::new();
    fs::write(f.coordinator.handle.root().join("project.md"), "---\ntitle: 合成出版工程\nauthorDisplayName: 合成作者\nwritingLanguage: zh-CN\nkind: project\nschemaVersion: 1\n---\n# 工程\n").unwrap();
    fs::create_dir(f.coordinator.handle.root().join("assets")).unwrap();
    let (evidence, method, candidate) = f.candidate();
    let receipt = f
        .coordinator
        .submit_chapter(
            &evidence,
            ChapterSubmission {
                finalize,
                ..submit(&candidate, &method)
            },
            "publication_seed",
            &CancellationToken::default(),
        )
        .unwrap();
    (f, receipt)
}

fn publication_turn(
    f: &Fixture,
    action: PublicationAction,
) -> (ChangeEvidence, PublicationRequest, ToolExecutionContext) {
    let session = read_conversation(&f.coordinator.handle).unwrap();
    if let Some(turn) = session.turns.last() {
        finish_conversation_turn(
            &f.coordinator.handle,
            &turn.turn_id,
            "合成上一轮结束。",
            ConversationTerminalKind::Completed,
            None,
            2,
        )
        .unwrap();
    }
    let started = start_conversation_turn(
        &f.coordinator.handle,
        ProviderId::Deepseek,
        "synthetic",
        "请按当前有效定稿章节构建本地 EPUB；不要修改原稿。",
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
    let reader = NovelistReadContextExecutorV1::new(
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
        call_id: "publish_actual".to_owned(),
    };
    let arguments = normalize_deepseek_novelist_read_context_arguments(
        json!({"capabilityId":"publish-novel","canonOperations":[]}),
    )
    .unwrap();
    let output = reader.execute(&context, &arguments, &CancellationToken::default());
    assert_eq!(output.status, ToolResultStatus::Succeeded);
    let method = output.output_json.unwrap()["changeEvidence"]["methodReceiptId"]
        .as_str()
        .unwrap()
        .to_owned();
    let evidence = evidence.lock().unwrap().clone();
    let source = evidence
        .source_references()
        .into_iter()
        .find(|source| evidence.current_user_source(source))
        .unwrap();
    (
        evidence,
        PublicationRequest {
            schema_version: 2,
            action,
            method_receipt_id: method,
            source,
        },
        context,
    )
}

#[test]
fn nr505_actual_final_commit_read_method_build_receipt_and_idempotency_preserve_canon() {
    let (f, chapter) = seed(true);
    let source_path = f
        .coordinator
        .handle
        .root()
        .join(&chapter.files[0].relative_path);
    let before = fs::read(&source_path).unwrap();
    let (evidence, request, context) = publication_turn(&f, PublicationAction::Build);
    crate::publication::tool::definition().validate().unwrap();
    let receipt = f
        .coordinator
        .publish(&evidence, &request, &context, &CancellationToken::default())
        .unwrap();
    assert_eq!(receipt.status, PublicationStatus::Generated, "{receipt:?}");
    assert_eq!(receipt.chapter_count, 1);
    assert!(receipt.without_cover);
    let artifact_path = f
        .coordinator
        .handle
        .root()
        .join(receipt.relative_path.as_ref().unwrap());
    let artifact = fs::read(&artifact_path).unwrap();
    assert_eq!(
        receipt.output_hash.as_deref(),
        Some(sha256_hex(&artifact).as_str())
    );
    assert_eq!(fs::read(&source_path).unwrap(), before);
    assert_eq!(
        f.coordinator
            .publish(&evidence, &request, &context, &CancellationToken::default())
            .unwrap(),
        receipt
    );
    let mut duplicate = context.clone();
    duplicate.call_id = "another_publish".into();
    assert!(f
        .coordinator
        .publish(
            &evidence,
            &request,
            &duplicate,
            &CancellationToken::default()
        )
        .is_err());
    assert_eq!(
        fs::read_dir(f.coordinator.handle.root().join("published"))
            .unwrap()
            .count(),
        1
    );
    let detail = f
        .coordinator
        .publication_detail(&receipt.artifact_id)
        .unwrap();
    assert_eq!(
        detail
            .inputs
            .iter()
            .filter(|input| input.confirmation_id.is_some())
            .count(),
        1
    );
    assert_eq!(detail.receipt, receipt);
    let page = f.coordinator.publication_list(None).unwrap();
    assert_eq!(page.items, vec![receipt.clone()]);
    let old_hash = sha256_hex(&artifact);
    let (evidence, request, context) = publication_turn(&f, PublicationAction::Build);
    let another = f
        .coordinator
        .publish(&evidence, &request, &context, &CancellationToken::default())
        .unwrap();
    assert_eq!(another.status, PublicationStatus::Generated);
    assert_ne!(another.artifact_id, receipt.artifact_id);
    assert_eq!(sha256_hex(&fs::read(artifact_path).unwrap()), old_hash);
}

#[test]
fn nr505_preflight_missing_metadata_draft_and_unconfirmed_final_never_generate() {
    for mode in 0..4 {
        let (f, chapter) = seed(mode == 0 || mode == 1);
        if mode == 1 {
            fs::write(
                f.coordinator.handle.root().join("project.md"),
                "# 无作者元数据",
            )
            .unwrap();
        }
        if mode == 3 {
            let path = f
                .coordinator
                .handle
                .root()
                .join(&chapter.files[0].relative_path);
            let text = fs::read_to_string(&path)
                .unwrap()
                .replace("status: draft", "status: final");
            fs::write(path, text).unwrap();
        }
        let (evidence, request, context) = publication_turn(&f, PublicationAction::Preflight);
        let receipt = f
            .coordinator
            .publish(&evidence, &request, &context, &CancellationToken::default())
            .unwrap();
        assert!(receipt.relative_path.is_none());
        assert_eq!(
            fs::read_dir(f.coordinator.handle.root().join("published"))
                .unwrap()
                .count(),
            0
        );
        match mode {
            0 => assert_eq!(receipt.status, PublicationStatus::PreflightPassed),
            1 => assert_eq!(receipt.issue, Some(PublicationIssue::MetadataRequired)),
            2 => assert_eq!(receipt.issue, Some(PublicationIssue::NoFinalChapters)),
            _ => assert_eq!(
                receipt.issue,
                Some(PublicationIssue::FinalConfirmationRequired)
            ),
        }
    }
}

#[test]
fn nr505_snapshot_and_post_promotion_external_change_latch_stale_without_mixed_book() {
    for checkpoint in [
        PublicationCheckpoint::Snapshot,
        PublicationCheckpoint::Built,
        PublicationCheckpoint::Promoted,
    ] {
        let (f, chapter) = seed(true);
        let path = f
            .coordinator
            .handle
            .root()
            .join(&chapter.files[0].relative_path);
        let original = fs::read(&path).unwrap();
        let (evidence, request, context) = publication_turn(&f, PublicationAction::Build);
        let receipt = f
            .coordinator
            .publish_with_hook(
                &evidence,
                &request,
                &context,
                &CancellationToken::default(),
                |stage| {
                    if stage == checkpoint {
                        fs::write(
                            &path,
                            [original.as_slice(), "外部修改。".as_bytes()].concat(),
                        )
                        .unwrap();
                    }
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(receipt.status, PublicationStatus::Stale, "{receipt:?}");
        if checkpoint == PublicationCheckpoint::Promoted {
            let bytes = fs::read(
                f.coordinator
                    .handle
                    .root()
                    .join(receipt.relative_path.as_ref().unwrap()),
            )
            .unwrap();
            assert!(!bytes
                .windows("外部修改。".len())
                .any(|v| v == "外部修改。".as_bytes()));
        } else {
            assert!(receipt.relative_path.is_none());
        }
        fs::write(&path, original).unwrap();
        let reopened = CanonChangeCoordinator::new_nr5(
            f.coordinator.handle.clone(),
            f.coordinator.lease.clone(),
            f.coordinator.recovery.clone(),
        )
        .unwrap();
        assert!(reopened.chapter_commit_receipt(&chapter.change_id).is_err());
        assert_eq!(
            reopened
                .publication_detail(&receipt.artifact_id)
                .unwrap()
                .receipt
                .status,
            PublicationStatus::Stale
        );
    }
}

#[test]
fn nr505_authority_forgery_cancellation_and_no_clobber_are_rejected() {
    let (f, _) = seed(true);
    let (evidence, request, context) = publication_turn(&f, PublicationAction::Build);
    let mut forged = request.clone();
    forged.method_receipt_id = Uuid::new_v4().to_string();
    assert!(f
        .coordinator
        .publish(&evidence, &forged, &context, &CancellationToken::default())
        .is_err());
    forged = request.clone();
    forged.source.content_hash = "0".repeat(64);
    assert!(f
        .coordinator
        .publish(&evidence, &forged, &context, &CancellationToken::default())
        .is_err());
    let receipt = f
        .coordinator
        .publish_with_hook(
            &evidence,
            &request,
            &context,
            &CancellationToken::default(),
            |stage| {
                if stage == PublicationCheckpoint::Snapshot {
                    return Err(PublicationIssue::Cancelled);
                }
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(receipt.issue, Some(PublicationIssue::Cancelled));
    assert!(receipt.relative_path.is_none());
    let directory = f.coordinator.handle.root().join("published");
    let temporary = directory.join("isolated-source.tmp");
    let target = directory.join("isolated-existing.epub");
    fs::write(&temporary, b"new").unwrap();
    fs::write(&target, b"old").unwrap();
    assert!(store::promote(&temporary, &target).is_err());
    assert_eq!(fs::read(target).unwrap(), b"old");
    assert_eq!(fs::read(temporary).unwrap(), b"new");
}

#[test]
fn nr505_reopen_ready_intent_reconciles_only_matching_artifact_never_rebuilds() {
    let (f, _) = seed(true);
    let (evidence, request, context) = publication_turn(&f, PublicationAction::Build);
    let receipt = f
        .coordinator
        .publish(&evidence, &request, &context, &CancellationToken::default())
        .unwrap();
    let mut record = store::load_all(&f.coordinator.handle).unwrap().remove(0);
    record.stage = store::Stage::ReadyToPromote;
    record.receipt.status = PublicationStatus::ValidationFailed;
    record.receipt.issue = Some(PublicationIssue::StoreUnavailable);
    store::save(&f.coordinator.handle, &record).unwrap();
    let reopened = CanonChangeCoordinator::new_nr5(
        f.coordinator.handle.clone(),
        f.coordinator.lease.clone(),
        f.coordinator.recovery.clone(),
    )
    .unwrap();
    assert_eq!(
        reopened
            .publication_detail(&receipt.artifact_id)
            .unwrap()
            .receipt,
        receipt
    );
    let path = f
        .coordinator
        .handle
        .root()
        .join(receipt.relative_path.as_ref().unwrap());
    fs::write(&path, b"external corrupt EPUB").unwrap();
    assert_eq!(
        reopened
            .publication_detail(&receipt.artifact_id)
            .unwrap()
            .receipt
            .status,
        PublicationStatus::ValidationFailed
    );
    assert_eq!(fs::read(path).unwrap(), b"external corrupt EPUB");
    assert_eq!(
        fs::read_dir(f.coordinator.handle.root().join("published"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn nr505_task_recovery_uses_persisted_generated_fact_not_cancel_or_replay() {
    for completed in [true, false] {
        let (f, _) = seed(true);
        let (evidence, request, context) = publication_turn(&f, PublicationAction::Build);
        let call = crate::tool_runtime::ProviderToolCall {
            call_id: context.call_id.clone(),
            name: "novelist.publish".to_owned(),
            arguments_json: serde_json::to_string(&request).unwrap(),
        };
        let task = crate::task_store::create_queued_task(
            &f.coordinator.handle,
            &context.conversation_session_id,
            &context.turn_id,
            &call,
            &crate::publication::tool::definition(),
            4,
        )
        .unwrap();
        crate::task_store::transition_task(
            &f.coordinator.handle,
            &task.task_id,
            task.task_revision,
            crate::task_store::TaskStatus::Running,
            None,
            None,
            5,
        )
        .unwrap();
        if completed {
            f.coordinator
                .publish(&evidence, &request, &context, &CancellationToken::default())
                .unwrap();
        }
        let session = read_conversation(&f.coordinator.handle).unwrap();
        let recovered = crate::task_store::runtime_view_for(
            &f.coordinator.handle,
            session,
            6,
            true,
            crate::protocol_profile::ToolProtocolProfile::Nr5,
        )
        .unwrap();
        assert_eq!(
            recovered.task_activities[0].status,
            if completed {
                crate::task_store::TaskStatus::Succeeded
            } else {
                crate::task_store::TaskStatus::Failed
            }
        );
        let records = store::load_all(&f.coordinator.handle).unwrap();
        assert_eq!(records.len(), usize::from(completed));
        if completed {
            assert_eq!(
                fs::read_dir(f.coordinator.handle.root().join("published"))
                    .unwrap()
                    .count(),
                1
            );
        }
    }
}

#[test]
fn nr505_persisted_record_missing_nullable_fields_and_forged_inputs_fail_closed() {
    let (f, _) = seed(true);
    let (evidence, request, context) = publication_turn(&f, PublicationAction::Build);
    let receipt = f
        .coordinator
        .publish(&evidence, &request, &context, &CancellationToken::default())
        .unwrap();
    let record = store::load_all(&f.coordinator.handle).unwrap().remove(0);
    let value = serde_json::to_value(&record).unwrap();
    let path = f.coordinator.handle.root().join(format!(
        ".novelgainker/publications/{}.json",
        receipt.artifact_id
    ));
    for key in ["relativePath", "outputHash", "sizeBytes", "issue"] {
        let mut missing = value.clone();
        missing["receipt"].as_object_mut().unwrap().remove(key);
        fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
        assert!(store::load_all(&f.coordinator.handle).is_err(), "{key}");
    }
    for key in ["confirmationId", "confirmationRevision"] {
        let mut missing = value.clone();
        missing["inputs"][0].as_object_mut().unwrap().remove(key);
        fs::write(&path, serde_json::to_vec(&missing).unwrap()).unwrap();
        assert!(store::load_all(&f.coordinator.handle).is_err(), "{key}");
    }
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(store::load_all(&f.coordinator.handle).unwrap().len(), 1);
    let chapter_index = record
        .inputs
        .iter()
        .position(|i| i.confirmation_id.is_some())
        .unwrap();
    for relative in [
        "chapters/nested/a.md",
        "chapters/../a.md",
        "chapters/CON.md",
        "assets/../a.png",
    ] {
        let mut forged = record.clone();
        forged.inputs[chapter_index].relative_path = relative.into();
        assert!(
            forged.validate(f.coordinator.handle.project_id()).is_err(),
            "{relative}"
        );
    }
    let mut forged = record.clone();
    forged.receipt.skipped_drafts = 1;
    assert!(forged.validate(f.coordinator.handle.project_id()).is_err());
    let mut oversized = record.clone();
    for index in 0..5 {
        oversized
            .inputs
            .push(crate::publication::store::InputReference {
                relative_path: format!("assets/{index}.png"),
                content_hash: "1".repeat(64),
                size_bytes: 16 * 1024 * 1024,
                confirmation_id: None,
                confirmation_revision: None,
            });
    }
    assert!(oversized
        .validate(f.coordinator.handle.project_id())
        .is_err());
    // Derived-record errors do not remove or rewrite a previously generated artifact.
    assert_eq!(
        sha256_hex(
            &fs::read(
                f.coordinator
                    .handle
                    .root()
                    .join(receipt.relative_path.unwrap())
            )
            .unwrap()
        ),
        receipt.output_hash.unwrap()
    );
}

#[test]
fn nr505_catalog_pagination_storage_quota_and_unsafe_targets_are_bounded() {
    let (f, _) = seed(true);
    let (evidence, request, context) = publication_turn(&f, PublicationAction::Preflight);
    f.coordinator
        .publish(&evidence, &request, &context, &CancellationToken::default())
        .unwrap();
    let record = store::load_all(&f.coordinator.handle).unwrap().remove(0);
    let mut records = (0..256)
        .map(|index| {
            let mut item = record.clone();
            item.created_at_ms += index;
            item.receipt.artifact_id = Uuid::new_v4().to_string();
            item
        })
        .collect::<Vec<_>>();
    let first = store::page(&records, None).unwrap();
    assert_eq!(first.items.len(), 20);
    let cursor = first.next_cursor.unwrap();
    let mut next = Some(cursor.clone());
    let mut count = 20;
    let mut pages = 1;
    while let Some(cursor) = next {
        let page = store::page(&records, Some(&cursor)).unwrap();
        count += page.items.len();
        pages += 1;
        next = page.next_cursor;
    }
    assert_eq!((count, pages), (256, 13));
    records[0].receipt.status = PublicationStatus::Stale;
    records[0].receipt.issue = Some(PublicationIssue::SourceChanged);
    assert!(store::page(&records, Some(&cursor)).is_err());
    for path in [
        "published/../outside.epub",
        "published/CON.epub",
        "published/a.epub:stream",
        "published/a.epub ",
        "published\\a.epub",
    ] {
        assert!(
            store::target(&f.coordinator.handle, path).is_err(),
            "{path}"
        );
    }
    let orphan = f
        .coordinator
        .handle
        .root()
        .join("published/.build-isolated-quota.tmp");
    let file = fs::File::create(&orphan).unwrap();
    file.set_len(crate::publication::MAX_PUBLICATION_STORE_BYTES)
        .unwrap();
    drop(file);
    assert_eq!(
        store::capacity(&f.coordinator.handle, 1).unwrap_err(),
        PublicationIssue::LimitExceeded
    );
    assert_eq!(
        fs::metadata(orphan).unwrap().len(),
        crate::publication::MAX_PUBLICATION_STORE_BYTES
    );
}
