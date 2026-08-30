use crate::agent_v2::{retained_source_bindings, SourceRefV2};
use crate::project::ProjectHandle;
use crate::workspace::{read_project_file, sha256_hex};
use crate::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

const MARKS_FILE: &str = ".novelgainker/agent/marks.json";
const MAX_ACTIVE_MARKS: usize = 64;
const MAX_MARK_STORE_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextMarkScopeV1 {
    Session,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextMarkPriorityV1 {
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextMarkCreatorV1 {
    User,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextMarkStatusV1 {
    Active,
    ReRead,
    Rebound,
    Stale,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextMarkV1 {
    pub schema_version: u16,
    pub mark_id: String,
    pub project_id: String,
    pub session_id: String,
    pub branch_id: String,
    pub scope: ContextMarkScopeV1,
    pub priority: ContextMarkPriorityV1,
    pub creator: ContextMarkCreatorV1,
    pub source_id: String,
    pub relative_path: String,
    pub field_path: Option<String>,
    pub heading_path: Vec<String>,
    pub anchor_hash: String,
    pub document_hash: String,
    pub status: ContextMarkStatusV1,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub binding_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextMarkStoreV1 {
    pub schema_version: u16,
    pub project_id: String,
    pub marks: Vec<ContextMarkV1>,
    pub store_hash: String,
}

pub struct UpsertContextMarkInputV1<'a> {
    pub session_id: &'a str,
    pub branch_id: &'a str,
    pub source: &'a SourceRefV2,
    pub scope: ContextMarkScopeV1,
    pub priority: ContextMarkPriorityV1,
    pub creator: ContextMarkCreatorV1,
    pub now_ms: u64,
}

pub fn load_context_marks(handle: &ProjectHandle) -> Result<ContextMarkStoreV1, AppError> {
    let path = handle.root().join(MARKS_FILE);
    if !path.exists() {
        return empty_store(handle);
    }
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| mark_error("agent-mark.store-invalid"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(mark_error("agent-mark.store-invalid"));
    }
    if metadata.len() > MAX_MARK_STORE_BYTES as u64 {
        return rebuild_context_marks(handle, &path);
    }
    let bytes = fs::read(&path).map_err(|_| mark_error("agent-mark.store-invalid"))?;
    let store = serde_json::from_slice::<ContextMarkStoreV1>(&bytes).ok();
    if let Some(store) = store.filter(|store| valid_store(handle, store)) {
        return Ok(store);
    }
    rebuild_context_marks(handle, &path)
}

fn valid_store(handle: &ProjectHandle, store: &ContextMarkStoreV1) -> bool {
    store.schema_version == 1
        && store.project_id == handle.project_id()
        && store.store_hash == hash_store(store).unwrap_or_default()
        && store.marks.len() <= MAX_ACTIVE_MARKS
        && store.marks.iter().all(|mark| {
            mark.project_id == handle.project_id()
                && mark.binding_hash == hash_mark(mark).unwrap_or_default()
        })
}

fn rebuild_context_marks(
    handle: &ProjectHandle,
    corrupted_path: &Path,
) -> Result<ContextMarkStoreV1, AppError> {
    let bindings = retained_source_bindings(handle, MAX_ACTIVE_MARKS)?;
    let quarantine =
        corrupted_path.with_file_name(format!("marks.invalid.{}.json", Uuid::new_v4().simple()));
    fs::rename(corrupted_path, &quarantine).map_err(|_| mark_error("agent-mark.store-invalid"))?;
    let mut store = empty_store(handle)?;
    for binding in bindings {
        let Ok(document) = read_project_file(handle, &binding.source.relative_path) else {
            continue;
        };
        if document.content_hash != binding.source.content_hash {
            continue;
        }
        let Some(anchor_hash) = anchored_hash(
            &document.content,
            binding.source.field_path.as_deref(),
            &binding.source.heading_path,
        ) else {
            continue;
        };
        let mut mark = ContextMarkV1 {
            schema_version: 1,
            mark_id: format!("mark_{}", Uuid::new_v4().simple()),
            project_id: handle.project_id().to_owned(),
            session_id: binding.session_id,
            branch_id: binding.branch_id,
            scope: ContextMarkScopeV1::Session,
            priority: ContextMarkPriorityV1::Medium,
            creator: ContextMarkCreatorV1::Agent,
            source_id: binding.source.source_id,
            relative_path: binding.source.relative_path,
            field_path: binding.source.field_path,
            heading_path: binding.source.heading_path,
            anchor_hash,
            document_hash: document.content_hash,
            status: ContextMarkStatusV1::Active,
            created_at_ms: 0,
            updated_at_ms: 0,
            binding_hash: String::new(),
        };
        mark.binding_hash = hash_mark(&mark)?;
        store.marks.push(mark);
    }
    write_store(handle, &mut store)?;
    Ok(store)
}

pub fn upsert_context_mark(
    handle: &ProjectHandle,
    input: UpsertContextMarkInputV1<'_>,
) -> Result<ContextMarkStoreV1, AppError> {
    if input.creator == ContextMarkCreatorV1::Agent
        && (input.scope != ContextMarkScopeV1::Session
            || input.priority != ContextMarkPriorityV1::Medium)
    {
        return Err(mark_error("agent-mark.agent-policy-blocked"));
    }
    if input.source.relative_path.starts_with(".novelgainker/")
        || !input.source.untrusted
        || input.source.content_hash.len() != 64
    {
        return Err(mark_error("agent-mark.source-invalid"));
    }
    let document = read_project_file(handle, &input.source.relative_path)?;
    if document.content_hash != input.source.content_hash {
        return Err(mark_error("agent-mark.source-changed"));
    }
    let anchor_hash = anchored_hash(
        &document.content,
        input.source.field_path.as_deref(),
        &input.source.heading_path,
    )
    .ok_or_else(|| mark_error("agent-mark.anchor-invalid"))?;
    let mut store = load_context_marks(handle)?;
    if let Some(existing) = store.marks.iter_mut().find(|mark| {
        mark.session_id == input.session_id
            && mark.relative_path == input.source.relative_path
            && mark.field_path == input.source.field_path
            && mark.heading_path == input.source.heading_path
            && mark.status != ContextMarkStatusV1::Archived
    }) {
        if input.creator == ContextMarkCreatorV1::Agent
            && existing.creator == ContextMarkCreatorV1::User
        {
            return Ok(store);
        }
        existing.scope = input.scope;
        existing.priority = input.priority;
        existing.document_hash = document.content_hash;
        existing.anchor_hash = anchor_hash;
        existing.status = ContextMarkStatusV1::Active;
        existing.updated_at_ms = input.now_ms;
        existing.binding_hash = hash_mark(existing)?;
    } else {
        if store
            .marks
            .iter()
            .filter(|mark| {
                !matches!(
                    mark.status,
                    ContextMarkStatusV1::Archived | ContextMarkStatusV1::Stale
                )
            })
            .count()
            >= MAX_ACTIVE_MARKS
        {
            return Err(mark_error("agent-mark.quota-exceeded"));
        }
        let mut mark = ContextMarkV1 {
            schema_version: 1,
            mark_id: format!("mark_{}", Uuid::new_v4().simple()),
            project_id: handle.project_id().to_owned(),
            session_id: input.session_id.to_owned(),
            branch_id: input.branch_id.to_owned(),
            scope: input.scope,
            priority: input.priority,
            creator: input.creator,
            source_id: input.source.source_id.clone(),
            relative_path: input.source.relative_path.clone(),
            field_path: input.source.field_path.clone(),
            heading_path: input.source.heading_path.clone(),
            anchor_hash,
            document_hash: document.content_hash,
            status: ContextMarkStatusV1::Active,
            created_at_ms: input.now_ms,
            updated_at_ms: input.now_ms,
            binding_hash: String::new(),
        };
        mark.binding_hash = hash_mark(&mark)?;
        store.marks.push(mark);
    }
    write_store(handle, &mut store)?;
    Ok(store)
}

pub fn remove_context_mark(
    handle: &ProjectHandle,
    mark_id: &str,
    binding_hash: &str,
    actor: ContextMarkCreatorV1,
) -> Result<ContextMarkStoreV1, AppError> {
    if actor == ContextMarkCreatorV1::Agent {
        return Err(mark_error("agent-mark.agent-delete-blocked"));
    }
    let mut store = load_context_marks(handle)?;
    let index = store
        .marks
        .iter()
        .position(|mark| mark.mark_id == mark_id && mark.binding_hash == binding_hash)
        .ok_or_else(|| mark_error("agent-mark.binding-invalid"))?;
    store.marks.remove(index);
    write_store(handle, &mut store)?;
    Ok(store)
}

pub fn reconcile_context_marks(
    handle: &ProjectHandle,
    now_ms: u64,
) -> Result<ContextMarkStoreV1, AppError> {
    let mut store = load_context_marks(handle)?;
    for mark in &mut store.marks {
        if mark.status == ContextMarkStatusV1::Archived {
            continue;
        }
        let Ok(document) = read_project_file(handle, &mark.relative_path) else {
            mark.status = ContextMarkStatusV1::Stale;
            mark.updated_at_ms = now_ms;
            mark.binding_hash = hash_mark(mark)?;
            continue;
        };
        mark.status = if document.content_hash == mark.document_hash {
            ContextMarkStatusV1::Active
        } else if anchored_hash(
            &document.content,
            mark.field_path.as_deref(),
            &mark.heading_path,
        )
        .is_some_and(|hash| hash == mark.anchor_hash)
        {
            mark.document_hash = document.content_hash;
            ContextMarkStatusV1::ReRead
        } else if all_anchor_hashes(&document.content)
            .iter()
            .filter(|hash| **hash == mark.anchor_hash)
            .count()
            == 1
        {
            mark.document_hash = document.content_hash;
            ContextMarkStatusV1::Rebound
        } else {
            ContextMarkStatusV1::Stale
        };
        mark.updated_at_ms = now_ms;
        mark.binding_hash = hash_mark(mark)?;
    }
    write_store(handle, &mut store)?;
    Ok(store)
}

pub fn archive_session_marks(
    handle: &ProjectHandle,
    session_id: &str,
    now_ms: u64,
) -> Result<(), AppError> {
    let mut store = load_context_marks(handle)?;
    for mark in &mut store.marks {
        if mark.session_id == session_id && mark.scope == ContextMarkScopeV1::Session {
            mark.status = ContextMarkStatusV1::Archived;
            mark.updated_at_ms = now_ms;
            mark.binding_hash = hash_mark(mark)?;
        }
    }
    write_store(handle, &mut store)
}

fn empty_store(handle: &ProjectHandle) -> Result<ContextMarkStoreV1, AppError> {
    let mut store = ContextMarkStoreV1 {
        schema_version: 1,
        project_id: handle.project_id().to_owned(),
        marks: Vec::new(),
        store_hash: String::new(),
    };
    store.store_hash = hash_store(&store)?;
    Ok(store)
}

fn write_store(handle: &ProjectHandle, store: &mut ContextMarkStoreV1) -> Result<(), AppError> {
    store
        .marks
        .sort_by(|left, right| left.mark_id.cmp(&right.mark_id));
    store.store_hash = hash_store(store)?;
    let bytes =
        serde_json::to_vec_pretty(store).map_err(|_| mark_error("agent-mark.store-invalid"))?;
    if bytes.len() > MAX_MARK_STORE_BYTES {
        return Err(mark_error("agent-mark.quota-exceeded"));
    }
    let path = handle.root().join(MARKS_FILE);
    let directory = path
        .parent()
        .ok_or_else(|| mark_error("agent-mark.store-invalid"))?;
    fs::create_dir_all(directory).map_err(|_| mark_error("agent-mark.store-invalid"))?;
    if fs::symlink_metadata(directory)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(true)
    {
        return Err(mark_error("agent-mark.store-invalid"));
    }
    let temporary = directory.join(format!(".marks.{}.tmp", Uuid::new_v4().simple()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| mark_error("agent-mark.store-invalid"))?;
    let result = file
        .write_all(&bytes)
        .and_then(|_| file.sync_all())
        .and_then(|_| atomic_replace_mark_store(&temporary, &path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(mark_error("agent-mark.store-invalid"));
    }
    Ok(())
}

#[cfg(windows)]
fn atomic_replace_mark_store(temporary: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace_mark_store(temporary: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(temporary, target)
}

fn anchored_hash(
    content: &str,
    field_path: Option<&str>,
    heading_path: &[String],
) -> Option<String> {
    if let Some(field) = field_path {
        let value = frontmatter_field(content, field)?;
        return Some(sha256_hex(value.as_bytes()));
    }
    if let Some(heading) = heading_path.last() {
        return heading_sections(content)
            .into_iter()
            .find(|(candidate, _)| candidate == heading)
            .map(|(_, body)| sha256_hex(body.trim_end().as_bytes()));
    }
    Some(sha256_hex(content.as_bytes()))
}

fn all_anchor_hashes(content: &str) -> Vec<String> {
    heading_sections(content)
        .into_iter()
        .map(|(_, body)| sha256_hex(body.trim_end().as_bytes()))
        .collect()
}

fn heading_sections(content: &str) -> Vec<(String, String)> {
    let lines = content.lines().collect::<Vec<_>>();
    let mut sections = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let level = line
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if level == 0 || !line[level..].starts_with(' ') {
            continue;
        }
        let heading = line[level + 1..].trim().to_owned();
        let end = lines[index + 1..]
            .iter()
            .position(|candidate| {
                let next_level = candidate
                    .chars()
                    .take_while(|character| *character == '#')
                    .count();
                next_level > 0
                    && next_level <= level
                    && candidate
                        .get(next_level..)
                        .is_some_and(|tail| tail.starts_with(' '))
            })
            .map_or(lines.len(), |offset| index + 1 + offset);
        sections.push((heading, lines[index..end].join("\n")));
    }
    sections
}

fn frontmatter_field<'a>(content: &'a str, field: &str) -> Option<&'a str> {
    let mut lines = content.lines();
    if lines.next()? != "---" {
        return None;
    }
    for line in lines {
        if line == "---" {
            return None;
        }
        if let Some(value) = line.strip_prefix(&format!("{field}:")) {
            return Some(value.trim());
        }
    }
    None
}

fn hash_mark(mark: &ContextMarkV1) -> Result<String, AppError> {
    let mut canonical = mark.clone();
    canonical.binding_hash.clear();
    serde_json::to_vec(&canonical)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|_| mark_error("agent-mark.hash-invalid"))
}

fn hash_store(store: &ContextMarkStoreV1) -> Result<String, AppError> {
    let mut canonical = store.clone();
    canonical.store_hash.clear();
    serde_json::to_vec(&canonical)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|_| mark_error("agent-mark.hash-invalid"))
}

fn mark_error(code: &str) -> AppError {
    AppError::agent_contract(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_v2::{append_completed_turn, load_agent_session, CompletedTurnInputV2};
    use crate::project::{create_project, inspect_project_root, ProjectScanMode};
    use std::path::PathBuf;

    struct TestProject {
        root: PathBuf,
        handle: ProjectHandle,
    }

    impl TestProject {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("novelgainker-agent-marks-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
            let handle = create_project(&inspection).unwrap();
            Self { root, handle }
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            if self.root.parent() == Some(std::env::temp_dir().as_path()) {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    fn source(project: &TestProject, body: &str) -> SourceRefV2 {
        let content = format!(
            "---\ntitle: 林舟\nwritingLanguage: zh-CN\n---\n\n# 林舟\n\n## 身份\n\n{body}\n"
        );
        fs::write(project.root.join("characters/林舟.md"), &content).unwrap();
        SourceRefV2 {
            source_id: "src_12345678".to_owned(),
            relative_path: "characters/林舟.md".to_owned(),
            entity: Some("林舟".to_owned()),
            field_path: None,
            heading_path: vec!["林舟".to_owned(), "身份".to_owned()],
            content_hash: sha256_hex(content.as_bytes()),
            excerpt_hash: sha256_hex(body.as_bytes()),
            excerpt: body.to_owned(),
            truncated: false,
            untrusted: true,
        }
    }

    #[test]
    fn marks_store_only_location_and_hashes_not_canon_text() {
        let project = TestProject::new();
        let source = source(&project, "守灯人秘密");
        let store = upsert_context_mark(
            &project.handle,
            UpsertContextMarkInputV1 {
                session_id: "ses_12345678",
                branch_id: "brn_12345678",
                source: &source,
                scope: ContextMarkScopeV1::Session,
                priority: ContextMarkPriorityV1::Medium,
                creator: ContextMarkCreatorV1::Agent,
                now_ms: 1,
            },
        )
        .unwrap();
        assert_eq!(store.marks.len(), 1);
        let persisted = fs::read_to_string(project.root.join(MARKS_FILE)).unwrap();
        assert!(!persisted.contains("守灯人秘密"));
        assert!(persisted.contains("characters/林舟.md"));
    }

    #[test]
    fn agent_cannot_create_high_project_mark_or_delete_user_mark() {
        let project = TestProject::new();
        let source = source(&project, "守灯人");
        let blocked = upsert_context_mark(
            &project.handle,
            UpsertContextMarkInputV1 {
                session_id: "ses_12345678",
                branch_id: "brn_12345678",
                source: &source,
                scope: ContextMarkScopeV1::Project,
                priority: ContextMarkPriorityV1::High,
                creator: ContextMarkCreatorV1::Agent,
                now_ms: 1,
            },
        );
        assert_eq!(blocked.unwrap_err().code, "agent-mark.agent-policy-blocked");
    }

    #[test]
    fn unrelated_change_rereads_while_target_change_goes_stale() {
        let project = TestProject::new();
        let source = source(&project, "守灯人");
        upsert_context_mark(
            &project.handle,
            UpsertContextMarkInputV1 {
                session_id: "ses_12345678",
                branch_id: "brn_12345678",
                source: &source,
                scope: ContextMarkScopeV1::Session,
                priority: ContextMarkPriorityV1::Medium,
                creator: ContextMarkCreatorV1::User,
                now_ms: 1,
            },
        )
        .unwrap();
        let path = project.root.join("characters/林舟.md");
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("{original}\n## 备注\n\n不影响身份。\n")).unwrap();
        let reread = reconcile_context_marks(&project.handle, 2).unwrap();
        assert_eq!(reread.marks[0].status, ContextMarkStatusV1::ReRead);
        let changed = fs::read_to_string(&path)
            .unwrap()
            .replace("守灯人", "巡夜人");
        fs::write(&path, changed).unwrap();
        let stale = reconcile_context_marks(&project.handle, 3).unwrap();
        assert_eq!(stale.marks[0].status, ContextMarkStatusV1::Stale);
    }

    #[test]
    fn corrupt_store_is_quarantined_and_rebuilt_from_history_and_current_canon() {
        let project = TestProject::new();
        let source = source(&project, "守灯人");
        let session = load_agent_session(&project.handle, true, 1).unwrap();
        append_completed_turn(
            &project.handle,
            CompletedTurnInputV2 {
                turn_id: "trn_12345678",
                user_message_id: None,
                user_content: "林舟是谁？",
                assistant_content: "林舟是守灯人。",
                selected_skill_ids: vec!["create-character".to_owned()],
                source_refs: vec![source.clone()],
                terminal_state: "completed",
                now_ms: 2,
            },
        )
        .unwrap();
        upsert_context_mark(
            &project.handle,
            UpsertContextMarkInputV1 {
                session_id: session.session_id.as_deref().unwrap(),
                branch_id: session.active_branch_id.as_deref().unwrap(),
                source: &source,
                scope: ContextMarkScopeV1::Project,
                priority: ContextMarkPriorityV1::High,
                creator: ContextMarkCreatorV1::User,
                now_ms: 3,
            },
        )
        .unwrap();
        let canon_before = fs::read(project.root.join("characters/林舟.md")).unwrap();
        fs::write(project.root.join(MARKS_FILE), b"{corrupted").unwrap();

        let rebuilt = load_context_marks(&project.handle).unwrap();
        assert_eq!(rebuilt.marks.len(), 1);
        assert_eq!(rebuilt.marks[0].source_id, source.source_id);
        assert_eq!(rebuilt.marks[0].scope, ContextMarkScopeV1::Session);
        assert_eq!(rebuilt.marks[0].priority, ContextMarkPriorityV1::Medium);
        assert_eq!(rebuilt.marks[0].creator, ContextMarkCreatorV1::Agent);
        assert_eq!(
            fs::read(project.root.join("characters/林舟.md")).unwrap(),
            canon_before
        );
        assert!(fs::read_dir(project.root.join(".novelgainker/agent"))
            .unwrap()
            .flatten()
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("marks.invalid.")));
    }
}
