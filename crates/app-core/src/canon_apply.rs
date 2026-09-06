use crate::project::ProjectHandle;
use crate::recovery::{RecoveryStatus, RecoveryStore};
use crate::workspace::{
    is_link_or_reparse, read_bounded, resolve_scoped_target, sha256_hex, DiagnosticState,
    LineEnding, ProjectFileSaveResult, Utf8Bom, MAX_EDITABLE_FILE_BYTES,
};
use crate::AppError;
use novelgainker_validation::validate_source_bytes;
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterPatchOperationV1 {
    Create,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonFileOperationKind {
    Create,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanonApplyStage {
    Write,
    Flush,
    Commit,
    PostWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterFileApplyResult {
    pub transaction_id: String,
    pub file: ProjectFileSaveResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChapterApplyStage {
    Write,
    TemporaryValidation,
    TargetRecheck,
    Backup,
    Commit,
    PostWrite,
}

pub fn encode_chapter_target_bytes(
    normalized_content: &str,
    bom: Utf8Bom,
    line_ending: LineEnding,
) -> Result<Vec<u8>, AppError> {
    if normalized_content.contains('\r') || !normalized_content.ends_with('\n') {
        return Err(AppError::file_save_failed().with_diagnostic("chapter-encode-invalid-content"));
    }
    if matches!(line_ending, LineEnding::Mixed | LineEnding::None) {
        return Err(AppError::file_save_failed().with_diagnostic("chapter-encode-line-ending"));
    }
    let body = if line_ending == LineEnding::CrLf {
        normalized_content.replace('\n', "\r\n")
    } else {
        normalized_content.to_owned()
    };
    let mut bytes = Vec::with_capacity(body.len() + usize::from(bom == Utf8Bom::Utf8) * 3);
    if bom == Utf8Bom::Utf8 {
        bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
    }
    bytes.extend_from_slice(body.as_bytes());
    Ok(bytes)
}

pub fn apply_chapter_file_recoverable(
    handle: &ProjectHandle,
    operation: ChapterPatchOperationV1,
    relative_path: &str,
    expected_source_hash: Option<&str>,
    proposed_bytes: &[u8],
    recovery_store: &RecoveryStore,
) -> Result<ChapterFileApplyResult, AppError> {
    apply_chapter_file_with_hook(
        handle,
        operation,
        relative_path,
        expected_source_hash,
        proposed_bytes,
        recovery_store,
        |_| Ok(()),
    )
}

fn apply_chapter_file_with_hook<F>(
    handle: &ProjectHandle,
    operation: ChapterPatchOperationV1,
    relative_path: &str,
    expected_source_hash: Option<&str>,
    proposed_bytes: &[u8],
    recovery_store: &RecoveryStore,
    mut hook: F,
) -> Result<ChapterFileApplyResult, AppError>
where
    F: FnMut(ChapterApplyStage) -> Result<(), AppError>,
{
    if proposed_bytes.is_empty()
        || u64::try_from(proposed_bytes.len()).unwrap_or(u64::MAX) > MAX_EDITABLE_FILE_BYTES
        || std::str::from_utf8(
            proposed_bytes
                .strip_prefix(&[0xef, 0xbb, 0xbf])
                .unwrap_or(proposed_bytes),
        )
        .is_err()
    {
        return Err(AppError::file_save_failed().with_diagnostic("chapter-apply-invalid-bytes"));
    }
    let validation = validate_source_bytes(relative_path, proposed_bytes);
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(AppError::file_save_failed().with_diagnostic("chapter-apply-validation-failed"));
    }
    let replacement_hash = sha256_hex(proposed_bytes);
    if validation.content_hash != replacement_hash {
        return Err(AppError::project_recovery_required());
    }
    let canon_operation = match operation {
        ChapterPatchOperationV1::Create => CanonFileOperationKind::Create,
        ChapterPatchOperationV1::Replace => CanonFileOperationKind::Replace,
    };
    let expected_hash = match operation {
        ChapterPatchOperationV1::Create if expected_source_hash.is_none() => "absent",
        ChapterPatchOperationV1::Replace => expected_source_hash
            .filter(|value| is_sha256(value))
            .ok_or_else(AppError::file_invalid_source_hash)?,
        ChapterPatchOperationV1::Create => return Err(AppError::file_invalid_source_hash()),
    };
    let target_path = resolve_scoped_target(handle.root(), relative_path)?;
    verify_target_state(&target_path, canon_operation, expected_hash)?;
    let temporary_file_id = temporary_file_id(&target_path)?;
    let backup_file_id = (operation == ChapterPatchOperationV1::Replace)
        .then(|| backup_file_id(&target_path))
        .transpose()?;
    let mut recovery_transaction = Some(match operation {
        ChapterPatchOperationV1::Create => recovery_store.begin_chapter_create_transaction(
            handle,
            relative_path,
            &replacement_hash,
            &temporary_file_id,
        )?,
        ChapterPatchOperationV1::Replace => recovery_store.begin_chapter_replace_transaction(
            handle,
            relative_path,
            expected_hash,
            &replacement_hash,
            &temporary_file_id,
            backup_file_id
                .as_deref()
                .ok_or_else(AppError::project_recovery_required)?,
        )?,
    });
    let transaction_id = recovery_transaction
        .as_ref()
        .ok_or_else(AppError::project_recovery_required)?
        .transaction_id()
        .to_owned();

    let outcome = (|| {
        let (mut temporary_file, mut temporary_guard) =
            create_temporary_file(&target_path, &temporary_file_id)?;
        hook(ChapterApplyStage::Write)?;
        temporary_file
            .write_all(proposed_bytes)
            .map_err(|_| apply_io("chapter-apply-write-failed"))?;
        temporary_file
            .flush()
            .map_err(|_| apply_io("chapter-apply-flush-failed"))?;
        temporary_file
            .sync_all()
            .map_err(|_| apply_io("chapter-apply-sync-failed"))?;
        drop(temporary_file);
        recovery_transaction
            .as_mut()
            .ok_or_else(AppError::project_recovery_required)?
            .mark_temporary_synced()?;

        hook(ChapterApplyStage::TemporaryValidation)?;
        let temporary_bytes = read_bounded(temporary_guard.path(), MAX_EDITABLE_FILE_BYTES)?;
        let temporary_validation = validate_source_bytes(relative_path, &temporary_bytes);
        if temporary_bytes != proposed_bytes
            || temporary_validation.content_hash != replacement_hash
            || !temporary_validation.eligible_for_ai_apply
            || !temporary_validation.diagnostics.is_empty()
        {
            return Err(AppError::file_save_failed()
                .with_diagnostic("chapter-apply-temp-validation-failed"));
        }

        hook(ChapterApplyStage::TargetRecheck)?;
        let revalidated_target = resolve_scoped_target(handle.root(), relative_path)?;
        if revalidated_target != target_path {
            return Err(AppError::file_source_changed());
        }
        verify_target_state(&revalidated_target, canon_operation, expected_hash)?;

        if let Some(backup_file_id) = backup_file_id.as_deref() {
            hook(ChapterApplyStage::Backup)?;
            let backup_path = target_path
                .parent()
                .ok_or_else(AppError::file_invalid_path)?
                .join(backup_file_id);
            fs::hard_link(&target_path, &backup_path)
                .map_err(|_| apply_io("chapter-apply-backup-create-failed"))?;
            let backup_file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&backup_path)
                .map_err(|_| apply_io("chapter-apply-backup-open-failed"))?;
            backup_file
                .sync_all()
                .map_err(|_| apply_io("chapter-apply-backup-sync-failed"))?;
            let backup_bytes = read_bounded(&backup_path, MAX_EDITABLE_FILE_BYTES)?;
            if sha256_hex(&backup_bytes) != expected_hash {
                return Err(AppError::project_recovery_required());
            }
        }
        recovery_transaction
            .as_mut()
            .ok_or_else(AppError::project_recovery_required)?
            .mark_replacing()?;
        hook(ChapterApplyStage::Commit)?;
        match operation {
            ChapterPatchOperationV1::Create => {
                atomic_create(temporary_guard.path(), &target_path)?;
                let _ = fs::remove_file(temporary_guard.path());
            }
            ChapterPatchOperationV1::Replace => {
                atomic_replace(temporary_guard.path(), &target_path)?;
            }
        }
        temporary_guard.disarm();
        hook(ChapterApplyStage::PostWrite)?;

        let committed = read_bounded(&target_path, MAX_EDITABLE_FILE_BYTES)
            .map_err(|_| AppError::project_recovery_required())?;
        if committed != proposed_bytes || sha256_hex(&committed) != replacement_hash {
            return Err(AppError::project_recovery_required());
        }
        let post_validation = validate_source_bytes(relative_path, &committed);
        if post_validation.content_hash != replacement_hash
            || !post_validation.eligible_for_ai_apply
            || !post_validation.diagnostics.is_empty()
        {
            return Err(AppError::project_recovery_required());
        }
        let metadata =
            fs::metadata(&target_path).map_err(|_| AppError::project_recovery_required())?;
        let (bom, line_ending) = chapter_encoding(&committed);
        Ok(ProjectFileSaveResult {
            relative_path: relative_path.to_owned(),
            content_hash: replacement_hash,
            modified_time_ms: modified_time_ms(&metadata),
            size_bytes: u64::try_from(committed.len()).unwrap_or(u64::MAX),
            bom,
            line_ending,
            diagnostics_triggered: true,
            diagnostic_state: DiagnosticState::Ready,
            validation: post_validation,
            saved_content: None,
            chapter_finalization_confirmed: None,
        })
    })();

    match outcome {
        Ok(file) => {
            let commit = recovery_transaction
                .take()
                .ok_or_else(AppError::project_recovery_required)?
                .commit_chapter(&target_path);
            if commit.is_err() {
                let recovered = recovery_store.recover_project(handle);
                if recovered.status == RecoveryStatus::Required {
                    return Err(AppError::project_recovery_required());
                }
                let target_hash = read_bounded(&target_path, MAX_EDITABLE_FILE_BYTES)
                    .ok()
                    .map(|bytes| sha256_hex(&bytes));
                if target_hash.as_deref() != Some(file.content_hash.as_str()) {
                    return Err(AppError::file_save_failed()
                        .with_diagnostic("chapter-apply-commit-failed-and-rolled-back"));
                }
            }
            Ok(ChapterFileApplyResult {
                transaction_id,
                file,
            })
        }
        Err(error) => {
            drop(recovery_transaction.take());
            let recovered = recovery_store.recover_project(handle);
            if recovered.status == RecoveryStatus::Required {
                Err(AppError::project_recovery_required())
            } else if error.code == "project.recovery_required" {
                Err(AppError::file_save_failed()
                    .with_diagnostic("chapter-apply-failed-and-rolled-back"))
            } else {
                Err(error)
            }
        }
    }
}

fn backup_file_id(target: &Path) -> Result<String, AppError> {
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(AppError::file_invalid_path)?;
    Ok(format!(".{name}.novelgainker-{}.bak", Uuid::new_v4()))
}

fn chapter_encoding(bytes: &[u8]) -> (Utf8Bom, LineEnding) {
    let (body, bom) = if let Some(body) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        (body, Utf8Bom::Utf8)
    } else {
        (bytes, Utf8Bom::None)
    };
    let line_ending = if body.windows(2).any(|window| window == b"\r\n") {
        LineEnding::CrLf
    } else if body.contains(&b'\n') {
        LineEnding::Lf
    } else {
        LineEnding::None
    };
    (bom, line_ending)
}

pub fn apply_canon_file_recoverable(
    handle: &ProjectHandle,
    operation: CanonFileOperationKind,
    relative_path: &str,
    expected_source_hash: &str,
    proposed_bytes: &[u8],
    recovery_store: &RecoveryStore,
) -> Result<ProjectFileSaveResult, AppError> {
    apply_canon_file_with_hook(
        handle,
        operation,
        relative_path,
        expected_source_hash,
        proposed_bytes,
        recovery_store,
        |_| Ok(()),
    )
}

fn apply_canon_file_with_hook<F>(
    handle: &ProjectHandle,
    operation: CanonFileOperationKind,
    relative_path: &str,
    expected_source_hash: &str,
    proposed_bytes: &[u8],
    recovery_store: &RecoveryStore,
    mut hook: F,
) -> Result<ProjectFileSaveResult, AppError>
where
    F: FnMut(CanonApplyStage) -> Result<(), AppError>,
{
    if proposed_bytes.is_empty()
        || u64::try_from(proposed_bytes.len()).unwrap_or(u64::MAX) > MAX_EDITABLE_FILE_BYTES
        || proposed_bytes.starts_with(&[0xef, 0xbb, 0xbf])
        || proposed_bytes.contains(&b'\r')
        || !proposed_bytes.ends_with(b"\n")
        || std::str::from_utf8(proposed_bytes).is_err()
    {
        return Err(AppError::file_save_failed().with_diagnostic("canon-apply-invalid-bytes"));
    }
    let validation = validate_source_bytes(relative_path, proposed_bytes);
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(AppError::file_save_failed().with_diagnostic("canon-apply-validation-failed"));
    }
    let replacement_hash = sha256_hex(proposed_bytes);
    if validation.content_hash != replacement_hash {
        return Err(AppError::project_recovery_required());
    }

    let target_path = resolve_scoped_target(handle.root(), relative_path)?;
    verify_target_state(&target_path, operation, expected_source_hash)?;
    let temporary_file_id = temporary_file_id(&target_path)?;
    let mut recovery_transaction = Some(match operation {
        CanonFileOperationKind::Create => recovery_store.begin_create_transaction(
            handle,
            relative_path,
            &replacement_hash,
            &temporary_file_id,
        )?,
        CanonFileOperationKind::Replace => recovery_store.begin_transaction(
            handle,
            relative_path,
            expected_source_hash,
            &replacement_hash,
            &temporary_file_id,
        )?,
    });

    let outcome = (|| {
        let (mut temporary_file, mut temporary_guard) =
            create_temporary_file(&target_path, &temporary_file_id)?;
        hook(CanonApplyStage::Write)?;
        temporary_file
            .write_all(proposed_bytes)
            .map_err(|_| apply_io("canon-apply-write-failed"))?;
        hook(CanonApplyStage::Flush)?;
        temporary_file
            .flush()
            .map_err(|_| apply_io("canon-apply-flush-failed"))?;
        temporary_file
            .sync_all()
            .map_err(|_| apply_io("canon-apply-sync-failed"))?;
        drop(temporary_file);
        recovery_transaction
            .as_mut()
            .ok_or_else(AppError::project_recovery_required)?
            .mark_temporary_synced()?;

        let revalidated_target = resolve_scoped_target(handle.root(), relative_path)?;
        if revalidated_target != target_path {
            return Err(AppError::file_source_changed());
        }
        verify_target_state(&revalidated_target, operation, expected_source_hash)?;
        recovery_transaction
            .as_mut()
            .ok_or_else(AppError::project_recovery_required)?
            .mark_replacing()?;
        hook(CanonApplyStage::Commit)?;
        match operation {
            CanonFileOperationKind::Create => {
                atomic_create(temporary_guard.path(), &target_path)?;
                let _ = fs::remove_file(temporary_guard.path());
            }
            CanonFileOperationKind::Replace => {
                atomic_replace(temporary_guard.path(), &target_path)?;
            }
        }
        temporary_guard.disarm();
        hook(CanonApplyStage::PostWrite)?;

        let committed = read_bounded(&target_path, MAX_EDITABLE_FILE_BYTES)
            .map_err(|_| AppError::project_recovery_required())?;
        if committed != proposed_bytes || sha256_hex(&committed) != replacement_hash {
            return Err(AppError::project_recovery_required());
        }
        let post_validation = validate_source_bytes(relative_path, &committed);
        if post_validation.content_hash != replacement_hash
            || !post_validation.eligible_for_ai_apply
            || !post_validation.diagnostics.is_empty()
        {
            return Err(AppError::project_recovery_required());
        }
        let metadata =
            fs::metadata(&target_path).map_err(|_| AppError::project_recovery_required())?;
        Ok(ProjectFileSaveResult {
            relative_path: relative_path.to_owned(),
            content_hash: replacement_hash,
            modified_time_ms: modified_time_ms(&metadata),
            size_bytes: u64::try_from(committed.len()).unwrap_or(u64::MAX),
            bom: Utf8Bom::None,
            line_ending: LineEnding::Lf,
            diagnostics_triggered: true,
            diagnostic_state: DiagnosticState::Ready,
            validation: post_validation,
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
        Err(error) if error.code == "project.recovery_required" => {
            drop(recovery_transaction.take());
            Err(error)
        }
        Err(error) => {
            if let Some(transaction) = recovery_transaction.take() {
                transaction.abort_best_effort();
            }
            Err(error)
        }
    }
}

fn verify_target_state(
    target: &Path,
    operation: CanonFileOperationKind,
    expected_source_hash: &str,
) -> Result<(), AppError> {
    match operation {
        CanonFileOperationKind::Create => {
            if expected_source_hash != "absent" {
                return Err(AppError::file_invalid_source_hash());
            }
            match fs::symlink_metadata(target) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Ok(_) => Err(AppError::file_source_changed()),
                Err(_) => Err(AppError::file_io()),
            }
        }
        CanonFileOperationKind::Replace => {
            if !is_sha256(expected_source_hash) {
                return Err(AppError::file_invalid_source_hash());
            }
            let metadata = fs::symlink_metadata(target).map_err(|_| AppError::file_not_found())?;
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(AppError::file_not_allowed());
            }
            let bytes = read_bounded(target, MAX_EDITABLE_FILE_BYTES)?;
            if sha256_hex(&bytes) != expected_source_hash {
                Err(AppError::file_source_changed())
            } else {
                Ok(())
            }
        }
    }
}

struct TemporaryGuard {
    path: PathBuf,
    parent: PathBuf,
    prefix: String,
    armed: bool,
}

impl TemporaryGuard {
    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TemporaryGuard {
    fn drop(&mut self) {
        if self.armed
            && self.path.parent() == Some(self.parent.as_path())
            && self
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&self.prefix) && name.ends_with(".tmp"))
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn temporary_file_id(target: &Path) -> Result<String, AppError> {
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(AppError::file_invalid_path)?;
    Ok(format!(".{name}.novelgainker-{}.tmp", Uuid::new_v4()))
}

fn create_temporary_file(
    target: &Path,
    temporary_file_id: &str,
) -> Result<(File, TemporaryGuard), AppError> {
    let parent = target
        .parent()
        .ok_or_else(AppError::file_invalid_path)?
        .to_path_buf();
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(AppError::file_invalid_path)?;
    let prefix = format!(".{name}.novelgainker-");
    if !temporary_file_id.starts_with(&prefix)
        || !temporary_file_id.ends_with(".tmp")
        || temporary_file_id.contains(['/', '\\'])
    {
        return Err(AppError::file_invalid_path());
    }
    let path = parent.join(temporary_file_id);
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| apply_io("canon-apply-temp-create-failed"))?;
    Ok((
        file,
        TemporaryGuard {
            path,
            parent,
            prefix,
            armed: true,
        },
    ))
}

fn atomic_create(temporary: &Path, target: &Path) -> Result<(), AppError> {
    fs::hard_link(temporary, target).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            AppError::file_source_changed()
        } else {
            apply_io("canon-apply-atomic-create-failed")
        }
    })
}

#[cfg(windows)]
fn atomic_replace(temporary: &Path, target: &Path) -> Result<(), AppError> {
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
        Err(apply_io("canon-apply-atomic-replace-failed"))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(temporary: &Path, target: &Path) -> Result<(), AppError> {
    fs::rename(temporary, target).map_err(|_| apply_io("canon-apply-atomic-replace-failed"))
}

fn modified_time_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .and_then(|value| u64::try_from(value.as_millis()).ok())
        .unwrap_or(0)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn apply_io(diagnostic: &'static str) -> AppError {
    AppError::file_save_failed().with_diagnostic(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::open_project;
    use crate::project::{create_project, inspect_project_root, ProjectScanMode};
    use crate::recovery::RecoveryStatus;
    use std::process::Command;

    struct Fixture {
        root: PathBuf,
        app_state: PathBuf,
        handle: ProjectHandle,
        recovery: RecoveryStore,
    }

    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("novelgainker-canon-apply-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
            let handle = create_project(&inspection).unwrap();
            let app_state = root.with_extension("state");
            let recovery = RecoveryStore::new(app_state.clone()).unwrap();
            Self {
                root,
                app_state,
                handle,
                recovery,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.app_state).unwrap();
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn markdown(title: &str) -> Vec<u8> {
        format!("---\ntitle: {title}\nwritingLanguage: zh-CN\n---\n\n# {title}\n").into_bytes()
    }

    #[test]
    fn create_is_exact_non_overwriting_and_replace_requires_current_source_hash() {
        let fixture = Fixture::new();
        let created = markdown("林遥");
        let result = apply_canon_file_recoverable(
            &fixture.handle,
            CanonFileOperationKind::Create,
            "characters/林遥.md",
            "absent",
            &created,
            &fixture.recovery,
        )
        .unwrap();
        assert_eq!(
            fs::read(fixture.root.join("characters/林遥.md")).unwrap(),
            created
        );
        assert_eq!(result.content_hash, sha256_hex(&created));
        let collision = apply_canon_file_recoverable(
            &fixture.handle,
            CanonFileOperationKind::Create,
            "characters/林遥.md",
            "absent",
            &markdown("覆盖"),
            &fixture.recovery,
        )
        .unwrap_err();
        assert_eq!(collision.code, "file.source_changed");
        assert_eq!(
            fs::read(fixture.root.join("characters/林遥.md")).unwrap(),
            created
        );

        let replacement = markdown("新林遥");
        let stale = apply_canon_file_recoverable(
            &fixture.handle,
            CanonFileOperationKind::Replace,
            "characters/林遥.md",
            &"0".repeat(64),
            &replacement,
            &fixture.recovery,
        )
        .unwrap_err();
        assert_eq!(stale.code, "file.source_changed");
        let replaced = apply_canon_file_recoverable(
            &fixture.handle,
            CanonFileOperationKind::Replace,
            "characters/林遥.md",
            &sha256_hex(&created),
            &replacement,
            &fixture.recovery,
        )
        .unwrap();
        assert_eq!(replaced.content_hash, sha256_hex(&replacement));
        assert_eq!(
            fs::read(fixture.root.join("characters/林遥.md")).unwrap(),
            replacement
        );
    }

    #[test]
    fn fault_before_commit_rolls_back_and_post_write_ambiguity_requires_recovery() {
        let fixture = Fixture::new();
        let proposed = markdown("故障人物");
        let before = apply_canon_file_with_hook(
            &fixture.handle,
            CanonFileOperationKind::Create,
            "characters/故障人物.md",
            "absent",
            &proposed,
            &fixture.recovery,
            |stage| {
                if stage == CanonApplyStage::Commit {
                    Err(AppError::file_save_failed())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(before.code, "file.save_failed");
        assert!(!fixture.root.join("characters/故障人物.md").exists());
        assert_eq!(
            fixture.recovery.recover_project(&fixture.handle).status,
            RecoveryStatus::Ready
        );

        let after = apply_canon_file_with_hook(
            &fixture.handle,
            CanonFileOperationKind::Create,
            "characters/故障人物.md",
            "absent",
            &proposed,
            &fixture.recovery,
            |stage| {
                if stage == CanonApplyStage::PostWrite {
                    Err(AppError::project_recovery_required())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(after.code, "project.recovery_required");
        assert_eq!(
            fs::read(fixture.root.join("characters/故障人物.md")).unwrap(),
            proposed
        );
        assert_eq!(
            fixture.recovery.recover_project(&fixture.handle).status,
            RecoveryStatus::Recovered
        );
    }

    fn chapter_markdown(title: &str, body: &str, status: &str) -> String {
        format!(
            "---\nid: chapter_test\ntitle: {title}\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 3\nstatus: {status}\n---\n\n# {title}\n\n## Draft\n\n{body}\n"
        )
    }

    #[test]
    fn ng_505_chapter_apply_creates_exactly_once_and_preserves_source_encoding_on_replace() {
        let fixture = Fixture::new();
        let created = chapter_markdown("潮痕", "初稿。", "draft");
        let created_bytes =
            encode_chapter_target_bytes(&created, Utf8Bom::None, LineEnding::Lf).unwrap();
        let result = apply_chapter_file_recoverable(
            &fixture.handle,
            ChapterPatchOperationV1::Create,
            "chapters/003.md",
            None,
            &created_bytes,
            &fixture.recovery,
        )
        .unwrap();
        assert!(Uuid::parse_str(&result.transaction_id).is_ok());
        assert_eq!(result.file.content_hash, sha256_hex(&created_bytes));
        assert_eq!(
            fs::read(fixture.root.join("chapters/003.md")).unwrap(),
            created_bytes
        );
        let collision = apply_chapter_file_recoverable(
            &fixture.handle,
            ChapterPatchOperationV1::Create,
            "chapters/003.md",
            None,
            &created_bytes,
            &fixture.recovery,
        )
        .unwrap_err();
        assert_eq!(collision.code, "file.source_changed");

        let source = chapter_markdown("潮痕", "旧稿。", "draft");
        let source_bytes =
            encode_chapter_target_bytes(&source, Utf8Bom::Utf8, LineEnding::CrLf).unwrap();
        fs::write(fixture.root.join("chapters/003.md"), &source_bytes).unwrap();
        let revised = chapter_markdown("潮痕", "新稿。", "draft");
        let revised_bytes =
            encode_chapter_target_bytes(&revised, Utf8Bom::Utf8, LineEnding::CrLf).unwrap();
        let result = apply_chapter_file_recoverable(
            &fixture.handle,
            ChapterPatchOperationV1::Replace,
            "chapters/003.md",
            Some(&sha256_hex(&source_bytes)),
            &revised_bytes,
            &fixture.recovery,
        )
        .unwrap();
        assert_eq!(result.file.bom, Utf8Bom::Utf8);
        assert_eq!(result.file.line_ending, LineEnding::CrLf);
        assert_eq!(
            fs::read(fixture.root.join("chapters/003.md")).unwrap(),
            revised_bytes
        );
    }

    #[test]
    fn ng_505_every_precommit_or_postwrite_failure_restores_the_original_chapter() {
        for failure_stage in [ChapterApplyStage::Commit, ChapterApplyStage::PostWrite] {
            let fixture = Fixture::new();
            let target = fixture.root.join("chapters/003.md");
            let source = chapter_markdown("潮痕", "不可丢失的旧稿。", "draft").into_bytes();
            let revised = chapter_markdown("潮痕", "尚未确认的新稿。", "draft").into_bytes();
            fs::write(&target, &source).unwrap();
            let failure = apply_chapter_file_with_hook(
                &fixture.handle,
                ChapterPatchOperationV1::Replace,
                "chapters/003.md",
                Some(&sha256_hex(&source)),
                &revised,
                &fixture.recovery,
                |stage| {
                    if stage == failure_stage {
                        Err(AppError::file_save_failed().with_diagnostic("injected-ng505-failure"))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            assert_eq!(failure.code, "file.save_failed");
            assert_eq!(fs::read(&target).unwrap(), source);
            assert_eq!(
                fixture.recovery.recover_project(&fixture.handle).status,
                RecoveryStatus::Ready
            );
            assert!(fs::read_dir(target.parent().unwrap())
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".novelgainker-")));
        }
    }

    const NG306_ROOT_ENV: &str = "NOVELGAINKER_NG306_CANON_ROOT";
    const NG306_STATE_ENV: &str = "NOVELGAINKER_NG306_STATE_ROOT";
    const NG306_STAGE_ENV: &str = "NOVELGAINKER_NG306_CRASH_STAGE";
    const NG306_CRASH_EXIT_CODE: i32 = 86;

    #[test]
    #[ignore = "subprocess fixture invoked by the NG-306 applying interruption test"]
    fn ng_306_canon_apply_crash_fixture() {
        let root = PathBuf::from(std::env::var_os(NG306_ROOT_ENV).unwrap());
        let state_root = PathBuf::from(std::env::var_os(NG306_STATE_ENV).unwrap());
        let crash_stage = std::env::var(NG306_STAGE_ENV).unwrap();
        assert!(matches!(crash_stage.as_str(), "commit" | "post-write"));
        let inspection = inspect_project_root(&root, ProjectScanMode::Open).unwrap();
        let handle = open_project(&inspection, false).unwrap();
        let recovery = RecoveryStore::new(state_root).unwrap();
        let proposed = markdown("中断人物");
        let result = apply_canon_file_with_hook(
            &handle,
            CanonFileOperationKind::Create,
            "characters/中断人物.md",
            "absent",
            &proposed,
            &recovery,
            |stage| {
                let should_exit = matches!(
                    (crash_stage.as_str(), stage),
                    ("commit", CanonApplyStage::Commit)
                        | ("post-write", CanonApplyStage::PostWrite)
                );
                if should_exit {
                    std::process::exit(NG306_CRASH_EXIT_CODE);
                }
                Ok(())
            },
        );
        panic!("NG-306 crash fixture unexpectedly returned: {result:?}");
    }

    #[test]
    fn ng_306_applying_subprocess_interrupt_is_rolled_back_or_completed_on_reopen() {
        for crash_stage in ["commit", "post-write"] {
            let fixture = Fixture::new();
            let child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "canon_apply::tests::ng_306_canon_apply_crash_fixture",
                    "--nocapture",
                ])
                .env(NG306_ROOT_ENV, &fixture.root)
                .env(NG306_STATE_ENV, &fixture.app_state)
                .env(NG306_STAGE_ENV, crash_stage)
                .output()
                .unwrap();
            assert_eq!(child.status.code(), Some(NG306_CRASH_EXIT_CODE));

            let inspection = inspect_project_root(&fixture.root, ProjectScanMode::Open).unwrap();
            let reopened = open_project(&inspection, false).unwrap();
            let reopened_recovery = RecoveryStore::new(fixture.app_state.clone()).unwrap();
            let target = fixture.root.join("characters/中断人物.md");
            let proposed = markdown("中断人物");
            match crash_stage {
                "commit" => assert!(!target.exists()),
                "post-write" => assert_eq!(fs::read(&target).unwrap(), proposed),
                _ => unreachable!(),
            }

            let recovered = reopened_recovery.recover_project(&reopened);
            assert_eq!(recovered.status, RecoveryStatus::Recovered);
            assert_eq!(recovered.recovered_transactions, 1);
            assert_eq!(recovered.required_transactions, 0);
            match crash_stage {
                "commit" => assert!(!target.exists()),
                "post-write" => assert_eq!(fs::read(&target).unwrap(), proposed),
                _ => unreachable!(),
            }
            assert_eq!(
                reopened_recovery.recover_project(&reopened).status,
                RecoveryStatus::Ready
            );
        }
    }
}
