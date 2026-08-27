use crate::project::ProjectHandle;
use crate::workspace::{
    is_link_or_reparse, read_bounded, resolve_scoped_target, sha256_hex,
    validate_writable_relative_path, MAX_EDITABLE_FILE_BYTES,
};
use crate::AppError;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const RECOVERY_SCHEMA_VERSION: u16 = 1;
const MAX_JOURNAL_BYTES: u64 = 8 * 1024;

#[derive(Debug, Clone)]
pub struct RecoveryStore {
    root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryStatus {
    Ready,
    Recovered,
    Required,
    NotChecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReport {
    pub status: RecoveryStatus,
    pub recovered_transactions: u32,
    pub required_transactions: u32,
}

impl RecoveryReport {
    pub fn ready() -> Self {
        Self {
            status: RecoveryStatus::Ready,
            recovered_transactions: 0,
            required_transactions: 0,
        }
    }

    pub fn not_checked() -> Self {
        Self {
            status: RecoveryStatus::NotChecked,
            recovered_transactions: 0,
            required_transactions: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryStage {
    Prepared,
    TemporarySynced,
    Replacing,
    Committed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RecoveryOperation {
    Create,
    #[default]
    Replace,
    ChapterCreate,
    ChapterReplace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryJournal {
    schema_version: u16,
    transaction_id: String,
    project_id: String,
    relative_path: String,
    stage: RecoveryStage,
    #[serde(default)]
    operation: RecoveryOperation,
    source_hash: String,
    replacement_hash: String,
    temporary_file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    backup_file_id: Option<String>,
}

pub struct RecoveryTransaction {
    store: RecoveryStore,
    journal: RecoveryJournal,
    journal_path: PathBuf,
}

impl RecoveryStore {
    pub fn new(app_state_root: PathBuf) -> Result<Self, AppError> {
        fs::create_dir_all(&app_state_root)
            .map_err(|_| recovery_io("recovery-state-root-create-failed"))?;
        let root = app_state_root
            .canonicalize()
            .map_err(|_| recovery_io("recovery-state-root-canonicalize-failed"))?;
        let metadata = fs::symlink_metadata(&root)
            .map_err(|_| recovery_io("recovery-state-root-metadata-failed"))?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err(recovery_io("recovery-state-root-not-safe"));
        }
        Ok(Self { root })
    }

    pub fn begin_transaction(
        &self,
        handle: &ProjectHandle,
        relative_path: &str,
        source_hash: &str,
        replacement_hash: &str,
        temporary_file_id: &str,
    ) -> Result<RecoveryTransaction, AppError> {
        validate_hash(source_hash)?;
        self.begin_transaction_inner(
            handle,
            relative_path,
            RecoveryOperation::Replace,
            source_hash,
            replacement_hash,
            temporary_file_id,
            None,
        )
    }

    pub fn begin_create_transaction(
        &self,
        handle: &ProjectHandle,
        relative_path: &str,
        replacement_hash: &str,
        temporary_file_id: &str,
    ) -> Result<RecoveryTransaction, AppError> {
        self.begin_transaction_inner(
            handle,
            relative_path,
            RecoveryOperation::Create,
            "absent",
            replacement_hash,
            temporary_file_id,
            None,
        )
    }

    pub fn begin_chapter_create_transaction(
        &self,
        handle: &ProjectHandle,
        relative_path: &str,
        replacement_hash: &str,
        temporary_file_id: &str,
    ) -> Result<RecoveryTransaction, AppError> {
        self.begin_transaction_inner(
            handle,
            relative_path,
            RecoveryOperation::ChapterCreate,
            "absent",
            replacement_hash,
            temporary_file_id,
            None,
        )
    }

    pub fn begin_chapter_replace_transaction(
        &self,
        handle: &ProjectHandle,
        relative_path: &str,
        source_hash: &str,
        replacement_hash: &str,
        temporary_file_id: &str,
        backup_file_id: &str,
    ) -> Result<RecoveryTransaction, AppError> {
        validate_hash(source_hash)?;
        self.begin_transaction_inner(
            handle,
            relative_path,
            RecoveryOperation::ChapterReplace,
            source_hash,
            replacement_hash,
            temporary_file_id,
            Some(backup_file_id),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn begin_transaction_inner(
        &self,
        handle: &ProjectHandle,
        relative_path: &str,
        operation: RecoveryOperation,
        source_hash: &str,
        replacement_hash: &str,
        temporary_file_id: &str,
        backup_file_id: Option<&str>,
    ) -> Result<RecoveryTransaction, AppError> {
        validate_writable_relative_path(relative_path)?;
        validate_hash(replacement_hash)?;
        validate_temporary_file_id(relative_path, temporary_file_id)?;
        if let Some(backup_file_id) = backup_file_id {
            validate_backup_file_id(relative_path, backup_file_id)?;
        }
        validate_uuid(handle.project_id())?;
        let transaction_id = Uuid::new_v4().to_string();
        let project_directory = self.ensure_project_directory(handle.project_id())?;
        let journal_path = project_directory.join(format!("{transaction_id}.json"));
        let journal = RecoveryJournal {
            schema_version: RECOVERY_SCHEMA_VERSION,
            transaction_id,
            project_id: handle.project_id().to_owned(),
            relative_path: relative_path.to_owned(),
            operation,
            stage: RecoveryStage::Prepared,
            source_hash: source_hash.to_owned(),
            replacement_hash: replacement_hash.to_owned(),
            temporary_file_id: temporary_file_id.to_owned(),
            backup_file_id: backup_file_id.map(ToOwned::to_owned),
        };
        write_new_journal(&journal_path, &journal)?;
        Ok(RecoveryTransaction {
            store: self.clone(),
            journal,
            journal_path,
        })
    }

    pub fn recover_project(&self, handle: &ProjectHandle) -> RecoveryReport {
        let project_directory = match self.ensure_project_directory(handle.project_id()) {
            Ok(value) => value,
            Err(_) => {
                return RecoveryReport {
                    status: RecoveryStatus::Required,
                    recovered_transactions: 0,
                    required_transactions: 1,
                };
            }
        };
        let entries = match fs::read_dir(&project_directory) {
            Ok(value) => value,
            Err(_) => {
                return RecoveryReport {
                    status: RecoveryStatus::Required,
                    recovered_transactions: 0,
                    required_transactions: 1,
                };
            }
        };

        let mut recovered = 0_u32;
        let mut required = 0_u32;
        for entry in entries {
            let Ok(entry) = entry else {
                required = required.saturating_add(1);
                continue;
            };
            let path = entry.path();
            let is_json = path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("json"));
            if !is_json {
                if is_verified_update_temporary(&path, &project_directory) {
                    match fs::symlink_metadata(&path) {
                        Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => {
                            if fs::remove_file(&path).is_err() {
                                required = required.saturating_add(1);
                            }
                        }
                        _ => required = required.saturating_add(1),
                    }
                }
                continue;
            }
            match self.recover_journal(handle, &project_directory, &path) {
                RecoveryDecision::Recovered => recovered = recovered.saturating_add(1),
                RecoveryDecision::Required => required = required.saturating_add(1),
            }
        }

        RecoveryReport {
            status: if required > 0 {
                RecoveryStatus::Required
            } else if recovered > 0 {
                RecoveryStatus::Recovered
            } else {
                RecoveryStatus::Ready
            },
            recovered_transactions: recovered,
            required_transactions: required,
        }
    }

    fn ensure_project_directory(&self, project_id: &str) -> Result<PathBuf, AppError> {
        validate_uuid(project_id)?;
        let recovery = ensure_child_directory(&self.root, "recovery")?;
        let version = ensure_child_directory(&recovery, "v1")?;
        ensure_child_directory(&version, project_id)
    }

    fn recover_journal(
        &self,
        handle: &ProjectHandle,
        project_directory: &Path,
        journal_path: &Path,
    ) -> RecoveryDecision {
        if journal_path.parent() != Some(project_directory) {
            return RecoveryDecision::Required;
        }
        let metadata = match fs::symlink_metadata(journal_path) {
            Ok(value) if value.is_file() && !is_link_or_reparse(&value) => value,
            _ => return RecoveryDecision::Required,
        };
        if metadata.len() > MAX_JOURNAL_BYTES {
            return RecoveryDecision::Required;
        }
        let journal = match read_journal(journal_path) {
            Ok(value) => value,
            Err(_) => return RecoveryDecision::Required,
        };
        if validate_journal(&journal, handle).is_err()
            || journal_path.file_stem().and_then(|value| value.to_str())
                != Some(journal.transaction_id.as_str())
        {
            return RecoveryDecision::Required;
        }

        let target = match resolve_scoped_target(handle.root(), &journal.relative_path) {
            Ok(value) => value,
            Err(_) => return RecoveryDecision::Required,
        };
        let parent = match target.parent() {
            Some(value) => value,
            None => return RecoveryDecision::Required,
        };
        let temporary = parent.join(&journal.temporary_file_id);
        let target_hash = match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => {
                match read_bounded(&target, MAX_EDITABLE_FILE_BYTES) {
                    Ok(bytes) => Some(sha256_hex(&bytes)),
                    Err(_) => return RecoveryDecision::Required,
                }
            }
            Ok(_) => return RecoveryDecision::Required,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return RecoveryDecision::Required,
        };
        let temporary_hash = match fs::symlink_metadata(&temporary) {
            Ok(value) if value.is_file() && !is_link_or_reparse(&value) => {
                match read_bounded(&temporary, MAX_EDITABLE_FILE_BYTES) {
                    Ok(bytes) => Some(sha256_hex(&bytes)),
                    Err(_) => return RecoveryDecision::Required,
                }
            }
            Ok(_) => return RecoveryDecision::Required,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return RecoveryDecision::Required,
        };
        let backup = journal
            .backup_file_id
            .as_ref()
            .map(|file_id| parent.join(file_id));
        let backup_hash = match backup.as_deref() {
            Some(path) => match fs::symlink_metadata(path) {
                Ok(value) if value.is_file() && !is_link_or_reparse(&value) => {
                    match read_bounded(path, MAX_EDITABLE_FILE_BYTES) {
                        Ok(bytes) => Some(sha256_hex(&bytes)),
                        Err(_) => return RecoveryDecision::Required,
                    }
                }
                Ok(_) => return RecoveryDecision::Required,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => return RecoveryDecision::Required,
            },
            None => None,
        };

        if matches!(
            journal.operation,
            RecoveryOperation::ChapterCreate | RecoveryOperation::ChapterReplace
        ) {
            return recover_chapter_journal(
                journal_path,
                project_directory,
                &journal,
                &target,
                &temporary,
                backup.as_deref(),
                target_hash.as_deref(),
                temporary_hash.as_deref(),
                backup_hash.as_deref(),
            );
        }

        let (can_finish_commit, can_finish_rollback) = match journal.operation {
            RecoveryOperation::Replace => (
                target_hash.as_deref() == Some(journal.replacement_hash.as_str())
                    && matches!(
                        journal.stage,
                        RecoveryStage::Replacing | RecoveryStage::Committed
                    ),
                target_hash.as_deref() == Some(journal.source_hash.as_str())
                    && temporary_hash
                        .as_deref()
                        .is_none_or(|hash| hash == journal.replacement_hash)
                    && journal.stage != RecoveryStage::Committed,
            ),
            RecoveryOperation::Create => (
                target_hash.as_deref() == Some(journal.replacement_hash.as_str())
                    && matches!(
                        journal.stage,
                        RecoveryStage::Replacing | RecoveryStage::Committed
                    ),
                target_hash.is_none()
                    && temporary_hash
                        .as_deref()
                        .is_none_or(|hash| hash == journal.replacement_hash)
                    && journal.stage != RecoveryStage::Committed,
            ),
            RecoveryOperation::ChapterCreate | RecoveryOperation::ChapterReplace => {
                unreachable!("chapter recovery is handled above")
            }
        };
        if !can_finish_commit && !can_finish_rollback {
            return RecoveryDecision::Required;
        }
        if temporary_hash.is_some() && fs::remove_file(&temporary).is_err() {
            return RecoveryDecision::Required;
        }
        if remove_verified_journal(journal_path, project_directory, &journal.transaction_id)
            .is_err()
        {
            return RecoveryDecision::Required;
        }
        RecoveryDecision::Recovered
    }
}

impl RecoveryTransaction {
    pub fn transaction_id(&self) -> &str {
        &self.journal.transaction_id
    }

    pub fn temporary_file_id(&self) -> &str {
        &self.journal.temporary_file_id
    }

    pub fn backup_file_id(&self) -> Option<&str> {
        self.journal.backup_file_id.as_deref()
    }

    pub fn mark_temporary_synced(&mut self) -> Result<(), AppError> {
        self.update_stage(RecoveryStage::TemporarySynced)
    }

    pub fn mark_replacing(&mut self) -> Result<(), AppError> {
        self.update_stage(RecoveryStage::Replacing)
    }

    pub fn commit_best_effort(mut self) {
        if self.update_stage(RecoveryStage::Committed).is_ok() {
            let _ = remove_verified_journal(
                &self.journal_path,
                self.journal_path.parent().unwrap_or(&self.store.root),
                &self.journal.transaction_id,
            );
        }
    }

    pub fn commit_chapter(mut self, target: &Path) -> Result<(), AppError> {
        if !matches!(
            self.journal.operation,
            RecoveryOperation::ChapterCreate | RecoveryOperation::ChapterReplace
        ) {
            return Err(AppError::project_recovery_required());
        }
        self.update_stage(RecoveryStage::Committed)?;
        if let Some(backup_file_id) = self.journal.backup_file_id.as_deref() {
            let parent = target
                .parent()
                .ok_or_else(AppError::project_recovery_required)?;
            let backup = parent.join(backup_file_id);
            remove_verified_auxiliary(
                &backup,
                parent,
                backup_file_id,
                self.journal.source_hash.as_str(),
            )?;
        }
        remove_verified_journal(
            &self.journal_path,
            self.journal_path.parent().unwrap_or(&self.store.root),
            &self.journal.transaction_id,
        )
    }

    pub fn abort_best_effort(self) {
        let _ = remove_verified_journal(
            &self.journal_path,
            self.journal_path.parent().unwrap_or(&self.store.root),
            &self.journal.transaction_id,
        );
    }

    fn update_stage(&mut self, stage: RecoveryStage) -> Result<(), AppError> {
        self.journal.stage = stage;
        replace_journal(&self.journal_path, &self.journal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryDecision {
    Recovered,
    Required,
}

#[allow(clippy::too_many_arguments)]
fn recover_chapter_journal(
    journal_path: &Path,
    project_directory: &Path,
    journal: &RecoveryJournal,
    target: &Path,
    temporary: &Path,
    backup: Option<&Path>,
    target_hash: Option<&str>,
    temporary_hash: Option<&str>,
    backup_hash: Option<&str>,
) -> RecoveryDecision {
    if temporary_hash.is_some_and(|hash| hash != journal.replacement_hash) {
        return RecoveryDecision::Required;
    }
    match journal.operation {
        RecoveryOperation::ChapterCreate => {
            if backup.is_some() || backup_hash.is_some() {
                return RecoveryDecision::Required;
            }
            match journal.stage {
                RecoveryStage::Committed => {
                    if target_hash != Some(journal.replacement_hash.as_str()) {
                        return RecoveryDecision::Required;
                    }
                }
                RecoveryStage::Prepared | RecoveryStage::TemporarySynced => {
                    if target_hash.is_some() {
                        return RecoveryDecision::Required;
                    }
                }
                RecoveryStage::Replacing => match target_hash {
                    None => {}
                    Some(hash) if hash == journal.replacement_hash => {
                        if remove_verified_target(target, &journal.relative_path).is_err() {
                            return RecoveryDecision::Required;
                        }
                    }
                    Some(_) => return RecoveryDecision::Required,
                },
            }
        }
        RecoveryOperation::ChapterReplace => {
            if backup_hash.is_some_and(|hash| hash != journal.source_hash) {
                return RecoveryDecision::Required;
            }
            match journal.stage {
                RecoveryStage::Committed => {
                    if target_hash != Some(journal.replacement_hash.as_str()) {
                        return RecoveryDecision::Required;
                    }
                }
                RecoveryStage::Prepared | RecoveryStage::TemporarySynced => {
                    if target_hash != Some(journal.source_hash.as_str()) {
                        return RecoveryDecision::Required;
                    }
                }
                RecoveryStage::Replacing => match target_hash {
                    Some(hash) if hash == journal.source_hash => {}
                    Some(hash) if hash == journal.replacement_hash => {
                        let Some(backup_path) = backup else {
                            return RecoveryDecision::Required;
                        };
                        if backup_hash != Some(journal.source_hash.as_str())
                            || replace_state_file(backup_path, target).is_err()
                        {
                            return RecoveryDecision::Required;
                        }
                        let restored = match read_bounded(target, MAX_EDITABLE_FILE_BYTES) {
                            Ok(bytes) => sha256_hex(&bytes),
                            Err(_) => return RecoveryDecision::Required,
                        };
                        if restored != journal.source_hash {
                            return RecoveryDecision::Required;
                        }
                    }
                    _ => return RecoveryDecision::Required,
                },
            }
        }
        RecoveryOperation::Create | RecoveryOperation::Replace => {
            return RecoveryDecision::Required
        }
    }

    if temporary.exists()
        && remove_verified_auxiliary(
            temporary,
            temporary.parent().unwrap_or(project_directory),
            &journal.temporary_file_id,
            &journal.replacement_hash,
        )
        .is_err()
    {
        return RecoveryDecision::Required;
    }
    if let (Some(backup), Some(backup_file_id)) = (backup, journal.backup_file_id.as_deref()) {
        if backup.exists()
            && remove_verified_auxiliary(
                backup,
                backup.parent().unwrap_or(project_directory),
                backup_file_id,
                &journal.source_hash,
            )
            .is_err()
        {
            return RecoveryDecision::Required;
        }
    }
    if remove_verified_journal(journal_path, project_directory, &journal.transaction_id).is_err() {
        return RecoveryDecision::Required;
    }
    RecoveryDecision::Recovered
}

fn remove_verified_target(target: &Path, relative_path: &str) -> Result<(), AppError> {
    if target.file_name() != Path::new(relative_path).file_name() {
        return Err(AppError::project_recovery_required());
    }
    let metadata =
        fs::symlink_metadata(target).map_err(|_| AppError::project_recovery_required())?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(AppError::project_recovery_required());
    }
    fs::remove_file(target).map_err(|_| AppError::project_recovery_required())
}

fn remove_verified_auxiliary(
    path: &Path,
    parent: &Path,
    expected_file_id: &str,
    expected_hash: &str,
) -> Result<(), AppError> {
    if path.parent() != Some(parent)
        || path.file_name().and_then(|value| value.to_str()) != Some(expected_file_id)
    {
        return Err(AppError::project_recovery_required());
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => {
            let bytes = read_bounded(path, MAX_EDITABLE_FILE_BYTES)?;
            if sha256_hex(&bytes) != expected_hash {
                return Err(AppError::project_recovery_required());
            }
            fs::remove_file(path).map_err(|_| AppError::project_recovery_required())
        }
        Ok(_) => Err(AppError::project_recovery_required()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AppError::project_recovery_required()),
    }
}

fn ensure_child_directory(parent: &Path, name: &str) -> Result<PathBuf, AppError> {
    let path = parent.join(name);
    match fs::create_dir(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(recovery_io("recovery-directory-create-failed")),
    }
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| recovery_io("recovery-directory-metadata-failed"))?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(recovery_io("recovery-directory-not-safe"));
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| recovery_io("recovery-directory-canonicalize-failed"))?;
    if !canonical.starts_with(parent) {
        return Err(recovery_io("recovery-directory-outside-root"));
    }
    Ok(canonical)
}

fn validate_journal(journal: &RecoveryJournal, handle: &ProjectHandle) -> Result<(), AppError> {
    if journal.schema_version != RECOVERY_SCHEMA_VERSION
        || journal.project_id != handle.project_id()
    {
        return Err(AppError::project_recovery_required());
    }
    validate_uuid(&journal.transaction_id)?;
    validate_uuid(&journal.project_id)?;
    validate_writable_relative_path(&journal.relative_path)?;
    match journal.operation {
        RecoveryOperation::Replace | RecoveryOperation::ChapterReplace => {
            validate_hash(&journal.source_hash)?
        }
        RecoveryOperation::Create | RecoveryOperation::ChapterCreate
            if journal.source_hash == "absent" => {}
        RecoveryOperation::Create | RecoveryOperation::ChapterCreate => {
            return Err(AppError::project_recovery_required())
        }
    }
    validate_hash(&journal.replacement_hash)?;
    validate_temporary_file_id(&journal.relative_path, &journal.temporary_file_id)?;
    match journal.operation {
        RecoveryOperation::ChapterReplace => validate_backup_file_id(
            &journal.relative_path,
            journal
                .backup_file_id
                .as_deref()
                .ok_or_else(AppError::project_recovery_required)?,
        ),
        RecoveryOperation::Create
        | RecoveryOperation::Replace
        | RecoveryOperation::ChapterCreate
            if journal.backup_file_id.is_none() =>
        {
            Ok(())
        }
        _ => Err(AppError::project_recovery_required()),
    }
}

fn validate_uuid(value: &str) -> Result<(), AppError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| AppError::project_recovery_required())
}

fn validate_hash(value: &str) -> Result<(), AppError> {
    let valid = value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(AppError::project_recovery_required())
    }
}

fn validate_temporary_file_id(
    relative_path: &str,
    temporary_file_id: &str,
) -> Result<(), AppError> {
    let target_name = Path::new(relative_path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(AppError::project_recovery_required)?;
    let prefix = format!(".{target_name}.novelgainker-");
    let Some(uuid) = temporary_file_id
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(".tmp"))
    else {
        return Err(AppError::project_recovery_required());
    };
    if temporary_file_id.contains(['/', '\\']) || Uuid::parse_str(uuid).is_err() {
        Err(AppError::project_recovery_required())
    } else {
        Ok(())
    }
}

fn validate_backup_file_id(relative_path: &str, backup_file_id: &str) -> Result<(), AppError> {
    let target_name = Path::new(relative_path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(AppError::project_recovery_required)?;
    let prefix = format!(".{target_name}.novelgainker-");
    let Some(uuid) = backup_file_id
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(".bak"))
    else {
        return Err(AppError::project_recovery_required());
    };
    if backup_file_id.contains(['/', '\\']) || Uuid::parse_str(uuid).is_err() {
        Err(AppError::project_recovery_required())
    } else {
        Ok(())
    }
}

fn is_verified_update_temporary(path: &Path, parent: &Path) -> bool {
    if path.parent() != Some(parent) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some(body) = name
        .strip_prefix('.')
        .and_then(|value| value.strip_suffix(".tmp"))
    else {
        return false;
    };
    let Some((transaction_id, update_id)) = body.split_once(".update-") else {
        return false;
    };
    Uuid::parse_str(transaction_id).is_ok() && Uuid::parse_str(update_id).is_ok()
}

fn read_journal(path: &Path) -> Result<RecoveryJournal, AppError> {
    let file = File::open(path).map_err(|_| recovery_io("recovery-journal-open-failed"))?;
    let mut bytes = Vec::new();
    file.take(MAX_JOURNAL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| recovery_io("recovery-journal-read-failed"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_JOURNAL_BYTES {
        return Err(AppError::project_recovery_required());
    }
    serde_json::from_slice(&bytes).map_err(|_| AppError::project_recovery_required())
}

fn write_new_journal(path: &Path, journal: &RecoveryJournal) -> Result<(), AppError> {
    let bytes = serialize_journal(journal)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| recovery_io("recovery-journal-create-failed"))?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(path);
        return Err(recovery_io(match error.kind() {
            std::io::ErrorKind::WriteZero => "recovery-journal-write-zero",
            _ => "recovery-journal-write-failed",
        }));
    }
    Ok(())
}

fn replace_journal(path: &Path, journal: &RecoveryJournal) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or_else(|| recovery_io("recovery-journal-parent-missing"))?;
    let transaction_id = journal.transaction_id.as_str();
    let update_name = format!(".{transaction_id}.update-{}.tmp", Uuid::new_v4());
    let update_path = parent.join(&update_name);
    let bytes = serialize_journal(journal)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&update_path)
            .map_err(|_| recovery_io("recovery-journal-update-create-failed"))?;
        file.write_all(&bytes)
            .map_err(|_| recovery_io("recovery-journal-update-write-failed"))?;
        file.sync_all()
            .map_err(|_| recovery_io("recovery-journal-update-sync-failed"))?;
        drop(file);
        replace_state_file(&update_path, path)
    })();
    if result.is_err()
        && update_path.parent() == Some(parent)
        && update_path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                value.starts_with(&format!(".{transaction_id}.update-")) && value.ends_with(".tmp")
            })
    {
        let _ = fs::remove_file(update_path);
    }
    result
}

fn serialize_journal(journal: &RecoveryJournal) -> Result<Vec<u8>, AppError> {
    let mut bytes = serde_json::to_vec(journal)
        .map_err(|_| recovery_io("recovery-journal-serialize-failed"))?;
    bytes.push(b'\n');
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_JOURNAL_BYTES {
        Err(AppError::project_recovery_required())
    } else {
        Ok(bytes)
    }
}

fn remove_verified_journal(
    path: &Path,
    parent: &Path,
    transaction_id: &str,
) -> Result<(), AppError> {
    let expected_name = format!("{transaction_id}.json");
    if path.parent() != Some(parent)
        || path.file_name().and_then(|value| value.to_str()) != Some(expected_name.as_str())
    {
        return Err(AppError::project_recovery_required());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(recovery_io("recovery-journal-remove-failed")),
    }
}

#[cfg(windows)]
fn replace_state_file(temporary_path: &Path, target_path: &Path) -> Result<(), AppError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let temporary = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target_path
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
        Err(recovery_io("recovery-journal-atomic-replace-failed"))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_state_file(temporary_path: &Path, target_path: &Path) -> Result<(), AppError> {
    fs::rename(temporary_path, target_path)
        .map_err(|_| recovery_io("recovery-journal-atomic-replace-failed"))
}

fn recovery_io(diagnostic: &'static str) -> AppError {
    AppError::project_recovery_required().with_diagnostic(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::{RecoveryStage, RecoveryStatus, RecoveryStore};
    use crate::project::{
        create_project, inspect_project_root, open_project, ProjectHandle, ProjectScanMode,
    };
    use crate::workspace::sha256_hex;
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::fs;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    struct TestRecoveryProject {
        root: PathBuf,
        app_state: PathBuf,
        handle: ProjectHandle,
        store: RecoveryStore,
    }

    impl TestRecoveryProject {
        fn new(label: &str) -> Self {
            let suffix = Uuid::new_v4();
            let root = std::env::temp_dir().join(format!("novelgainker-ng104-{label}-{suffix}"));
            let app_state =
                std::env::temp_dir().join(format!("novelgainker-ng104-state-{label}-{suffix}"));
            fs::create_dir(&root).expect("project fixture root must create");
            let inspection = inspect_project_root(&root, ProjectScanMode::Create)
                .expect("project fixture must inspect");
            let handle = create_project(&inspection).expect("project fixture must create");
            let store = RecoveryStore::new(app_state.clone()).expect("recovery store must create");
            Self {
                root,
                app_state,
                handle,
                store,
            }
        }

        fn target(&self, name: &str) -> PathBuf {
            self.root.join("chapters").join(name)
        }

        fn temporary_id(name: &str) -> String {
            format!(".{name}.novelgainker-{}.tmp", Uuid::new_v4())
        }
    }

    impl Drop for TestRecoveryProject {
        fn drop(&mut self) {
            for path in [&self.app_state, &self.root] {
                if path.parent() == Some(std::env::temp_dir().as_path())
                    && path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.starts_with("novelgainker-ng104-"))
                {
                    let _ = fs::remove_dir_all(path);
                }
            }
        }
    }

    fn only_journal(project: &TestRecoveryProject) -> PathBuf {
        let directory = project
            .store
            .ensure_project_directory(project.handle.project_id())
            .expect("journal directory must resolve");
        let journals = fs::read_dir(directory)
            .expect("journal directory must read")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        assert_eq!(journals.len(), 1, "exactly one journal must exist");
        journals[0].clone()
    }

    #[test]
    fn ng_104_recovery_journal_has_an_exact_redacted_schema() {
        let project = TestRecoveryProject::new("journal-schema");
        let target = project.target("private.md");
        let old_body = b"unpublished manuscript text with sk-private";
        let new_body = b"replacement manuscript text";
        fs::write(&target, old_body).expect("fixture target must write");
        let temporary_id = TestRecoveryProject::temporary_id("private.md");
        let transaction = project
            .store
            .begin_transaction(
                &project.handle,
                "chapters/private.md",
                &sha256_hex(old_body),
                &sha256_hex(new_body),
                &temporary_id,
            )
            .expect("journal must begin");
        let serialized = fs::read_to_string(only_journal(&project)).expect("journal must read");
        let value: Value = serde_json::from_str(&serialized).expect("journal must be JSON");
        let keys = value
            .as_object()
            .expect("journal must be an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys,
            BTreeSet::from([
                "operation",
                "projectId",
                "relativePath",
                "replacementHash",
                "schemaVersion",
                "sourceHash",
                "stage",
                "temporaryFileId",
                "transactionId",
            ])
        );
        assert_eq!(value["operation"], "replace");
        for sensitive in [
            String::from_utf8_lossy(old_body).as_ref(),
            String::from_utf8_lossy(new_body).as_ref(),
            project.root.to_string_lossy().as_ref(),
            "sk-private",
        ] {
            assert!(
                !serialized.contains(sensitive),
                "journal leaked sensitive data"
            );
        }
        assert_eq!(value["stage"], Value::String("prepared".to_owned()));
        transaction.abort_best_effort();
    }

    #[test]
    fn ng_104_finishes_a_commit_after_the_target_was_replaced() {
        let project = TestRecoveryProject::new("finish-commit");
        let target = project.target("commit.md");
        let old_body = b"old";
        let new_body = b"new committed body";
        fs::write(&target, old_body).expect("fixture target must write");
        let temporary_id = TestRecoveryProject::temporary_id("commit.md");
        let temporary = target
            .parent()
            .expect("target parent must exist")
            .join(&temporary_id);
        let mut transaction = project
            .store
            .begin_transaction(
                &project.handle,
                "chapters/commit.md",
                &sha256_hex(old_body),
                &sha256_hex(new_body),
                &temporary_id,
            )
            .expect("journal must begin");
        fs::write(&temporary, new_body).expect("temporary fixture must write");
        transaction
            .mark_temporary_synced()
            .expect("temporary stage must persist");
        transaction
            .mark_replacing()
            .expect("replace stage must persist");
        fs::write(&target, new_body).expect("completed replacement fixture must write");
        drop(transaction);

        let report = project.store.recover_project(&project.handle);
        assert_eq!(report.status, RecoveryStatus::Recovered);
        assert_eq!(report.recovered_transactions, 1);
        assert_eq!(fs::read(&target).expect("target must read"), new_body);
        assert!(!temporary.exists());
    }

    #[test]
    fn ng_104_blocks_new_work_when_recovery_is_ambiguous_or_schema_is_unknown() {
        let project = TestRecoveryProject::new("required");
        let target = project.target("ambiguous.md");
        let old_body = b"old";
        let new_body = b"new";
        fs::write(&target, old_body).expect("fixture target must write");
        let temporary_id = TestRecoveryProject::temporary_id("ambiguous.md");
        let temporary = target
            .parent()
            .expect("target parent must exist")
            .join(&temporary_id);
        let mut transaction = project
            .store
            .begin_transaction(
                &project.handle,
                "chapters/ambiguous.md",
                &sha256_hex(old_body),
                &sha256_hex(new_body),
                &temporary_id,
            )
            .expect("journal must begin");
        fs::write(&temporary, new_body).expect("temporary fixture must write");
        transaction
            .mark_temporary_synced()
            .expect("temporary stage must persist");
        fs::write(&target, b"third-party-content").expect("external fixture must write");
        drop(transaction);

        let report = project.store.recover_project(&project.handle);
        assert_eq!(report.status, RecoveryStatus::Required);
        assert_eq!(report.required_transactions, 1);
        assert_eq!(
            fs::read(&target).expect("target must read"),
            b"third-party-content"
        );
        assert!(
            temporary.exists(),
            "ambiguous temporary evidence must be preserved"
        );

        let journal_path = only_journal(&project);
        let mut value: Value =
            serde_json::from_slice(&fs::read(&journal_path).expect("journal fixture must read"))
                .expect("journal fixture must parse");
        value
            .as_object_mut()
            .expect("journal must be an object")
            .insert(
                "manuscriptText".to_owned(),
                Value::String("must reject".to_owned()),
            );
        fs::write(
            &journal_path,
            serde_json::to_vec(&value).expect("corrupt fixture must serialize"),
        )
        .expect("corrupt fixture must write");
        let strict_report = project.store.recover_project(&project.handle);
        assert_eq!(strict_report.status, RecoveryStatus::Required);
        assert_eq!(strict_report.required_transactions, 1);
    }

    #[test]
    fn ng_505_restart_restores_original_chapter_before_transaction_commit() {
        let project = TestRecoveryProject::new("chapter-restore");
        let target = project.target("003.md");
        let old_body = b"old chapter body";
        let new_body = b"new chapter body";
        fs::write(&target, old_body).unwrap();
        let temporary_id = TestRecoveryProject::temporary_id("003.md");
        let backup_id = format!(".003.md.novelgainker-{}.bak", Uuid::new_v4());
        let temporary = target.parent().unwrap().join(&temporary_id);
        let backup = target.parent().unwrap().join(&backup_id);
        let mut transaction = project
            .store
            .begin_chapter_replace_transaction(
                &project.handle,
                "chapters/003.md",
                &sha256_hex(old_body),
                &sha256_hex(new_body),
                &temporary_id,
                &backup_id,
            )
            .unwrap();
        fs::write(&temporary, new_body).unwrap();
        transaction.mark_temporary_synced().unwrap();
        fs::hard_link(&target, &backup).unwrap();
        transaction.mark_replacing().unwrap();
        super::replace_state_file(&temporary, &target).unwrap();
        drop(transaction);

        let recovered = project.store.recover_project(&project.handle);
        assert_eq!(recovered.status, RecoveryStatus::Recovered);
        assert_eq!(fs::read(&target).unwrap(), old_body);
        assert!(!temporary.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn ng_505_restart_removes_an_uncommitted_created_chapter() {
        let project = TestRecoveryProject::new("chapter-create-rollback");
        let target = project.target("003.md");
        let new_body = b"new chapter body";
        let temporary_id = TestRecoveryProject::temporary_id("003.md");
        let temporary = target.parent().unwrap().join(&temporary_id);
        let mut transaction = project
            .store
            .begin_chapter_create_transaction(
                &project.handle,
                "chapters/003.md",
                &sha256_hex(new_body),
                &temporary_id,
            )
            .unwrap();
        fs::write(&temporary, new_body).unwrap();
        transaction.mark_temporary_synced().unwrap();
        transaction.mark_replacing().unwrap();
        fs::hard_link(&temporary, &target).unwrap();
        drop(transaction);

        let recovered = project.store.recover_project(&project.handle);
        assert_eq!(recovered.status, RecoveryStatus::Recovered);
        assert!(!target.exists());
        assert!(!temporary.exists());
    }

    #[cfg(windows)]
    #[test]
    fn ng_104_recovers_a_temporary_file_left_by_an_aborted_process() {
        use std::process::Command;

        let project = TestRecoveryProject::new("process-crash");
        let target = project.target("crash.md");
        let marker = project.app_state.join("crash-fixture-ready");
        fs::write(&target, b"old body").expect("fixture target must write");
        let child = Command::new(std::env::current_exe().expect("test executable must resolve"))
            .args([
                "--ignored",
                "--exact",
                "recovery::tests::ng_104_recovery_crash_fixture",
            ])
            .env("NOVELGAINKER_TEST_PROJECT_ROOT", &project.root)
            .env("NOVELGAINKER_TEST_APP_STATE", &project.app_state)
            .env("NOVELGAINKER_TEST_RECOVERY_MARKER", &marker)
            .output()
            .expect("crash fixture must run");
        assert!(!child.status.success(), "crash fixture must abort");
        assert_eq!(
            fs::read(&marker).expect("crash readiness marker must exist"),
            b"journal-synced",
            "fixture must sync its journal and temporary file before aborting"
        );
        fs::remove_file(&marker).expect("verified crash marker must remove");

        let report = project.store.recover_project(&project.handle);
        assert_eq!(report.status, RecoveryStatus::Recovered);
        assert_eq!(report.recovered_transactions, 1);
        assert_eq!(fs::read(&target).expect("target must read"), b"old body");
        assert!(
            fs::read_dir(target.parent().expect("target parent must exist"))
                .expect("chapter directory must read")
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".novelgainker-")),
            "verified recovery temporary file must be removed"
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "subprocess fixture invoked by the recovery crash test"]
    fn ng_104_recovery_crash_fixture() {
        let root = Path::new(
            &std::env::var_os("NOVELGAINKER_TEST_PROJECT_ROOT")
                .expect("fixture project root must exist"),
        )
        .to_path_buf();
        let app_state = Path::new(
            &std::env::var_os("NOVELGAINKER_TEST_APP_STATE").expect("fixture app state must exist"),
        )
        .to_path_buf();
        let inspection = inspect_project_root(&root, ProjectScanMode::Open)
            .expect("fixture project must inspect");
        let handle = open_project(&inspection, false).expect("fixture project must open");
        let store = RecoveryStore::new(app_state).expect("fixture recovery store must open");
        let target = root.join("chapters/crash.md");
        let old_body = fs::read(&target).expect("fixture target must read");
        let new_body = b"new body waiting for replace";
        let temporary_id = TestRecoveryProject::temporary_id("crash.md");
        let temporary = target
            .parent()
            .expect("target parent must exist")
            .join(&temporary_id);
        let mut transaction = store
            .begin_transaction(
                &handle,
                "chapters/crash.md",
                &sha256_hex(&old_body),
                &sha256_hex(new_body),
                &temporary_id,
            )
            .expect("fixture journal must begin");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .expect("fixture temporary must create");
        file.write_all(new_body)
            .expect("fixture temporary must write");
        file.sync_all().expect("fixture temporary must sync");
        transaction
            .mark_temporary_synced()
            .expect("fixture journal stage must sync");
        let marker = PathBuf::from(
            std::env::var_os("NOVELGAINKER_TEST_RECOVERY_MARKER")
                .expect("fixture marker must exist"),
        );
        let mut marker_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker)
            .expect("fixture marker must create");
        marker_file
            .write_all(b"journal-synced")
            .expect("fixture marker must write");
        marker_file.sync_all().expect("fixture marker must sync");
        std::process::abort();
    }

    #[test]
    fn recovery_stage_serialization_is_stable() {
        assert_eq!(
            serde_json::to_string(&RecoveryStage::TemporarySynced).expect("stage must serialize"),
            "\"temporarySynced\""
        );
    }
}
