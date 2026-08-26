use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::context::{
    CompiledWorkflowPrompt, ContextManifestV1, ContextSourceBinding, PreparedCharacterContext,
};
use crate::proposal::{CanonProposalViewV1, RetainedCanonProposal};
use crate::{registry, CHARACTER_CREATE_WORKFLOW_ID};

pub const WORKFLOW_RUN_SCHEMA_VERSION: u16 = 1;
pub const MAX_RETAINED_TERMINAL_RUNS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    Created,
    Routed,
    AwaitingInput,
    ContextReady,
    Generating,
    ProposalReady,
    AwaitingConfirmation,
    Applying,
    Validating,
    Completed,
    Failed,
    Cancelled,
    RecoveryRequired,
}

impl WorkflowRunState {
    pub const ALL: [Self; 13] = [
        Self::Created,
        Self::Routed,
        Self::AwaitingInput,
        Self::ContextReady,
        Self::Generating,
        Self::ProposalReady,
        Self::AwaitingConfirmation,
        Self::Applying,
        Self::Validating,
        Self::Completed,
        Self::Failed,
        Self::Cancelled,
        Self::RecoveryRequired,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Routed => "routed",
            Self::AwaitingInput => "awaiting_input",
            Self::ContextReady => "context_ready",
            Self::Generating => "generating",
            Self::ProposalReady => "proposal_ready",
            Self::AwaitingConfirmation => "awaiting_confirmation",
            Self::Applying => "applying",
            Self::Validating => "validating",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::RecoveryRequired => "recovery_required",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub const fn can_cancel(self) -> bool {
        matches!(self, Self::Generating)
    }

    pub const fn can_abandon(self) -> bool {
        !self.is_terminal() && !matches!(self, Self::Applying | Self::Validating)
    }
}

impl TryFrom<&str> for WorkflowRunState {
    type Error = WorkflowManagerError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|state| state.as_str() == value)
            .ok_or(WorkflowManagerError::InvalidRecord)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunRecord {
    pub run_id: String,
    pub project_id: String,
    pub workflow_id: String,
    pub workflow_version: u16,
    pub pack_hash: String,
    pub state: WorkflowRunState,
    pub sequence: u64,
    pub provider_id: Option<String>,
    pub preset_version: Option<u16>,
    pub model: Option<String>,
    pub manifest_hash: Option<String>,
    pub proposal_hash: Option<String>,
    pub target_relative_path: Option<String>,
    pub target_source_hash: Option<String>,
    pub confirmed_at: Option<i64>,
    pub updated_at: i64,
}

impl WorkflowRunRecord {
    pub fn validate(&self) -> Result<(), WorkflowManagerError> {
        self.validate_common()?;
        let workflow = registry()
            .workflows
            .iter()
            .find(|workflow| workflow.id == self.workflow_id)
            .ok_or(WorkflowManagerError::InvalidRecord)?;
        if workflow.version != self.workflow_version || registry().pack_hash != self.pack_hash {
            return Err(WorkflowManagerError::InvalidRecord);
        }
        Ok(())
    }

    pub fn validate_persisted_checkpoint(&self) -> Result<(), WorkflowManagerError> {
        self.validate_common()?;
        let matches_current_pack = registry().workflows.iter().any(|workflow| {
            workflow.id == self.workflow_id
                && workflow.version == self.workflow_version
                && registry().pack_hash == self.pack_hash
        });
        let is_terminal_character_v1_history = self.workflow_id == CHARACTER_CREATE_WORKFLOW_ID
            && self.workflow_version == 1
            && self.state.is_terminal();
        if !matches_current_pack && !is_terminal_character_v1_history {
            return Err(WorkflowManagerError::InvalidRecord);
        }
        Ok(())
    }

    fn validate_common(&self) -> Result<(), WorkflowManagerError> {
        if Uuid::parse_str(&self.run_id).is_err()
            || Uuid::parse_str(&self.project_id).is_err()
            || !is_sha256(&self.pack_hash)
            || self.updated_at < 0
            || self.confirmed_at.is_some_and(|timestamp| timestamp < 0)
        {
            return Err(WorkflowManagerError::InvalidRecord);
        }

        let provider_snapshot_valid = matches!(
            (
                self.provider_id.as_deref(),
                self.preset_version,
                self.model.as_deref(),
            ),
            (None, None, None)
                | (Some("deepseek"), Some(1), Some("deepseek-v4-pro"))
                | (Some("openai"), Some(1), Some("gpt-5.6-terra"))
        );
        if !provider_snapshot_valid
            || !self.manifest_hash.as_deref().is_none_or(is_sha256)
            || !self.proposal_hash.as_deref().is_none_or(is_sha256)
            || !self.target_source_hash.as_deref().is_none_or(is_sha256)
            || !self
                .target_relative_path
                .as_deref()
                .is_none_or(is_safe_relative_path)
        {
            return Err(WorkflowManagerError::InvalidRecord);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowManagerError {
    InvalidRecord,
    RunConflict,
    RunNotFound,
    InvalidState,
    StaleEvent,
    TerminalEvent,
    ConfirmationRequired,
    ManifestMismatch,
    ProposalMismatch,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct SensitiveRunMemory {
    prepared: Option<PreparedCharacterContext>,
    proposal: Option<RetainedCanonProposal>,
}

impl SensitiveRunMemory {
    fn clear(&mut self) {
        self.prepared = None;
        self.proposal = None;
    }
}

impl fmt::Debug for SensitiveRunMemory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SensitiveRunMemory")
            .field("prepared", &self.prepared.is_some())
            .field("proposal", &self.proposal.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Debug, Clone)]
struct ManagedWorkflowRun {
    record: WorkflowRunRecord,
    sensitive: SensitiveRunMemory,
}

#[derive(Clone, PartialEq, Eq)]
pub struct WorkflowGenerationMaterial {
    pub prompt: CompiledWorkflowPrompt,
    pub source_bindings: Vec<ContextSourceBinding>,
}

impl fmt::Debug for WorkflowGenerationMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkflowGenerationMaterial")
            .field("prompt", &"[REDACTED]")
            .field("source_bindings", &self.source_bindings)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WorkflowProposalApplyMaterial {
    pub proposal: RetainedCanonProposal,
    pub source_bindings: Vec<ContextSourceBinding>,
}

impl fmt::Debug for WorkflowProposalApplyMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkflowProposalApplyMaterial")
            .field("proposal", &"[REDACTED]")
            .field("source_bindings", &self.source_bindings)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowManager {
    runs: BTreeMap<String, ManagedWorkflowRun>,
    active_run_id: Option<String>,
}

impl WorkflowManager {
    pub fn restore(
        records: Vec<WorkflowRunRecord>,
        recovered_at: i64,
    ) -> Result<(Self, Vec<WorkflowRunRecord>), WorkflowManagerError> {
        if recovered_at < 0 {
            return Err(WorkflowManagerError::InvalidRecord);
        }
        let mut manager = Self::default();
        let mut recovered = Vec::new();
        for mut record in records {
            record.validate()?;
            if manager.runs.contains_key(&record.run_id) {
                return Err(WorkflowManagerError::InvalidRecord);
            }
            if !record.state.is_terminal() {
                if manager.active_run_id.is_some() {
                    return Err(WorkflowManagerError::RunConflict);
                }
                if record.state != WorkflowRunState::RecoveryRequired {
                    record.state = WorkflowRunState::RecoveryRequired;
                    record.sequence = record.sequence.saturating_add(1);
                    record.updated_at = recovered_at;
                    recovered.push(record.clone());
                }
                manager.active_run_id = Some(record.run_id.clone());
            }
            manager.runs.insert(
                record.run_id.clone(),
                ManagedWorkflowRun {
                    record,
                    sensitive: SensitiveRunMemory::default(),
                },
            );
        }
        manager.prune_terminal_runs();
        Ok((manager, recovered))
    }

    pub fn create(&mut self, record: WorkflowRunRecord) -> Result<(), WorkflowManagerError> {
        record.validate()?;
        if record.state != WorkflowRunState::Created || record.sequence != 0 {
            return Err(WorkflowManagerError::InvalidRecord);
        }
        if self.active_run_id.is_some() {
            return Err(WorkflowManagerError::RunConflict);
        }
        if self.runs.contains_key(&record.run_id) {
            return Err(WorkflowManagerError::InvalidRecord);
        }
        self.active_run_id = Some(record.run_id.clone());
        self.runs.insert(
            record.run_id.clone(),
            ManagedWorkflowRun {
                record,
                sensitive: SensitiveRunMemory::default(),
            },
        );
        Ok(())
    }

    pub fn create_prepared(
        &mut self,
        record: WorkflowRunRecord,
        prepared: PreparedCharacterContext,
    ) -> Result<(), WorkflowManagerError> {
        if record.manifest_hash.as_deref() != Some(prepared.manifest.manifest_hash.as_str())
            || record.run_id != prepared.manifest.run_id
            || record.project_id != prepared.manifest.project_id
            || record.workflow_id != prepared.manifest.workflow_id
            || record.workflow_version != prepared.manifest.workflow_version
            || record.pack_hash != prepared.manifest.pack_hash
            || record.provider_id.as_deref() != Some(prepared.manifest.provider_id.as_str())
            || record.preset_version != Some(prepared.manifest.preset_version)
            || record.model.as_deref() != Some(prepared.manifest.model.as_str())
        {
            return Err(WorkflowManagerError::ManifestMismatch);
        }
        self.create(record.clone())?;
        let Some(run) = self.runs.get_mut(&record.run_id) else {
            return Err(WorkflowManagerError::RunNotFound);
        };
        run.sensitive.prepared = Some(prepared);
        Ok(())
    }

    pub fn advance_prepared_to_context_ready(
        &mut self,
        run_id: &str,
        updated_at: i64,
    ) -> Result<WorkflowRunRecord, WorkflowManagerError> {
        for next in [
            WorkflowRunState::Routed,
            WorkflowRunState::AwaitingInput,
            WorkflowRunState::ContextReady,
        ] {
            let sequence = self
                .record(run_id)
                .ok_or(WorkflowManagerError::RunNotFound)?
                .sequence;
            self.transition(run_id, sequence.saturating_add(1), next, updated_at)?;
        }
        self.record(run_id).ok_or(WorkflowManagerError::RunNotFound)
    }

    pub fn transition(
        &mut self,
        run_id: &str,
        event_sequence: u64,
        next: WorkflowRunState,
        updated_at: i64,
    ) -> Result<WorkflowRunRecord, WorkflowManagerError> {
        if updated_at < 0 {
            return Err(WorkflowManagerError::InvalidRecord);
        }
        let run = self
            .runs
            .get_mut(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        if run.record.state.is_terminal() {
            return Err(WorkflowManagerError::TerminalEvent);
        }
        if event_sequence != run.record.sequence.saturating_add(1) {
            return Err(WorkflowManagerError::StaleEvent);
        }
        if !transition_allowed(run.record.state, next) {
            return Err(WorkflowManagerError::InvalidState);
        }
        run.record.state = next;
        run.record.sequence = event_sequence;
        run.record.updated_at = updated_at;
        if next.is_terminal() || next == WorkflowRunState::RecoveryRequired {
            run.sensitive.clear();
        }
        if next.is_terminal() {
            self.active_run_id = None;
        } else {
            self.active_run_id = Some(run_id.to_owned());
        }
        let record = run.record.clone();
        self.prune_terminal_runs();
        Ok(record)
    }

    pub fn cancel(
        &mut self,
        run_id: &str,
        updated_at: i64,
    ) -> Result<WorkflowRunRecord, WorkflowManagerError> {
        let run = self
            .runs
            .get(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        if !run.record.state.can_cancel() {
            return Err(WorkflowManagerError::InvalidState);
        }
        self.transition(
            run_id,
            run.record.sequence.saturating_add(1),
            WorkflowRunState::Cancelled,
            updated_at,
        )
    }

    pub fn abandon(
        &mut self,
        run_id: &str,
        confirmed: bool,
        updated_at: i64,
    ) -> Result<WorkflowRunRecord, WorkflowManagerError> {
        if !confirmed {
            return Err(WorkflowManagerError::ConfirmationRequired);
        }
        let run = self
            .runs
            .get(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        if !run.record.state.can_abandon() {
            return Err(WorkflowManagerError::InvalidState);
        }
        let run = self
            .runs
            .get_mut(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        run.record.state = WorkflowRunState::Cancelled;
        run.record.sequence = run.record.sequence.saturating_add(1);
        run.record.updated_at = updated_at;
        run.sensitive.clear();
        self.active_run_id = None;
        let record = run.record.clone();
        self.prune_terminal_runs();
        Ok(record)
    }

    pub fn prepared_manifest_for_project(&self, project_id: &str) -> Option<ContextManifestV1> {
        self.runs
            .values()
            .filter(|run| {
                run.record.project_id == project_id
                    && matches!(
                        run.record.state,
                        WorkflowRunState::ContextReady | WorkflowRunState::Generating
                    )
            })
            .max_by_key(|run| (run.record.updated_at, run.record.sequence))
            .and_then(|run| run.sensitive.prepared.as_ref())
            .map(|prepared| prepared.manifest.clone())
    }

    pub fn generation_material(
        &self,
        run_id: &str,
        manifest_hash: &str,
    ) -> Result<WorkflowGenerationMaterial, WorkflowManagerError> {
        let run = self
            .runs
            .get(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        if run.record.state != WorkflowRunState::ContextReady {
            return Err(WorkflowManagerError::InvalidState);
        }
        let prepared = run
            .sensitive
            .prepared
            .as_ref()
            .ok_or(WorkflowManagerError::InvalidState)?;
        if run.record.manifest_hash.as_deref() != Some(manifest_hash)
            || prepared.manifest.manifest_hash != manifest_hash
        {
            return Err(WorkflowManagerError::ManifestMismatch);
        }
        Ok(WorkflowGenerationMaterial {
            prompt: prepared.prompt.clone(),
            source_bindings: prepared.source_bindings.clone(),
        })
    }

    pub fn confirm_generation(
        &mut self,
        run_id: &str,
        manifest_hash: &str,
        confirmed: bool,
        updated_at: i64,
    ) -> Result<WorkflowRunRecord, WorkflowManagerError> {
        if !confirmed {
            return Err(WorkflowManagerError::ConfirmationRequired);
        }
        let material = self.generation_material(run_id, manifest_hash)?;
        drop(material);
        let sequence = self
            .record(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?
            .sequence;
        self.transition(
            run_id,
            sequence.saturating_add(1),
            WorkflowRunState::Generating,
            updated_at,
        )?;
        let run = self
            .runs
            .get_mut(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        run.record.confirmed_at = Some(updated_at);
        Ok(run.record.clone())
    }

    pub fn complete_generation_with_proposal(
        &mut self,
        run_id: &str,
        proposal: RetainedCanonProposal,
        updated_at: i64,
    ) -> Result<WorkflowRunRecord, WorkflowManagerError> {
        proposal
            .verify_integrity()
            .map_err(|_| WorkflowManagerError::ProposalMismatch)?;
        let metadata = proposal.metadata();
        let run = self
            .runs
            .get(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        if run.record.state != WorkflowRunState::Generating {
            return Err(WorkflowManagerError::InvalidState);
        }
        if metadata.run_id != run.record.run_id
            || metadata.workflow_id != run.record.workflow_id
            || metadata.workflow_version != run.record.workflow_version
            || metadata.pack_hash != run.record.pack_hash
            || Some(metadata.provider_id.as_str()) != run.record.provider_id.as_deref()
            || Some(metadata.preset_version) != run.record.preset_version
            || Some(metadata.model.as_str()) != run.record.model.as_deref()
            || Some(metadata.manifest_hash.as_str()) != run.record.manifest_hash.as_deref()
        {
            return Err(WorkflowManagerError::ProposalMismatch);
        }
        {
            let run = self
                .runs
                .get_mut(run_id)
                .ok_or(WorkflowManagerError::RunNotFound)?;
            run.record.proposal_hash = Some(metadata.proposal_hash.clone());
            run.record.target_relative_path = Some(metadata.operation.relative_path.clone());
            run.record.target_source_hash = (metadata.operation.expected_source_hash != "absent")
                .then(|| metadata.operation.expected_source_hash.clone());
            run.record.confirmed_at = None;
            run.sensitive.proposal = Some(proposal);
        }
        for next in [
            WorkflowRunState::ProposalReady,
            WorkflowRunState::AwaitingConfirmation,
        ] {
            let sequence = self
                .record(run_id)
                .ok_or(WorkflowManagerError::RunNotFound)?
                .sequence;
            self.transition(run_id, sequence.saturating_add(1), next, updated_at)?;
        }
        self.record(run_id).ok_or(WorkflowManagerError::RunNotFound)
    }

    pub fn proposal_view_for_project(&self, project_id: &str) -> Option<CanonProposalViewV1> {
        self.runs
            .values()
            .filter(|run| {
                run.record.project_id == project_id
                    && matches!(
                        run.record.state,
                        WorkflowRunState::ProposalReady
                            | WorkflowRunState::AwaitingConfirmation
                            | WorkflowRunState::Applying
                            | WorkflowRunState::Validating
                    )
            })
            .max_by_key(|run| (run.record.updated_at, run.record.sequence))
            .and_then(|run| run.sensitive.proposal.as_ref())
            .map(RetainedCanonProposal::view)
    }

    pub fn proposal_apply_material(
        &self,
        run_id: &str,
        proposal_hash: &str,
    ) -> Result<WorkflowProposalApplyMaterial, WorkflowManagerError> {
        let run = self
            .runs
            .get(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        if run.record.state != WorkflowRunState::AwaitingConfirmation {
            return Err(WorkflowManagerError::InvalidState);
        }
        let proposal = run
            .sensitive
            .proposal
            .as_ref()
            .ok_or(WorkflowManagerError::ProposalMismatch)?;
        proposal
            .verify_integrity()
            .map_err(|_| WorkflowManagerError::ProposalMismatch)?;
        if run.record.proposal_hash.as_deref() != Some(proposal_hash)
            || proposal.metadata().proposal_hash != proposal_hash
        {
            return Err(WorkflowManagerError::ProposalMismatch);
        }
        let prepared = run
            .sensitive
            .prepared
            .as_ref()
            .ok_or(WorkflowManagerError::InvalidState)?;
        Ok(WorkflowProposalApplyMaterial {
            proposal: proposal.clone(),
            source_bindings: prepared.source_bindings.clone(),
        })
    }

    pub fn confirm_proposal_apply(
        &mut self,
        run_id: &str,
        proposal_hash: &str,
        confirmed: bool,
        updated_at: i64,
    ) -> Result<WorkflowRunRecord, WorkflowManagerError> {
        if !confirmed {
            return Err(WorkflowManagerError::ConfirmationRequired);
        }
        drop(self.proposal_apply_material(run_id, proposal_hash)?);
        let sequence = self
            .record(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?
            .sequence;
        let applying = self.transition(
            run_id,
            sequence.saturating_add(1),
            WorkflowRunState::Applying,
            updated_at,
        )?;
        let run = self
            .runs
            .get_mut(run_id)
            .ok_or(WorkflowManagerError::RunNotFound)?;
        run.record.confirmed_at = Some(updated_at);
        Ok(WorkflowRunRecord {
            confirmed_at: Some(updated_at),
            ..applying
        })
    }

    pub fn status_for_project(&self, project_id: &str) -> Option<WorkflowRunRecord> {
        self.runs
            .values()
            .filter(|run| run.record.project_id == project_id)
            .max_by_key(|run| (run.record.updated_at, run.record.sequence))
            .map(|run| run.record.clone())
    }

    pub fn record(&self, run_id: &str) -> Option<WorkflowRunRecord> {
        self.runs.get(run_id).map(|run| run.record.clone())
    }

    pub fn has_active_run(&self) -> bool {
        self.active_run_id.is_some()
    }

    pub fn active_state(&self) -> Option<WorkflowRunState> {
        self.active_run_id
            .as_deref()
            .and_then(|run_id| self.runs.get(run_id))
            .map(|run| run.record.state)
    }

    pub fn active_project_id(&self) -> Option<&str> {
        self.active_run_id
            .as_deref()
            .and_then(|run_id| self.runs.get(run_id))
            .map(|run| run.record.project_id.as_str())
    }

    fn prune_terminal_runs(&mut self) {
        let mut terminal = self
            .runs
            .values()
            .filter(|run| run.record.state.is_terminal())
            .map(|run| {
                (
                    run.record.updated_at,
                    run.record.sequence,
                    run.record.run_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        terminal.sort();
        let remove_count = terminal.len().saturating_sub(MAX_RETAINED_TERMINAL_RUNS);
        for (_, _, run_id) in terminal.into_iter().take(remove_count) {
            self.runs.remove(&run_id);
        }
    }
}

fn transition_allowed(current: WorkflowRunState, next: WorkflowRunState) -> bool {
    use WorkflowRunState as State;
    matches!(
        (current, next),
        (State::Created, State::Routed)
            | (State::Routed, State::AwaitingInput)
            | (State::AwaitingInput, State::ContextReady)
            | (State::ContextReady, State::Generating)
            | (State::Generating, State::ProposalReady)
            | (State::ProposalReady, State::AwaitingConfirmation)
            | (State::AwaitingConfirmation, State::Applying)
            | (State::Applying, State::Validating)
            | (State::Validating, State::Completed)
            | (State::Generating, State::Cancelled)
    ) || (!current.is_terminal() && matches!(next, State::Failed | State::RecoveryRequired))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn is_safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.contains(':')
        && !value.contains('\0')
        && value
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[cfg(test)]
mod tests {
    use super::{
        WorkflowManager, WorkflowManagerError, WorkflowRunRecord, WorkflowRunState,
        MAX_RETAINED_TERMINAL_RUNS,
    };
    use crate::context::{
        prepare_character_context, CharacterBriefModeV2, CharacterBriefV2, ContextSourceInput,
        PrepareCharacterContextInput,
    };
    use crate::{registry, CHARACTER_CREATE_WORKFLOW_ID};
    use sha2::{Digest, Sha256};

    fn record(run_suffix: u8, state: WorkflowRunState, sequence: u64) -> WorkflowRunRecord {
        WorkflowRunRecord {
            run_id: format!("10000000-0000-4000-8000-{run_suffix:012}"),
            project_id: "20000000-0000-4000-8000-000000000002".to_owned(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: registry().pack_hash.to_owned(),
            state,
            sequence,
            provider_id: Some("deepseek".to_owned()),
            preset_version: Some(1),
            model: Some("deepseek-v4-pro".to_owned()),
            manifest_hash: None,
            proposal_hash: None,
            target_relative_path: None,
            target_source_hash: None,
            confirmed_at: None,
            updated_at: i64::from(run_suffix),
        }
    }

    fn prepared(source_body: &str) -> crate::context::PreparedCharacterContext {
        let source_hash = format!("{:x}", Sha256::digest(source_body.as_bytes()));
        prepare_character_context(PrepareCharacterContextInput {
            run_id: record(1, WorkflowRunState::Created, 0).run_id,
            project_id: record(1, WorkflowRunState::Created, 0).project_id,
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            provider_id: "deepseek".to_owned(),
            preset_version: 1,
            model: "deepseek-v4-pro".to_owned(),
            brief: CharacterBriefV2 {
                schema_version: 2,
                mode: CharacterBriefModeV2::Create,
                brief: "人物简报".to_owned(),
            },
            sources: vec![ContextSourceInput {
                relative_path: "project.md".to_owned(),
                content: source_body.to_owned(),
                content_hash: source_hash,
                source_byte_count: source_body.len() as u64,
            }],
        })
        .unwrap()
    }

    #[test]
    fn current_writes_reject_v1_but_terminal_v1_checkpoints_remain_readable() {
        let mut legacy = record(9, WorkflowRunState::Completed, 9);
        legacy.workflow_version = 1;
        legacy.pack_hash = "b".repeat(64);
        assert_eq!(legacy.validate(), Err(WorkflowManagerError::InvalidRecord));
        assert_eq!(legacy.validate_persisted_checkpoint(), Ok(()));

        legacy.state = WorkflowRunState::Created;
        assert_eq!(
            legacy.validate_persisted_checkpoint(),
            Err(WorkflowManagerError::InvalidRecord)
        );
        legacy.state = WorkflowRunState::Cancelled;
        legacy.pack_hash = "not-a-hash".to_owned();
        assert_eq!(
            legacy.validate_persisted_checkpoint(),
            Err(WorkflowManagerError::InvalidRecord)
        );
    }

    #[test]
    fn approved_state_path_is_deterministic_and_snapshots_are_immutable() {
        let initial = record(1, WorkflowRunState::Created, 0);
        let identity = (
            initial.workflow_id.clone(),
            initial.workflow_version,
            initial.pack_hash.clone(),
            initial.project_id.clone(),
            initial.provider_id.clone(),
            initial.preset_version,
            initial.model.clone(),
        );
        let (mut manager, recovered) = WorkflowManager::restore(Vec::new(), 0).unwrap();
        assert!(recovered.is_empty());
        manager.create(initial).unwrap();
        let path = [
            WorkflowRunState::Routed,
            WorkflowRunState::AwaitingInput,
            WorkflowRunState::ContextReady,
            WorkflowRunState::Generating,
            WorkflowRunState::ProposalReady,
            WorkflowRunState::AwaitingConfirmation,
            WorkflowRunState::Applying,
            WorkflowRunState::Validating,
            WorkflowRunState::Completed,
        ];
        for (index, state) in path.into_iter().enumerate() {
            manager
                .transition(
                    &record(1, WorkflowRunState::Created, 0).run_id,
                    index as u64 + 1,
                    state,
                    index as i64 + 2,
                )
                .unwrap();
        }
        let final_record = manager
            .record(&record(1, WorkflowRunState::Created, 0).run_id)
            .unwrap();
        assert_eq!(final_record.state, WorkflowRunState::Completed);
        assert_eq!(final_record.sequence, 9);
        assert!(!manager.has_active_run());
        assert_eq!(
            (
                final_record.workflow_id,
                final_record.workflow_version,
                final_record.pack_hash,
                final_record.project_id,
                final_record.provider_id,
                final_record.preset_version,
                final_record.model,
            ),
            identity
        );
    }

    #[test]
    fn proposal_hash_confirmation_and_apply_state_are_strictly_bound() {
        use crate::proposal::{
            build_canon_proposal, BuildCanonProposalInput, CanonFileOperationKind,
        };

        let (mut manager, _) = WorkflowManager::restore(Vec::new(), 0).unwrap();
        let prepared = prepared("# 项目\n安全正文");
        let mut initial = record(1, WorkflowRunState::Created, 0);
        initial.manifest_hash = Some(prepared.manifest.manifest_hash.clone());
        manager
            .create_prepared(initial.clone(), prepared.clone())
            .unwrap();
        manager
            .advance_prepared_to_context_ready(&initial.run_id, 2)
            .unwrap();
        manager
            .confirm_generation(&initial.run_id, &prepared.manifest.manifest_hash, true, 3)
            .unwrap();
        let proposed = "---\ntitle: 林遥\nwritingLanguage: zh-CN\n---\n\n# 林遥\n";
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: "30000000-0000-4000-8000-000000000003".to_owned(),
            workflow_id: initial.workflow_id.clone(),
            workflow_version: initial.workflow_version,
            pack_hash: initial.pack_hash.clone(),
            run_id: initial.run_id.clone(),
            provider_id: initial.provider_id.clone().unwrap(),
            preset_version: initial.preset_version.unwrap(),
            model: initial.model.clone().unwrap(),
            manifest_hash: prepared.manifest.manifest_hash.clone(),
            operation_kind: CanonFileOperationKind::Create,
            relative_path: "characters/林遥.md".to_owned(),
            expected_source_hash: None,
            original_content: None,
            proposed_content: proposed.to_owned(),
            warning_codes: Vec::new(),
        })
        .unwrap();
        let proposal_hash = proposal.metadata().proposal_hash.clone();
        let ready = manager
            .complete_generation_with_proposal(&initial.run_id, proposal, 4)
            .unwrap();
        assert_eq!(ready.state, WorkflowRunState::AwaitingConfirmation);
        assert_eq!(ready.proposal_hash.as_deref(), Some(proposal_hash.as_str()));
        assert_eq!(ready.confirmed_at, None);
        assert_eq!(
            manager.proposal_apply_material(&initial.run_id, &"0".repeat(64)),
            Err(WorkflowManagerError::ProposalMismatch)
        );
        assert_eq!(
            manager.confirm_proposal_apply(&initial.run_id, &proposal_hash, false, 5),
            Err(WorkflowManagerError::ConfirmationRequired)
        );
        let material = manager
            .proposal_apply_material(&initial.run_id, &proposal_hash)
            .unwrap();
        assert_eq!(material.proposal.proposed_bytes(), proposed.as_bytes());
        let applying = manager
            .confirm_proposal_apply(&initial.run_id, &proposal_hash, true, 5)
            .unwrap();
        assert_eq!(applying.state, WorkflowRunState::Applying);
        assert_eq!(applying.confirmed_at, Some(5));
        assert_eq!(
            manager.confirm_proposal_apply(&initial.run_id, &proposal_hash, true, 6),
            Err(WorkflowManagerError::InvalidState)
        );
        let validating = manager
            .transition(
                &initial.run_id,
                applying.sequence + 1,
                WorkflowRunState::Validating,
                6,
            )
            .unwrap();
        manager
            .transition(
                &initial.run_id,
                validating.sequence + 1,
                WorkflowRunState::Completed,
                7,
            )
            .unwrap();
        assert!(manager
            .proposal_view_for_project(&initial.project_id)
            .is_none());
    }

    #[test]
    fn rejects_duplicate_runs_illegal_transitions_wrong_cancel_and_stale_events() {
        let (mut manager, _) = WorkflowManager::restore(Vec::new(), 0).unwrap();
        let first = record(1, WorkflowRunState::Created, 0);
        manager.create(first.clone()).unwrap();
        assert_eq!(
            manager.create(record(2, WorkflowRunState::Created, 0)),
            Err(WorkflowManagerError::RunConflict)
        );
        assert_eq!(
            manager.cancel(&first.run_id, 2),
            Err(WorkflowManagerError::InvalidState)
        );
        assert_eq!(
            manager.transition(&first.run_id, 1, WorkflowRunState::Generating, 2),
            Err(WorkflowManagerError::InvalidState)
        );
        manager
            .transition(&first.run_id, 1, WorkflowRunState::Routed, 2)
            .unwrap();
        assert_eq!(
            manager.transition(&first.run_id, 1, WorkflowRunState::AwaitingInput, 3),
            Err(WorkflowManagerError::StaleEvent)
        );
    }

    #[test]
    fn cancellation_and_terminal_events_cannot_revive_a_run() {
        let mut generating = record(1, WorkflowRunState::Generating, 4);
        generating.updated_at = 4;
        let (mut manager, recovered) = WorkflowManager::restore(vec![generating], 5).unwrap();
        assert_eq!(recovered[0].state, WorkflowRunState::RecoveryRequired);
        let recovered_run = recovered[0].clone();
        assert_eq!(
            manager.cancel(&recovered_run.run_id, 6),
            Err(WorkflowManagerError::InvalidState)
        );
        let abandoned = manager.abandon(&recovered_run.run_id, true, 6).unwrap();
        assert_eq!(abandoned.state, WorkflowRunState::Cancelled);
        assert_eq!(
            manager.transition(
                &recovered_run.run_id,
                abandoned.sequence + 1,
                WorkflowRunState::Completed,
                7,
            ),
            Err(WorkflowManagerError::TerminalEvent)
        );
        assert_eq!(manager.record(&recovered_run.run_id), Some(abandoned));
    }

    #[test]
    fn startup_never_replays_nonterminal_work_and_abandon_clears_memory() {
        for state in WorkflowRunState::ALL {
            let source = record(1, state, 3);
            let (manager, recovered) = WorkflowManager::restore(vec![source.clone()], 9).unwrap();
            let restored = manager.record(&source.run_id).unwrap();
            if state.is_terminal() || state == WorkflowRunState::RecoveryRequired {
                assert!(recovered.is_empty());
                assert_eq!(restored.state, state);
            } else {
                assert_eq!(recovered.len(), 1);
                assert_eq!(restored.state, WorkflowRunState::RecoveryRequired);
                assert_eq!(restored.sequence, 4);
            }
        }

        let (mut manager, _) = WorkflowManager::restore(Vec::new(), 0).unwrap();
        let active = record(1, WorkflowRunState::Created, 0);
        manager.create(active.clone()).unwrap();
        manager.runs.get_mut(&active.run_id).unwrap().sensitive = super::SensitiveRunMemory {
            prepared: Some(prepared("private context")),
            proposal: None,
        };
        assert_eq!(
            manager.abandon(&active.run_id, false, 1),
            Err(WorkflowManagerError::ConfirmationRequired)
        );
        manager.abandon(&active.run_id, true, 1).unwrap();
        let memory = &manager.runs.get(&active.run_id).unwrap().sensitive;
        assert_eq!(memory, &super::SensitiveRunMemory::default());
    }

    #[test]
    fn prepared_context_requires_the_bound_manifest_and_clears_after_generation() {
        let (mut manager, _) = WorkflowManager::restore(Vec::new(), 0).unwrap();
        let prepared = prepared("# 项目\n安全正文");
        let mut initial = record(1, WorkflowRunState::Created, 0);
        initial.manifest_hash = Some(prepared.manifest.manifest_hash.clone());
        manager
            .create_prepared(initial.clone(), prepared.clone())
            .unwrap();
        let context_ready = manager
            .advance_prepared_to_context_ready(&initial.run_id, 2)
            .unwrap();
        assert_eq!(context_ready.state, WorkflowRunState::ContextReady);
        assert_eq!(context_ready.sequence, 3);
        assert_eq!(
            manager.prepared_manifest_for_project(&initial.project_id),
            Some(prepared.manifest.clone())
        );
        assert_eq!(
            manager.generation_material(&initial.run_id, &"0".repeat(64)),
            Err(WorkflowManagerError::ManifestMismatch)
        );
        assert_eq!(
            manager.confirm_generation(&initial.run_id, &prepared.manifest.manifest_hash, false, 3),
            Err(WorkflowManagerError::ConfirmationRequired)
        );
        let generating = manager
            .confirm_generation(&initial.run_id, &prepared.manifest.manifest_hash, true, 3)
            .unwrap();
        assert_eq!(generating.state, WorkflowRunState::Generating);
        assert_eq!(generating.confirmed_at, Some(3));
        let failed = manager
            .transition(
                &initial.run_id,
                generating.sequence.saturating_add(1),
                WorkflowRunState::Failed,
                4,
            )
            .unwrap();
        assert_eq!(failed.state, WorkflowRunState::Failed);
        assert!(manager
            .prepared_manifest_for_project(&initial.project_id)
            .is_none());
        assert_eq!(
            manager.runs.get(&initial.run_id).unwrap().sensitive,
            super::SensitiveRunMemory::default()
        );
    }

    #[test]
    fn terminal_retention_is_bounded() {
        let records = (0..=MAX_RETAINED_TERMINAL_RUNS)
            .map(|index| record(index as u8 + 1, WorkflowRunState::Completed, 9))
            .collect();
        let (manager, _) = WorkflowManager::restore(records, 99).unwrap();
        assert_eq!(manager.runs.len(), MAX_RETAINED_TERMINAL_RUNS);
        assert!(manager
            .record(&record(1, WorkflowRunState::Completed, 9).run_id)
            .is_none());
    }
}
