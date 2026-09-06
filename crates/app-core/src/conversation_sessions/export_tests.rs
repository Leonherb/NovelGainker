use super::*;
#[cfg(windows)]
#[test]
fn nr602_export_rejects_actual_reparse_parent_without_outside_writes() {
    let f = Fixture::new();
    let id = list_sessions(&f.handle, false, None, 1)
        .unwrap()
        .default_session_id;
    let output = ExportDirectory::new();
    let outside = ExportDirectory::new();
    let link = output.0.join("redirect");
    if std::os::windows::fs::symlink_dir(&outside.0, &link).is_err() {
        let result = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside.0)
            .output()
            .unwrap();
        assert!(result.status.success());
    }
    let prepared = prepare_export(&f.handle, &id, ConversationExportFormat::Json).unwrap();
    assert!(commit_export(prepared, &link.join("chat.json"), &[]).is_err());
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
    fs::remove_dir(&link).unwrap();
}

struct ExportDirectory(PathBuf);
impl ExportDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("novelgainker-nr602-export-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for ExportDirectory {
    fn drop(&mut self) {
        if self.0.parent() == Some(std::env::temp_dir().as_path())
            && self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("novelgainker-nr602-export-")
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
#[test]
fn nr602_export_visible_projection_excludes_internal_identity_and_preserves_user_text() {
    let f = Fixture::new();
    complete(&f.handle, "这是用户自愿输入的可见内容。", 1);
    let page = list_sessions(&f.handle, false, None, 3).unwrap();
    let id = page.active_session_id.unwrap();
    let output = ExportDirectory::new();
    let history_before = fs::read(f.root.join(".novelgainker/conversation/session.json")).unwrap();
    for format in [
        ConversationExportFormat::Json,
        ConversationExportFormat::Markdown,
    ] {
        let prepared = prepare_export(&f.handle, &id, format).unwrap();
        let target = output.0.join(format!("聊天.{}", format.extension()));
        let receipt = commit_export(prepared, &target, std::slice::from_ref(&f.handle)).unwrap();
        let bytes = fs::read(&target).unwrap();
        assert_eq!(receipt.bytes, bytes.len());
        assert_eq!(receipt.sha256, sha256_hex(&bytes));
        assert_eq!(
            receipt.file_name,
            target.file_name().unwrap().to_str().unwrap()
        );
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("这是用户自愿输入的可见内容。"));
        for absent in [
            "projectId",
            "sessionId",
            "turnId",
            "messageId",
            "errorCode",
            "taskActivities",
            "providerId",
            "arguments",
        ] {
            assert!(!text.contains(absent));
        }
        if matches!(format, ConversationExportFormat::Json) {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(json.as_object().unwrap().len(), 2);
            assert_eq!(json["turns"][0].as_object().unwrap().len(), 2);
            assert_eq!(
                json["turns"][0]["messages"][0].as_object().unwrap().len(),
                2
            );
        }
    }
    assert_eq!(
        history_before,
        fs::read(f.root.join(".novelgainker/conversation/session.json")).unwrap()
    );
}
#[test]
fn nr602_export_stale_running_wrong_scope_and_existing_destination_fail_closed() {
    let f = Fixture::new();
    complete(&f.handle, "首轮", 1);
    let id = list_sessions(&f.handle, false, None, 3)
        .unwrap()
        .active_session_id
        .unwrap();
    let output = ExportDirectory::new();
    let existing = output.0.join("existing.json");
    fs::write(&existing, b"keep-existing").unwrap();
    let export = || prepare_export(&f.handle, &id, ConversationExportFormat::Json).unwrap();
    assert!(commit_export(export(), &existing, &[]).is_err());
    assert_eq!(fs::read(&existing).unwrap(), b"keep-existing");
    for bad in [
        f.root.join("chapter.json"),
        output.0.join("missing").join("chat.json"),
        output.0.join("chat.md"),
        output.0.join("chat:stream.json"),
        PathBuf::from("relative.json"),
        PathBuf::from(r"\\localhost\never-connect\chat.json"),
    ] {
        assert!(commit_export(export(), &bad, std::slice::from_ref(&f.handle)).is_err());
    }
    let prepared = export();
    complete(&f.handle, "快照之后的新轮次", 4);
    let target = output.0.join("stale.json");
    assert!(commit_export(prepared, &target, &[]).is_err());
    assert!(!target.exists());
    assert_eq!(fs::read_dir(&output.0).unwrap().count(), 1);
    let foreign = Fixture::new();
    assert!(prepare_export(&foreign.handle, &id, ConversationExportFormat::Json).is_err());
    start_conversation_turn(
        &f.handle,
        ProviderId::Deepseek,
        "deepseek-v4-pro",
        "进行中",
        7,
    )
    .unwrap();
    assert!(prepare_export(&f.handle, &id, ConversationExportFormat::Json).is_err());
}
