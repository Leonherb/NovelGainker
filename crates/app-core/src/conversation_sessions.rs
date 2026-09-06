//! Bounded NR-601 local conversation catalog. Canon remains project-wide.
use crate::conversation::{ensure_conversation_session, ConversationSession};
use crate::project::ProjectHandle;
use crate::workspace::{atomic_replace, is_link_or_reparse, read_bounded, sha256_hex};
use crate::AppError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use uuid::Uuid;

mod export;
mod management;
mod quota;
pub use export::{
    commit_export, prepare_export, ConversationExportFormat, ConversationExportReceipt,
    PreparedConversationExport,
};
pub use management::{manage_session, SessionManagement, SessionManagementResult};
#[cfg(test)]
mod tests;
pub(crate) use quota::with_store_write;
pub use quota::{bind_run_turn, release_run, reserve_run, RunReservation};

const MAX_ENTRIES: usize = 256;
const MAX_ACTIVE: usize = 32;
const MAX_CATALOG_BYTES: u64 = 1024 * 1024;
const PAGE_SIZE: usize = 32;
const MAX_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone)]
pub(crate) struct SessionBinding {
    pub id: String,
    pub legacy: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StoreKind {
    Conversation,
    Memory,
    Task,
}
impl StoreKind {
    fn directory(self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::Memory => "memory",
            Self::Task => "tasks",
        }
    }
    fn legacy_file(self) -> &'static str {
        if matches!(self, Self::Memory) {
            "state.json"
        } else {
            "session.json"
        }
    }
    fn single_limit(self) -> u64 {
        match self {
            Self::Conversation => 16 * 1024 * 1024,
            Self::Memory => 32 * 1024,
            Self::Task => 4 * 1024 * 1024,
        }
    }
    fn total_limit(self) -> u64 {
        match self {
            Self::Conversation => 512 * 1024 * 1024,
            Self::Memory => 8 * 1024 * 1024,
            Self::Task => 128 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionEntry {
    pub session_id: String,
    pub name: String,
    pub archived: bool,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub creation_request_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Catalog {
    schema_version: u16,
    project_id: String,
    revision: u64,
    default_session_id: String,
    #[serde(deserialize_with = "required_nullable")]
    active_session_id: Option<String>,
    entries: Vec<SessionEntry>,
    #[serde(deserialize_with = "required_nullable")]
    run: Option<RunReservation>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPage {
    pub can_mutate: bool,
    pub schema_version: u16,
    pub revision: u64,
    pub active_session_id: Option<String>,
    pub default_session_id: String,
    pub items: Vec<SessionEntry>,
    pub non_archived_count: usize,
    pub total_count: usize,
    pub next_cursor: Option<String>,
    pub active_run: Option<RunReservation>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cursor {
    project_id: String,
    archived: bool,
    snapshot: String,
    offset: usize,
}

fn error(code: &'static str) -> AppError {
    AppError {
        code: code.into(),
        message_key: "errors.runtimeUnavailable".into(),
        diagnostic: None,
    }
}
fn required_nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}
fn valid_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|value| value.to_string() == id)
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.len() <= 180
        && name.chars().count() <= 60
        && !name.chars().any(char::is_control)
}
fn coordination(handle: &ProjectHandle) -> Result<Arc<Mutex<()>>, AppError> {
    static LOCKS: OnceLock<Mutex<BTreeMap<String, Weak<Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(Mutex::default)
        .lock()
        .map_err(|_| error("sessions.unavailable"))?;
    locks.retain(|_, value| value.strong_count() != 0);
    let key = handle.coordination_key();
    if let Some(value) = locks.get(&key).and_then(Weak::upgrade) {
        return Ok(value);
    }
    let value = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&value));
    Ok(value)
}
pub(crate) fn with_store_read<T>(
    handle: &ProjectHandle,
    operation: impl FnOnce() -> Result<T, AppError>,
) -> Result<T, AppError> {
    if handle.session_binding().is_none() {
        return operation();
    }
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    operation()
}
fn plain_directory(path: &Path, create: bool) -> Result<(), AppError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !is_link_or_reparse(&meta) => Ok(()),
        Err(e) if !create && e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) if create && e.kind() == std::io::ErrorKind::NotFound => {
            if let Err(failure) = fs::create_dir(path) {
                if failure.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error("sessions.unavailable"));
                }
            }
            plain_directory(path, false)
        }
        _ => Err(error("sessions.boundary")),
    }
}
pub(crate) fn store_path(
    handle: &ProjectHandle,
    kind: StoreKind,
    create: bool,
) -> Result<PathBuf, AppError> {
    let state = handle.root().join(".novelgainker");
    plain_directory(&state, create)?;
    let directory = state.join(kind.directory());
    plain_directory(&directory, create)?;
    let path = if let Some(binding) = handle.session_binding().filter(|value| !value.legacy) {
        if !valid_id(&binding.id) {
            return Err(error("sessions.invalid"));
        }
        let sessions = directory.join("sessions");
        plain_directory(&sessions, create)?;
        sessions.join(format!("{}.json", binding.id))
    } else {
        directory.join(kind.legacy_file())
    };
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if !meta.is_file() || is_link_or_reparse(&meta) {
            return Err(error("sessions.boundary"));
        }
    }
    Ok(path)
}
fn catalog_path(handle: &ProjectHandle) -> Result<PathBuf, AppError> {
    let base = handle.without_session_binding();
    Ok(store_path(&base, StoreKind::Conversation, false)?.with_file_name("catalog.json"))
}
fn validate(catalog: &Catalog, handle: &ProjectHandle) -> Result<(), AppError> {
    let mut ids = HashSet::new();
    let mut requests = HashSet::new();
    if catalog.schema_version != 1
        || catalog.project_id != handle.project_id()
        || !(1..=MAX_REVISION).contains(&catalog.revision)
        || catalog.entries.is_empty()
        || catalog.entries.len() > MAX_ENTRIES
        || catalog
            .entries
            .iter()
            .filter(|entry| !entry.archived)
            .count()
            > MAX_ACTIVE
        || !valid_id(&catalog.default_session_id)
    {
        return Err(error("sessions.invalid"));
    }
    for entry in &catalog.entries {
        if !valid_id(&entry.session_id)
            || !ids.insert(entry.session_id.as_str())
            || !valid_name(&entry.name)
            || entry.created_at_ms == 0
            || entry.updated_at_ms < entry.created_at_ms
            || entry
                .creation_request_id
                .as_ref()
                .is_some_and(|id| !valid_id(id) || !requests.insert(id.as_str()))
        {
            return Err(error("sessions.invalid"));
        }
    }
    if !ids.contains(catalog.default_session_id.as_str())
        || catalog.active_session_id.as_ref().is_some_and(|id| {
            !catalog
                .entries
                .iter()
                .any(|entry| &entry.session_id == id && !entry.archived)
        })
        || (catalog.active_session_id.is_none()
            && catalog.entries.iter().any(|entry| !entry.archived))
    {
        return Err(error("sessions.invalid"));
    }
    if let Some(run) = &catalog.run {
        if !valid_id(&run.reservation_id)
            || run.turn_id.as_ref().is_some_and(|id| !valid_id(id))
            || !catalog
                .entries
                .iter()
                .any(|entry| entry.session_id == run.session_id && !entry.archived)
        {
            return Err(error("sessions.invalid"));
        }
    }
    Ok(())
}
fn read_catalog(handle: &ProjectHandle) -> Result<Catalog, AppError> {
    let meta =
        fs::symlink_metadata(catalog_path(handle)?).map_err(|_| error("sessions.unavailable"))?;
    if !meta.is_file() || is_link_or_reparse(&meta) {
        return Err(error("sessions.boundary"));
    }
    let bytes = read_bounded(&catalog_path(handle)?, MAX_CATALOG_BYTES)
        .map_err(|_| error("sessions.unavailable"))?;
    let catalog: Catalog = serde_json::from_slice(&bytes).map_err(|_| error("sessions.invalid"))?;
    validate(&catalog, handle)?;
    Ok(catalog)
}
fn write_catalog(handle: &ProjectHandle, catalog: &Catalog) -> Result<(), AppError> {
    validate(catalog, handle)?;
    let bytes = serde_json::to_vec_pretty(catalog).map_err(|_| error("sessions.invalid"))?;
    if bytes.len() as u64 + 1 > MAX_CATALOG_BYTES {
        return Err(error("sessions.full"));
    }
    let path = catalog_path(handle)?;
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if !meta.is_file() || is_link_or_reparse(&meta) {
            return Err(error("sessions.boundary"));
        }
    }
    let temporary = path.with_file_name(format!(".catalog-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        write_new(&temporary, &bytes)?;
        #[cfg(test)]
        if FAIL_NEXT_CATALOG_PROMOTION.with(|flag| flag.replace(false)) {
            return Err(error("sessions.unavailable"));
        }
        atomic_replace(&temporary, &path).map_err(|_| error("sessions.unavailable"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| error("sessions.unavailable"))?;
    file.write_all(bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| error("sessions.unavailable"))
}
#[cfg(test)]
thread_local! {
    static FAIL_NEXT_CATALOG_PROMOTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn ensure_catalog(handle: &ProjectHandle, now: u64) -> Result<Catalog, AppError> {
    store_path(
        &handle.without_session_binding(),
        StoreKind::Conversation,
        true,
    )?;
    let path = catalog_path(handle)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => return read_catalog(handle),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(error("sessions.unavailable")),
    }
    if now == 0 {
        return Err(error("sessions.invalid"));
    }
    let legacy = ensure_conversation_session(&handle.without_session_binding())?;
    let catalog = Catalog {
        schema_version: 1,
        project_id: handle.project_id().into(),
        revision: 1,
        default_session_id: legacy.session_id.clone(),
        active_session_id: Some(legacy.session_id.clone()),
        entries: vec![SessionEntry {
            session_id: legacy.session_id,
            name: "默认会话".into(),
            archived: false,
            created_at_ms: now,
            updated_at_ms: now,
            creation_request_id: None,
        }],
        run: None,
    };
    write_catalog(handle, &catalog)?;
    Ok(catalog)
}
fn binding(handle: &ProjectHandle, catalog: &Catalog, id: &str) -> Result<ProjectHandle, AppError> {
    if !catalog.entries.iter().any(|entry| entry.session_id == id) {
        return Err(error("sessions.not_found"));
    }
    Ok(handle.with_session_binding(SessionBinding {
        id: id.into(),
        legacy: id == catalog.default_session_id,
    }))
}
pub fn bind_session(
    handle: &ProjectHandle,
    id: Option<&str>,
    now: u64,
) -> Result<ProjectHandle, AppError> {
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let catalog = ensure_catalog(handle, now)?;
    let id = id
        .or(catalog.active_session_id.as_deref())
        .ok_or_else(|| error("sessions.not_found"))?;
    binding(handle, &catalog, id)
}
/// Never initializes catalog/history or recovers another process's work.
pub fn bind_existing_session(
    handle: &ProjectHandle,
    id: Option<&str>,
) -> Result<ProjectHandle, AppError> {
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let catalog = read_catalog(handle)?;
    let id = id
        .or(catalog.active_session_id.as_deref())
        .ok_or_else(|| error("sessions.not_found"))?;
    binding(handle, &catalog, id)
}
fn page(catalog: &Catalog, archived: bool, offset: usize) -> Result<SessionPage, AppError> {
    let entries: Vec<_> = catalog
        .entries
        .iter()
        .filter(|entry| entry.archived == archived)
        .cloned()
        .collect();
    if offset > entries.len() || !offset.is_multiple_of(PAGE_SIZE) {
        return Err(error("sessions.cursor_invalid"));
    }
    let next = offset + PAGE_SIZE;
    let snapshot = sha256_hex(&serde_json::to_vec(catalog).map_err(|_| error("sessions.invalid"))?);
    let next_cursor = if next < entries.len() {
        Some(
            URL_SAFE_NO_PAD.encode(
                serde_json::to_vec(&Cursor {
                    project_id: catalog.project_id.clone(),
                    archived,
                    snapshot,
                    offset: next,
                })
                .map_err(|_| error("sessions.invalid"))?,
            ),
        )
    } else {
        None
    };
    Ok(SessionPage {
        can_mutate: true,
        schema_version: 1,
        revision: catalog.revision,
        active_session_id: catalog.active_session_id.clone(),
        default_session_id: catalog.default_session_id.clone(),
        items: entries.into_iter().skip(offset).take(PAGE_SIZE).collect(),
        non_archived_count: catalog
            .entries
            .iter()
            .filter(|entry| !entry.archived)
            .count(),
        total_count: catalog.entries.len(),
        next_cursor,
        active_run: catalog.run.clone(),
    })
}
pub fn list_sessions(
    handle: &ProjectHandle,
    archived: bool,
    cursor: Option<&str>,
    now: u64,
) -> Result<SessionPage, AppError> {
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let catalog = ensure_catalog(handle, now)?;
    catalog_page(handle, &catalog, archived, cursor)
}
pub fn list_existing_sessions(
    handle: &ProjectHandle,
    archived: bool,
    cursor: Option<&str>,
) -> Result<SessionPage, AppError> {
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let mut result = catalog_page(handle, &read_catalog(handle)?, archived, cursor)?;
    result.can_mutate = false;
    Ok(result)
}
pub fn is_archived(handle: &ProjectHandle) -> Result<bool, AppError> {
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let catalog = read_catalog(handle)?;
    let id = handle
        .conversation_session_id()
        .ok_or_else(|| error("sessions.invalid"))?;
    catalog
        .entries
        .iter()
        .find(|entry| entry.session_id == id)
        .map(|entry| entry.archived)
        .ok_or_else(|| error("sessions.not_found"))
}
fn catalog_page(
    handle: &ProjectHandle,
    catalog: &Catalog,
    archived: bool,
    cursor: Option<&str>,
) -> Result<SessionPage, AppError> {
    let offset = if let Some(cursor) = cursor {
        if cursor.len() > 1024 {
            return Err(error("sessions.cursor_invalid"));
        }
        let value: Cursor = URL_SAFE_NO_PAD
            .decode(cursor)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or_else(|| error("sessions.cursor_invalid"))?;
        let snapshot =
            sha256_hex(&serde_json::to_vec(&catalog).map_err(|_| error("sessions.invalid"))?);
        if value.project_id != handle.project_id()
            || value.archived != archived
            || value.snapshot != snapshot
        {
            return Err(error("sessions.cursor_invalid"));
        }
        value.offset
    } else {
        0
    };
    page(catalog, archived, offset)
}
fn compare_revision(catalog: &Catalog, revision: u64) -> Result<(), AppError> {
    if catalog.revision != revision || revision >= MAX_REVISION {
        return Err(error("sessions.revision_conflict"));
    }
    Ok(())
}
pub fn create_session(
    handle: &ProjectHandle,
    request_id: &str,
    name: &str,
    revision: u64,
    now: u64,
) -> Result<SessionPage, AppError> {
    if !valid_id(request_id) || !valid_name(name) || now == 0 {
        return Err(error("sessions.invalid"));
    }
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let mut catalog = ensure_catalog(handle, now)?;
    if let Some(existing) = catalog
        .entries
        .iter()
        .find(|entry| entry.creation_request_id.as_deref() == Some(request_id))
    {
        return if existing.name == name {
            page(&catalog, false, 0)
        } else {
            Err(error("sessions.revision_conflict"))
        };
    }
    compare_revision(&catalog, revision)?;
    if catalog.entries.len() >= MAX_ENTRIES
        || catalog
            .entries
            .iter()
            .filter(|entry| !entry.archived)
            .count()
            >= MAX_ACTIVE
    {
        return Err(error("sessions.full"));
    }
    let id = Uuid::new_v4().to_string();
    let bound = handle.with_session_binding(SessionBinding {
        id: id.clone(),
        legacy: false,
    });
    let session = ConversationSession {
        schema_version: 1,
        project_id: handle.project_id().into(),
        session_id: id.clone(),
        turns: vec![],
    };
    let bytes = serde_json::to_vec_pretty(&session).map_err(|_| error("sessions.invalid"))?;
    quota::check_write_locked(
        &bound,
        &catalog,
        StoreKind::Conversation,
        bytes.len() as u64 + 1,
    )?;
    let path = store_path(&bound, StoreKind::Conversation, true)?;
    write_new(&path, &bytes)?;
    catalog.entries.push(SessionEntry {
        session_id: id.clone(),
        name: name.into(),
        archived: false,
        created_at_ms: now,
        updated_at_ms: now,
        creation_request_id: Some(request_id.into()),
    });
    catalog.active_session_id = Some(id);
    catalog.revision += 1;
    write_catalog(handle, &catalog)?;
    page(&catalog, false, 0)
}
pub fn rename_session(
    handle: &ProjectHandle,
    id: &str,
    name: &str,
    revision: u64,
    now: u64,
) -> Result<SessionPage, AppError> {
    if !valid_name(name) || !valid_id(id) || now == 0 {
        return Err(error("sessions.invalid"));
    }
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let mut catalog = ensure_catalog(handle, now)?;
    let entry = catalog
        .entries
        .iter()
        .find(|entry| entry.session_id == id)
        .ok_or_else(|| error("sessions.not_found"))?;
    if entry.name == name {
        return page(&catalog, entry.archived, 0);
    }
    compare_revision(&catalog, revision)?;
    let entry = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.session_id == id)
        .unwrap();
    entry.name = name.into();
    entry.updated_at_ms = now.max(entry.created_at_ms);
    let archived = entry.archived;
    catalog.revision += 1;
    write_catalog(handle, &catalog)?;
    page(&catalog, archived, 0)
}
pub fn activate_session(
    handle: &ProjectHandle,
    id: &str,
    revision: u64,
    now: u64,
) -> Result<SessionPage, AppError> {
    if !valid_id(id) {
        return Err(error("sessions.invalid"));
    }
    let lock = coordination(handle)?;
    let _guard = lock.lock().map_err(|_| error("sessions.unavailable"))?;
    let mut catalog = ensure_catalog(handle, now)?;
    if !catalog
        .entries
        .iter()
        .any(|entry| entry.session_id == id && !entry.archived)
    {
        return Err(error("sessions.not_found"));
    }
    if catalog.active_session_id.as_deref() == Some(id) {
        return page(&catalog, false, 0);
    }
    compare_revision(&catalog, revision)?;
    catalog.active_session_id = Some(id.into());
    catalog.revision += 1;
    write_catalog(handle, &catalog)?;
    page(&catalog, false, 0)
}
