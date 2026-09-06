use super::*;
use crate::coordination::{try_acquire_project_write_lease, ProjectWriteLease};
#[test]
fn nr602_clear_failure_and_tampered_prepared_orphan_preserve_old_history() {
    let (f, lease) = fixture();
    complete(&f.handle, "不能丢失", 1);
    let page = list_sessions(&f.handle, false, None, 3).unwrap();
    let before = fs::read(catalog_path(&f.handle).unwrap()).unwrap();
    let history = read_conversation(&f.handle).unwrap();
    let operation = SessionManagement::Clear {
        conversation_session_id: history.session_id.clone(),
        request_id: Uuid::new_v4().to_string(),
        name: "空白".into(),
        expected_revision: page.revision,
    };
    FAIL_NEXT_CATALOG_PROMOTION.with(|flag| flag.set(true));
    assert!(manage_session(&f.handle, &lease, &operation, 4).is_err());
    assert_eq!(before, fs::read(catalog_path(&f.handle).unwrap()).unwrap());
    let directory = f.root.join(".novelgainker/conversation/sessions");
    let orphan = fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&orphan, b"unknown content must not be overwritten").unwrap();
    assert!(manage_session(&f.handle, &lease, &operation, 5).is_err());
    assert_eq!(
        fs::read(&orphan).unwrap(),
        b"unknown content must not be overwritten"
    );
    assert_eq!(before, fs::read(catalog_path(&f.handle).unwrap()).unwrap());
    assert_eq!(history, read_conversation(&f.handle).unwrap());
}
#[test]
fn nr602_branch_rejects_foreign_boundary_and_total_catalog_limit_without_preparing_files() {
    let (f, lease) = fixture();
    complete(&f.handle, "唯一轮次", 1);
    let page = list_sessions(&f.handle, false, None, 3).unwrap();
    let old = read_conversation(&f.handle).unwrap();
    let mut operation = SessionManagement::Branch {
        conversation_session_id: old.session_id.clone(),
        request_id: Uuid::new_v4().to_string(),
        boundary_turn_id: Uuid::new_v4().to_string(),
        name: "分支".into(),
        expected_revision: page.revision,
    };
    assert!(manage_session(&f.handle, &lease, &operation, 4).is_err());
    if let SessionManagement::Branch {
        boundary_turn_id, ..
    } = &mut operation
    {
        *boundary_turn_id = old.turns[0].turn_id.clone();
    }
    let mut catalog = read_catalog(&f.handle).unwrap();
    while catalog.entries.len() < MAX_ENTRIES {
        catalog.entries.push(SessionEntry {
            session_id: Uuid::new_v4().to_string(),
            name: "合成归档".into(),
            archived: true,
            created_at_ms: 1,
            updated_at_ms: 1,
            creation_request_id: None,
        });
    }
    write_catalog(&f.handle, &catalog).unwrap();
    assert_eq!(
        manage_session(&f.handle, &lease, &operation, 5)
            .unwrap_err()
            .code,
        "sessions.full"
    );
    assert!(!f.root.join(".novelgainker/conversation/sessions").exists());
}
fn fixture() -> (Fixture, ProjectWriteLease) {
    let f = Fixture::new();
    let lease = try_acquire_project_write_lease(&f.handle.coordination_key())
        .unwrap()
        .unwrap();
    (f, lease)
}
#[test]
fn nr602_readonly_missing_catalog_and_tasks_never_create_directories() {
    let (f, _lease) = fixture();
    let directory = f.root.join(".novelgainker/conversation");
    assert!(!directory.exists());
    assert!(list_existing_sessions(&f.handle, false, None).is_err());
    assert!(bind_existing_session(&f.handle, None).is_err());
    assert!(!directory.exists());
    let bound = bind_session(&f.handle, None, 1).unwrap();
    let tasks = f.root.join(".novelgainker/tasks");
    let task_existed = tasks.exists();
    crate::task_store::runtime_view_for(
        &bound,
        read_conversation(&bound).unwrap(),
        2,
        false,
        crate::protocol_profile::ToolProtocolProfile::Nr5,
    )
    .unwrap();
    assert_eq!(tasks.exists(), task_existed);
}
#[test]
fn nr602_branch_copies_only_terminal_visible_prefix_with_new_ids_and_idempotent_receipt() {
    let (f, lease) = fixture();
    complete(&f.handle, "第一轮", 1);
    complete(&f.handle, "第二轮", 3);
    let initial = read_conversation(&f.handle).unwrap();
    let page = list_sessions(&f.handle, false, None, 5).unwrap();
    let before = fs::read(f.root.join(".novelgainker/conversation/session.json")).unwrap();
    let canon = fs::read(f.root.join("project.md")).unwrap();
    let request = SessionManagement::Branch {
        conversation_session_id: initial.session_id.clone(),
        request_id: Uuid::new_v4().to_string(),
        boundary_turn_id: initial.turns[0].turn_id.clone(),
        name: "第一轮分支".into(),
        expected_revision: page.revision,
    };
    let made = manage_session(&f.handle, &lease, &request, 6).unwrap();
    assert!(made.changed);
    assert_eq!(made.source_session_id, initial.session_id);
    let copied = bind_session(&f.handle, Some(&made.target_session_id), 7).unwrap();
    let history = read_conversation(&copied).unwrap();
    assert_eq!(history.turns.len(), 1);
    assert_ne!(history.session_id, initial.session_id);
    assert_ne!(history.turns[0].turn_id, initial.turns[0].turn_id);
    for (new, old) in history.turns[0]
        .messages
        .iter()
        .zip(&initial.turns[0].messages)
    {
        assert_eq!(new.content, old.content);
        assert_ne!(new.message_id, old.message_id);
    }
    for category in ["memory", "tasks"] {
        assert!(!f
            .root
            .join(".novelgainker")
            .join(category)
            .join("sessions")
            .join(format!("{}.json", history.session_id))
            .exists());
    }
    let replay = manage_session(&f.handle, &lease, &request, 8).unwrap();
    assert!(!replay.changed);
    assert_eq!(replay.target_session_id, made.target_session_id);
    assert_eq!(replay.catalog.total_count, 2);
    assert_eq!(
        fs::read(f.root.join(".novelgainker/conversation/session.json")).unwrap(),
        before
    );
    assert_eq!(fs::read(f.root.join("project.md")).unwrap(), canon);
}
#[test]
fn nr602_clear_at_32_exchanges_one_active_slot_atomically_and_restore_full_rejects() {
    let (f, lease) = fixture();
    complete(&f.handle, "必须归档保留", 1);
    let mut page = list_sessions(&f.handle, false, None, 3).unwrap();
    let original = page.default_session_id.clone();
    for _ in 1..32 {
        page = create_session(
            &f.handle,
            &Uuid::new_v4().to_string(),
            "合成会话",
            page.revision,
            4,
        )
        .unwrap();
    }
    let bytes = fs::read(f.root.join(".novelgainker/conversation/session.json")).unwrap();
    let clear = SessionManagement::Clear {
        conversation_session_id: original.clone(),
        request_id: Uuid::new_v4().to_string(),
        name: "清空后".into(),
        expected_revision: page.revision,
    };
    let made = manage_session(&f.handle, &lease, &clear, 5).unwrap();
    assert_eq!(made.catalog.non_archived_count, 32);
    assert_eq!(made.catalog.total_count, 33);
    assert_eq!(
        list_sessions(&f.handle, true, None, 6).unwrap().items[0].session_id,
        original
    );
    assert!(
        read_conversation(&bind_session(&f.handle, Some(&made.target_session_id), 6).unwrap())
            .unwrap()
            .turns
            .is_empty()
    );
    let restore = SessionManagement::Restore {
        conversation_session_id: original.clone(),
        expected_revision: made.catalog.revision,
    };
    assert_eq!(
        manage_session(&f.handle, &lease, &restore, 7)
            .unwrap_err()
            .code,
        "sessions.full"
    );
    let freed = manage_session(
        &f.handle,
        &lease,
        &SessionManagement::Archive {
            conversation_session_id: made.target_session_id,
            expected_revision: made.catalog.revision,
        },
        8,
    )
    .unwrap();
    let restored = manage_session(
        &f.handle,
        &lease,
        &SessionManagement::Restore {
            conversation_session_id: original.clone(),
            expected_revision: freed.catalog.revision,
        },
        9,
    )
    .unwrap();
    assert_eq!(restored.catalog.non_archived_count, 32);
    assert_eq!(restored.catalog.active_session_id, Some(original));
    assert_eq!(
        fs::read(f.root.join(".novelgainker/conversation/session.json")).unwrap(),
        bytes
    );
}
#[test]
fn nr602_failed_catalog_commit_reuses_exact_prepared_branch_without_duplicate_or_history_loss() {
    let (f, lease) = fixture();
    complete(&f.handle, "故障源", 1);
    let old = read_conversation(&f.handle).unwrap();
    let page = list_sessions(&f.handle, false, None, 3).unwrap();
    let request = SessionManagement::Branch {
        conversation_session_id: old.session_id.clone(),
        request_id: Uuid::new_v4().to_string(),
        boundary_turn_id: old.turns[0].turn_id.clone(),
        name: "可恢复分支".into(),
        expected_revision: page.revision,
    };
    let before = fs::read(catalog_path(&f.handle).unwrap()).unwrap();
    FAIL_NEXT_CATALOG_PROMOTION.with(|flag| flag.set(true));
    assert_eq!(
        manage_session(&f.handle, &lease, &request, 4)
            .unwrap_err()
            .code,
        "sessions.unavailable"
    );
    assert_eq!(fs::read(catalog_path(&f.handle).unwrap()).unwrap(), before);
    let made = manage_session(&f.handle, &lease, &request, 5).unwrap();
    assert_eq!(made.catalog.total_count, 2);
    assert_eq!(
        fs::read_dir(f.root.join(".novelgainker/conversation/sessions"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        read_conversation(&bind_session(&f.handle, Some(&old.session_id), 6).unwrap()).unwrap(),
        old
    );
}
#[test]
fn nr602_active_run_foreign_lease_wrong_scope_and_unknown_action_reject_without_mutation() {
    let (f, lease) = fixture();
    let page = list_sessions(&f.handle, false, None, 1).unwrap();
    let bound = bind_session(&f.handle, None, 2).unwrap();
    let request = SessionManagement::Archive {
        conversation_session_id: page.default_session_id.clone(),
        expected_revision: page.revision,
    };
    let (foreign, foreign_lease) = fixture();
    assert_eq!(
        manage_session(&f.handle, &foreign_lease, &request, 3)
            .unwrap_err()
            .code,
        "project.writer_required"
    );
    let run = reserve_run(&bound, 3).unwrap();
    assert_eq!(
        manage_session(&f.handle, &lease, &request, 4)
            .unwrap_err()
            .code,
        "sessions.run_active"
    );
    release_run(&bound, &run.reservation_id).unwrap();
    let foreign_id = list_sessions(&foreign.handle, false, None, 1)
        .unwrap()
        .default_session_id;
    assert_eq!(
        manage_session(
            &f.handle,
            &lease,
            &SessionManagement::Archive {
                conversation_session_id: foreign_id,
                expected_revision: page.revision
            },
            5
        )
        .unwrap_err()
        .code,
        "sessions.not_found"
    );
    assert!(serde_json::from_value::<SessionManagement>(serde_json::json!({"action":"delete","conversationSessionId":page.default_session_id,"expectedRevision":1})).is_err());
    assert!(serde_json::from_value::<SessionManagement>(serde_json::json!({"action":"archive","conversationSessionId":page.default_session_id,"expectedRevision":1,"path":"../escape"})).is_err());
    assert_eq!(
        list_sessions(&f.handle, false, None, 6)
            .unwrap()
            .total_count,
        1
    );
}
