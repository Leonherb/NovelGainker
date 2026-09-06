use super::*;
use crate::project::ProjectHandle;
use crate::workspace::{atomic_replace, is_link_or_reparse, read_bounded, sha256_hex};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

// A nullable field is still required on disk: missing evidence is not explicit null.
fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationStatus {
    PreflightPassed,
    Generated,
    ValidationFailed,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicationReceipt {
    pub schema_version: u16,
    pub artifact_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub status: PublicationStatus,
    pub chapter_count: u16,
    pub skipped_drafts: u16,
    pub without_cover: bool,
    #[serde(deserialize_with = "required_nullable")]
    pub relative_path: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub output_hash: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub size_bytes: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    pub issue: Option<PublicationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputReference {
    pub relative_path: String,
    pub content_hash: String,
    pub size_bytes: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub confirmation_id: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub confirmation_revision: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicationDetail {
    pub receipt: PublicationReceipt,
    pub inputs: Vec<InputReference>,
    pub validation_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicationPage {
    pub schema_version: u16,
    pub items: Vec<PublicationReceipt>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    Building,
    ReadyToPromote,
    Terminal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    pub created_at_ms: u64,
    pub project_id: String,
    pub call_id: String,
    pub request_hash: String,
    pub method_receipt_id: String,
    pub source: crate::canon_change::SourceReference,
    pub action: PublicationAction,
    pub stage: Stage,
    pub receipt: PublicationReceipt,
    pub inputs: Vec<InputReference>,
}

fn id(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Record {
    pub fn validate(&self, project_id: &str) -> Result<(), PublicationIssue> {
        let receipt = &self.receipt;
        let output = receipt.relative_path.is_some();
        if self.project_id != project_id
            || self.created_at_ms > 9_007_199_254_740_991
            || !id(project_id)
            || receipt.schema_version != 1
            || ![
                &receipt.artifact_id,
                &receipt.session_id,
                &receipt.turn_id,
                &self.method_receipt_id,
                &self.source.message_id,
            ]
            .into_iter()
            .all(|value| id(value))
            || !hash(&self.source.content_hash)
            || !hash(&self.request_hash)
            || self.call_id.is_empty()
            || self.call_id.len() > 128
            || self.call_id.chars().any(char::is_control)
            || receipt.chapter_count as usize > MAX_PUBLICATION_CHAPTERS
            || receipt.skipped_drafts as usize > MAX_PUBLICATION_CHAPTERS
            || usize::from(receipt.chapter_count) + usize::from(receipt.skipped_drafts)
                > MAX_PUBLICATION_CHAPTERS
            || output != receipt.output_hash.is_some()
            || output != receipt.size_bytes.is_some()
            || receipt
                .relative_path
                .as_ref()
                .is_some_and(|p| p != &format!("published/{}.epub", receipt.artifact_id))
            || receipt.output_hash.as_deref().is_some_and(|h| !hash(h))
            || receipt
                .size_bytes
                .is_some_and(|v| v == 0 || v > MAX_PUBLICATION_OUTPUT)
            || receipt.status == PublicationStatus::Generated
                && (!output || receipt.issue.is_some())
            || receipt.status == PublicationStatus::PreflightPassed
                && (output || receipt.issue.is_some())
            || matches!(
                receipt.status,
                PublicationStatus::Stale | PublicationStatus::ValidationFailed
            ) && receipt.issue.is_none()
            || self.inputs.len() > 701
        {
            return Err(PublicationIssue::StoreUnavailable);
        }
        if (output || receipt.status == PublicationStatus::PreflightPassed)
            && (receipt.chapter_count == 0
                || self
                    .inputs
                    .iter()
                    .filter(|input| input.relative_path == "project.md")
                    .count()
                    != 1
                || self
                    .inputs
                    .iter()
                    .filter(|input| input.confirmation_id.is_some())
                    .count()
                    != usize::from(receipt.chapter_count))
        {
            return Err(PublicationIssue::StoreUnavailable);
        }
        let mut paths = std::collections::BTreeSet::new();
        let mut chapter_count = 0usize;
        let mut chapter_bytes = 0u64;
        let mut asset_count = 0usize;
        let mut asset_bytes = 0u64;
        for input in &self.inputs {
            let is_chapter = input.relative_path.starts_with("chapters/")
                && input.relative_path.split('/').count() == 2
                && !input.relative_path.contains('\\')
                && crate::canon_change::CanonEntityKind::Chapter
                    .validate_path(&input.relative_path)
                    .is_ok();
            let is_asset = input.relative_path.starts_with("assets/")
                && assets::asset_path(&input.relative_path).is_ok();
            if !(input.relative_path == "project.md" || is_chapter || is_asset)
                || !paths.insert(&input.relative_path)
                || !hash(&input.content_hash)
                || input.size_bytes
                    > if is_asset {
                        16 * 1024 * 1024
                    } else {
                        32 * 1024
                    }
                || input.confirmation_id.is_some() != input.confirmation_revision.is_some()
                || input
                    .confirmation_id
                    .as_deref()
                    .is_some_and(|v| !id(v) || !is_chapter)
                || input
                    .confirmation_revision
                    .is_some_and(|v| v == 0 || v > 9_007_199_254_740_991)
            {
                return Err(PublicationIssue::StoreUnavailable);
            }
            if is_chapter {
                chapter_count += 1;
                chapter_bytes += input.size_bytes;
            } else if is_asset {
                asset_count += 1;
                asset_bytes += input.size_bytes;
            }
        }
        if chapter_count > MAX_PUBLICATION_CHAPTERS
            || chapter_bytes > MAX_PUBLICATION_TEXT as u64
            || asset_count > MAX_PUBLICATION_ASSETS
            || asset_bytes > MAX_PUBLICATION_ASSET_BYTES as u64
            || (output || receipt.status == PublicationStatus::PreflightPassed)
                && chapter_count
                    != usize::from(receipt.chapter_count) + usize::from(receipt.skipped_drafts)
        {
            return Err(PublicationIssue::StoreUnavailable);
        }
        Ok(())
    }
}

pub(crate) fn directory(
    handle: &ProjectHandle,
    relative: &str,
    create: bool,
) -> Result<PathBuf, PublicationIssue> {
    let mut path = handle.root().to_path_buf();
    for part in relative.split('/') {
        if part.is_empty() || part == ".." || part.contains(['\\', ':']) {
            return Err(PublicationIssue::UnsafePath);
        }
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
            Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&path).map_err(|_| PublicationIssue::StoreUnavailable)?;
            }
            _ => return Err(PublicationIssue::UnsafePath),
        }
        let canonical = path
            .canonicalize()
            .map_err(|_| PublicationIssue::UnsafePath)?;
        if !canonical.starts_with(handle.root()) {
            return Err(PublicationIssue::UnsafePath);
        }
    }
    Ok(path)
}

pub(crate) fn target(handle: &ProjectHandle, relative: &str) -> Result<PathBuf, PublicationIssue> {
    if relative.len() > 512
        || relative.contains(['\\', ':'])
        || relative.chars().any(char::is_control)
        || relative.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.trim() != part
                || part.ends_with('.')
                || assets::reserved(part)
        })
    {
        return Err(PublicationIssue::UnsafePath);
    }
    let (parent, name) = relative.rsplit_once('/').unwrap_or(("", relative));
    let parent = if parent.is_empty() {
        handle.root().to_path_buf()
    } else {
        directory(handle, parent, false)?
    };
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => {
            if !path
                .canonicalize()
                .map_err(|_| PublicationIssue::UnsafePath)?
                .starts_with(handle.root())
            {
                return Err(PublicationIssue::UnsafePath);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(PublicationIssue::UnsafePath),
    }
    Ok(path)
}

pub(crate) fn chapter_paths(handle: &ProjectHandle) -> Result<Vec<String>, PublicationIssue> {
    let dir = directory(handle, "chapters", false)?;
    let mut paths = Vec::new();
    for (index, entry) in fs::read_dir(dir)
        .map_err(|_| PublicationIssue::InvalidChapter)?
        .enumerate()
    {
        if index >= 1000 {
            return Err(PublicationIssue::LimitExceeded);
        }
        let entry = entry.map_err(|_| PublicationIssue::InvalidChapter)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| PublicationIssue::UnsafePath)?;
        if is_link_or_reparse(&metadata) || !metadata.is_file() {
            return Err(PublicationIssue::UnsafePath);
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| PublicationIssue::UnsafePath)?;
        if !name.to_ascii_lowercase().ends_with(".md") {
            continue;
        }
        let path = format!("chapters/{name}");
        crate::canon_change::CanonEntityKind::Chapter
            .validate_path(&path)
            .map_err(|_| PublicationIssue::InvalidChapter)?;
        paths.push(path);
        if paths.len() > MAX_PUBLICATION_CHAPTERS {
            return Err(PublicationIssue::LimitExceeded);
        }
    }
    paths.sort();
    Ok(paths)
}

pub(crate) fn load_all(handle: &ProjectHandle) -> Result<Vec<Record>, PublicationIssue> {
    let dir = directory(handle, ".novelgainker/publications", true)?;
    let mut records = Vec::new();
    for (index, entry) in fs::read_dir(dir)
        .map_err(|_| PublicationIssue::StoreUnavailable)?
        .enumerate()
    {
        if index >= MAX_PUBLICATION_RECORDS * 3 {
            return Err(PublicationIssue::LimitExceeded);
        }
        let entry = entry.map_err(|_| PublicationIssue::StoreUnavailable)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| PublicationIssue::StoreUnavailable)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| PublicationIssue::StoreUnavailable)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(PublicationIssue::UnsafePath);
        }
        if name.starts_with(".receipt-") && name.ends_with(".tmp") {
            continue;
        }
        let artifact = name
            .strip_suffix(".json")
            .filter(|value| id(value))
            .ok_or(PublicationIssue::StoreUnavailable)?;
        let record: Record = serde_json::from_slice(
            &read_bounded(&entry.path(), MAX_PUBLICATION_RECORD_BYTES)
                .map_err(|_| PublicationIssue::StoreUnavailable)?,
        )
        .map_err(|_| PublicationIssue::StoreUnavailable)?;
        record.validate(handle.project_id())?;
        if artifact != record.receipt.artifact_id {
            return Err(PublicationIssue::StoreUnavailable);
        }
        records.push(record);
        if records.len() > MAX_PUBLICATION_RECORDS {
            return Err(PublicationIssue::LimitExceeded);
        }
    }
    records.sort_by(|a, b| {
        b.created_at_ms
            .cmp(&a.created_at_ms)
            .then_with(|| a.receipt.artifact_id.cmp(&b.receipt.artifact_id))
    });
    Ok(records)
}

pub(crate) fn capacity(handle: &ProjectHandle, extra: u64) -> Result<(), PublicationIssue> {
    let mut total = extra;
    for relative in [".novelgainker/publications", "published"] {
        let dir = directory(handle, relative, true)?;
        for (index, entry) in fs::read_dir(dir)
            .map_err(|_| PublicationIssue::StoreUnavailable)?
            .enumerate()
        {
            if index >= MAX_PUBLICATION_RECORDS * 3 {
                return Err(PublicationIssue::LimitExceeded);
            }
            let entry = entry.map_err(|_| PublicationIssue::StoreUnavailable)?;
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| PublicationIssue::StoreUnavailable)?;
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(PublicationIssue::UnsafePath);
            }
            total = total
                .checked_add(metadata.len())
                .ok_or(PublicationIssue::LimitExceeded)?;
            if total > MAX_PUBLICATION_STORE_BYTES {
                return Err(PublicationIssue::LimitExceeded);
            }
        }
    }
    Ok(())
}

pub(crate) fn save(handle: &ProjectHandle, record: &Record) -> Result<(), PublicationIssue> {
    record.validate(handle.project_id())?;
    let bytes = serde_json::to_vec(record).map_err(|_| PublicationIssue::StoreUnavailable)?;
    if bytes.len() as u64 > MAX_PUBLICATION_RECORD_BYTES {
        return Err(PublicationIssue::LimitExceeded);
    }
    capacity(handle, 3 * MAX_PUBLICATION_RECORD_BYTES)?;
    let directory = directory(handle, ".novelgainker/publications", true)?;
    let relative = format!(
        ".novelgainker/publications/{}.json",
        record.receipt.artifact_id
    );
    let target_path = target(handle, &relative)?;
    let temporary = directory.join(format!(".receipt-{}.tmp", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| PublicationIssue::StoreUnavailable)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| PublicationIssue::StoreUnavailable)?;
    drop(file);
    if target(handle, &relative)? != target_path {
        return Err(PublicationIssue::UnsafePath);
    }
    atomic_replace(&temporary, &target_path).map_err(|_| PublicationIssue::StoreUnavailable)
}

pub(crate) fn file_hash(
    handle: &ProjectHandle,
    relative: &str,
    limit: u64,
) -> Result<(String, u64), PublicationIssue> {
    use sha2::{Digest, Sha256};
    let path = target(handle, relative)?;
    let mut file = fs::File::open(&path).map_err(|_| PublicationIssue::SourceChanged)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| PublicationIssue::SourceChanged)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > limit {
            return Err(PublicationIssue::LimitExceeded);
        }
        hash.update(&buffer[..n]);
    }
    if target(handle, relative)? != path {
        return Err(PublicationIssue::UnsafePath);
    }
    Ok((format!("{:x}", hash.finalize()), total))
}

#[cfg(windows)]
pub(crate) fn promote(temporary: &Path, target: &Path) -> Result<(), PublicationIssue> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
    let from = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let to = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // Deliberately no MOVEFILE_REPLACE_EXISTING: even a UUID collision must not overwrite.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
        return Err(PublicationIssue::StoreUnavailable);
    }
    Ok(())
}
#[cfg(not(windows))]
pub(crate) fn promote(temporary: &Path, target: &Path) -> Result<(), PublicationIssue> {
    fs::hard_link(temporary, target).map_err(|_| PublicationIssue::StoreUnavailable)?;
    fs::remove_file(temporary).map_err(|_| PublicationIssue::StoreUnavailable)
}

pub(crate) fn page(
    records: &[Record],
    cursor: Option<&str>,
) -> Result<PublicationPage, PublicationIssue> {
    let fingerprint = sha256_hex(
        &serde_json::to_vec(&records.iter().map(|r| &r.receipt).collect::<Vec<_>>())
            .map_err(|_| PublicationIssue::StoreUnavailable)?,
    );
    let start = if let Some(cursor) = cursor {
        if cursor.len() > 72 {
            return Err(PublicationIssue::SourceChanged);
        }
        let (hash, index) = cursor
            .split_once(':')
            .ok_or(PublicationIssue::SourceChanged)?;
        if hash != fingerprint {
            return Err(PublicationIssue::SourceChanged);
        }
        index
            .parse::<usize>()
            .map_err(|_| PublicationIssue::SourceChanged)?
    } else {
        0
    };
    if start > records.len() {
        return Err(PublicationIssue::SourceChanged);
    }
    let end = (start + 20).min(records.len());
    Ok(PublicationPage {
        schema_version: 1,
        items: records[start..end]
            .iter()
            .map(|r| r.receipt.clone())
            .collect(),
        next_cursor: (end < records.len()).then(|| format!("{fingerprint}:{end}")),
    })
}
