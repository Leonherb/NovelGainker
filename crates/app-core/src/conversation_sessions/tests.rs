use super::*;
#[path = "export_tests.rs"]
mod export_tests;
#[path = "management_tests.rs"]
mod management_tests;
use crate::conversation::{
    finish_conversation_turn, read_conversation, start_conversation_turn, ConversationTerminalKind,
};
use crate::credential_vault::ProviderId;
use crate::project::{inspect_project_root, open_project, ProjectScanMode};

struct Fixture {
    root: PathBuf,
    handle: ProjectHandle,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("novelgainker-nr601-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("project.md"), "# 合成小说\n").unwrap();
        let handle = open_project(
            &inspect_project_root(&root, ProjectScanMode::Open).unwrap(),
            true,
        )
        .unwrap();
        Self { root, handle }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.root.parent() == Some(std::env::temp_dir().as_path())
            && self
                .root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("novelgainker-nr601-")
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
fn complete(handle: &ProjectHandle, text: &str, now: u64) {
    let turn = start_conversation_turn(handle, ProviderId::Deepseek, "deepseek-v4-pro", text, now)
        .unwrap();
    finish_conversation_turn(
        handle,
        &turn.turn_id,
        &format!("回复：{text}"),
        ConversationTerminalKind::Completed,
        None,
        now + 1,
    )
    .unwrap();
}
#[test]
fn nr601_entry_budget_reserves_target_and_temporary_before_writes() {
    let f = Fixture::new();
    let bound = bind_session(&f.handle, None, 1).unwrap();
    let target = store_path(&bound, StoreKind::Task, true).unwrap();
    let directory = target.parent().unwrap();
    for index in 0..1023 {
        fs::write(directory.join(format!("orphan-{index}.tmp")), []).unwrap();
    }
    assert_eq!(reserve_run(&bound, 2).unwrap_err().code, "sessions.full");
    assert!(!target.exists());
    assert!(read_conversation(&bound).is_ok());
    assert_eq!(fs::read_dir(directory).unwrap().count(), 1023);
}

#[cfg(windows)]
#[test]
fn nr601_actual_session_namespace_reparse_is_rejected_without_target_writes() {
    let f = Fixture::new();
    let page = list_sessions(&f.handle, false, None, 1).unwrap();
    let outside = f.root.join("synthetic-link-target");
    fs::create_dir(&outside).unwrap();
    let link = f
        .root
        .join(".novelgainker")
        .join("conversation")
        .join("sessions");
    if std::os::windows::fs::symlink_dir(&outside, &link).is_err() {
        let result = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "synthetic junction creation must succeed"
        );
    }
    assert_eq!(
        create_session(
            &f.handle,
            &Uuid::new_v4().to_string(),
            "不跟随链接",
            page.revision,
            2
        )
        .unwrap_err()
        .code,
        "sessions.boundary"
    );
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    assert_eq!(
        list_sessions(&f.handle, false, None, 3)
            .unwrap()
            .total_count,
        1
    );
    fs::remove_dir(&link).unwrap();
}

#[test]
fn nr601_catalog_fault_keeps_old_identity_and_history_with_bounded_orphan() {
    let f = Fixture::new();
    complete(&f.handle, "不能丢失的旧聊天", 1);
    let page = list_sessions(&f.handle, false, None, 3).unwrap();
    let catalog_before = fs::read(catalog_path(&f.handle).unwrap()).unwrap();
    let history = f.root.join(".novelgainker/conversation/session.json");
    let history_before = fs::read(&history).unwrap();
    let request = Uuid::new_v4().to_string();
    FAIL_NEXT_CATALOG_PROMOTION.with(|flag| flag.set(true));
    assert_eq!(
        create_session(&f.handle, &request, "故障后重试", page.revision, 4)
            .unwrap_err()
            .code,
        "sessions.unavailable"
    );
    assert_eq!(
        fs::read(catalog_path(&f.handle).unwrap()).unwrap(),
        catalog_before
    );
    assert_eq!(fs::read(&history).unwrap(), history_before);
    let orphan_directory = f.root.join(".novelgainker/conversation/sessions");
    assert_eq!(fs::read_dir(&orphan_directory).unwrap().count(), 1);
    let retried = create_session(&f.handle, &request, "故障后重试", page.revision, 5).unwrap();
    assert_eq!(retried.total_count, 2);
    assert_eq!(fs::read_dir(&orphan_directory).unwrap().count(), 2);
    assert_eq!(
        create_session(&f.handle, &request, "故障后重试", page.revision, 6)
            .unwrap()
            .total_count,
        2
    );
    assert_eq!(fs::read(&history).unwrap(), history_before);
}

#[test]
fn nr601_256_catalog_entries_page_snapshots_and_archived_storage_stay_bounded() {
    let f = Fixture::new();
    list_sessions(&f.handle, false, None, 1).unwrap();
    let mut catalog = read_catalog(&f.handle).unwrap();
    for index in 1..MAX_ENTRIES {
        catalog.entries.push(SessionEntry {
            session_id: Uuid::new_v4().to_string(),
            name: format!("合成归档{index}"),
            archived: true,
            created_at_ms: 1,
            updated_at_ms: 1,
            creation_request_id: None,
        });
    }
    write_catalog(&f.handle, &catalog).unwrap();
    let first = list_sessions(&f.handle, true, None, 2).unwrap();
    assert_eq!(first.items.len(), 32);
    let cursor = first.next_cursor.clone().unwrap();
    let mut next = Some(cursor.clone());
    let mut total = first.items.len();
    while let Some(value) = next {
        let page = list_sessions(&f.handle, true, Some(&value), 3).unwrap();
        total += page.items.len();
        next = page.next_cursor;
    }
    assert_eq!(total, 255);
    assert_eq!(
        create_session(
            &f.handle,
            &Uuid::new_v4().to_string(),
            "超出总目录",
            catalog.revision,
            4
        )
        .unwrap_err()
        .code,
        "sessions.full"
    );
    assert!(list_sessions(&f.handle, false, Some(&cursor), 4).is_err());
    let another = Fixture::new();
    assert!(list_sessions(&another.handle, true, Some(&cursor), 4).is_err());
    rename_session(
        &f.handle,
        &catalog.default_session_id,
        "新名称",
        catalog.revision,
        5,
    )
    .unwrap();
    assert!(list_sessions(&f.handle, true, Some(&cursor), 6).is_err());
}

#[test]
fn nr601_concurrent_creation_cas_has_one_winner_without_duplicate_visible_sessions() {
    let f = Fixture::new();
    let page = list_sessions(&f.handle, false, None, 1).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(5));
    let mut threads = Vec::new();
    for _ in 0..4 {
        let handle = f.handle.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            create_session(
                &handle,
                &Uuid::new_v4().to_string(),
                "并发合成",
                page.revision,
                2,
            )
        }));
    }
    barrier.wait();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(results
        .iter()
        .filter_map(|result| result.as_ref().err())
        .all(|error| error.code == "sessions.revision_conflict"));
    assert_eq!(
        list_sessions(&f.handle, false, None, 3)
            .unwrap()
            .total_count,
        2
    );
}

#[test]
fn nr601_all_three_aggregate_categories_and_unknown_subdirectories_fail_closed() {
    for kind in [StoreKind::Conversation, StoreKind::Task, StoreKind::Memory] {
        let f = Fixture::new();
        let bound = bind_session(&f.handle, None, 1).unwrap();
        let target = store_path(&bound, kind, true)
            .unwrap()
            .with_file_name("orphan.tmp");
        let file = fs::File::create(target).unwrap();
        file.set_len(kind.total_limit()).unwrap();
        assert_eq!(reserve_run(&bound, 2).unwrap_err().code, "sessions.full");
        assert!(list_sessions(&f.handle, false, None, 3)
            .unwrap()
            .active_run
            .is_none());
        assert!(read_conversation(&bound).is_ok());
        drop(file);
    }
    let f = Fixture::new();
    let bound = bind_session(&f.handle, None, 1).unwrap();
    let directory = f.root.join(".novelgainker/conversation/unexpected");
    fs::create_dir(&directory).unwrap();
    assert_eq!(
        reserve_run(&bound, 2).unwrap_err().code,
        "sessions.boundary"
    );
    assert!(read_conversation(&bound).is_ok());
}
#[test]
fn nr601_default_identity_history_memory_and_tasks_survive_catalog_registration() {
    use crate::memory::*;
    let f = Fixture::new();
    for index in 0..8 {
        complete(
            &f.handle,
            &format!("旧会话决定{index}{}", "甲".repeat(40)),
            10 + index * 10,
        );
    }
    let current = start_conversation_turn(
        &f.handle,
        ProviderId::Deepseek,
        "deepseek-v4-pro",
        "继续",
        200,
    )
    .unwrap();
    let session = read_conversation(&f.handle).unwrap();
    let profile = ContextProfile {
        trigger_bytes: 900,
        protected_tail_bytes: 240,
        segment_content_bytes: 128,
        compactor_input_bytes: 8192,
        backfill_input_bytes: 256,
        summary_bytes: 1024,
        compactor_requested_max: 128,
        main_reply_requested_max: 512,
        cumulative_requested_output_max: 640,
    };
    let plan = plan_compaction(&session, None, profile).unwrap().unwrap();
    let summary = MEMORY_HEADINGS
        .iter()
        .map(|heading| format!("{heading}\n- 保留旧决定"))
        .collect::<Vec<_>>()
        .join("\n");
    let memory = commit_compaction(&f.handle, &session, &plan, &summary, 300, profile).unwrap();
    let definition = crate::canon_context::novelist_read_context_definition();
    let call = crate::tool_runtime::ProviderToolCall {
        call_id: "old_call".into(),
        name: definition.name.clone(),
        arguments_json: "{}".into(),
    };
    crate::task_store::create_queued_task(
        &f.handle,
        &session.session_id,
        &current.turn_id,
        &call,
        &definition,
        301,
    )
    .unwrap();
    let paths = [
        ".novelgainker/conversation/session.json",
        ".novelgainker/memory/state.json",
        ".novelgainker/tasks/session.json",
        "project.md",
    ];
    let before: Vec<_> = paths
        .iter()
        .map(|p| fs::read(f.root.join(p)).unwrap())
        .collect();
    let page = list_sessions(&f.handle, false, None, 400).unwrap();
    assert_eq!(page.default_session_id, session.session_id);
    let old = bind_session(&f.handle, None, 401).unwrap();
    assert_eq!(read_conversation(&old).unwrap(), session);
    assert_eq!(
        load_valid_memory(&old, &session, profile),
        Some(memory.clone())
    );
    let next = create_session(
        &f.handle,
        &Uuid::new_v4().to_string(),
        "独立会话",
        page.revision,
        402,
    )
    .unwrap();
    let new = bind_session(&f.handle, next.active_session_id.as_deref(), 403).unwrap();
    assert!(read_conversation(&new).unwrap().turns.is_empty());
    assert!(load_valid_memory(&new, &read_conversation(&new).unwrap(), profile).is_none());
    assert!(crate::task_store::find_turn_tasks(&new, &current.turn_id)
        .unwrap()
        .is_empty());
    assert_eq!(
        crate::task_store::find_turn_tasks(&old, &current.turn_id)
            .unwrap()
            .len(),
        1
    );
    complete(&new, "新会话私有决定", 404);
    assert_eq!(load_valid_memory(&old, &session, profile), Some(memory));
    for (path, bytes) in paths.iter().zip(before) {
        assert_eq!(fs::read(f.root.join(path)).unwrap(), bytes);
    }
    assert!(!read_conversation(&new).unwrap().turns[0].messages[0]
        .content
        .contains("旧会话"));
}
#[test]
fn nr601_create_cas_idempotency_32_limit_and_corrupt_catalog_fail_closed() {
    let f = Fixture::new();
    let mut page = list_sessions(&f.handle, false, None, 1).unwrap();
    let request = Uuid::new_v4().to_string();
    let first = create_session(&f.handle, &request, "讨论", page.revision, 2).unwrap();
    assert_eq!(
        create_session(&f.handle, &request, "讨论", page.revision, 2)
            .unwrap()
            .revision,
        first.revision
    );
    assert!(create_session(&f.handle, &request, "不同输入", first.revision, 3).is_err());
    assert!(create_session(
        &f.handle,
        &Uuid::new_v4().to_string(),
        "过期",
        page.revision,
        3
    )
    .is_err());
    page = first;
    for index in 2..32 {
        page = create_session(
            &f.handle,
            &Uuid::new_v4().to_string(),
            &format!("会话{index}"),
            page.revision,
            index + 3,
        )
        .unwrap();
    }
    assert_eq!(page.non_archived_count, 32);
    let before = fs::read(catalog_path(&f.handle).unwrap()).unwrap();
    assert!(create_session(
        &f.handle,
        &Uuid::new_v4().to_string(),
        "超额",
        page.revision,
        100
    )
    .is_err());
    assert_eq!(fs::read(catalog_path(&f.handle).unwrap()).unwrap(), before);
    let mut corrupt: serde_json::Value = serde_json::from_slice(&before).unwrap();
    corrupt.as_object_mut().unwrap().remove("run");
    fs::write(
        catalog_path(&f.handle).unwrap(),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    assert!(list_sessions(&f.handle, false, None, 101).is_err());
    assert!(bind_session(&f.handle, None, 102).is_err());
}
#[test]
fn nr601_names_scope_missing_file_and_session_identity_are_strict() {
    let f = Fixture::new();
    let other = Fixture::new();
    let page = list_sessions(&f.handle, false, None, 1).unwrap();
    let other_page = list_sessions(&other.handle, false, None, 1).unwrap();
    for name in ["", " ", "a\nb", "a\0b"] {
        assert!(create_session(
            &f.handle,
            &Uuid::new_v4().to_string(),
            name,
            page.revision,
            2
        )
        .is_err());
    }
    assert!(create_session(&f.handle, "../bad", "名字", page.revision, 2).is_err());
    assert!(bind_session(&f.handle, Some(&other_page.default_session_id), 2).is_err());
    let next = create_session(
        &f.handle,
        &Uuid::new_v4().to_string(),
        "新会话",
        page.revision,
        3,
    )
    .unwrap();
    let bound = bind_session(&f.handle, next.active_session_id.as_deref(), 4).unwrap();
    let path = store_path(&bound, StoreKind::Conversation, false).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["sessionId"] = serde_json::json!(page.default_session_id);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(read_conversation(&bound).is_err());
    fs::remove_file(&path).unwrap();
    assert!(read_conversation(&bound).is_err());
}
#[test]
fn nr601_run_reservation_survives_view_switch_and_is_released_by_exact_identity() {
    let f = Fixture::new();
    let page = list_sessions(&f.handle, false, None, 1).unwrap();
    let original = bind_session(&f.handle, None, 2).unwrap();
    let run = reserve_run(&original, 3).unwrap();
    assert!(reserve_run(&original, 3).is_err());
    let page = list_sessions(&f.handle, false, None, 4).unwrap_or(page);
    let next = create_session(
        &f.handle,
        &Uuid::new_v4().to_string(),
        "生成中切换",
        page.revision,
        5,
    )
    .unwrap();
    let other = bind_session(&f.handle, next.active_session_id.as_deref(), 6).unwrap();
    assert_ne!(
        other.conversation_session_id(),
        original.conversation_session_id()
    );
    let turn = start_conversation_turn(
        &original,
        ProviderId::Deepseek,
        "deepseek-v4-pro",
        "原会话继续",
        7,
    )
    .unwrap();
    bind_run_turn(&original, &run.reservation_id, &turn.turn_id).unwrap();
    assert!(release_run(&other, &Uuid::new_v4().to_string()).is_err());
    assert!(reserve_run(&other, 8).is_err());
    finish_conversation_turn(
        &original,
        &turn.turn_id,
        "原会话完成",
        ConversationTerminalKind::Completed,
        None,
        9,
    )
    .unwrap();
    assert!(read_conversation(&other).unwrap().turns.is_empty());
    assert_eq!(
        list_sessions(&f.handle, false, None, 10)
            .unwrap()
            .active_run
            .unwrap()
            .turn_id,
        Some(turn.turn_id)
    );
    release_run(&original, &run.reservation_id).unwrap();
    assert!(list_sessions(&f.handle, false, None, 11)
        .unwrap()
        .active_run
        .is_none());
}
#[test]
fn nr601_aggregate_quota_counts_orphans_and_keeps_reserved_reply_capacity() {
    let f = Fixture::new();
    let page = list_sessions(&f.handle, false, None, 1).unwrap();
    let bound = bind_session(&f.handle, None, 2).unwrap();
    let root = f.root.join(".novelgainker/conversation");
    let orphan = root.join(".session-orphan.tmp");
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&orphan)
        .unwrap();
    file.set_len(512 * 1024 * 1024).unwrap();
    assert!(reserve_run(&bound, 3).is_err());
    // Existing safe reads remain available even when a new reservation cannot fit.
    assert!(read_conversation(&bound).is_ok());
    let before = fs::read(catalog_path(&f.handle).unwrap()).unwrap();
    assert!(create_session(
        &f.handle,
        &Uuid::new_v4().to_string(),
        "容量不足",
        page.revision,
        4
    )
    .is_err());
    assert_eq!(fs::read(catalog_path(&f.handle).unwrap()).unwrap(), before);
    // A real reservation leaves only 128 bytes unreserved: a new empty
    // session cannot fit, while the reserved conversation can still finish.
    file.set_len(512 * 1024 * 1024 - 16 * 1024 * 1024 - 128)
        .unwrap();
    let run = reserve_run(&bound, 5).unwrap();
    let current = list_sessions(&f.handle, false, None, 6).unwrap();
    let failure = create_session(
        &f.handle,
        &Uuid::new_v4().to_string(),
        "不能挤占回复",
        current.revision,
        7,
    )
    .unwrap_err();
    assert_eq!(failure.code, "sessions.full");
    complete(&bound, "仍然保存预留会话的回复", 8);
    release_run(&bound, &run.reservation_id).unwrap();
    assert!(read_conversation(&bound).unwrap().turns[0].messages[1]
        .content
        .contains("仍然保存"));
    drop(file);
}
