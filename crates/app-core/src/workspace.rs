use crate::project::ProjectHandle;
use crate::recovery::RecoveryStore;
use crate::AppError;
pub use novelgainker_validation::ValidationReport;
use novelgainker_validation::{validate_source, validate_source_bytes};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Take, Write};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const MAX_EDITABLE_FILE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_PROJECT_INDEX_ENTRIES: usize = 5_000;
pub const MAX_PROJECT_INDEX_DEPTH: usize = 16;
const MAX_METADATA_FILE_BYTES: u64 = 32 * 1024;
const SOURCE_DIRECTORIES: &[&str] = &[
    "characters",
    "materials",
    "macguffins",
    "plot",
    "outlines",
    "world",
    "style",
    "chapters",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectFileKind {
    Directory,
    Markdown,
    Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileNode {
    pub name: String,
    pub relative_path: String,
    pub kind: ProjectFileKind,
    pub size_bytes: Option<u64>,
    pub read_only: bool,
    pub children: Vec<ProjectFileNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileIndex {
    pub nodes: Vec<ProjectFileNode>,
    pub max_editable_file_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Utf8Bom {
    None,
    Utf8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEnding {
    None,
    Lf,
    CrLf,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileDocument {
    pub relative_path: String,
    pub content: String,
    pub content_hash: String,
    pub modified_time_ms: u64,
    pub size_bytes: u64,
    pub kind: ProjectFileKind,
    pub read_only: bool,
    pub bom: Utf8Bom,
    pub line_ending: LineEnding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticState {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileSaveResult {
    pub relative_path: String,
    pub content_hash: String,
    pub modified_time_ms: u64,
    pub size_bytes: u64,
    pub bom: Utf8Bom,
    pub line_ending: LineEnding,
    pub diagnostics_triggered: bool,
    pub diagnostic_state: DiagnosticState,
    pub validation: ValidationReport,
    /// NR-5 formal manual chapter saves return the actual editor text after
    /// version-bound status normalization. Ordinary editor saves omit this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter_finalization_confirmed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectFileWatchState {
    Unchanged,
    Modified,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileWatchStatus {
    pub relative_path: String,
    pub state: ProjectFileWatchState,
    pub observed_hash: Option<String>,
    pub modified_time_ms: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveStage {
    Write,
    Flush,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AllowedFile {
    Markdown,
    Metadata,
}

struct IndexBudget {
    included_entries: usize,
}

impl IndexBudget {
    fn new() -> Self {
        Self {
            included_entries: 0,
        }
    }

    fn include(&mut self) -> Result<(), AppError> {
        self.included_entries += 1;
        if self.included_entries > MAX_PROJECT_INDEX_ENTRIES {
            Err(AppError::file_index_too_large())
        } else {
            Ok(())
        }
    }
}

pub fn list_project_files(handle: &ProjectHandle) -> Result<ProjectFileIndex, AppError> {
    let root = handle.root();
    let mut budget = IndexBudget::new();
    let mut nodes = collect_root_markdown(root, &mut budget)?;

    for source_directory in SOURCE_DIRECTORIES {
        if let Some(node) = scan_source_directory(
            root,
            &root.join(source_directory),
            source_directory,
            0,
            &mut budget,
        )? {
            nodes.push(node);
        }
    }

    if let Some(metadata_node) = controlled_metadata_node(root, &mut budget)? {
        nodes.push(metadata_node);
    }

    Ok(ProjectFileIndex {
        nodes,
        max_editable_file_bytes: MAX_EDITABLE_FILE_BYTES,
    })
}

pub fn validate_project_draft(
    _handle: &ProjectHandle,
    relative_path: &str,
    content: &str,
) -> Result<ValidationReport, AppError> {
    validate_writable_relative_path(relative_path)?;
    if u64::try_from(content.len()).unwrap_or(u64::MAX) > MAX_EDITABLE_FILE_BYTES {
        return Err(AppError::file_too_large());
    }
    Ok(validate_source(relative_path, content))
}

pub fn read_project_file(
    handle: &ProjectHandle,
    relative_path: &str,
) -> Result<ProjectFileDocument, AppError> {
    let allowed_file = classify_relative_file(relative_path)?;
    let root = handle.root();
    let file_path = resolve_scoped_file(root, relative_path)?;
    let metadata = fs::metadata(&file_path).map_err(|_| AppError::file_not_found())?;
    if !metadata.is_file() {
        return Err(AppError::file_not_found());
    }

    let maximum_bytes = match allowed_file {
        AllowedFile::Markdown => MAX_EDITABLE_FILE_BYTES,
        AllowedFile::Metadata => MAX_METADATA_FILE_BYTES,
    };
    if metadata.len() > maximum_bytes {
        return Err(AppError::file_too_large());
    }

    let bytes = read_bounded(&file_path, maximum_bytes)?;
    let (body, bom) = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        (&bytes[3..], Utf8Bom::Utf8)
    } else {
        (bytes.as_slice(), Utf8Bom::None)
    };
    let content = std::str::from_utf8(body)
        .map_err(|_| AppError::file_unsupported_encoding())?
        .to_owned();
    let content_hash = sha256_hex(&bytes);
    let kind = match allowed_file {
        AllowedFile::Markdown => ProjectFileKind::Markdown,
        AllowedFile::Metadata => ProjectFileKind::Metadata,
    };

    Ok(ProjectFileDocument {
        relative_path: relative_path.to_owned(),
        content,
        content_hash,
        modified_time_ms: modified_time_ms(&metadata),
        size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        kind,
        read_only: allowed_file == AllowedFile::Metadata,
        bom,
        line_ending: detect_line_ending(body),
    })
}

pub fn check_project_file(
    handle: &ProjectHandle,
    relative_path: &str,
    expected_hash: &str,
) -> Result<ProjectFileWatchStatus, AppError> {
    let allowed_file = classify_relative_file(relative_path)?;
    validate_source_hash(expected_hash)?;
    let maximum_bytes = match allowed_file {
        AllowedFile::Markdown => MAX_EDITABLE_FILE_BYTES,
        AllowedFile::Metadata => MAX_METADATA_FILE_BYTES,
    };
    let target = match resolve_scoped_file(handle.root(), relative_path) {
        Ok(value) => value,
        Err(error) if error.code == "file.not_found" => {
            return Ok(ProjectFileWatchStatus {
                relative_path: relative_path.to_owned(),
                state: ProjectFileWatchState::Missing,
                observed_hash: None,
                modified_time_ms: 0,
            });
        }
        Err(error) => return Err(error),
    };
    let metadata = match fs::metadata(&target) {
        Ok(value) if value.is_file() => value,
        Ok(_) => return Err(AppError::file_not_found()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProjectFileWatchStatus {
                relative_path: relative_path.to_owned(),
                state: ProjectFileWatchState::Missing,
                observed_hash: None,
                modified_time_ms: 0,
            });
        }
        Err(_) => return Err(file_io("file-watch-metadata-failed")),
    };
    if metadata.len() > maximum_bytes {
        return Err(AppError::file_too_large());
    }
    let bytes = read_bounded(&target, maximum_bytes)?;
    let observed_hash = sha256_hex(&bytes);
    Ok(ProjectFileWatchStatus {
        relative_path: relative_path.to_owned(),
        state: if observed_hash == expected_hash {
            ProjectFileWatchState::Unchanged
        } else {
            ProjectFileWatchState::Modified
        },
        observed_hash: Some(observed_hash),
        modified_time_ms: modified_time_ms(&metadata),
    })
}

pub fn save_project_file(
    handle: &ProjectHandle,
    relative_path: &str,
    content: &str,
    source_hash: &str,
) -> Result<ProjectFileSaveResult, AppError> {
    save_project_file_with_hook(
        handle,
        relative_path,
        content,
        source_hash,
        None,
        |_| Ok(()),
    )
}

pub fn save_project_file_recoverable(
    handle: &ProjectHandle,
    relative_path: &str,
    content: &str,
    source_hash: &str,
    recovery_store: &RecoveryStore,
) -> Result<ProjectFileSaveResult, AppError> {
    save_project_file_with_hook(
        handle,
        relative_path,
        content,
        source_hash,
        Some(recovery_store),
        |_| Ok(()),
    )
}

fn save_project_file_with_hook<F>(
    handle: &ProjectHandle,
    relative_path: &str,
    content: &str,
    source_hash: &str,
    recovery_store: Option<&RecoveryStore>,
    mut hook: F,
) -> Result<ProjectFileSaveResult, AppError>
where
    F: FnMut(SaveStage) -> Result<(), AppError>,
{
    validate_writable_relative_path(relative_path)?;
    validate_source_hash(source_hash)?;

    let root = handle.root();
    let target_path = resolve_scoped_file(root, relative_path)?;
    let original_metadata = fs::metadata(&target_path).map_err(|_| AppError::file_not_found())?;
    if !original_metadata.is_file() {
        return Err(AppError::file_not_found());
    }
    if original_metadata.len() > MAX_EDITABLE_FILE_BYTES {
        return Err(AppError::file_too_large());
    }

    let original_bytes = read_bounded(&target_path, MAX_EDITABLE_FILE_BYTES)?;
    let (original_body, bom) = split_utf8_bom(&original_bytes);
    std::str::from_utf8(original_body).map_err(|_| AppError::file_unsupported_encoding())?;
    if sha256_hex(&original_bytes) != source_hash {
        return Err(AppError::file_source_changed());
    }

    let original_line_ending = detect_line_ending(original_body);
    let encoded_body = encode_line_endings(content, original_line_ending);
    let mut replacement_bytes =
        Vec::with_capacity(encoded_body.len().saturating_add(if bom == Utf8Bom::Utf8 {
            3
        } else {
            0
        }));
    if bom == Utf8Bom::Utf8 {
        replacement_bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
    }
    replacement_bytes.extend_from_slice(encoded_body.as_bytes());
    if u64::try_from(replacement_bytes.len()).unwrap_or(u64::MAX) > MAX_EDITABLE_FILE_BYTES {
        return Err(AppError::file_too_large());
    }

    let replacement_hash = sha256_hex(&replacement_bytes);
    let temporary_file_id = new_scoped_temporary_file_id(&target_path)?;
    let mut recovery_transaction = match recovery_store {
        Some(store) => Some(store.begin_transaction(
            handle,
            relative_path,
            source_hash,
            &replacement_hash,
            &temporary_file_id,
        )?),
        None => None,
    };

    let outcome = (|| {
        let (mut temporary_file, mut temporary_guard) =
            create_scoped_temporary_file(&target_path, &temporary_file_id)?;
        hook(SaveStage::Write)?;
        temporary_file
            .write_all(&replacement_bytes)
            .map_err(|_| file_save_failed("file-save-write-failed"))?;
        hook(SaveStage::Flush)?;
        temporary_file
            .flush()
            .map_err(|_| file_save_failed("file-save-flush-failed"))?;
        temporary_file
            .sync_all()
            .map_err(|_| file_save_failed("file-save-sync-failed"))?;
        drop(temporary_file);
        let replacement_metadata = fs::metadata(temporary_guard.path())
            .map_err(|_| file_save_failed("file-save-temp-metadata-failed"))?;
        let replacement_modified_time_ms = modified_time_ms(&replacement_metadata);
        if let Some(transaction) = recovery_transaction.as_mut() {
            transaction.mark_temporary_synced()?;
        }

        let revalidated_target = resolve_scoped_file(root, relative_path)?;
        if revalidated_target != target_path {
            return Err(AppError::file_source_changed());
        }
        let current_bytes = read_bounded(&revalidated_target, MAX_EDITABLE_FILE_BYTES)?;
        if sha256_hex(&current_bytes) != source_hash {
            return Err(AppError::file_source_changed());
        }

        if let Some(transaction) = recovery_transaction.as_mut() {
            transaction.mark_replacing()?;
        }
        hook(SaveStage::Replace)?;
        atomic_replace(temporary_guard.path(), &target_path)?;
        temporary_guard.disarm();

        let saved_body = if bom == Utf8Bom::Utf8 {
            &replacement_bytes[3..]
        } else {
            replacement_bytes.as_slice()
        };
        let validation = validate_source_bytes(relative_path, &replacement_bytes);
        debug_assert_eq!(validation.content_hash, replacement_hash);
        Ok(ProjectFileSaveResult {
            relative_path: relative_path.to_owned(),
            content_hash: replacement_hash,
            modified_time_ms: replacement_modified_time_ms,
            size_bytes: u64::try_from(replacement_bytes.len()).unwrap_or(u64::MAX),
            bom,
            line_ending: detect_line_ending(saved_body),
            diagnostics_triggered: true,
            diagnostic_state: DiagnosticState::Ready,
            validation,
            saved_content: None,
            chapter_finalization_confirmed: None,
        })
    })();

    match outcome {
        Ok(result) => {
            if let Some(transaction) = recovery_transaction.take() {
                transaction.commit_best_effort();
            }
            Ok(result)
        }
        Err(error) => {
            if let Some(transaction) = recovery_transaction.take() {
                transaction.abort_best_effort();
            }
            Err(error)
        }
    }
}

pub(crate) fn validate_writable_relative_path(relative_path: &str) -> Result<(), AppError> {
    if classify_relative_file(relative_path)? == AllowedFile::Markdown {
        Ok(())
    } else {
        Err(AppError::file_not_allowed())
    }
}

fn validate_source_hash(source_hash: &str) -> Result<(), AppError> {
    let valid = source_hash.len() == 64
        && source_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(AppError::file_invalid_source_hash())
    }
}

fn split_utf8_bom(bytes: &[u8]) -> (&[u8], Utf8Bom) {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        (&bytes[3..], Utf8Bom::Utf8)
    } else {
        (bytes, Utf8Bom::None)
    }
}

fn encode_line_endings(content: &str, original: LineEnding) -> String {
    let normalized = normalize_to_lf(content);
    if original == LineEnding::CrLf {
        normalized.replace('\n', "\r\n")
    } else {
        normalized
    }
}

fn normalize_to_lf(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut normalized = String::with_capacity(content.len());
    let mut start = 0_usize;
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index] == b'\r' {
            normalized.push_str(&content[start..index]);
            normalized.push('\n');
            index += if bytes.get(index + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
            start = index;
        } else {
            index += 1;
        }
    }
    normalized.push_str(&content[start..]);
    normalized
}

struct ScopedTemporaryFile {
    path: PathBuf,
    parent: PathBuf,
    expected_prefix: String,
    armed: bool,
}

impl ScopedTemporaryFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn is_verified(&self) -> bool {
        self.path.parent() == Some(self.parent.as_path())
            && self
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(&self.expected_prefix) && name.ends_with(".tmp")
                })
    }
}

impl Drop for ScopedTemporaryFile {
    fn drop(&mut self) {
        if self.armed && self.is_verified() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn new_scoped_temporary_file_id(target_path: &Path) -> Result<String, AppError> {
    let target_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(AppError::file_invalid_path)?;
    Ok(format!(
        ".{target_name}.novelgainker-{}.tmp",
        uuid::Uuid::new_v4()
    ))
}

fn create_scoped_temporary_file(
    target_path: &Path,
    temporary_file_id: &str,
) -> Result<(File, ScopedTemporaryFile), AppError> {
    let parent = target_path
        .parent()
        .ok_or_else(|| file_save_failed("file-save-parent-missing"))?
        .to_path_buf();
    let target_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(AppError::file_invalid_path)?;
    let expected_prefix = format!(".{target_name}.novelgainker-");
    if !temporary_file_id.starts_with(&expected_prefix)
        || !temporary_file_id.ends_with(".tmp")
        || temporary_file_id.contains(['/', '\\'])
    {
        return Err(AppError::file_invalid_path());
    }
    let temporary_path = parent.join(temporary_file_id);
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(|_| file_save_failed("file-save-temp-create-failed"))?;
    Ok((
        file,
        ScopedTemporaryFile {
            path: temporary_path,
            parent,
            expected_prefix,
            armed: true,
        },
    ))
}

#[cfg(windows)]
pub(crate) fn atomic_replace(temporary_path: &Path, target_path: &Path) -> Result<(), AppError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let temporary_wide = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target_wide = target_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            temporary_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(file_save_failed("file-save-atomic-replace-failed"))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn atomic_replace(temporary_path: &Path, target_path: &Path) -> Result<(), AppError> {
    fs::rename(temporary_path, target_path)
        .map_err(|_| file_save_failed("file-save-atomic-replace-failed"))
}
fn collect_root_markdown(
    root: &Path,
    budget: &mut IndexBudget,
) -> Result<Vec<ProjectFileNode>, AppError> {
    let mut nodes = Vec::new();
    let entries = fs::read_dir(root).map_err(|_| file_io("file-index-root-read-failed"))?;
    for entry in entries {
        let entry = entry.map_err(|_| file_io("file-index-root-entry-failed"))?;
        let Some(name) = safe_visible_name(&entry.file_name()) else {
            continue;
        };
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| file_io("file-index-metadata-failed"))?;
        if is_link_or_reparse(&metadata) || !metadata.is_file() || !is_markdown_path(&path) {
            continue;
        }
        budget.include()?;
        nodes.push(file_node(name.clone(), name, metadata.len()));
    }
    sort_nodes(&mut nodes);
    Ok(nodes)
}

fn scan_source_directory(
    root: &Path,
    directory_path: &Path,
    relative_path: &str,
    depth: usize,
    budget: &mut IndexBudget,
) -> Result<Option<ProjectFileNode>, AppError> {
    if depth > MAX_PROJECT_INDEX_DEPTH {
        return Err(AppError::file_index_too_large());
    }
    let metadata = match fs::symlink_metadata(directory_path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(file_io("file-index-directory-metadata-failed")),
    };
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Ok(None);
    }

    let name = directory_path
        .file_name()
        .and_then(safe_visible_name)
        .ok_or_else(AppError::file_not_allowed)?;
    budget.include()?;
    let mut children = Vec::new();
    let entries =
        fs::read_dir(directory_path).map_err(|_| file_io("file-index-directory-read-failed"))?;
    for entry in entries {
        let entry = entry.map_err(|_| file_io("file-index-directory-entry-failed"))?;
        let Some(child_name) = safe_visible_name(&entry.file_name()) else {
            continue;
        };
        let child_path = entry.path();
        let child_metadata =
            fs::symlink_metadata(&child_path).map_err(|_| file_io("file-index-metadata-failed"))?;
        if is_link_or_reparse(&child_metadata) {
            continue;
        }
        let child_relative_path = format!("{relative_path}/{child_name}");
        if child_metadata.is_dir() {
            if let Some(child) =
                scan_source_directory(root, &child_path, &child_relative_path, depth + 1, budget)?
            {
                children.push(child);
            }
        } else if child_metadata.is_file() && is_markdown_path(&child_path) {
            let canonical_child = child_path
                .canonicalize()
                .map_err(|_| file_io("file-index-canonicalize-failed"))?;
            if !canonical_child.starts_with(root) {
                continue;
            }
            budget.include()?;
            children.push(file_node(
                child_name,
                child_relative_path,
                child_metadata.len(),
            ));
        }
    }
    sort_nodes(&mut children);

    Ok(Some(ProjectFileNode {
        name,
        relative_path: relative_path.to_owned(),
        kind: ProjectFileKind::Directory,
        size_bytes: None,
        read_only: false,
        children,
    }))
}

fn controlled_metadata_node(
    root: &Path,
    budget: &mut IndexBudget,
) -> Result<Option<ProjectFileNode>, AppError> {
    let directory_path = root.join(".novelgainker");
    let directory_metadata = match fs::symlink_metadata(&directory_path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(file_io("file-index-metadata-directory-failed")),
    };
    if is_link_or_reparse(&directory_metadata) || !directory_metadata.is_dir() {
        return Ok(None);
    }

    let file_path = directory_path.join("project.json");
    let file_metadata = match fs::symlink_metadata(&file_path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(file_io("file-index-project-metadata-failed")),
    };
    if is_link_or_reparse(&file_metadata) || !file_metadata.is_file() {
        return Ok(None);
    }

    budget.include()?;
    budget.include()?;
    Ok(Some(ProjectFileNode {
        name: ".novelgainker".to_owned(),
        relative_path: ".novelgainker".to_owned(),
        kind: ProjectFileKind::Directory,
        size_bytes: None,
        read_only: true,
        children: vec![ProjectFileNode {
            name: "project.json".to_owned(),
            relative_path: ".novelgainker/project.json".to_owned(),
            kind: ProjectFileKind::Metadata,
            size_bytes: Some(file_metadata.len()),
            read_only: true,
            children: Vec::new(),
        }],
    }))
}

fn file_node(name: String, relative_path: String, size_bytes: u64) -> ProjectFileNode {
    ProjectFileNode {
        name,
        relative_path,
        kind: ProjectFileKind::Markdown,
        size_bytes: Some(size_bytes),
        read_only: false,
        children: Vec::new(),
    }
}

fn sort_nodes(nodes: &mut [ProjectFileNode]) {
    nodes.sort_by(|left, right| {
        node_sort_rank(left.kind)
            .cmp(&node_sort_rank(right.kind))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.name.cmp(&right.name))
    });
}

fn node_sort_rank(kind: ProjectFileKind) -> u8 {
    match kind {
        ProjectFileKind::Directory => 0,
        ProjectFileKind::Markdown => 1,
        ProjectFileKind::Metadata => 2,
    }
}

fn classify_relative_file(relative_path: &str) -> Result<AllowedFile, AppError> {
    if relative_path.is_empty()
        || relative_path.len() > 512
        || relative_path.contains('\\')
        || relative_path.contains(':')
        || relative_path.chars().any(char::is_control)
    {
        return Err(AppError::file_invalid_path());
    }
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(AppError::file_invalid_path());
    }

    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let Some(value) = value.to_str() else {
                    return Err(AppError::file_invalid_path());
                };
                if value.is_empty()
                    || value == "."
                    || value == ".."
                    || is_reserved_windows_component(value)
                {
                    return Err(AppError::file_invalid_path());
                }
                components.push(value);
            }
            _ => return Err(AppError::file_invalid_path()),
        }
    }
    if components.is_empty() {
        return Err(AppError::file_invalid_path());
    }

    if components == [".novelgainker", "project.json"] {
        return Ok(AllowedFile::Metadata);
    }
    if components
        .iter()
        .any(|component| component.starts_with('.'))
    {
        return Err(AppError::file_not_allowed());
    }
    let is_markdown = components
        .last()
        .and_then(|name| Path::new(name).extension())
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
    if !is_markdown {
        return Err(AppError::file_not_allowed());
    }
    if components.len() == 1 || SOURCE_DIRECTORIES.contains(&components[0]) {
        Ok(AllowedFile::Markdown)
    } else {
        Err(AppError::file_not_allowed())
    }
}

fn is_reserved_windows_component(component: &str) -> bool {
    if component.ends_with([' ', '.']) {
        return true;
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}
pub(crate) fn resolve_scoped_file(root: &Path, relative_path: &str) -> Result<PathBuf, AppError> {
    let mut current = root.to_path_buf();
    let components = Path::new(relative_path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();

    for (index, component) in components.iter().enumerate() {
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(AppError::file_not_found());
            }
            Err(_) => return Err(file_io("file-read-metadata-failed")),
        };
        if is_link_or_reparse(&metadata) {
            return Err(AppError::file_not_allowed());
        }
        if index + 1 < components.len() && !metadata.is_dir() {
            return Err(AppError::file_not_found());
        }
    }

    let canonical = current
        .canonicalize()
        .map_err(|_| AppError::file_not_found())?;
    if !canonical.starts_with(root) {
        return Err(AppError::file_not_allowed());
    }
    Ok(canonical)
}

pub(crate) fn resolve_scoped_target(root: &Path, relative_path: &str) -> Result<PathBuf, AppError> {
    validate_writable_relative_path(relative_path)?;
    let path = Path::new(relative_path);
    let file_name = path.file_name().ok_or_else(AppError::file_invalid_path)?;
    let parent_relative = path.parent().unwrap_or_else(|| Path::new(""));
    let mut parent = root.to_path_buf();
    for component in parent_relative.components() {
        let Component::Normal(value) = component else {
            return Err(AppError::file_invalid_path());
        };
        parent.push(value);
        let metadata = fs::symlink_metadata(&parent).map_err(|_| AppError::file_not_found())?;
        if is_link_or_reparse(&metadata) {
            return Err(AppError::file_not_allowed());
        }
        if !metadata.is_dir() {
            return Err(AppError::file_not_found());
        }
    }
    let parent = parent
        .canonicalize()
        .map_err(|_| AppError::file_not_found())?;
    if !parent.starts_with(root) {
        return Err(AppError::file_not_allowed());
    }
    let target = parent.join(file_name);
    match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if is_link_or_reparse(&metadata) || !metadata.is_file() {
                return Err(AppError::file_not_allowed());
            }
            let canonical = target
                .canonicalize()
                .map_err(|_| AppError::file_not_found())?;
            if !canonical.starts_with(root) {
                return Err(AppError::file_not_allowed());
            }
            Ok(canonical)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(target),
        Err(_) => Err(file_io("file-target-metadata-failed")),
    }
}

pub(crate) fn read_bounded(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, AppError> {
    let file = File::open(path).map_err(|_| file_io("file-read-open-failed"))?;
    let mut reader: Take<File> = file.take(maximum_bytes + 1);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| file_io("file-read-failed"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(AppError::file_too_large());
    }
    Ok(bytes)
}

fn detect_line_ending(bytes: &[u8]) -> LineEnding {
    let mut crlf_count = 0_u64;
    let mut lf_count = 0_u64;
    let mut bare_cr_count = 0_u64;
    let mut index = 0_usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                crlf_count += 1;
                index += 2;
            }
            b'\r' => {
                bare_cr_count += 1;
                index += 1;
            }
            b'\n' => {
                lf_count += 1;
                index += 1;
            }
            _ => index += 1,
        }
    }

    match (crlf_count, lf_count, bare_cr_count) {
        (0, 0, 0) => LineEnding::None,
        (_, 0, 0) if crlf_count > 0 => LineEnding::CrLf,
        (0, _, 0) if lf_count > 0 => LineEnding::Lf,
        _ => LineEnding::Mixed,
    }
}

fn modified_time_ms(metadata: &Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn safe_visible_name(value: &std::ffi::OsStr) -> Option<String> {
    let name = value.to_str()?;
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.chars().any(char::is_control)
    {
        None
    } else {
        Some(name.to_owned())
    }
}

pub(crate) fn is_link_or_reparse(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink() || is_windows_reparse_point(metadata)
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &Metadata) -> bool {
    false
}

fn file_io(diagnostic: &'static str) -> AppError {
    AppError::file_io().with_diagnostic(diagnostic)
}

fn file_save_failed(diagnostic: &'static str) -> AppError {
    AppError::file_save_failed().with_diagnostic(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::{
        check_project_file, file_save_failed, list_project_files, read_project_file,
        save_project_file, save_project_file_with_hook, DiagnosticState, LineEnding,
        ProjectFileKind, ProjectFileWatchState, SaveStage, Utf8Bom, MAX_EDITABLE_FILE_BYTES,
    };
    use crate::project::{create_project, inspect_project_root, ProjectHandle, ProjectScanMode};
    use std::fs::{self, File};
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    struct TestProject {
        root: PathBuf,
        handle: ProjectHandle,
    }

    impl TestProject {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("novelgainker-ng102-{label}-{}", Uuid::new_v4()));
            fs::create_dir(&root).expect("test root must be created");
            let inspection = inspect_project_root(&root, ProjectScanMode::Create)
                .expect("test project must scan");
            let handle = create_project(&inspection).expect("test project must create");
            Self { root, handle }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let temporary_root = std::env::temp_dir();
            let is_verified_test_root = self.root.parent() == Some(temporary_root.as_path())
                && self
                    .root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("novelgainker-ng102-"));
            if is_verified_test_root {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    fn flatten_paths(nodes: &[super::ProjectFileNode], paths: &mut Vec<String>) {
        for node in nodes {
            paths.push(node.relative_path.clone());
            flatten_paths(&node.children, paths);
        }
    }

    #[test]
    fn index_is_sorted_and_contains_only_approved_sources_and_metadata() {
        let project = TestProject::new("index");
        fs::write(project.path().join("draft.md"), "# 根目录\n").expect("fixture must write");
        fs::write(project.path().join("chapters/002.md"), "# 二\n").expect("fixture must write");
        fs::write(project.path().join("chapters/001.md"), "# 一\n").expect("fixture must write");
        fs::create_dir(project.path().join("chapters/part")).expect("fixture dir must write");
        fs::write(project.path().join("chapters/part/003.md"), "# 三\n")
            .expect("fixture must write");
        fs::write(project.path().join("published/leak.md"), "secret").expect("fixture must write");
        fs::create_dir(project.path().join(".git")).expect("fixture dir must write");
        fs::write(project.path().join(".git/config"), "secret").expect("fixture must write");
        fs::create_dir(project.path().join("private")).expect("fixture dir must write");
        fs::write(project.path().join("private/secret.md"), "secret").expect("fixture must write");
        fs::write(project.path().join("characters/.secret.md"), "secret")
            .expect("fixture must write");

        let index = list_project_files(&project.handle).expect("index must build");
        let mut paths = Vec::new();
        flatten_paths(&index.nodes, &mut paths);

        for allowed in [
            "AGENTS.md",
            "draft.md",
            "project.md",
            "chapters",
            "chapters/001.md",
            "chapters/002.md",
            "chapters/part",
            "chapters/part/003.md",
            ".novelgainker",
            ".novelgainker/project.json",
        ] {
            assert!(paths.contains(&allowed.to_owned()), "missing {allowed}");
        }
        for forbidden in [
            "published",
            "published/leak.md",
            ".git",
            ".git/config",
            "private",
            "private/secret.md",
            "characters/.secret.md",
        ] {
            assert!(
                !paths.contains(&forbidden.to_owned()),
                "unexpected {forbidden}"
            );
        }

        let chapters = index
            .nodes
            .iter()
            .find(|node| node.relative_path == "chapters")
            .expect("chapters directory must exist");
        assert_eq!(
            chapters
                .children
                .iter()
                .map(|node| node.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["chapters/part", "chapters/001.md", "chapters/002.md"]
        );
        let serialized = serde_json::to_string(&index).expect("index must serialize");
        assert!(!serialized.contains(project.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn reads_utf8_bom_and_crlf_without_exposing_the_bom_to_the_editor() {
        let project = TestProject::new("encoding");
        let bytes = b"\xef\xbb\xbf# Chapter\r\n\r\nBody\r\n";
        fs::write(project.path().join("chapters/bom.md"), bytes).expect("fixture must write");

        let document =
            read_project_file(&project.handle, "chapters/bom.md").expect("file must read");

        assert_eq!(document.content, "# Chapter\r\n\r\nBody\r\n");
        assert_eq!(document.bom, Utf8Bom::Utf8);
        assert_eq!(document.line_ending, LineEnding::CrLf);
        assert_eq!(document.content_hash.len(), 64);
        assert_eq!(document.size_bytes, bytes.len() as u64);
        assert!(!document.read_only);
    }

    #[test]
    fn controlled_metadata_is_read_only() {
        let project = TestProject::new("metadata");

        let document = read_project_file(&project.handle, ".novelgainker/project.json")
            .expect("metadata must read");

        assert_eq!(document.kind, ProjectFileKind::Metadata);
        assert!(document.read_only);
        assert!(document.content.contains("\"schemaVersion\": 2"));
    }

    #[test]
    fn rejects_non_utf8_oversized_and_forbidden_paths() {
        let project = TestProject::new("negative");
        fs::write(project.path().join("chapters/non-utf8.md"), [0xff, 0xfe])
            .expect("fixture must write");
        let large_path = project.path().join("chapters/large.md");
        let large_file = File::create(&large_path).expect("large fixture must create");
        large_file
            .set_len(MAX_EDITABLE_FILE_BYTES + 1)
            .expect("large fixture size must set");

        let encoding_error = read_project_file(&project.handle, "chapters/non-utf8.md")
            .expect_err("non UTF-8 must fail");
        assert_eq!(encoding_error.code, "file.unsupported_encoding");
        let size_error =
            read_project_file(&project.handle, "chapters/large.md").expect_err("large must fail");
        assert_eq!(size_error.code, "file.too_large");

        for forbidden in [
            "../secret.md",
            "/secret.md",
            "C:/secret.md",
            r"chapters\secret.md",
            ".git/config",
            "published/leak.md",
            "private/secret.md",
            ".novelgainker/secret.md",
            "chapters/file.txt",
        ] {
            let error = read_project_file(&project.handle, forbidden)
                .expect_err("forbidden path must fail");
            assert!(
                error.code == "file.invalid_path" || error.code == "file.not_allowed",
                "unexpected code for {forbidden}: {}",
                error.code
            );
        }
    }

    #[test]
    fn detects_lf_mixed_and_no_line_endings() {
        let project = TestProject::new("line-endings");
        for (name, contents, expected) in [
            ("lf.md", "a\nb\n", LineEnding::Lf),
            ("mixed.md", "a\r\nb\n", LineEnding::Mixed),
            ("none.md", "abc", LineEnding::None),
        ] {
            fs::write(project.path().join("chapters").join(name), contents)
                .expect("fixture must write");
            let document = read_project_file(&project.handle, &format!("chapters/{name}"))
                .expect("file must read");
            assert_eq!(document.line_ending, expected);
            assert_eq!(document.bom, Utf8Bom::None);
        }
    }

    #[test]
    fn hash_watcher_detects_rapid_external_edits_and_missing_files_without_mtime_guessing() {
        let project = TestProject::new("watcher-external-edit");
        let path = project.path().join("chapters/watched.md");
        fs::write(&path, b"baseline").expect("fixture must write");
        let document =
            read_project_file(&project.handle, "chapters/watched.md").expect("file must read");

        let unchanged = check_project_file(
            &project.handle,
            "chapters/watched.md",
            &document.content_hash,
        )
        .expect("watch check must succeed");
        assert_eq!(unchanged.state, ProjectFileWatchState::Unchanged);
        assert_eq!(
            unchanged.observed_hash.as_deref(),
            Some(document.content_hash.as_str())
        );

        for revision in [b"external-1".as_slice(), b"external-2", b"external-3"] {
            fs::write(&path, revision).expect("external editor fixture must write");
            let modified = check_project_file(
                &project.handle,
                "chapters/watched.md",
                &document.content_hash,
            )
            .expect("watch check must succeed after an external edit");
            assert_eq!(modified.state, ProjectFileWatchState::Modified);
            assert_ne!(
                modified.observed_hash.as_deref(),
                Some(document.content_hash.as_str())
            );
        }

        fs::remove_file(&path).expect("external delete fixture must succeed");
        let missing = check_project_file(
            &project.handle,
            "chapters/watched.md",
            &document.content_hash,
        )
        .expect("missing watch check must remain controlled");
        assert_eq!(missing.state, ProjectFileWatchState::Missing);
        assert!(missing.observed_hash.is_none());
        assert_eq!(missing.modified_time_ms, 0);
    }

    #[test]
    fn atomically_saves_utf8_while_preserving_bom_and_standard_line_endings() {
        let project = TestProject::new("save-encoding");
        let path = project.path().join("chapters/bom.md");
        fs::write(&path, b"\xef\xbb\xbf# Old\r\n\r\nBody\r\n").expect("fixture must write");
        let document =
            read_project_file(&project.handle, "chapters/bom.md").expect("file must read");

        let result = save_project_file(
            &project.handle,
            "chapters/bom.md",
            "# New\n\nBody changed\n",
            &document.content_hash,
        )
        .expect("file must save");

        assert_eq!(
            fs::read(&path).expect("saved file must read"),
            b"\xef\xbb\xbf# New\r\n\r\nBody changed\r\n"
        );
        assert_eq!(result.bom, Utf8Bom::Utf8);
        assert_eq!(result.line_ending, LineEnding::CrLf);
        assert_eq!(result.diagnostic_state, DiagnosticState::Ready);
        assert!(result.diagnostics_triggered);
        assert_eq!(result.content_hash.len(), 64);
        assert!(result.modified_time_ms > 0);
    }

    #[test]
    fn mixed_line_endings_normalize_to_lf_and_invalid_markdown_still_saves() {
        let project = TestProject::new("save-mixed");
        let path = project.path().join("chapters/mixed.md");
        fs::write(&path, b"a\r\nb\nc").expect("fixture must write");
        let document =
            read_project_file(&project.handle, "chapters/mixed.md").expect("file must read");

        let result = save_project_file(
            &project.handle,
            "chapters/mixed.md",
            "not strict markdown\r\nmanual edit\r",
            &document.content_hash,
        )
        .expect("format diagnostics must never block a manual save");

        assert_eq!(
            fs::read(&path).expect("saved file must read"),
            b"not strict markdown\nmanual edit\n"
        );
        assert_eq!(result.line_ending, LineEnding::Lf);
        assert_eq!(result.diagnostic_state, DiagnosticState::Ready);
        assert_eq!(result.validation.content_hash, result.content_hash);
        assert!(result
            .validation
            .diagnostics
            .iter()
            .any(|item| item.code == "frontmatter.missing"));
        assert!(!result.validation.eligible_for_ai_apply);
        assert!(!result.validation.eligible_for_final);
        assert!(!result.validation.eligible_for_epub);
    }

    #[test]
    fn rejects_stale_or_malformed_hashes_and_read_only_metadata_without_writing() {
        let project = TestProject::new("save-conflict");
        let path = project.path().join("chapters/conflict.md");
        fs::write(&path, b"old").expect("fixture must write");
        let document =
            read_project_file(&project.handle, "chapters/conflict.md").expect("file must read");
        fs::write(&path, b"external").expect("external edit must write");

        let stale = save_project_file(
            &project.handle,
            "chapters/conflict.md",
            "new",
            &document.content_hash,
        )
        .expect_err("stale save must fail");
        assert_eq!(stale.code, "file.source_changed");
        assert_eq!(fs::read(&path).expect("target must read"), b"external");

        let malformed = save_project_file(
            &project.handle,
            "chapters/conflict.md",
            "new",
            "not-a-sha256",
        )
        .expect_err("malformed hash must fail");
        assert_eq!(malformed.code, "file.invalid_source_hash");
        assert_eq!(fs::read(&path).expect("target must read"), b"external");

        let metadata = read_project_file(&project.handle, ".novelgainker/project.json")
            .expect("metadata must read");
        let metadata_error = save_project_file(
            &project.handle,
            ".novelgainker/project.json",
            "{}",
            &metadata.content_hash,
        )
        .expect_err("metadata save must fail");
        assert_eq!(metadata_error.code, "file.not_allowed");
    }

    #[test]
    fn injected_write_flush_and_replace_failures_keep_the_old_file_and_clean_only_the_temp() {
        let project = TestProject::new("save-faults");
        let path = project.path().join("chapters/fault.md");
        let unrelated = project.path().join("chapters/keep.tmp");
        fs::write(&unrelated, b"keep").expect("unrelated fixture must write");

        for fault in [SaveStage::Write, SaveStage::Flush, SaveStage::Replace] {
            fs::write(&path, b"old content").expect("fixture must reset");
            let document =
                read_project_file(&project.handle, "chapters/fault.md").expect("file must read");
            let error = save_project_file_with_hook(
                &project.handle,
                "chapters/fault.md",
                "new content",
                &document.content_hash,
                None,
                |stage| {
                    if stage == fault {
                        if stage == SaveStage::Replace {
                            let names = fs::read_dir(project.path().join("chapters"))
                                .expect("fixture directory must read")
                                .filter_map(Result::ok)
                                .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
                                .filter(|name| {
                                    name.starts_with(".fault.md.novelgainker-")
                                        && name.ends_with(".tmp")
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(names.len(), 1, "one sibling temp must exist");
                            let uuid = names[0]
                                .trim_start_matches(".fault.md.novelgainker-")
                                .trim_end_matches(".tmp");
                            Uuid::parse_str(uuid).expect("temporary name must contain a UUID");
                        }
                        Err(file_save_failed("file-save-fault-injected"))
                    } else {
                        Ok(())
                    }
                },
            )
            .expect_err("injected failure must stop save");

            assert_eq!(error.code, "file.save_failed");
            assert_eq!(fs::read(&path).expect("target must read"), b"old content");
            assert_eq!(
                fs::read(&unrelated).expect("unrelated file must read"),
                b"keep"
            );
            let leaked_temps = fs::read_dir(project.path().join("chapters"))
                .expect("fixture directory must read")
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
                .filter(|name| name.contains(".novelgainker-") && name.ends_with(".tmp"))
                .collect::<Vec<_>>();
            assert!(leaked_temps.is_empty(), "temporary files leaked");
        }
    }

    #[test]
    fn path_property_matrix_rejects_traversal_absolute_hidden_and_windows_devices() {
        let project = TestProject::new("save-paths");
        for forbidden in [
            "",
            ".",
            "..",
            "../escape.md",
            "/escape.md",
            "C:/escape.md",
            "//server/share/escape.md",
            "chapters\\escape.md",
            ".git/config.md",
            "published/escape.md",
            "private/escape.md",
            "chapters/CON.md",
            "chapters/prn.story.md",
            "chapters/AUX.md",
            "chapters/NUL.md",
            "chapters/COM1.md",
            "chapters/lpt9.any.md",
            "chapters/CLOCK$.md",
            "chapters/trailing.md.",
            "chapters/trailing.md ",
        ] {
            let error = save_project_file(&project.handle, forbidden, "new", &"a".repeat(64))
                .expect_err("malicious path must fail");
            assert!(
                error.code == "file.invalid_path" || error.code == "file.not_allowed",
                "unexpected code for {forbidden:?}: {}",
                error.code
            );
        }
    }
    #[cfg(windows)]
    fn create_junction(junction: &Path, target: &Path) -> std::io::Result<()> {
        use std::ffi::c_void;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr::{null, null_mut};

        const GENERIC_WRITE: u32 = 0x4000_0000;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_WRITE: u32 = 0x0000_0002;
        const FILE_SHARE_DELETE: u32 = 0x0000_0004;
        const OPEN_EXISTING: u32 = 3;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        const INVALID_HANDLE_VALUE: isize = -1;
        const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00a4;
        const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xa000_0003;

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CreateFileW(
                file_name: *const u16,
                desired_access: u32,
                share_mode: u32,
                security_attributes: *const c_void,
                creation_disposition: u32,
                flags_and_attributes: u32,
                template_file: isize,
            ) -> isize;
            fn DeviceIoControl(
                device: isize,
                control_code: u32,
                input: *mut c_void,
                input_size: u32,
                output: *mut c_void,
                output_size: u32,
                bytes_returned: *mut u32,
                overlapped: *mut c_void,
            ) -> i32;
            fn CloseHandle(object: isize) -> i32;
        }

        fs::create_dir(junction)?;
        let canonical_target = target.canonicalize()?;
        let print_name = canonical_target
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>();
        let substitute_name = format!(r"\??\{}", canonical_target.display())
            .encode_utf16()
            .collect::<Vec<_>>();
        let substitute_bytes = u16::try_from(substitute_name.len() * 2)
            .map_err(|_| std::io::Error::other("junction target too long"))?;
        let print_bytes = u16::try_from(print_name.len() * 2)
            .map_err(|_| std::io::Error::other("junction target too long"))?;
        let print_offset = substitute_bytes
            .checked_add(2)
            .ok_or_else(|| std::io::Error::other("junction target too long"))?;
        let mut path_buffer = substitute_name;
        path_buffer.push(0);
        path_buffer.extend_from_slice(&print_name);
        path_buffer.push(0);
        let reparse_data_length = u16::try_from(8 + path_buffer.len() * 2)
            .map_err(|_| std::io::Error::other("junction target too long"))?;
        let total_length = usize::from(reparse_data_length) + 8;
        let mut buffer = vec![0_u8; total_length];
        buffer[0..4].copy_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
        buffer[4..6].copy_from_slice(&reparse_data_length.to_le_bytes());
        buffer[8..10].copy_from_slice(&0_u16.to_le_bytes());
        buffer[10..12].copy_from_slice(&substitute_bytes.to_le_bytes());
        buffer[12..14].copy_from_slice(&print_offset.to_le_bytes());
        buffer[14..16].copy_from_slice(&print_bytes.to_le_bytes());
        for (index, value) in path_buffer.iter().enumerate() {
            let offset = 16 + index * 2;
            buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }

        let junction_wide = junction
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let handle = unsafe {
            CreateFileW(
                junction_wide.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                0,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            let error = std::io::Error::last_os_error();
            let _ = fs::remove_dir(junction);
            return Err(error);
        }

        let mut returned = 0_u32;
        let result = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_SET_REPARSE_POINT,
                buffer.as_mut_ptr().cast(),
                u32::try_from(buffer.len())
                    .map_err(|_| std::io::Error::other("junction buffer too long"))?,
                null_mut(),
                0,
                &mut returned,
                null_mut(),
            )
        };
        unsafe {
            CloseHandle(handle);
        }
        if result == 0 {
            let error = std::io::Error::last_os_error();
            let _ = fs::remove_dir(junction);
            Err(error)
        } else {
            Ok(())
        }
    }

    #[cfg(windows)]
    #[test]
    fn rejects_a_real_windows_junction_that_points_outside_the_project() {
        let project = TestProject::new("junction");
        let outside = project.path().with_extension("junction-outside");
        fs::create_dir(&outside).expect("outside fixture must create");
        fs::write(outside.join("escape.md"), b"outside").expect("outside fixture must write");
        let junction = project.path().join("chapters/junction");
        create_junction(&junction, &outside).expect("Windows junction fixture must create");

        let index = list_project_files(&project.handle).expect("index must build");
        let mut paths = Vec::new();
        flatten_paths(&index.nodes, &mut paths);
        assert!(!paths.contains(&"chapters/junction".to_owned()));
        let read_error = read_project_file(&project.handle, "chapters/junction/escape.md")
            .expect_err("junction read must fail");
        assert_eq!(read_error.code, "file.not_allowed");
        let save_error = save_project_file(
            &project.handle,
            "chapters/junction/escape.md",
            "overwrite",
            &"a".repeat(64),
        )
        .expect_err("junction save must fail");
        assert_eq!(save_error.code, "file.not_allowed");
        assert_eq!(
            fs::read(outside.join("escape.md")).expect("outside must read"),
            b"outside"
        );

        fs::remove_dir(&junction).expect("junction fixture must remove without following");
        fs::remove_dir_all(&outside).expect("verified outside fixture must remove");
    }

    #[cfg(windows)]
    #[test]
    fn an_actual_windows_replace_failure_keeps_the_old_file() {
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_SHARE_READ: u32 = 0x0000_0001;
        let project = TestProject::new("replace-locked");
        let path = project.path().join("chapters/locked.md");
        fs::write(&path, b"old content").expect("fixture must write");
        let document =
            read_project_file(&project.handle, "chapters/locked.md").expect("file must read");
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .expect("fixture lock must open");

        let error = save_project_file(
            &project.handle,
            "chapters/locked.md",
            "new content",
            &document.content_hash,
        )
        .expect_err("Windows replace must fail while delete sharing is denied");

        assert_eq!(error.code, "file.save_failed");
        assert_eq!(fs::read(&path).expect("target must read"), b"old content");
        drop(locked);
    }
    #[cfg(windows)]
    #[test]
    fn does_not_index_or_read_a_symlink_even_when_windows_allows_its_creation() {
        use std::os::windows::fs::symlink_file;

        let project = TestProject::new("symlink");
        let outside = project.path().with_extension("outside.md");
        fs::write(&outside, "outside").expect("outside fixture must write");
        let link = project.path().join("chapters/link.md");
        if symlink_file(&outside, &link).is_ok() {
            let index = list_project_files(&project.handle).expect("index must build");
            let mut paths = Vec::new();
            flatten_paths(&index.nodes, &mut paths);
            assert!(!paths.contains(&"chapters/link.md".to_owned()));
            let error = read_project_file(&project.handle, "chapters/link.md")
                .expect_err("symlink read must fail");
            assert_eq!(error.code, "file.not_allowed");
            let save_error = save_project_file(
                &project.handle,
                "chapters/link.md",
                "overwrite",
                &"a".repeat(64),
            )
            .expect_err("symlink save must fail");
            assert_eq!(save_error.code, "file.not_allowed");
            assert_eq!(fs::read(&outside).expect("outside must read"), b"outside");
        }
        let _ = fs::remove_file(outside);
    }
}
