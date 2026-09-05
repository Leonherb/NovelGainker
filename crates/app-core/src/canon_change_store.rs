//! NR-402 durable coordinator. Only this Host-owned object can apply a batch.
use crate::canon_apply::{apply_canon_file_recoverable, CanonFileOperationKind};
use crate::canon_change::*;
use crate::conversation::read_conversation;
use crate::coordination::ProjectWriteLease;
use crate::project::ProjectHandle;
use crate::provider_core::CancellationToken;
use crate::recovery::{RecoveryStatus, RecoveryStore};
use crate::workspace::{
    atomic_replace, is_link_or_reparse, list_project_files, read_bounded, read_project_file,
    resolve_scoped_target, save_project_file_recoverable, sha256_hex, ProjectFileNode,
    ProjectFileSaveResult,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use uuid::Uuid;

const MAX_RECORD_BYTES: u64 = 256 * 1024;

mod decision;
mod undo;

/// Concrete production executor; the runtime binding is attached in NR-403.
pub struct NovelistProposeChangeExecutorV1 {
    coordinator: Arc<CanonChangeCoordinator>,
    evidence: Arc<Mutex<ChangeEvidence>>,
}

impl NovelistProposeChangeExecutorV1 {
    pub fn new(
        coordinator: Arc<CanonChangeCoordinator>,
        evidence: Arc<Mutex<ChangeEvidence>>,
    ) -> Self {
        Self {
            coordinator,
            evidence,
        }
    }
}

impl crate::tool_runtime::ToolExecutor for NovelistProposeChangeExecutorV1 {
    fn validate_arguments(&self, arguments: &Value) -> Result<(), &'static str> {
        self.evidence
            .lock()
            .map_err(|_| "agent.tool_executor_failed")?
            .validate_request(arguments)
            .map(|_| ())
            .map_err(|_| "agent.tool_arguments_invalid")
    }

    fn execute(
        &self,
        context: &crate::tool_runtime::ToolExecutionContext,
        arguments: &Value,
        cancellation: &CancellationToken,
    ) -> crate::tool_runtime::ToolExecutionOutput {
        use crate::tool_runtime::{ToolExecutionOutput, ToolResultStatus};
        let result = (|| {
            let evidence = self
                .evidence
                .try_lock()
                .map_err(|_| ChangeError::WriterBusy)?;
            if !evidence.matches_context(
                &context.project_id,
                &context.conversation_session_id,
                &context.turn_id,
            ) {
                return Err(ChangeError::SourceInvalid);
            }
            self.coordinator
                .submit(&evidence, arguments, &context.call_id, cancellation)
        })();
        match result {
            Ok(receipt) => ToolExecutionOutput {
                status: ToolResultStatus::Succeeded,
                output_json: serde_json::to_value(receipt).ok(),
                error_code: None,
            },
            Err(ChangeError::Cancelled) => ToolExecutionOutput {
                status: ToolResultStatus::Cancelled,
                output_json: None,
                error_code: Some("agent.tool_cancelled"),
            },
            Err(error) => ToolExecutionOutput {
                status: ToolResultStatus::Failed,
                output_json: None,
                error_code: Some(error.code()),
            },
        }
    }
}

#[derive(Default)]
struct CoordinatorState {
    buffers: BTreeMap<String, (u64, bool)>,
}

pub struct CanonChangeCoordinator {
    handle: ProjectHandle,
    lease: Arc<ProjectWriteLease>,
    recovery: RecoveryStore,
    state: Arc<Mutex<CoordinatorState>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitStage {
    Prepared,
    BeforeFile(usize),
    AfterFile(usize),
    BeforeCommit,
    Committed,
}

impl CanonChangeCoordinator {
    pub fn new(
        handle: ProjectHandle,
        lease: Arc<ProjectWriteLease>,
        recovery: RecoveryStore,
    ) -> Result<Self, ChangeError> {
        if !lease.is_for_project(&handle.coordination_key()) {
            return Err(ChangeError::TargetInvalid);
        }
        let state = shared_project_state(&handle.coordination_key())?;
        Ok(Self {
            handle,
            lease,
            recovery,
            state,
        })
    }

    pub fn project_id(&self) -> &str {
        self.handle.project_id()
    }

    pub fn submit(
        &self,
        evidence: &ChangeEvidence,
        value: &Value,
        call_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<ChangeReceipt, ChangeError> {
        self.submit_with_hook(evidence, value, call_id, cancellation, |_| Ok(()))
    }

    fn submit_with_hook(
        &self,
        evidence: &ChangeEvidence,
        value: &Value,
        call_id: &str,
        cancellation: &CancellationToken,
        mut hook: impl FnMut(CommitStage) -> Result<(), ChangeError>,
    ) -> Result<ChangeReceipt, ChangeError> {
        check_cancelled(cancellation)?;
        let state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        self.verify_lease()?;
        let session = read_conversation(&self.handle).map_err(|_| ChangeError::SourceInvalid)?;
        let turn = session.turns.last().ok_or(ChangeError::SourceInvalid)?;
        if !evidence.matches_context(self.project_id(), &session.session_id, &turn.turn_id) {
            return Err(ChangeError::SourceInvalid);
        }
        let validated = evidence.validate_request(value)?;
        let fresh = ChangeEvidence::from_conversation(&session, &turn.turn_id)?;
        let fresh_sources = fresh.source_references();
        if validated
            .request
            .sources
            .iter()
            .any(|source| !fresh_sources.contains(source))
        {
            return Err(ChangeError::SourceInvalid);
        }
        let records = self.records()?;
        for record in &records {
            if record.conversation_session_id == session.session_id
                && record.turn_id == turn.turn_id
                && record.call_id == call_id
            {
                return if record.request_hash == validated.request_hash {
                    Ok(record.receipt.clone())
                } else {
                    Err(ChangeError::RevisionConflict)
                };
            }
        }
        ensure_recovered(&records)?;
        if validated.eligibility == ChangeEligibility::ConversationOnly {
            return Ok(empty_receipt(ChangeStatus::Rejected));
        }
        if validated.request.risk == ChangeRisk::AmbiguousTarget {
            return Err(ChangeError::TargetInvalid);
        }
        let mut snapshots = Vec::new();
        let mut files = Vec::new();
        let mut confirmation = validated.eligibility == ChangeEligibility::ConfirmationRequired;
        for operation in &validated.request.operations {
            check_cancelled(cancellation)?;
            let (relative_path, before, after) = match operation.target_id {
                Some(_) => {
                    let path = operation
                        .read_receipt_id
                        .as_deref()
                        .and_then(|id| evidence.target_path(id))
                        .ok_or(ChangeError::IncompleteRead)?;
                    let current = read_project_file(&self.handle, path)
                        .map_err(|_| ChangeError::SourceChanged)?;
                    if operation.expected_source_hash.as_deref() != Some(&current.content_hash)
                        || current.size_bytes as usize > MAX_CHANGE_FILE_BYTES
                        || sha256_hex(current.content.as_bytes()) != current.content_hash
                    {
                        return Err(ChangeError::SourceChanged);
                    }
                    confirmation |= state
                        .buffers
                        .get(&path.to_lowercase())
                        .is_some_and(|(_, dirty)| *dirty);
                    (
                        path.to_owned(),
                        Some(current.content),
                        operation.proposed_markdown.clone(),
                    )
                }
                None => {
                    self.check_new_identity(operation, cancellation)?;
                    let identity = Uuid::new_v4().to_string();
                    let after = assign_new_identity(&operation.proposed_markdown, &identity)?;
                    (
                        format!("{}/{identity}.md", operation.entity_kind.directory()),
                        None,
                        after,
                    )
                }
            };
            operation.entity_kind.validate_path(&relative_path)?;
            validate_candidate(operation.entity_kind, &after)?;
            files.push(ChangeFileReceipt {
                relative_path: relative_path.clone(),
                before_hash: before.as_ref().map(|text| sha256_hex(text.as_bytes())),
                after_hash: sha256_hex(after.as_bytes()),
            });
            snapshots.push(ChangeSnapshot {
                entity_kind: operation.entity_kind,
                relative_path,
                before,
                after,
            });
        }
        let mut record = ChangeRecord {
            schema_version: CHANGE_SCHEMA_VERSION,
            project_id: self.project_id().to_owned(),
            conversation_session_id: session.session_id,
            turn_id: turn.turn_id.clone(),
            call_id: call_id.to_owned(),
            request_hash: validated.request_hash,
            source_kind: validated.request.source_kind,
            sources: validated.request.sources,
            receipt: ChangeReceipt {
                schema_version: CHANGE_SCHEMA_VERSION,
                change_id: Uuid::new_v4().to_string(),
                revision: 1,
                status: ChangeStatus::Prepared,
                files,
            },
            editor_revisions: snapshots
                .iter()
                .filter_map(|snapshot| {
                    state
                        .buffers
                        .get(&snapshot.relative_path.to_lowercase())
                        .map(|(revision, _)| (snapshot.relative_path.clone(), *revision))
                })
                .collect(),
            snapshots,
            undo_revision: None,
        };
        if record
            .snapshots
            .iter()
            .all(|file| file.before.as_deref() == Some(&file.after))
        {
            record.receipt.status = ChangeStatus::NoChange;
        } else if confirmation {
            record.receipt.status = ChangeStatus::PendingConfirmation;
        }
        self.save_record(&record)?;
        if record.receipt.status != ChangeStatus::Prepared {
            return Ok(record.receipt);
        }
        hook(CommitStage::Prepared)?;
        self.apply_record(&mut record, cancellation, &mut hook)?;
        Ok(record.receipt)
    }

    fn apply_record(
        &self,
        record: &mut ChangeRecord,
        cancellation: &CancellationToken,
        hook: &mut impl FnMut(CommitStage) -> Result<(), ChangeError>,
    ) -> Result<(), ChangeError> {
        record.receipt.status = ChangeStatus::Applying;
        record.receipt.revision += 1;
        self.save_record(record)?;
        let outcome = (|| {
            for (index, snapshot) in record.snapshots.iter().enumerate() {
                check_cancelled(cancellation)?;
                hook(CommitStage::BeforeFile(index))?;
                self.verify_lease()?;
                let before_hash = record.receipt.files[index]
                    .before_hash
                    .as_deref()
                    .unwrap_or("absent");
                apply_canon_file_recoverable(
                    &self.handle,
                    if snapshot.before.is_some() {
                        CanonFileOperationKind::Replace
                    } else {
                        CanonFileOperationKind::Create
                    },
                    &snapshot.relative_path,
                    before_hash,
                    snapshot.after.as_bytes(),
                    &self.recovery,
                )
                .map_err(|error| {
                    if error.code == "file.source_changed" {
                        ChangeError::SourceChanged
                    } else {
                        ChangeError::Io
                    }
                })?;
                hook(CommitStage::AfterFile(index))?;
            }
            check_cancelled(cancellation)?;
            hook(CommitStage::BeforeCommit)?;
            // Verify the whole batch again: an external editor does not share our lock.
            for (snapshot, receipt) in record.snapshots.iter().zip(&record.receipt.files) {
                if self.current_hash(&snapshot.relative_path)?.as_deref()
                    != Some(&receipt.after_hash)
                {
                    return Err(ChangeError::SourceChanged);
                }
            }
            record.receipt.status = ChangeStatus::Committed;
            record.receipt.revision += 1;
            self.save_record(record)
        })();
        if let Err(error) = outcome {
            // A commit marker may have reached disk even if the acknowledgement
            // failed. Never roll back a durable committed transaction.
            if self
                .load_record(&record.receipt.change_id)
                .is_ok_and(|stored| stored.receipt.status == ChangeStatus::Committed)
            {
                record.receipt.status = ChangeStatus::Committed;
                return Ok(());
            }
            self.rollback(record)?;
            return Err(error);
        }
        // Failure after commit cannot change the recorded Canon outcome.
        let _ = hook(CommitStage::Committed);
        Ok(())
    }

    fn rollback(&self, record: &mut ChangeRecord) -> Result<(), ChangeError> {
        let result = (|| {
            if self.recovery.recover_project(&self.handle).status == RecoveryStatus::Required {
                return Err(ChangeError::RecoveryRequired);
            }
            for (snapshot, receipt) in record.snapshots.iter().zip(&record.receipt.files).rev() {
                let current = self.current_hash(&snapshot.relative_path)?;
                if current == receipt.before_hash {
                    continue;
                }
                if current.as_deref() != Some(&receipt.after_hash) {
                    return Err(ChangeError::RecoveryRequired);
                }
                match &snapshot.before {
                    Some(before) => {
                        apply_canon_file_recoverable(
                            &self.handle,
                            CanonFileOperationKind::Replace,
                            &snapshot.relative_path,
                            &receipt.after_hash,
                            before.as_bytes(),
                            &self.recovery,
                        )
                        .map_err(|_| ChangeError::RecoveryRequired)?;
                    }
                    None => {
                        let target =
                            resolve_scoped_target(self.handle.root(), &snapshot.relative_path)
                                .map_err(|_| ChangeError::RecoveryRequired)?;
                        if self.current_hash(&snapshot.relative_path)?.as_deref()
                            != Some(&receipt.after_hash)
                        {
                            return Err(ChangeError::RecoveryRequired);
                        }
                        fs::remove_file(target).map_err(|_| ChangeError::RecoveryRequired)?;
                    }
                }
            }
            Ok(())
        })();
        record.receipt.status = if result.is_ok() {
            ChangeStatus::Cancelled
        } else {
            ChangeStatus::RecoveryRequired
        };
        record.receipt.revision += 1;
        self.save_record(record)
            .map_err(|_| ChangeError::RecoveryRequired)?;
        result
    }

    /// Called on open before allowing any further writes. Does not replay a
    /// Provider request or silently finish a batch lacking its commit marker.
    pub fn recover(&self) -> Result<Vec<ChangeReceipt>, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        self.verify_lease()?;
        let mut receipts = Vec::new();
        for mut record in self.records()? {
            match record.receipt.status {
                ChangeStatus::Prepared
                | ChangeStatus::Applying
                | ChangeStatus::RecoveryRequired => {
                    if record.undo_revision.is_some() {
                        self.rollback_undo(&mut record)?;
                    } else {
                        self.rollback(&mut record)?;
                    }
                }
                _ => {}
            }
            receipts.push(record.receipt);
        }
        Ok(receipts)
    }

    pub fn receipts(&self) -> Result<Vec<ChangeReceipt>, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        Ok(self
            .records()?
            .into_iter()
            .map(|record| record.receipt)
            .collect())
    }

    pub fn record_editor_buffer(
        &self,
        path: &str,
        revision: u64,
        dirty: bool,
    ) -> Result<(), ChangeError> {
        crate::workspace::validate_writable_relative_path(path)
            .map_err(|_| ChangeError::TargetInvalid)?;
        let mut state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        if revision == 0
            || state
                .buffers
                .get(&path.to_lowercase())
                .is_some_and(|(old, _)| *old >= revision)
        {
            return Err(ChangeError::RevisionConflict);
        }
        if state.buffers.len() >= 64 && !state.buffers.contains_key(&path.to_lowercase()) {
            return Err(ChangeError::BudgetExceeded);
        }
        state.buffers.insert(path.to_lowercase(), (revision, dirty));
        Ok(())
    }

    /// Reserve a manual editing buffer before its bytes reach an editable UI.
    /// The frontend releases this conservative reservation only when clean.
    pub fn open_editor(
        &self,
        path: &str,
    ) -> Result<(crate::workspace::ProjectFileDocument, u64), ChangeError> {
        let mut state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        let document = read_project_file(&self.handle, path).map_err(|_| ChangeError::Io)?;
        if document.read_only {
            return Ok((document, 0));
        }
        if state.buffers.len() >= 64 && !state.buffers.contains_key(&path.to_lowercase()) {
            return Err(ChangeError::BudgetExceeded);
        }
        let revision = state
            .buffers
            .get(&path.to_lowercase())
            .map_or(1, |(old, _)| old.saturating_add(1));
        state.buffers.insert(path.to_lowercase(), (revision, true));
        Ok((document, revision))
    }

    /// Host manual-save path shares the transaction mutex with AI writes.
    pub fn save_manual(
        &self,
        path: &str,
        content: &str,
        source_hash: &str,
    ) -> Result<ProjectFileSaveResult, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        self.verify_lease()?;
        ensure_recovered(&self.records()?)?;
        save_project_file_recoverable(&self.handle, path, content, source_hash, &self.recovery)
            .map_err(|error| {
                if error.code == "file.source_changed" {
                    ChangeError::SourceChanged
                } else {
                    ChangeError::Io
                }
            })
    }

    pub fn read_consistent<T>(
        &self,
        read: impl FnOnce() -> Result<T, ChangeError>,
    ) -> Result<T, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        ensure_recovered(&self.records()?)?;
        read()
    }

    /// Read-only access remains available during unresolved recovery. The caller
    /// must mark this snapshot inconsistent and must not issue mutation receipts.
    pub fn read_snapshot<T>(
        &self,
        read: impl FnOnce(bool) -> Result<T, ChangeError>,
    ) -> Result<T, ChangeError> {
        let _state = self.state.try_lock().map_err(|_| ChangeError::WriterBusy)?;
        let consistent = self
            .records()
            .and_then(|records| ensure_recovered(&records))
            .is_ok();
        read(consistent)
    }

    fn verify_lease(&self) -> Result<(), ChangeError> {
        if !self.lease.is_for_project(&self.handle.coordination_key()) {
            return Err(ChangeError::TargetInvalid);
        }
        let inspection = crate::project::inspect_project_root(
            self.handle.root(),
            crate::project::ProjectScanMode::Open,
        )
        .map_err(|_| ChangeError::TargetInvalid)?;
        let current = crate::project::open_project(&inspection, false)
            .map_err(|_| ChangeError::TargetInvalid)?;
        if current.project_id() != self.project_id() {
            return Err(ChangeError::TargetInvalid);
        }
        Ok(())
    }

    fn check_new_identity(
        &self,
        operation: &CanonMutation,
        cancellation: &CancellationToken,
    ) -> Result<(), ChangeError> {
        let title = metadata_field(&operation.proposed_markdown, "title")
            .ok_or(ChangeError::CandidateInvalid)?;
        let index = list_project_files(&self.handle).map_err(|_| ChangeError::TargetInvalid)?;
        let mut paths = Vec::new();
        collect_paths(&index.nodes, &mut paths);
        for path in paths
            .into_iter()
            .filter(|path| path.starts_with(&format!("{}/", operation.entity_kind.directory())))
        {
            check_cancelled(cancellation)?;
            let file =
                read_project_file(&self.handle, &path).map_err(|_| ChangeError::TargetInvalid)?;
            if file.size_bytes as usize > MAX_CHANGE_FILE_BYTES {
                return Err(ChangeError::BudgetExceeded);
            }
            if metadata_field(&file.content, "title")
                .is_none_or(|existing| existing.to_lowercase() == title.to_lowercase())
            {
                return Err(ChangeError::DuplicateTarget);
            }
        }
        Ok(())
    }

    fn current_hash(&self, path: &str) -> Result<Option<String>, ChangeError> {
        let target = resolve_scoped_target(self.handle.root(), path)
            .map_err(|_| ChangeError::TargetInvalid)?;
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => {
                let bytes = read_bounded(&target, MAX_CHANGE_FILE_BYTES as u64)
                    .map_err(|_| ChangeError::SourceChanged)?;
                Ok(Some(sha256_hex(&bytes)))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            _ => Err(ChangeError::TargetInvalid),
        }
    }

    fn directory(&self) -> Result<PathBuf, ChangeError> {
        let mut path = self.handle.root().to_path_buf();
        for component in [".novelgainker", "changes"] {
            path.push(component);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&path).map_err(|_| ChangeError::Io)?;
                }
                _ => return Err(ChangeError::TargetInvalid),
            }
            let canonical = path.canonicalize().map_err(|_| ChangeError::Io)?;
            if !canonical.starts_with(self.handle.root()) {
                return Err(ChangeError::TargetInvalid);
            }
        }
        Ok(path)
    }

    fn records(&self) -> Result<Vec<ChangeRecord>, ChangeError> {
        let directory = self.directory()?;
        let mut records = Vec::new();
        let mut total = 0u64;
        for (index, entry) in fs::read_dir(directory)
            .map_err(|_| ChangeError::Io)?
            .enumerate()
        {
            if index >= MAX_CHANGE_RECORDS * 3 {
                return Err(ChangeError::StoreFull);
            }
            let entry = entry.map_err(|_| ChangeError::Io)?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|_| ChangeError::Io)?;
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(ChangeError::RecoveryRequired);
            }
            total = total
                .checked_add(metadata.len())
                .ok_or(ChangeError::StoreFull)?;
            if total > MAX_CHANGE_STORE_BYTES {
                return Err(ChangeError::StoreFull);
            }
            let name = entry
                .file_name()
                .to_str()
                .ok_or(ChangeError::RecoveryRequired)?
                .to_owned();
            // Interrupted atomic state writes contain no Canon side effect.
            if name.starts_with(".record-") && name.ends_with(".tmp") {
                continue;
            }
            let change_id = name
                .strip_suffix(".json")
                .ok_or(ChangeError::RecoveryRequired)?;
            records.push(self.load_record(change_id)?);
            if records.len() > MAX_CHANGE_RECORDS {
                return Err(ChangeError::StoreFull);
            }
        }
        Ok(records)
    }

    fn load_record(&self, change_id: &str) -> Result<ChangeRecord, ChangeError> {
        if Uuid::parse_str(change_id).is_err() {
            return Err(ChangeError::TargetInvalid);
        }
        let target = self.directory()?.join(format!("{change_id}.json"));
        let metadata = fs::symlink_metadata(&target).map_err(|_| ChangeError::Io)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(ChangeError::RecoveryRequired);
        }
        let bytes =
            read_bounded(&target, MAX_RECORD_BYTES).map_err(|_| ChangeError::RecoveryRequired)?;
        let record: ChangeRecord =
            serde_json::from_slice(&bytes).map_err(|_| ChangeError::RecoveryRequired)?;
        record.validate()?;
        if record.project_id != self.project_id() || record.receipt.change_id != change_id {
            return Err(ChangeError::RecoveryRequired);
        }
        Ok(record)
    }

    fn save_record(&self, record: &ChangeRecord) -> Result<(), ChangeError> {
        record.validate()?;
        if record.project_id != self.project_id() {
            return Err(ChangeError::TargetInvalid);
        }
        let bytes = serde_json::to_vec(record).map_err(|_| ChangeError::Io)?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(ChangeError::StoreFull);
        }
        let records = self.records()?;
        let exists = records
            .iter()
            .any(|old| old.receipt.change_id == record.receipt.change_id);
        if !exists
            && (records.len() >= MAX_CHANGE_RECORDS
                || record.receipt.status == ChangeStatus::PendingConfirmation
                    && records
                        .iter()
                        .filter(|r| r.receipt.status == ChangeStatus::PendingConfirmation)
                        .count()
                        >= MAX_PENDING_CHANGES)
        {
            return Err(ChangeError::StoreFull);
        }
        // Include orphaned atomic-write temporaries and actual on-disk JSON
        // whitespace, not merely the size of reserialized valid records.
        let directory = self.directory()?;
        let mut stored_bytes = 0u64;
        let mut entries = 0usize;
        for entry in fs::read_dir(&directory).map_err(|_| ChangeError::Io)? {
            let entry = entry.map_err(|_| ChangeError::Io)?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|_| ChangeError::Io)?;
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(ChangeError::RecoveryRequired);
            }
            entries += 1;
            if entry.file_name() != format!("{}.json", record.receipt.change_id).as_str() {
                stored_bytes = stored_bytes
                    .checked_add(metadata.len())
                    .ok_or(ChangeError::StoreFull)?;
            }
        }
        // Leave room for a replacement and its recovery/Undo status update.
        if stored_bytes + 3 * MAX_RECORD_BYTES > MAX_CHANGE_STORE_BYTES
            || entries >= MAX_CHANGE_RECORDS * 3 - 3
        {
            return Err(ChangeError::StoreFull);
        }
        let target = directory.join(format!("{}.json", record.receipt.change_id));
        if let Ok(metadata) = fs::symlink_metadata(&target) {
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(ChangeError::TargetInvalid);
            }
        }
        let temporary = directory.join(format!(".record-{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|_| ChangeError::Io)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| ChangeError::Io)?;
            drop(file);
            if self.directory()? != directory {
                return Err(ChangeError::TargetInvalid);
            }
            atomic_replace(&temporary, &target).map_err(|_| ChangeError::Io)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn check_cancelled(token: &CancellationToken) -> Result<(), ChangeError> {
    if token.is_cancelled() {
        Err(ChangeError::Cancelled)
    } else {
        Ok(())
    }
}

/// Read-only startup reconciliation. A missing/corrupt receipt is uncertainty,
/// not evidence that a write did not happen.
pub fn task_recovery_result(
    handle: &ProjectHandle,
    session_id: &str,
    turn_id: &str,
    call_id_hash: &str,
) -> Result<crate::tool_runtime::ToolResult, ChangeError> {
    use crate::tool_runtime::{ToolResult, ToolResultStatus};
    let mut directory = handle.root().to_path_buf();
    for component in [".novelgainker", "changes"] {
        directory.push(component);
        let metadata =
            fs::symlink_metadata(&directory).map_err(|_| ChangeError::RecoveryRequired)?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(ChangeError::RecoveryRequired);
        }
    }
    let mut found = None;
    let mut total = 0u64;
    for (index, entry) in fs::read_dir(&directory)
        .map_err(|_| ChangeError::RecoveryRequired)?
        .enumerate()
    {
        if index >= MAX_CHANGE_RECORDS * 3 {
            return Err(ChangeError::StoreFull);
        }
        let entry = entry.map_err(|_| ChangeError::RecoveryRequired)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| ChangeError::RecoveryRequired)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(ChangeError::RecoveryRequired);
        }
        total = total.saturating_add(metadata.len());
        if total > MAX_CHANGE_STORE_BYTES {
            return Err(ChangeError::StoreFull);
        }
        if entry
            .path()
            .extension()
            .is_none_or(|extension| extension != "json")
        {
            continue;
        }
        let bytes = read_bounded(&entry.path(), MAX_RECORD_BYTES)
            .map_err(|_| ChangeError::RecoveryRequired)?;
        let record: ChangeRecord =
            serde_json::from_slice(&bytes).map_err(|_| ChangeError::RecoveryRequired)?;
        record.validate()?;
        if record.project_id != handle.project_id()
            || entry.file_name().to_str() != Some(&format!("{}.json", record.receipt.change_id))
        {
            return Err(ChangeError::RecoveryRequired);
        }
        if record.conversation_session_id != session_id
            || record.turn_id != turn_id
            || sha256_hex(record.call_id.as_bytes()) != call_id_hash
        {
            continue;
        }
        if found.is_some() {
            return Err(ChangeError::RecoveryRequired);
        }
        let (status, error_code) = match record.receipt.status {
            ChangeStatus::Committed
            | ChangeStatus::PendingConfirmation
            | ChangeStatus::NoChange
            | ChangeStatus::Rejected
            | ChangeStatus::Undone => (ToolResultStatus::Succeeded, None),
            ChangeStatus::Cancelled => (
                ToolResultStatus::Cancelled,
                Some("agent.tool_cancelled".to_owned()),
            ),
            _ => return Err(ChangeError::RecoveryRequired),
        };
        found = Some(ToolResult {
            call_id: record.call_id,
            name: PROPOSE_CHANGE_TOOL_NAME.to_owned(),
            version: 1,
            status,
            output_json: if status == ToolResultStatus::Succeeded {
                Some(
                    serde_json::to_value(record.receipt)
                        .map_err(|_| ChangeError::RecoveryRequired)?,
                )
            } else {
                None
            },
            error_code,
        });
    }
    found.ok_or(ChangeError::RecoveryRequired)
}

fn ensure_recovered(records: &[ChangeRecord]) -> Result<(), ChangeError> {
    if records.iter().any(|record| {
        matches!(
            record.receipt.status,
            ChangeStatus::Prepared | ChangeStatus::Applying | ChangeStatus::RecoveryRequired
        )
    }) {
        Err(ChangeError::RecoveryRequired)
    } else {
        Ok(())
    }
}

type SharedProjectStates = Mutex<BTreeMap<String, Weak<Mutex<CoordinatorState>>>>;

fn shared_project_state(key: &str) -> Result<Arc<Mutex<CoordinatorState>>, ChangeError> {
    static STATES: OnceLock<SharedProjectStates> = OnceLock::new();
    let mut states = STATES
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .map_err(|_| ChangeError::WriterBusy)?;
    states.retain(|_, value| value.strong_count() != 0);
    if let Some(state) = states.get(key).and_then(Weak::upgrade) {
        return Ok(state);
    }
    if states.len() >= 64 {
        return Err(ChangeError::BudgetExceeded);
    }
    let state = Arc::new(Mutex::new(CoordinatorState::default()));
    states.insert(key.to_owned(), Arc::downgrade(&state));
    Ok(state)
}

fn empty_receipt(status: ChangeStatus) -> ChangeReceipt {
    ChangeReceipt {
        schema_version: CHANGE_SCHEMA_VERSION,
        change_id: Uuid::new_v4().to_string(),
        revision: 1,
        status,
        files: Vec::new(),
    }
}

fn assign_new_identity(content: &str, identity: &str) -> Result<String, ChangeError> {
    let mut lines: Vec<&str> = content.lines().collect();
    let end = lines
        .iter()
        .skip(1)
        .position(|line| *line == "---")
        .ok_or(ChangeError::CandidateInvalid)?
        + 1;
    let index = (1..end)
        .find(|index| lines[*index].starts_with("id:"))
        .ok_or(ChangeError::CandidateInvalid)?;
    let id_line = format!("id: {identity}");
    lines[index] = &id_line;
    Ok(format!("{}\n", lines.join("\n")))
}

fn collect_paths(nodes: &[ProjectFileNode], paths: &mut Vec<String>) {
    for node in nodes {
        if node.relative_path.ends_with(".md") && node.children.is_empty() {
            paths.push(node.relative_path.clone());
        }
        collect_paths(&node.children, paths);
    }
}

#[cfg(test)]
mod tests;
