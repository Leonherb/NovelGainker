use super::*;
use crate::capability_pack::embedded_pack;
use crate::conversation::start_conversation_turn;
use crate::coordination::try_acquire_project_write_lease;
use crate::credential_vault::ProviderId;
use crate::project::{inspect_project_root, open_project, ProjectScanMode};
use serde_json::json;

#[path = "nr503_tests.rs"]
mod nr503;
#[path = "nr505_tests.rs"]
mod nr505;

struct Fixture {
    root: PathBuf,
    coordinator: CanonChangeCoordinator,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("novelgainker-nr502-{}", Uuid::new_v4()));
        let project = root.join("project");
        fs::create_dir_all(project.join("chapters")).unwrap();
        fs::write(project.join("project.md"), "# 合成章节工程\n").unwrap();
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
            coordinator: CanonChangeCoordinator::new_nr5(handle, lease, recovery).unwrap(),
        }
    }
    fn candidate(&self) -> (ChangeEvidence, String, ChapterCandidate) {
        start_conversation_turn(
            &self.coordinator.handle,
            ProviderId::Deepseek,
            "synthetic",
            "写并保存第一章",
            1,
        )
        .unwrap();
        let session = read_conversation(&self.coordinator.handle).unwrap();
        let turn = session.turns.last().unwrap();
        let mut evidence = ChangeEvidence::from_conversation(&session, &turn.turn_id).unwrap();
        let pack = embedded_pack().unwrap();
        let capability = pack.capability("novel-story-telling").unwrap();
        let method = evidence
            .record_method(
                &pack,
                "novel-story-telling",
                pack.resource_sha256(&capability.instruction_resource)
                    .unwrap(),
            )
            .unwrap();
        let identity = ChapterIdentity {
            chapter_id: Uuid::new_v4().to_string(),
            chapter_number: 1,
        };
        let markdown = body(&identity.chapter_id, "第一章", "桥上有风。", "draft");
        let candidate = ChapterCandidate {
            schema_version: 1,
            candidate_id: Uuid::new_v4().to_string(),
            scope: crate::chapter_contract::ChapterScope {
                project_id: session.project_id,
                session_id: session.session_id,
                turn_id: turn.turn_id.clone(),
            },
            source: SourceReference {
                message_id: turn.messages[0].message_id.clone(),
                content_hash: sha256_hex(turn.messages[0].content.as_bytes()),
            },
            intent: ChapterIntent::WriteAndSave,
            target: ChapterTarget::Create { identity },
            content_hash: sha256_hex(markdown.as_bytes()),
            proposed_markdown: markdown,
            complete: true,
            discarded: false,
        };
        (evidence, method, candidate)
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
                .starts_with("novelgainker-nr502-")
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
fn body(id: &str, title: &str, text: &str, status: &str) -> String {
    format!("---\nid: {id}\ntitle: {title}\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 1\nstatus: {status}\n---\n\n# {title}\n\n## Draft\n\n{text}\n")
}
fn submit<'a>(candidate: &'a ChapterCandidate, method: &'a str) -> ChapterSubmission<'a> {
    ChapterSubmission {
        candidate,
        method_receipt_id: method,
        finalize: false,
        confirmation_required: false,
        displayed: None,
    }
}

#[test]
fn nr502_pagination_is_bounded_scoped_and_rejects_stale_cursors() {
    let f = Fixture::new();
    let (evidence, method, candidate) = f.candidate();
    let receipt = f
        .coordinator
        .submit_chapter(
            &evidence,
            submit(&candidate, &method),
            "page_seed",
            &CancellationToken::default(),
        )
        .unwrap();
    let original = f.coordinator.load_record(&receipt.change_id).unwrap();
    // Synthetic historical metadata, not a substitute for the separate 500
    // real-commit test. Distinct IDs exercise actual on-disk page enumeration.
    for index in 1..205 {
        let mut record = original.clone();
        record.receipt.change_id = Uuid::new_v4().to_string();
        record.call_id = format!("page_fixture_{index}");
        fs::write(
            f.coordinator
                .directory()
                .unwrap()
                .join(format!("{}.json", record.receipt.change_id)),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }
    let session_id = &candidate.scope.session_id;
    let mut request = ChangeListRequest {
        project_session_id: Uuid::new_v4().to_string(),
        turn_id: None,
        cursor: None,
        limit: 100,
    };
    let first = f.coordinator.activities_page(session_id, &request).unwrap();
    assert_eq!(first.items.len(), 100);
    request.cursor = first.next_cursor.clone();
    let second = f.coordinator.activities_page(session_id, &request).unwrap();
    assert_eq!(second.items.len(), 100);
    request.cursor = second.next_cursor.clone();
    let third = f.coordinator.activities_page(session_id, &request).unwrap();
    assert_eq!(third.items.len(), 5);
    assert!(third.next_cursor.is_none());
    let ids = first
        .items
        .iter()
        .chain(&second.items)
        .chain(&third.items)
        .map(|item| &item.receipt.change_id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 205);
    request.cursor = first.next_cursor;
    request.limit = 99;
    assert!(matches!(
        f.coordinator.activities_page(session_id, &request),
        Err(ChangeError::RevisionConflict)
    ));
    request.limit = 100;
    request.turn_id = Some(candidate.scope.turn_id.clone());
    assert!(matches!(
        f.coordinator.activities_page(session_id, &request),
        Err(ChangeError::RevisionConflict)
    ));
    request.turn_id = None;
    assert!(f
        .coordinator
        .activities_page(&Uuid::new_v4().to_string(), &request)
        .is_err());
    let mut changed = original.clone();
    changed.receipt.revision += 1;
    f.coordinator.save_record(&changed).unwrap();
    assert!(matches!(
        f.coordinator.activities_page(session_id, &request),
        Err(ChangeError::RevisionConflict)
    ));
    request.cursor = None;
    assert_eq!(
        f.coordinator
            .activities_page(session_id, &request)
            .unwrap()
            .items
            .len(),
        100
    );
    for field in ["turnId", "cursor"] {
        let mut wire = serde_json::to_value(&request).unwrap();
        wire.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<ChangeListRequest>(wire).is_err());
    }
    for limit in [0, 101] {
        request.limit = limit;
        assert!(f.coordinator.activities_page(session_id, &request).is_err());
    }
    // Old tool v1 tasks are reconciled by the current Host's capacity, without
    // changing their tool/result version or fabricating a retry.
    assert!(
        task_recovery_result_for(
            &f.coordinator.handle,
            session_id,
            &candidate.scope.turn_id,
            &sha256_hex(b"page_seed"),
            1,
            crate::protocol_profile::ToolProtocolProfile::Nr5
        )
        .is_err(),
        "v1 must not manufacture chapter authority"
    );
    crate::conversation::finish_conversation_turn(
        &f.coordinator.handle,
        &candidate.scope.turn_id,
        "已保存。",
        crate::conversation::ConversationTerminalKind::Completed,
        None,
        2,
    )
    .unwrap();
    let started = start_conversation_turn(
        &f.coordinator.handle,
        ProviderId::Deepseek,
        "synthetic",
        "人物甲喜欢观察，请保存人物资料。",
        3,
    )
    .unwrap();
    let session = read_conversation(&f.coordinator.handle).unwrap();
    let mut legacy_evidence =
        ChangeEvidence::from_conversation(&session, &started.turn_id).unwrap();
    let pack = embedded_pack().unwrap();
    let method = legacy_evidence
        .record_method(
            &pack,
            "create-character",
            pack.resource_sha256(
                &pack
                    .capability("create-character")
                    .unwrap()
                    .instruction_resource,
            )
            .unwrap(),
        )
        .unwrap();
    fs::create_dir(f.coordinator.handle.root().join("characters")).unwrap();
    let source = &session.turns.last().unwrap().messages[0];
    let wire = json!({"schemaVersion":1,"sourceKind":"user_decision","risk":"low","sources":[{"messageId":source.message_id,"contentHash":sha256_hex(source.content.as_bytes())}],"operations":[{"kind":"create","entityKind":"character","capabilityId":"create-character","methodReceiptId":method,"targetId":null,"readReceiptId":null,"expectedSourceHash":null,"proposedMarkdown":"---\nid: new\ntitle: 人物甲\nkind: character\nschemaVersion: 2\nwritingLanguage: zh-CN\n---\n\n# 人物甲\n\n## 摘要\n\n喜欢观察。\n"}]});
    let legacy = f
        .coordinator
        .submit(
            &legacy_evidence,
            &wire,
            "legacy_page_seed",
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(legacy.status, ChangeStatus::Committed);
    let restored = task_recovery_result_for(
        &f.coordinator.handle,
        session_id,
        &started.turn_id,
        &sha256_hex(b"legacy_page_seed"),
        1,
        crate::protocol_profile::ToolProtocolProfile::Nr5,
    )
    .unwrap();
    assert_eq!(restored.version, 1);
    assert_eq!(restored.output_json.unwrap()["status"], "committed");
    assert!(task_recovery_result(
        &f.coordinator.handle,
        session_id,
        &started.turn_id,
        &sha256_hex(b"legacy_page_seed")
    )
    .is_err());
}

#[test]
fn nr502_store_record_candidate_and_byte_limits_preserve_canon_and_receipts() {
    let f = Fixture::new();
    let (evidence, method, candidate) = f.candidate();
    let receipt = f
        .coordinator
        .submit_chapter(
            &evidence,
            submit(&candidate, &method),
            "quota_seed",
            &CancellationToken::default(),
        )
        .unwrap();
    let original = f.coordinator.load_record(&receipt.change_id).unwrap();
    let directory = f.coordinator.directory().unwrap();
    let canon_path = f
        .coordinator
        .handle
        .root()
        .join(&receipt.files[0].relative_path);
    let canon = fs::read(&canon_path).unwrap();
    let receipt_path = directory.join(format!("{}.json", receipt.change_id));
    let receipt_bytes = fs::read(&receipt_path).unwrap();
    let recover = || {
        task_recovery_result_for(
            &f.coordinator.handle,
            &candidate.scope.session_id,
            &candidate.scope.turn_id,
            &sha256_hex(b"quota_seed"),
            2,
            crate::protocol_profile::ToolProtocolProfile::Nr5,
        )
    };
    for index in 1..2048 {
        let mut record = original.clone();
        record.receipt.change_id = Uuid::new_v4().to_string();
        record.call_id = format!("quota_fixture_{index}");
        fs::write(
            directory.join(format!("{}.json", record.receipt.change_id)),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(f.coordinator.records().unwrap().len(), 2048);
    assert!(recover().is_ok());
    let mut extra = original.clone();
    extra.receipt.change_id = Uuid::new_v4().to_string();
    extra.call_id = "over_limit".to_owned();
    assert!(matches!(
        f.coordinator.save_record(&extra),
        Err(ChangeError::StoreFull)
    ));
    let extra_path = directory.join(format!("{}.json", extra.receipt.change_id));
    fs::write(&extra_path, serde_json::to_vec(&extra).unwrap()).unwrap();
    assert!(matches!(
        f.coordinator.records(),
        Err(ChangeError::StoreFull)
    ));
    assert!(matches!(recover(), Err(ChangeError::StoreFull)));
    fs::remove_file(extra_path).unwrap();
    // An oversized record must not evade bounded reading via valid JSON plus
    // whitespace. These isolated fixtures never overwrite a real manuscript.
    let mut padded = receipt_bytes.clone();
    padded.resize(512 * 1024 + 1, b' ');
    fs::write(&receipt_path, padded).unwrap();
    assert!(f.coordinator.records().is_err());
    assert!(recover().is_err());
    fs::write(&receipt_path, &receipt_bytes).unwrap();
    let mut candidate_paths = Vec::new();
    for _ in 0..64 {
        let path = directory.join(format!(".candidate-{}.json", Uuid::new_v4()));
        fs::write(&path, b"{}").unwrap();
        candidate_paths.push(path);
    }
    assert_eq!(f.coordinator.records().unwrap().len(), 2048);
    let over_candidate = directory.join(format!(".candidate-{}.json", Uuid::new_v4()));
    fs::write(&over_candidate, b"{}").unwrap();
    assert!(matches!(
        f.coordinator.records(),
        Err(ChangeError::StoreFull)
    ));
    assert!(matches!(recover(), Err(ChangeError::StoreFull)));
    fs::remove_file(over_candidate).unwrap();
    fs::write(&candidate_paths[0], vec![b' '; 256 * 1024 + 1]).unwrap();
    assert!(matches!(
        f.coordinator.records(),
        Err(ChangeError::StoreFull)
    ));
    assert!(matches!(recover(), Err(ChangeError::StoreFull)));
    for path in candidate_paths {
        fs::remove_file(path).unwrap();
    }
    let orphan = directory.join(format!(".record-{}.tmp", Uuid::new_v4()));
    fs::File::create(&orphan)
        .unwrap()
        .set_len(256 * 1024 * 1024)
        .unwrap();
    assert!(matches!(
        f.coordinator.records(),
        Err(ChangeError::StoreFull)
    ));
    assert!(matches!(recover(), Err(ChangeError::StoreFull)));
    fs::remove_file(orphan).unwrap();
    assert_eq!(fs::read(canon_path).unwrap(), canon);
    assert_eq!(fs::read(receipt_path).unwrap(), receipt_bytes);
    assert!(recover().is_ok());
}

#[test]
fn nr502_host_create_is_durable_idempotent_and_undo_uses_exact_after_bytes() {
    let f = Fixture::new();
    let (evidence, method, candidate) = f.candidate();
    let cancel = CancellationToken::default();
    let receipt = f
        .coordinator
        .submit_chapter(
            &evidence,
            submit(&candidate, &method),
            "chapter_one",
            &cancel,
        )
        .unwrap();
    assert_eq!(receipt.status, ChangeStatus::Committed);
    let path = f
        .coordinator
        .handle
        .root()
        .join(&receipt.files[0].relative_path);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(sha256_hex(&bytes), receipt.files[0].after_hash);
    let proof = f
        .coordinator
        .chapter_commit_receipt(&receipt.change_id)
        .unwrap();
    assert_ne!(
        proof.identity.chapter_id,
        match &candidate.target {
            ChapterTarget::Create { identity } => &identity.chapter_id,
            _ => unreachable!(),
        }
        .as_str()
    );
    assert_eq!(
        f.coordinator
            .submit_chapter(
                &evidence,
                submit(&candidate, &method),
                "chapter_one",
                &cancel
            )
            .unwrap()
            .change_id,
        receipt.change_id
    );
    assert!(f
        .coordinator
        .submit_chapter(
            &evidence,
            submit(&candidate, &method),
            "chapter_two",
            &cancel
        )
        .is_err());
    let diff = f
        .coordinator
        .detail(&candidate.scope.session_id, &receipt.change_id)
        .unwrap();
    assert_eq!(diff.files.len(), 1);
    fs::write(&path, [bytes.as_slice(), b"external"].concat()).unwrap();
    assert!(f
        .coordinator
        .decide(
            &candidate.scope.session_id,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Undo,
            &cancel
        )
        .is_err());
    assert!(f
        .coordinator
        .chapter_commit_receipt(&receipt.change_id)
        .is_err());
    fs::write(&path, bytes).unwrap();
    let undone = f
        .coordinator
        .decide(
            &candidate.scope.session_id,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Undo,
            &cancel,
        )
        .unwrap();
    assert_eq!(undone.status, ChangeStatus::Undone);
    assert!(!path.exists());
    assert!(f
        .coordinator
        .chapter_commit_receipt(&receipt.change_id)
        .is_err());
}

#[test]
fn nr502_replacement_preserves_bom_crlf_and_rejects_stale_or_dirty_targets() {
    let f = Fixture::new();
    let (mut evidence, method, mut candidate) = f.candidate();
    let identity = match &candidate.target {
        ChapterTarget::Create { identity } => identity.clone(),
        _ => unreachable!(),
    };
    let before = format!(
        "\u{feff}{}",
        body(&identity.chapter_id, "第一章", "旧正文。", "final").replace('\n', "\r\n")
    );
    let path = f.coordinator.handle.root().join("chapters/one.md");
    fs::write(&path, &before).unwrap();
    let hash = sha256_hex(before.as_bytes());
    let (_, read) = evidence
        .record_complete_read(
            CanonEntityKind::Chapter,
            "chapters/one.md",
            &before,
            &hash,
            None,
        )
        .unwrap();
    candidate.target = ChapterTarget::Replace {
        identity,
        expected_hash: hash.clone(),
        read_receipt_id: read,
    };
    f.coordinator
        .record_editor_buffer("chapters/one.md", 1, true)
        .unwrap();
    let receipt = f
        .coordinator
        .submit_chapter(
            &evidence,
            submit(&candidate, &method),
            "update_one",
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(receipt.status, ChangeStatus::PendingConfirmation);
    assert_eq!(fs::read(&path).unwrap(), before.as_bytes());
    assert!(f
        .coordinator
        .decide(
            &candidate.scope.session_id,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Confirm,
            &CancellationToken::default()
        )
        .is_err());
    f.coordinator
        .record_editor_buffer("chapters/one.md", 2, false)
        .unwrap();
    assert!(f
        .coordinator
        .decide(
            &candidate.scope.session_id,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Confirm,
            &CancellationToken::default()
        )
        .is_err());
    f.coordinator
        .decide(
            &candidate.scope.session_id,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Discard,
            &CancellationToken::default(),
        )
        .unwrap();
    let committed = f
        .coordinator
        .submit_chapter(
            &evidence,
            submit(&candidate, &method),
            "update_fresh",
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(committed.status, ChangeStatus::Committed);
    let actual = fs::read_to_string(&path).unwrap();
    assert!(actual.starts_with('\u{feff}'));
    assert!(actual.contains("status: draft\r\n"));
    assert!(!actual.replace("\r\n", "").contains('\n'));
    f.coordinator
        .decide(
            &candidate.scope.session_id,
            &committed.change_id,
            committed.revision,
            ChangeDecision::Undo,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), before.as_bytes());
}

#[test]
fn nr502_fault_stages_recover_without_false_commit_or_provider_replay() {
    for failure in [
        CommitStage::Prepared,
        CommitStage::BeforeFile(0),
        CommitStage::AfterFile(0),
        CommitStage::BeforeCommit,
        CommitStage::Committed,
    ] {
        let f = Fixture::new();
        let (evidence, method, candidate) = f.candidate();
        let result = f.coordinator.submit_chapter_with_hook(
            &evidence,
            submit(&candidate, &method),
            "fault",
            &CancellationToken::default(),
            |stage| {
                if stage == failure {
                    Err(ChangeError::Io)
                } else {
                    Ok(())
                }
            },
        );
        let reopened = CanonChangeCoordinator::new_nr5(
            f.coordinator.handle.clone(),
            f.coordinator.lease.clone(),
            f.coordinator.recovery.clone(),
        )
        .unwrap();
        let receipts = reopened.recover().unwrap();
        if failure == CommitStage::Committed {
            assert_eq!(result.unwrap().status, ChangeStatus::Committed);
            assert_eq!(receipts[0].status, ChangeStatus::Committed);
            assert!(reopened
                .chapter_commit_receipt(&receipts[0].change_id)
                .is_ok());
        } else {
            assert!(result.is_err());
            assert_eq!(receipts[0].status, ChangeStatus::Cancelled);
            assert_eq!(
                fs::read_dir(f.coordinator.handle.root().join("chapters"))
                    .unwrap()
                    .count(),
                0
            );
        }
    }
}

#[test]
fn nr502_manual_save_has_local_event_source_without_inventing_chat_and_preserves_no_change() {
    let f = Fixture::new();
    let id = Uuid::new_v4().to_string();
    let original = body(&id, "第一章", "旧正文。", "final");
    let path = f.coordinator.handle.root().join("chapters/manual.md");
    fs::write(&path, &original).unwrap();
    let result = f
        .coordinator
        .save_manual(
            "chapters/manual.md",
            &original.replace("旧正文", "手动新正文"),
            &sha256_hex(original.as_bytes()),
        )
        .unwrap();
    let actual = fs::read_to_string(&path).unwrap();
    assert!(actual.contains("status: draft\n"));
    assert_eq!(sha256_hex(actual.as_bytes()), result.content_hash);
    assert_eq!(result.saved_content.as_deref(), Some(actual.as_str()));
    assert_eq!(result.chapter_finalization_confirmed, Some(false));
    let session = read_conversation(&f.coordinator.handle).unwrap();
    assert!(session.turns.is_empty());
    assert_eq!(
        session.session_id,
        read_conversation(&f.coordinator.handle).unwrap().session_id
    );
    let receipts = f.coordinator.receipts().unwrap();
    assert_eq!(receipts.len(), 1);
    let proof = f
        .coordinator
        .chapter_commit_receipt(&receipts[0].change_id)
        .unwrap();
    assert!(matches!(
        proof.source,
        crate::chapter_contract::ChapterCommitSource::ManualSave { .. }
    ));
    let unchanged = f
        .coordinator
        .save_manual("chapters/manual.md", &actual, &result.content_hash)
        .unwrap();
    assert_eq!(unchanged.content_hash, result.content_hash);
    assert_eq!(f.coordinator.receipts().unwrap().len(), 1);
    assert_eq!(fs::read_to_string(path).unwrap(), actual);
}

#[test]
fn nr502_external_change_invalidation_survives_restore_and_reopen() {
    let f = Fixture::new();
    let (evidence, method, candidate) = f.candidate();
    let receipt = f
        .coordinator
        .submit_chapter(
            &evidence,
            ChapterSubmission {
                finalize: true,
                ..submit(&candidate, &method)
            },
            "final",
            &CancellationToken::default(),
        )
        .unwrap();
    let path = f
        .coordinator
        .handle
        .root()
        .join(&receipt.files[0].relative_path);
    let original = fs::read(&path).unwrap();
    fs::write(&path, [original.as_slice(), b"external"].concat()).unwrap();
    assert!(f
        .coordinator
        .chapter_commit_receipt(&receipt.change_id)
        .is_err());
    fs::write(&path, &original).unwrap();
    let reopened = CanonChangeCoordinator::new_nr5(
        f.coordinator.handle.clone(),
        f.coordinator.lease.clone(),
        f.coordinator.recovery.clone(),
    )
    .unwrap();
    assert!(reopened.chapter_commit_receipt(&receipt.change_id).is_err());
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn nr502_invalid_sources_and_windows_aliases_write_no_chapter() {
    let f = Fixture::new();
    let (evidence, method, candidate) = f.candidate();
    for fault in 0..4 {
        let mut invalid = candidate.clone();
        match fault {
            0 => invalid.source.content_hash = sha256_hex(b"forged"),
            1 => invalid.complete = false,
            2 => invalid.discard(),
            _ => invalid.proposed_markdown.push('x'),
        }
        assert!(f
            .coordinator
            .submit_chapter(
                &evidence,
                submit(&invalid, &method),
                "invalid",
                &CancellationToken::default()
            )
            .is_err());
    }
    for path in [
        "chapters/CON.md",
        "chapters/x.md:stream",
        "chapters/../x.md",
        "chapters/x.md.",
    ] {
        assert!(f
            .coordinator
            .save_manual(path, &candidate.proposed_markdown, &candidate.content_hash)
            .is_err());
    }
    assert_eq!(
        fs::read_dir(f.coordinator.handle.root().join("chapters"))
            .unwrap()
            .count(),
        0
    );
    assert!(f.coordinator.receipts().unwrap().is_empty());
}

#[test]
fn nr502_complete_chapter_read_preserves_raw_bytes_and_never_authorizes_truncation() {
    use crate::canon_context::{
        normalize_deepseek_novelist_read_context_arguments, NovelistReadContextExecutorV1,
    };
    use crate::tool_runtime::{ToolExecutionContext, ToolExecutor, ToolResultStatus};
    let f = Fixture::new();
    let (evidence, _, candidate) = f.candidate();
    let identity = match &candidate.target {
        ChapterTarget::Create { identity } => identity,
        _ => unreachable!(),
    };
    let raw = format!(
        "\u{feff}{}",
        body(
            &identity.chapter_id,
            "第一章",
            &"风雨".repeat(4000),
            "draft"
        )
        .replace('\n', "\r\n")
    );
    assert!(raw.len() > MAX_CHANGE_FILE_BYTES && raw.len() < MAX_CHAPTER_BYTES);
    fs::write(f.coordinator.handle.root().join("chapters/read.md"), &raw).unwrap();
    let binding = Arc::new(Mutex::new(evidence));
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
        candidate.scope.session_id.clone(),
        candidate.scope.turn_id.clone(),
    )
    .with_change_evidence(coordinator, binding.clone());
    let context = ToolExecutionContext {
        project_id: candidate.scope.project_id,
        conversation_session_id: candidate.scope.session_id,
        turn_id: candidate.scope.turn_id,
        call_id: "read_chapter".to_owned(),
    };
    let mut arguments = normalize_deepseek_novelist_read_context_arguments(json!({"capabilityId":"novel-story-telling","canonOperations":[{"kind":"read","relativePath":"chapters/read.md","startLine":1,"maxBytes":4096}]})).unwrap();
    let partial = executor.execute(&context, &arguments, &CancellationToken::default());
    assert_eq!(partial.status, ToolResultStatus::Succeeded);
    let partial = partial.output_json.unwrap();
    assert_eq!(partial["canon"][0]["truncated"], true);
    assert_eq!(partial["changeEvidence"]["completeTargets"], json!([]));
    arguments["canonOperations"][0]["maxBytes"] = json!(MAX_CHAPTER_BYTES);
    executor.validate_arguments(&arguments).unwrap();
    let full = executor.execute(&context, &arguments, &CancellationToken::default());
    assert_eq!(full.status, ToolResultStatus::Succeeded);
    let full = full.output_json.unwrap();
    assert_eq!(full["canon"][0]["content"], raw);
    assert_eq!(full["canon"][0]["contentHash"], sha256_hex(raw.as_bytes()));
    assert_eq!(full["canon"][0]["truncated"], false);
    let receipt = full["changeEvidence"]["completeTargets"][0]["readReceiptId"]
        .as_str()
        .unwrap();
    assert!(binding.lock().unwrap().chapter_read(receipt).is_some());
    assert!(
        serde_json::to_vec(&full).unwrap().len() <= crate::chapter_contract::NR5_TOOL_RESULT_BYTES
    );
    let legacy = NovelistReadContextExecutorV1::new(
        f.coordinator.handle.clone(),
        embedded_pack().unwrap(),
        context.conversation_session_id.clone(),
        context.turn_id.clone(),
    );
    assert!(legacy.validate_arguments(&arguments).is_err());
    arguments["canonOperations"][0]["maxBytes"] = json!(MAX_CHAPTER_BYTES + 1);
    assert!(executor.validate_arguments(&arguments).is_err());
}

#[test]
fn nr502_unreadable_committed_source_stays_invalid_after_original_bytes_are_restored() {
    let f = Fixture::new();
    let (evidence, method, candidate) = f.candidate();
    let receipt = f
        .coordinator
        .submit_chapter(
            &evidence,
            submit(&candidate, &method),
            "large_external",
            &CancellationToken::default(),
        )
        .unwrap();
    let path = f
        .coordinator
        .handle
        .root()
        .join(&receipt.files[0].relative_path);
    let original = fs::read(&path).unwrap();
    fs::write(&path, vec![b'x'; MAX_CHAPTER_BYTES + 1]).unwrap();
    assert!(f
        .coordinator
        .chapter_commit_receipt(&receipt.change_id)
        .is_err());
    fs::write(&path, &original).unwrap();
    assert!(f
        .coordinator
        .chapter_commit_receipt(&receipt.change_id)
        .is_err());
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn nr502_registered_chapter_tool_uses_closed_schema_and_replays_one_durable_commit() {
    use crate::tool_runtime::{
        ProviderToolCall, ToolExecutionContext, ToolRegistry, ToolResultStatus,
    };
    let f = Fixture::new();
    let (evidence, method, candidate) = f.candidate();
    let coordinator = Arc::new(
        CanonChangeCoordinator::new_nr5(
            f.coordinator.handle.clone(),
            f.coordinator.lease.clone(),
            f.coordinator.recovery.clone(),
        )
        .unwrap(),
    );
    let registry = ToolRegistry::novelist_with_changes(
        &f.coordinator.handle,
        embedded_pack().unwrap(),
        candidate.scope.session_id.clone(),
        candidate.scope.turn_id.clone(),
        coordinator.clone(),
        Arc::new(Mutex::new(evidence)),
    )
    .unwrap();
    let definitions = registry.definitions();
    // The NR-502 read/change surface stays exact; NR-505 adds only publication.
    assert_eq!(
        definitions
            .iter()
            .filter(|definition| definition.name != "novelist.publish")
            .count(),
        2
    );
    assert_eq!(
        definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "novelist.read_context",
            "novelist.propose_change",
            "novelist.publish"
        ]
    );
    assert!(registry
        .definitions()
        .iter()
        .all(|definition| definition.version == 2));
    let arguments = json!({"schemaVersion":2,"sourceKind":"user_decision","sources":[candidate.source],"risk":"low","chapterIntent":"write_and_save","finalize":false,"candidateId":null,"operations":[{"kind":"create","entityKind":"chapter","capabilityId":"novel-story-telling","methodReceiptId":method,"targetId":null,"readReceiptId":null,"expectedSourceHash":null,"proposedMarkdown":replace_field(&candidate.proposed_markdown, "id", "new").unwrap()}]});
    let context = ToolExecutionContext {
        project_id: candidate.scope.project_id,
        conversation_session_id: candidate.scope.session_id,
        turn_id: candidate.scope.turn_id,
        call_id: "wire_chapter".to_owned(),
    };
    for fault in 0..7 {
        let mut invalid = arguments.clone();
        match fault {
            0 => {
                invalid.as_object_mut().unwrap().remove("finalize");
            }
            1 => invalid["operations"][0]["relativePath"] = json!("chapters/arbitrary.md"),
            2 => invalid["chapterIntent"] = json!("trial_draft"),
            3 => invalid["chapterIntent"] = json!("ambiguous"),
            4 => {
                invalid["chapterIntent"] = json!("accept_candidate");
                invalid["candidateId"] = json!(Uuid::new_v4().to_string());
            }
            5 => invalid["sources"][0]["contentHash"] = json!(sha256_hex(b"forged")),
            _ => invalid["operations"]
                .as_array_mut()
                .unwrap()
                .push(arguments["operations"][0].clone()),
        }
        let call = ProviderToolCall {
            call_id: context.call_id.clone(),
            name: PROPOSE_CHANGE_TOOL_NAME.to_owned(),
            arguments_json: serde_json::to_string(&invalid).unwrap(),
        };
        if let Ok((_, executor, decoded)) = registry.resolve(&call) {
            assert!(executor.validate_arguments(&decoded).is_err());
            assert_eq!(
                executor
                    .execute(&context, &decoded, &CancellationToken::default())
                    .status,
                ToolResultStatus::Failed
            );
        }
        assert!(coordinator.receipts().unwrap().is_empty());
    }
    let call = ProviderToolCall {
        call_id: context.call_id.clone(),
        name: PROPOSE_CHANGE_TOOL_NAME.to_owned(),
        arguments_json: serde_json::to_string(&arguments).unwrap(),
    };
    let (definition, executor, decoded) = registry.resolve(&call).unwrap();
    definition.validate_arguments(&decoded).unwrap();
    executor.validate_arguments(&decoded).unwrap();
    let first = executor.execute(&context, &decoded, &CancellationToken::default());
    assert_eq!(first.status, ToolResultStatus::Succeeded);
    assert_eq!(first.output_json.as_ref().unwrap()["status"], "committed");
    let second = executor.execute(&context, &decoded, &CancellationToken::default());
    assert_eq!(first.output_json, second.output_json);
    assert_eq!(coordinator.receipts().unwrap().len(), 1);
    let receipt = coordinator.receipts().unwrap().remove(0);
    assert!(coordinator
        .chapter_commit_receipt(&receipt.change_id)
        .is_ok());
}

#[test]
fn nr502_five_hundred_real_commits_exceed_legacy_record_limit_without_dropping_undo() {
    let f = Fixture::new();
    let mut first = None;
    for number in 1..=500 {
        let (evidence, method, mut candidate) = f.candidate();
        if let ChapterTarget::Create { identity } = &mut candidate.target {
            identity.chapter_number = number;
        }
        candidate.proposed_markdown = candidate
            .proposed_markdown
            .replace("chapterNumber: 1\n", &format!("chapterNumber: {number}\n"));
        candidate.content_hash = sha256_hex(candidate.proposed_markdown.as_bytes());
        let receipt = f
            .coordinator
            .submit_chapter(
                &evidence,
                submit(&candidate, &method),
                "scale",
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(receipt.status, ChangeStatus::Committed);
        if first.is_none() {
            first = Some((candidate.scope.session_id, receipt));
        }
    }
    assert_eq!(f.coordinator.receipts().unwrap().len(), 500);
    assert_eq!(
        fs::read_dir(f.coordinator.handle.root().join("chapters"))
            .unwrap()
            .count(),
        500
    );
    let (evidence, method, mut overflow) = f.candidate();
    if let ChapterTarget::Create { identity } = &mut overflow.target {
        identity.chapter_number = 501;
    }
    overflow.proposed_markdown = overflow
        .proposed_markdown
        .replace("chapterNumber: 1\n", "chapterNumber: 501\n");
    overflow.content_hash = sha256_hex(overflow.proposed_markdown.as_bytes());
    assert!(f
        .coordinator
        .submit_chapter(
            &evidence,
            submit(&overflow, &method),
            "overflow",
            &CancellationToken::default()
        )
        .is_err());
    let (session, receipt) = first.unwrap();
    assert!(f
        .coordinator
        .chapter_commit_receipt(&receipt.change_id)
        .is_ok());
    let undone = f
        .coordinator
        .decide(
            &session,
            &receipt.change_id,
            receipt.revision,
            ChangeDecision::Undo,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(undone.status, ChangeStatus::Undone);
    assert_eq!(f.coordinator.receipts().unwrap().len(), 500);
    assert_eq!(
        fs::read_dir(f.coordinator.handle.root().join("chapters"))
            .unwrap()
            .count(),
        499
    );
}

#[test]
fn nr502_acceptance_requires_exact_completed_visible_candidate_and_current_user() {
    use crate::conversation::{finish_conversation_turn, ConversationTerminalKind};
    let f = Fixture::new();
    let (old_evidence, _, old) = f.candidate();
    let shown = body(
        old_evidence.chapter_draft_id(),
        "第一章",
        "只展示的完整候选。",
        "draft",
    );
    let visible = format!("这是未保存的试写。\n```markdown\n{shown}```\n");
    finish_conversation_turn(
        &f.coordinator.handle,
        &old.scope.turn_id,
        &visible,
        ConversationTerminalKind::Completed,
        None,
        2,
    )
    .unwrap();
    f.coordinator
        .capture_displayed_chapter(&old_evidence)
        .unwrap();
    let stored = f
        .coordinator
        .displayed_chapter_candidate(old_evidence.chapter_draft_id(), &old.scope.session_id)
        .unwrap();
    assert_eq!(stored.proposed_markdown, shown);
    assert!(f.coordinator.receipts().unwrap().is_empty());
    assert_eq!(
        fs::read_dir(f.coordinator.handle.root().join("chapters"))
            .unwrap()
            .count(),
        0
    );
    let (evidence, method, current) = f.candidate();
    let arguments = json!({"schemaVersion":2,"sourceKind":"user_decision","sources":[current.source],"risk":"low","chapterIntent":"accept_candidate","finalize":false,"candidateId":stored.candidate_id,"operations":[{"kind":"create","entityKind":"chapter","capabilityId":"novel-story-telling","methodReceiptId":method,"targetId":null,"readReceiptId":null,"expectedSourceHash":null,"proposedMarkdown":shown}]});
    for fault in 0..3 {
        let mut invalid = arguments.clone();
        match fault {
            0 => {
                invalid["operations"][0]["proposedMarkdown"] = json!(shown.replace("完整", "篡改"))
            }
            1 => invalid["candidateId"] = json!(Uuid::new_v4().to_string()),
            _ => invalid["sources"] = json!([old.source]),
        }
        assert!(tool::decode(&evidence, &invalid, &f.coordinator).is_err());
    }
    let tool::Request::Chapter(prepared) =
        tool::decode(&evidence, &arguments, &f.coordinator).unwrap()
    else {
        panic!("expected chapter");
    };
    let receipt = prepared
        .submit(
            &f.coordinator,
            &evidence,
            "accept_displayed",
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(receipt.status, ChangeStatus::Committed);
    assert_eq!(
        fs::read_to_string(
            f.coordinator
                .handle
                .root()
                .join(&receipt.files[0].relative_path)
        )
        .unwrap(),
        shown
    );
    assert_eq!(
        f.coordinator
            .chapter_commit_receipt(&receipt.change_id)
            .unwrap()
            .after_hash,
        stored.content_hash
    );
    assert_eq!(
        f.coordinator
            .displayed_chapter_catalog(&stored.scope.session_id)
            .unwrap(),
        json!([])
    );
    assert!(f
        .coordinator
        .discard_displayed_chapter(&stored.candidate_id, &stored.scope.session_id)
        .is_err());
}

#[test]
fn nr502_candidate_discard_and_partial_reply_never_create_or_resurrect_authority() {
    use crate::conversation::{finish_conversation_turn, ConversationTerminalKind};
    for partial in [false, true] {
        let f = Fixture::new();
        let (evidence, _, candidate) = f.candidate();
        let shown = body(
            evidence.chapter_draft_id(),
            "第一章",
            "未保存候选。",
            "draft",
        );
        let visible = format!("```markdown\n{shown}```\n");
        finish_conversation_turn(
            &f.coordinator.handle,
            &candidate.scope.turn_id,
            &visible,
            if partial {
                ConversationTerminalKind::Failed
            } else {
                ConversationTerminalKind::Completed
            },
            None,
            2,
        )
        .unwrap();
        f.coordinator.capture_displayed_chapter(&evidence).unwrap();
        if !partial {
            let request = ChangeListRequest {
                project_session_id: Uuid::new_v4().to_string(),
                turn_id: None,
                cursor: None,
                limit: 100,
            };
            let page = f
                .coordinator
                .activities_page(&candidate.scope.session_id, &request)
                .unwrap();
            assert!(page.items.is_empty());
            assert_eq!(page.candidates.len(), 1);
            assert_eq!(page.candidates[0].candidate_id, evidence.chapter_draft_id());
            assert!(f
                .coordinator
                .decide(
                    &candidate.scope.session_id,
                    evidence.chapter_draft_id(),
                    2,
                    ChangeDecision::Discard,
                    &CancellationToken::default()
                )
                .is_err());
            assert!(f
                .coordinator
                .decide(
                    &Uuid::new_v4().to_string(),
                    evidence.chapter_draft_id(),
                    1,
                    ChangeDecision::Discard,
                    &CancellationToken::default()
                )
                .is_err());
            let discarded = f
                .coordinator
                .decide(
                    &candidate.scope.session_id,
                    evidence.chapter_draft_id(),
                    1,
                    ChangeDecision::Discard,
                    &CancellationToken::default(),
                )
                .unwrap();
            assert_eq!(discarded.status, ChangeStatus::Rejected);
            assert_eq!(discarded.revision, 2);
            assert!(discarded.files.is_empty());
            assert!(f
                .coordinator
                .activities_page(&candidate.scope.session_id, &request)
                .unwrap()
                .candidates
                .is_empty());
            f.coordinator.capture_displayed_chapter(&evidence).unwrap();
        }
        assert!(f
            .coordinator
            .displayed_chapter_candidate(evidence.chapter_draft_id(), &candidate.scope.session_id)
            .is_err());
        assert_eq!(
            f.coordinator
                .displayed_chapter_catalog(&candidate.scope.session_id)
                .unwrap(),
            json!([])
        );
        assert_eq!(
            fs::read_dir(f.coordinator.handle.root().join("chapters"))
                .unwrap()
                .count(),
            0
        );
        assert!(
            read_conversation(&f.coordinator.handle).unwrap().turns[0].messages[1]
                .content
                .contains("未保存候选")
        );
    }
}
