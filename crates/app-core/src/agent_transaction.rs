use crate::agent_write::{AgentWriteOperationV1, PreparedAgentMutationV1, PreparedAgentWriteV1};
use crate::canon_apply::apply_canon_file_recoverable;
use crate::project::ProjectHandle;
use crate::recovery::RecoveryStore;
use crate::workspace::{read_bounded, resolve_scoped_target, sha256_hex, MAX_EDITABLE_FILE_BYTES};
use crate::AppError;
use novelgainker_agent::action_contracts::ProjectRelativePathV1;
use novelgainker_agent::contracts::{ContentHashV1, EpochMillisV1};
use novelgainker_agent::persistence_contracts::{
    UndoBlobIdV1, UndoRecordV1, UndoTargetV1, MAX_UNDO_BYTES_PER_PROJECT, UNDO_RETENTION_MS,
};
use novelgainker_agent::transaction_contracts::{
    CommittedFileResultV1, ObservedFileStateV1, PreflightStatusV1, TransactionReceiptV1,
    MAX_UNDO_BYTES_PER_TURN,
};
use novelgainker_workflows::proposal::CanonFileOperationKind;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const PAYLOAD_MAGIC: &[u8; 8] = b"NGAUNDO1";
const JOURNAL_MAX_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone)]
pub struct AgentUndoStore {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommitResultV1 {
    pub receipt: TransactionReceiptV1,
    pub undo_record: UndoRecordV1,
    pub project_write_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUndoApplyResultV1 {
    pub transaction_hash: String,
    pub restored_file_count: u16,
    pub project_write_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentGroupRecoveryReportV1 {
    pub recovered_transactions: u16,
    pub recovery_required_transactions: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalMode {
    Applying,
    Undoing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    schema_version: u16,
    mode: JournalMode,
    record: UndoRecordV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Payload {
    project_id: String,
    transaction_hash: String,
    entries: Vec<PayloadEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PayloadEntry {
    path: String,
    expected_current_hash: String,
    source_bytes: Option<Vec<u8>>,
}

impl AgentUndoStore {
    pub fn new(app_state_root: PathBuf) -> Result<Self, AppError> {
        let root = app_state_root.join("agent-undo-v1");
        fs::create_dir_all(root.join("blobs")).map_err(|_| tx_error())?;
        fs::create_dir_all(root.join("transactions")).map_err(|_| tx_error())?;
        Ok(Self { root })
    }

    pub fn blob_path(&self, project_id: &str, blob_id: &str) -> Result<PathBuf, AppError> {
        validate_component(project_id)?;
        validate_component(blob_id)?;
        Ok(self
            .root
            .join("blobs")
            .join(project_id)
            .join(format!("{blob_id}.bin")))
    }

    pub fn remove_record_blob(&self, record: &UndoRecordV1) -> Result<(), AppError> {
        remove_if_exists(&self.blob_path(record.project_id.as_str(), record.blob_id.as_str())?)
    }

    fn journal_path(&self, hash: &str) -> Result<PathBuf, AppError> {
        validate_component(hash)?;
        Ok(self.root.join("transactions").join(format!("{hash}.json")))
    }

    fn write_journal(&self, record: &UndoRecordV1, mode: JournalMode) -> Result<(), AppError> {
        let value = Journal {
            schema_version: 1,
            mode,
            record: record.clone(),
        };
        let bytes = serde_json::to_vec(&value).map_err(|_| tx_error())?;
        write_replace(
            &self.journal_path(value.record.transaction_hash.as_str())?,
            &bytes,
        )
    }

    fn remove_journal(&self, hash: &str) -> Result<(), AppError> {
        remove_if_exists(&self.journal_path(hash)?)
    }

    fn prepare(
        &self,
        prepared: &PreparedAgentWriteV1,
        now: EpochMillisV1,
        clear_generation: u32,
    ) -> Result<(UndoRecordV1, Payload), AppError> {
        prepared.plan.validate().map_err(|_| tx_error())?;
        let mut entries = prepared
            .mutations
            .iter()
            .map(|mutation| PayloadEntry {
                path: mutation.classification.path.as_str().to_owned(),
                expected_current_hash: sha256_hex(&mutation.target_bytes),
                source_bytes: mutation.source_bytes.clone(),
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        let payload = Payload {
            project_id: prepared.plan.project_id.as_str().to_owned(),
            transaction_hash: prepared.plan.transaction_hash.as_str().to_owned(),
            entries,
        };
        let plaintext = encode_payload(&payload)?;
        let encrypted = protect_for_project(&plaintext, &payload.project_id)?;
        if encrypted.is_empty()
            || encrypted.len() > MAX_UNDO_BYTES_PER_TURN as usize
            || encrypted.len() > MAX_UNDO_BYTES_PER_PROJECT as usize
        {
            return Err(tx_error());
        }
        let blob_id = UndoBlobIdV1::parse(format!("ubl_{}", Uuid::new_v4().simple()))
            .map_err(|_| tx_error())?;
        let blob_hash = ContentHashV1::digest(&encrypted);
        let blob_path = self.blob_path(&payload.project_id, blob_id.as_str())?;
        write_new(&blob_path, &encrypted)?;
        let targets = payload
            .entries
            .iter()
            .map(|entry| {
                Ok(UndoTargetV1 {
                    path: ProjectRelativePathV1::parse(entry.path.clone())
                        .map_err(|_| tx_error())?,
                    expected_current_hash: ContentHashV1::parse(
                        entry.expected_current_hash.clone(),
                    )
                    .map_err(|_| tx_error())?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        let built = UndoRecordV1::build(
            prepared.plan.project_id.clone(),
            prepared.plan.session_id.clone(),
            prepared.plan.turn_id.clone(),
            prepared.plan.transaction_hash.clone(),
            blob_id,
            blob_hash,
            encrypted.len() as u32,
            targets,
            now,
            EpochMillisV1(now.0.saturating_add(UNDO_RETENTION_MS)),
            clear_generation,
        )
        .map_err(|_| tx_error());
        match built {
            Ok(record) => Ok((record, payload)),
            Err(error) => {
                let _ = remove_if_exists(&blob_path);
                Err(error)
            }
        }
    }

    fn load(&self, record: &UndoRecordV1) -> Result<Payload, AppError> {
        record.validate().map_err(|_| tx_error())?;
        let encrypted = read_bounded(
            &self.blob_path(record.project_id.as_str(), record.blob_id.as_str())?,
            MAX_UNDO_BYTES_PER_TURN as u64,
        )?;
        if ContentHashV1::digest(&encrypted) != record.blob_hash {
            return Err(tx_error());
        }
        let payload = decode_payload(&unprotect_for_project(
            &encrypted,
            record.project_id.as_str(),
        )?)?;
        if payload.project_id != record.project_id.as_str()
            || payload.transaction_hash != record.transaction_hash.as_str()
            || payload.entries.len() != record.targets.len()
            || !payload
                .entries
                .iter()
                .zip(&record.targets)
                .all(|(entry, target)| {
                    entry.path == target.path.as_str()
                        && entry.expected_current_hash == target.expected_current_hash.as_str()
                })
        {
            return Err(tx_error());
        }
        Ok(payload)
    }
}

pub fn commit_prepared_agent_write(
    handle: &ProjectHandle,
    prepared: &PreparedAgentWriteV1,
    recovery: &RecoveryStore,
    undo: &AgentUndoStore,
    now: EpochMillisV1,
    clear_generation: u32,
) -> Result<AgentCommitResultV1, AppError> {
    commit_with_hook(
        handle,
        prepared,
        recovery,
        undo,
        now,
        clear_generation,
        |_| Ok(()),
    )
}

fn commit_with_hook<F>(
    handle: &ProjectHandle,
    prepared: &PreparedAgentWriteV1,
    recovery: &RecoveryStore,
    undo: &AgentUndoStore,
    now: EpochMillisV1,
    clear_generation: u32,
    mut before_write: F,
) -> Result<AgentCommitResultV1, AppError>
where
    F: FnMut(usize) -> Result<(), AppError>,
{
    let preflight = prepared
        .plan
        .preflight(&observe(handle, prepared)?)
        .map_err(|_| tx_error())?;
    if preflight.status != PreflightStatusV1::ReadyForSingleWriter
        || !preflight.all_hashes_match
        || preflight.writes_performed != 0
    {
        return Err(tx_error());
    }
    let (record, _) = undo.prepare(prepared, now, clear_generation)?;
    undo.write_journal(&record, JournalMode::Applying)?;
    let mut mutations = prepared.mutations.iter().collect::<Vec<_>>();
    mutations.sort_by(|left, right| left.classification.path.cmp(&right.classification.path));
    let mut committed = Vec::new();
    for (index, mutation) in mutations.iter().enumerate() {
        let result = before_write(index).and_then(|()| apply_mutation(handle, mutation, recovery));
        if let Err(error) = result {
            if rollback_mutations(handle, &committed, recovery).is_err() {
                return Err(AppError::project_recovery_required());
            }
            undo.remove_journal(prepared.plan.transaction_hash.as_str())?;
            undo.remove_record_blob(&record)?;
            return Err(error);
        }
        committed.push(*mutation);
    }
    let results = mutations
        .iter()
        .map(|mutation| CommittedFileResultV1 {
            path: mutation.classification.path.clone(),
            result_hash: ContentHashV1::digest(&mutation.target_bytes),
        })
        .collect();
    let receipt = TransactionReceiptV1::from_committed_plan(&prepared.plan, results, now)
        .map_err(|_| AppError::project_recovery_required())?;
    undo.remove_journal(prepared.plan.transaction_hash.as_str())?;
    Ok(AgentCommitResultV1 {
        receipt,
        undo_record: record,
        project_write_count: mutations.len().min(u16::MAX as usize) as u16,
    })
}

pub fn apply_agent_undo(
    handle: &ProjectHandle,
    record: &UndoRecordV1,
    recovery: &RecoveryStore,
    undo: &AgentUndoStore,
) -> Result<AgentUndoApplyResultV1, AppError> {
    let payload = undo.load(record)?;
    for entry in &payload.entries {
        let target = resolve_scoped_target(handle.root(), &entry.path)?;
        if sha256_hex(&read_bounded(&target, MAX_EDITABLE_FILE_BYTES)?)
            != entry.expected_current_hash
        {
            return Err(tx_error());
        }
    }
    undo.write_journal(record, JournalMode::Undoing)?;
    rollback_payload(handle, &payload, recovery)
        .map_err(|_| AppError::project_recovery_required())?;
    undo.remove_journal(record.transaction_hash.as_str())?;
    undo.remove_record_blob(record)?;
    let count = payload.entries.len().min(u16::MAX as usize) as u16;
    Ok(AgentUndoApplyResultV1 {
        transaction_hash: record.transaction_hash.as_str().to_owned(),
        restored_file_count: count,
        project_write_count: count,
    })
}

pub fn recover_incomplete_agent_transactions(
    handle: &ProjectHandle,
    _: &[UndoRecordV1],
    recovery: &RecoveryStore,
    undo: &AgentUndoStore,
) -> AgentGroupRecoveryReportV1 {
    let mut report = AgentGroupRecoveryReportV1 {
        recovered_transactions: 0,
        recovery_required_transactions: 0,
    };
    let Ok(files) = fs::read_dir(undo.root.join("transactions")) else {
        report.recovery_required_transactions = 1;
        return report;
    };
    for file in files.flatten() {
        let path = file.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let outcome: Result<bool, AppError> = (|| {
            let journal: Journal = serde_json::from_slice(&read_bounded(&path, JOURNAL_MAX_BYTES)?)
                .map_err(|_| tx_error())?;
            if journal.schema_version != 1
                || journal.record.project_id.as_str() != agent_project_id(handle)?
            {
                return Ok(false);
            }
            let record = &journal.record;
            let payload = undo.load(record)?;
            rollback_payload_if_partial(handle, &payload, recovery)?;
            remove_if_exists(&path)?;
            if journal.mode == JournalMode::Applying {
                undo.remove_record_blob(record)?;
            }
            Ok(true)
        })();
        match outcome {
            Ok(true) => {
                report.recovered_transactions = report.recovered_transactions.saturating_add(1)
            }
            Ok(false) => {}
            Err(_) => {
                report.recovery_required_transactions =
                    report.recovery_required_transactions.saturating_add(1)
            }
        }
    }
    report
}

fn observe(
    handle: &ProjectHandle,
    prepared: &PreparedAgentWriteV1,
) -> Result<Vec<ObservedFileStateV1>, AppError> {
    prepared
        .plan
        .mutations
        .iter()
        .map(|mutation| {
            let target = resolve_scoped_target(handle.root(), mutation.frozen.path.as_str())?;
            let current_hash = match fs::symlink_metadata(&target) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Ok(metadata) if metadata.is_file() => Some(ContentHashV1::digest(&read_bounded(
                    &target,
                    MAX_EDITABLE_FILE_BYTES,
                )?)),
                _ => return Err(tx_error()),
            };
            Ok(ObservedFileStateV1 {
                path: mutation.frozen.path.clone(),
                current_hash,
            })
        })
        .collect()
}

fn apply_mutation(
    handle: &ProjectHandle,
    mutation: &PreparedAgentMutationV1,
    recovery: &RecoveryStore,
) -> Result<(), AppError> {
    let operation = match mutation.operation {
        AgentWriteOperationV1::Create => CanonFileOperationKind::Create,
        AgentWriteOperationV1::Replace => CanonFileOperationKind::Replace,
    };
    let expected = mutation
        .source_bytes
        .as_deref()
        .map(sha256_hex)
        .unwrap_or_else(|| "absent".to_owned());
    apply_canon_file_recoverable(
        handle,
        operation,
        mutation.classification.path.as_str(),
        &expected,
        &mutation.target_bytes,
        recovery,
    )?;
    Ok(())
}

fn rollback_mutations(
    handle: &ProjectHandle,
    mutations: &[&PreparedAgentMutationV1],
    recovery: &RecoveryStore,
) -> Result<(), AppError> {
    for mutation in mutations.iter().rev() {
        rollback_entry(
            handle,
            mutation.classification.path.as_str(),
            &sha256_hex(&mutation.target_bytes),
            mutation.source_bytes.as_deref(),
            recovery,
        )?;
    }
    Ok(())
}

fn rollback_payload(
    handle: &ProjectHandle,
    payload: &Payload,
    recovery: &RecoveryStore,
) -> Result<(), AppError> {
    for entry in payload.entries.iter().rev() {
        rollback_entry(
            handle,
            &entry.path,
            &entry.expected_current_hash,
            entry.source_bytes.as_deref(),
            recovery,
        )?;
    }
    Ok(())
}

fn rollback_payload_if_partial(
    handle: &ProjectHandle,
    payload: &Payload,
    recovery: &RecoveryStore,
) -> Result<(), AppError> {
    for entry in payload.entries.iter().rev() {
        let target = resolve_scoped_target(handle.root(), &entry.path)?;
        match fs::symlink_metadata(&target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if entry.source_bytes.is_some() {
                    return Err(tx_error());
                }
            }
            Ok(metadata) if metadata.is_file() => {
                let current_hash = sha256_hex(&read_bounded(&target, MAX_EDITABLE_FILE_BYTES)?);
                let source_hash = entry.source_bytes.as_deref().map(sha256_hex);
                if current_hash == entry.expected_current_hash {
                    rollback_entry(
                        handle,
                        &entry.path,
                        &entry.expected_current_hash,
                        entry.source_bytes.as_deref(),
                        recovery,
                    )?;
                } else if source_hash.as_deref() != Some(current_hash.as_str()) {
                    return Err(tx_error());
                }
            }
            _ => return Err(tx_error()),
        }
    }
    Ok(())
}

fn rollback_entry(
    handle: &ProjectHandle,
    path: &str,
    expected: &str,
    source: Option<&[u8]>,
    recovery: &RecoveryStore,
) -> Result<(), AppError> {
    if let Some(source) = source {
        apply_canon_file_recoverable(
            handle,
            CanonFileOperationKind::Replace,
            path,
            expected,
            source,
            recovery,
        )?;
    } else {
        let target = resolve_scoped_target(handle.root(), path)?;
        if sha256_hex(&read_bounded(&target, MAX_EDITABLE_FILE_BYTES)?) != expected {
            return Err(tx_error());
        }
        fs::remove_file(&target).map_err(|_| tx_error())?;
        if fs::symlink_metadata(&target).is_ok() {
            return Err(AppError::project_recovery_required());
        }
    }
    Ok(())
}

fn encode_payload(payload: &Payload) -> Result<Vec<u8>, AppError> {
    let mut output = Vec::new();
    output.extend_from_slice(PAYLOAD_MAGIC);
    output.extend_from_slice(&1_u16.to_le_bytes());
    write_string(&mut output, &payload.project_id)?;
    write_string(&mut output, &payload.transaction_hash)?;
    let count = u16::try_from(payload.entries.len()).map_err(|_| tx_error())?;
    output.extend_from_slice(&count.to_le_bytes());
    for entry in &payload.entries {
        write_string(&mut output, &entry.path)?;
        write_string(&mut output, &entry.expected_current_hash)?;
        match &entry.source_bytes {
            Some(source) => {
                output.push(1);
                let length = u32::try_from(source.len()).map_err(|_| tx_error())?;
                output.extend_from_slice(&length.to_le_bytes());
                output.extend_from_slice(source);
            }
            None => output.push(0),
        }
    }
    if output.len() > MAX_UNDO_BYTES_PER_TURN as usize {
        return Err(tx_error());
    }
    Ok(output)
}

fn decode_payload(bytes: &[u8]) -> Result<Payload, AppError> {
    let mut cursor = 0usize;
    if take(bytes, &mut cursor, PAYLOAD_MAGIC.len())? != PAYLOAD_MAGIC
        || u16::from_le_bytes(take_array(bytes, &mut cursor)?) != 1
    {
        return Err(tx_error());
    }
    let project_id = read_string(bytes, &mut cursor)?;
    let transaction_hash = read_string(bytes, &mut cursor)?;
    let count = u16::from_le_bytes(take_array(bytes, &mut cursor)?) as usize;
    if count == 0
        || count > novelgainker_agent::contracts::MAX_MODIFIED_PROJECT_FILES_PER_TURN as usize
    {
        return Err(tx_error());
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let path = read_string(bytes, &mut cursor)?;
        let expected_current_hash = read_string(bytes, &mut cursor)?;
        let flag = take(bytes, &mut cursor, 1)?[0];
        let source_bytes = match flag {
            0 => None,
            1 => {
                let length = u32::from_le_bytes(take_array(bytes, &mut cursor)?) as usize;
                Some(take(bytes, &mut cursor, length)?.to_vec())
            }
            _ => return Err(tx_error()),
        };
        entries.push(PayloadEntry {
            path,
            expected_current_hash,
            source_bytes,
        });
    }
    if cursor != bytes.len()
        || !entries.windows(2).all(|pair| pair[0].path < pair[1].path)
        || entries.iter().any(|entry| {
            ProjectRelativePathV1::parse(entry.path.clone()).is_err()
                || ContentHashV1::parse(entry.expected_current_hash.clone()).is_err()
        })
    {
        return Err(tx_error());
    }
    Ok(Payload {
        project_id,
        transaction_hash,
        entries,
    })
}

fn write_string(output: &mut Vec<u8>, value: &str) -> Result<(), AppError> {
    let length = u16::try_from(value.len()).map_err(|_| tx_error())?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, AppError> {
    let length = u16::from_le_bytes(take_array(bytes, cursor)?) as usize;
    std::str::from_utf8(take(bytes, cursor, length)?)
        .map(str::to_owned)
        .map_err(|_| tx_error())
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], AppError> {
    let end = cursor
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(tx_error)?;
    let value = &bytes[*cursor..end];
    *cursor = end;
    Ok(value)
}

fn take_array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], AppError> {
    take(bytes, cursor, N)?.try_into().map_err(|_| tx_error())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    fs::create_dir_all(path.parent().ok_or_else(tx_error)?).map_err(|_| tx_error())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| tx_error())?;
    file.write_all(bytes).map_err(|_| tx_error())?;
    file.flush().map_err(|_| tx_error())?;
    file.sync_all().map_err(|_| tx_error())
}

fn write_replace(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(tx_error)?;
    fs::create_dir_all(parent).map_err(|_| tx_error())?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    write_new(&temporary, bytes)?;
    let result = fs::rename(&temporary, path).map_err(|_| tx_error());
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn remove_if_exists(path: &Path) -> Result<(), AppError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(tx_error()),
    }
}

fn validate_component(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(tx_error());
    }
    Ok(())
}

fn agent_project_id(handle: &ProjectHandle) -> Result<String, AppError> {
    let value = format!("prj_{}", handle.project_id().replace('-', ""));
    validate_component(&value)?;
    Ok(value)
}

#[cfg(windows)]
fn protect_for_project(plaintext: &[u8], project_id: &str) -> Result<Vec<u8>, AppError> {
    use std::ptr::null;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: plaintext.len() as u32,
        pbData: plaintext.as_ptr().cast_mut(),
    };
    let entropy_bytes = format!("NovelGainker/agent-undo/v1/{project_id}").into_bytes();
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        CryptProtectData(
            &input,
            null(),
            &entropy,
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() || output.cbData == 0 {
        return Err(tx_error());
    }
    let value =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(value)
}

#[cfg(windows)]
fn unprotect_for_project(encrypted: &[u8], project_id: &str) -> Result<Vec<u8>, AppError> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: encrypted.len() as u32,
        pbData: encrypted.as_ptr().cast_mut(),
    };
    let entropy_bytes = format!("NovelGainker/agent-undo/v1/{project_id}").into_bytes();
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let mut description = null_mut();
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            &mut description,
            &entropy,
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() || output.cbData == 0 {
        return Err(tx_error());
    }
    let value =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe { LocalFree(output.pbData.cast()) };
    if !description.is_null() {
        unsafe { LocalFree(description.cast()) };
    }
    Ok(value)
}

#[cfg(not(windows))]
fn protect_for_project(_: &[u8], _: &str) -> Result<Vec<u8>, AppError> {
    Err(tx_error())
}

#[cfg(not(windows))]
fn unprotect_for_project(_: &[u8], _: &str) -> Result<Vec<u8>, AppError> {
    Err(tx_error())
}

fn tx_error() -> AppError {
    AppError::agent_contract("agent-transaction")
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::agent_write::prepare_write_batch;
    use crate::project::{create_project, inspect_project_root, ProjectScanMode};
    use novelgainker_agent::action_contracts::{
        ProjectToolBindingV1, RegisteredToolIdV1, ToolArgumentsV1, ToolCallIdV1, ToolRequestV1,
        AGENT_ACTION_SCHEMA_VERSION,
    };
    use novelgainker_agent::contracts::{
        AuthorizationPolicyV1, ProjectIdV1, SessionIdV1, TurnIdV1,
    };

    fn fixture() -> (
        PathBuf,
        PathBuf,
        ProjectHandle,
        RecoveryStore,
        AgentUndoStore,
    ) {
        let root = std::env::temp_dir().join(format!("nga202-project-{}", Uuid::new_v4()));
        let state = std::env::temp_dir().join(format!("nga202-state-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
        let handle = create_project(&inspection).unwrap();
        let recovery = RecoveryStore::new(state.clone()).unwrap();
        let undo = AgentUndoStore::new(state.clone()).unwrap();
        (root, state, handle, recovery, undo)
    }

    fn create_request(handle: &ProjectHandle, turn: &str, path: &str) -> ToolRequestV1 {
        let content = if path.starts_with("chapters/") {
            "---\nid: chapter_fixture0001\ntitle: 第一章\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 1\nstatus: draft\n---\n\n# 第一章\n\n## Draft\n\n内容。\n".to_owned()
        } else {
            format!("---\ntitle: {path}\nwritingLanguage: zh-CN\n---\n\n内容。\n")
        };
        ToolRequestV1 {
            schema_version: AGENT_ACTION_SCHEMA_VERSION,
            binding: ProjectToolBindingV1 {
                project_id: ProjectIdV1::parse(agent_project_id(handle).unwrap()).unwrap(),
                session_id: SessionIdV1::parse("ses_fixture0001").unwrap(),
                turn_id: TurnIdV1::parse(turn).unwrap(),
            },
            serial_ordinal: 0,
            call_id: ToolCallIdV1::parse(format!("call_{}", Uuid::new_v4().simple())).unwrap(),
            tool_id: RegisteredToolIdV1::ProjectCreateMarkdown,
            arguments: ToolArgumentsV1 {
                path: Some(ProjectRelativePathV1::parse(path).unwrap()),
                directory: None,
                query: None,
                content: Some(content),
                patch: None,
                expected_source_hash: None,
            },
            purpose: "创建设定".to_owned(),
        }
    }

    fn prepared(handle: &ProjectHandle, turn: &str, paths: &[&str]) -> PreparedAgentWriteV1 {
        let requests = paths
            .iter()
            .map(|path| create_request(handle, turn, path))
            .collect::<Vec<_>>();
        prepare_write_batch(
            handle,
            format!("pnd_{}", Uuid::new_v4().simple()),
            &requests,
            &[
                "create-setting".to_owned(),
                "novel-story-telling".to_owned(),
            ],
            AuthorizationPolicyV1::AutoApplySafe,
        )
        .unwrap()
    }

    #[test]
    fn dpapi_group_commit_and_undo_are_bound_and_atomic() {
        let (root, state, handle, recovery, undo) = fixture();
        let prepared = prepared(
            &handle,
            "trn_fixture0001",
            &["world/a.md", "chapters/001.md"],
        );
        let committed = commit_prepared_agent_write(
            &handle,
            &prepared,
            &recovery,
            &undo,
            EpochMillisV1(1_000),
            0,
        )
        .unwrap();
        assert!(root.join("world/a.md").is_file());
        assert!(root.join("chapters/001.md").is_file());
        assert_eq!(committed.project_write_count, 2);
        assert!(undo
            .blob_path(
                committed.undo_record.project_id.as_str(),
                committed.undo_record.blob_id.as_str(),
            )
            .unwrap()
            .is_file());
        let restored = apply_agent_undo(&handle, &committed.undo_record, &recovery, &undo).unwrap();
        assert_eq!(restored.restored_file_count, 2);
        assert!(!root.join("world/a.md").exists());
        assert!(!root.join("chapters/001.md").exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(state).unwrap();
    }

    #[test]
    fn injected_second_write_failure_rolls_back_first_and_discards_undo() {
        let (root, state, handle, recovery, undo) = fixture();
        let prepared = prepared(
            &handle,
            "trn_fixture0002",
            &["world/a.md", "chapters/001.md"],
        );
        let result = commit_with_hook(
            &handle,
            &prepared,
            &recovery,
            &undo,
            EpochMillisV1(2_000),
            0,
            |index| if index == 1 { Err(tx_error()) } else { Ok(()) },
        );
        assert!(result.is_err());
        assert!(!root.join("world/a.md").exists());
        assert!(!root.join("chapters/001.md").exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(state).unwrap();
    }
}
