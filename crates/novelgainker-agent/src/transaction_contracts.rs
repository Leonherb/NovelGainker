use crate::action_contracts::ProjectRelativePathV1;
use crate::authorization::{
    GovernanceErrorCodeV1, RiskLevelV1, WriteToolClassificationV1, GOVERNANCE_SCHEMA_VERSION,
};
use crate::contracts::{
    ContentHashV1, EpochMillisV1, ProjectIdV1, SessionIdV1, TurnIdV1,
    MAX_MODIFIED_PROJECT_FILES_PER_TURN,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_UNDO_BYTES_PER_TURN: u32 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenFileStateV1 {
    pub path: ProjectRelativePathV1,
    pub source_hash: Option<ContentHashV1>,
    pub target_must_be_absent: bool,
}

impl FrozenFileStateV1 {
    fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if self.target_must_be_absent != self.source_hash.is_none() {
            return Err(GovernanceErrorCodeV1::TransactionInvalid);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlannedFileMutationV1 {
    pub frozen: FrozenFileStateV1,
    pub classification: WriteToolClassificationV1,
    pub planned_result_hash: ContentHashV1,
    pub estimated_undo_bytes: u32,
}

impl PlannedFileMutationV1 {
    fn validate(
        &self,
        project_id: &ProjectIdV1,
        session_id: &SessionIdV1,
        turn_id: &TurnIdV1,
    ) -> Result<(), GovernanceErrorCodeV1> {
        self.frozen.validate()?;
        self.classification.validate()?;
        if self.frozen.path != self.classification.path
            || &self.classification.binding.project_id != project_id
            || &self.classification.binding.session_id != session_id
            || &self.classification.binding.turn_id != turn_id
        {
            return Err(GovernanceErrorCodeV1::TransactionInvalid);
        }
        if !matches!(
            self.classification.risk,
            RiskLevelV1::WriteSafe | RiskLevelV1::AlwaysConfirm
        ) || self.estimated_undo_bytes == 0
        {
            return Err(GovernanceErrorCodeV1::TransactionInvalid);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MultiFileTransactionPlanV1 {
    pub schema_version: u16,
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub turn_id: TurnIdV1,
    pub policy_snapshot_hash: ContentHashV1,
    pub mutations: Vec<PlannedFileMutationV1>,
    pub source_set_hash: ContentHashV1,
    pub action_plan_hash: ContentHashV1,
    pub transaction_hash: ContentHashV1,
    pub total_estimated_undo_bytes: u32,
}

impl MultiFileTransactionPlanV1 {
    pub fn build(
        project_id: ProjectIdV1,
        session_id: SessionIdV1,
        turn_id: TurnIdV1,
        policy_snapshot_hash: ContentHashV1,
        mut mutations: Vec<PlannedFileMutationV1>,
    ) -> Result<Self, GovernanceErrorCodeV1> {
        validate_mutations(&mutations, &project_id, &session_id, &turn_id)?;
        mutations.sort_by(|left, right| left.frozen.path.cmp(&right.frozen.path));
        let total_estimated_undo_bytes = mutations
            .iter()
            .try_fold(0_u32, |total, mutation| {
                total.checked_add(mutation.estimated_undo_bytes)
            })
            .ok_or(GovernanceErrorCodeV1::UndoLimitExceeded)?;
        if total_estimated_undo_bytes > MAX_UNDO_BYTES_PER_TURN {
            return Err(GovernanceErrorCodeV1::UndoLimitExceeded);
        }
        let source_set_hash = source_set_hash(&mutations);
        let action_plan_hash = action_plan_hash(&mutations);
        let transaction_hash = transaction_hash(
            &project_id,
            &session_id,
            &turn_id,
            &policy_snapshot_hash,
            &source_set_hash,
            &action_plan_hash,
            total_estimated_undo_bytes,
        );
        Ok(Self {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            project_id,
            session_id,
            turn_id,
            policy_snapshot_hash,
            mutations,
            source_set_hash,
            action_plan_hash,
            transaction_hash,
            total_estimated_undo_bytes,
        })
    }

    pub fn validate(&self) -> Result<(), GovernanceErrorCodeV1> {
        if self.schema_version != GOVERNANCE_SCHEMA_VERSION {
            return Err(GovernanceErrorCodeV1::InvalidSchemaVersion);
        }
        validate_mutations(
            &self.mutations,
            &self.project_id,
            &self.session_id,
            &self.turn_id,
        )?;
        if !self
            .mutations
            .windows(2)
            .all(|pair| pair[0].frozen.path < pair[1].frozen.path)
        {
            return Err(GovernanceErrorCodeV1::TransactionInvalid);
        }
        let total = self
            .mutations
            .iter()
            .try_fold(0_u32, |total, mutation| {
                total.checked_add(mutation.estimated_undo_bytes)
            })
            .ok_or(GovernanceErrorCodeV1::UndoLimitExceeded)?;
        if total != self.total_estimated_undo_bytes || total > MAX_UNDO_BYTES_PER_TURN {
            return Err(GovernanceErrorCodeV1::UndoLimitExceeded);
        }
        if self.source_set_hash != source_set_hash(&self.mutations)
            || self.action_plan_hash != action_plan_hash(&self.mutations)
            || self.transaction_hash
                != transaction_hash(
                    &self.project_id,
                    &self.session_id,
                    &self.turn_id,
                    &self.policy_snapshot_hash,
                    &self.source_set_hash,
                    &self.action_plan_hash,
                    self.total_estimated_undo_bytes,
                )
        {
            return Err(GovernanceErrorCodeV1::TransactionInvalid);
        }
        Ok(())
    }

    pub fn preflight(
        &self,
        observed: &[ObservedFileStateV1],
    ) -> Result<PreflightResultV1, GovernanceErrorCodeV1> {
        self.validate()?;
        let mut observed = observed.to_vec();
        observed.sort_by(|left, right| left.path.cmp(&right.path));
        let matches = observed.len() == self.mutations.len()
            && self
                .mutations
                .iter()
                .zip(&observed)
                .all(|(planned, current)| {
                    planned.frozen.path == current.path
                        && planned.frozen.source_hash == current.current_hash
                        && planned.frozen.target_must_be_absent == current.current_hash.is_none()
                });
        Ok(PreflightResultV1 {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            transaction_hash: self.transaction_hash.clone(),
            status: if matches {
                PreflightStatusV1::ReadyForSingleWriter
            } else {
                PreflightStatusV1::RejectedZeroWrite
            },
            all_hashes_match: matches,
            writes_performed: 0,
        })
    }
}

fn validate_mutations(
    mutations: &[PlannedFileMutationV1],
    project_id: &ProjectIdV1,
    session_id: &SessionIdV1,
    turn_id: &TurnIdV1,
) -> Result<(), GovernanceErrorCodeV1> {
    if mutations.is_empty() || mutations.len() > MAX_MODIFIED_PROJECT_FILES_PER_TURN as usize {
        return Err(GovernanceErrorCodeV1::TransactionInvalid);
    }
    let mut paths = BTreeSet::new();
    for mutation in mutations {
        mutation.validate(project_id, session_id, turn_id)?;
        if !paths.insert(&mutation.frozen.path) {
            return Err(GovernanceErrorCodeV1::TransactionInvalid);
        }
    }
    Ok(())
}

fn source_set_hash(mutations: &[PlannedFileMutationV1]) -> ContentHashV1 {
    ContentHashV1::digest(
        &serde_json::to_vec(
            &mutations
                .iter()
                .map(|mutation| &mutation.frozen)
                .collect::<Vec<_>>(),
        )
        .expect("source set serialization is infallible"),
    )
}

fn action_plan_hash(mutations: &[PlannedFileMutationV1]) -> ContentHashV1 {
    ContentHashV1::digest(
        &serde_json::to_vec(
            &mutations
                .iter()
                .map(|mutation| {
                    (
                        &mutation.frozen.path,
                        &mutation.classification.classification_hash,
                        &mutation.planned_result_hash,
                        mutation.estimated_undo_bytes,
                    )
                })
                .collect::<Vec<_>>(),
        )
        .expect("action plan serialization is infallible"),
    )
}

#[allow(clippy::too_many_arguments)]
fn transaction_hash(
    project_id: &ProjectIdV1,
    session_id: &SessionIdV1,
    turn_id: &TurnIdV1,
    policy_snapshot_hash: &ContentHashV1,
    source_set_hash: &ContentHashV1,
    action_plan_hash: &ContentHashV1,
    total_estimated_undo_bytes: u32,
) -> ContentHashV1 {
    ContentHashV1::digest(
        &serde_json::to_vec(&(
            GOVERNANCE_SCHEMA_VERSION,
            project_id,
            session_id,
            turn_id,
            policy_snapshot_hash,
            source_set_hash,
            action_plan_hash,
            total_estimated_undo_bytes,
        ))
        .expect("transaction serialization is infallible"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservedFileStateV1 {
    pub path: ProjectRelativePathV1,
    pub current_hash: Option<ContentHashV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightStatusV1 {
    ReadyForSingleWriter,
    RejectedZeroWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreflightResultV1 {
    pub schema_version: u16,
    pub transaction_hash: ContentHashV1,
    pub status: PreflightStatusV1,
    pub all_hashes_match: bool,
    pub writes_performed: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDispositionV1 {
    NotRequired,
    RolledBackToSourceSet,
    RecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommittedFileResultV1 {
    pub path: ProjectRelativePathV1,
    pub result_hash: ContentHashV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransactionReceiptV1 {
    pub schema_version: u16,
    pub project_id: ProjectIdV1,
    pub session_id: SessionIdV1,
    pub turn_id: TurnIdV1,
    pub transaction_hash: ContentHashV1,
    pub committed_results: Vec<CommittedFileResultV1>,
    pub result_set_hash: ContentHashV1,
    pub recovery: RecoveryDispositionV1,
    pub committed_at_ms: EpochMillisV1,
}

impl TransactionReceiptV1 {
    pub fn from_committed_plan(
        plan: &MultiFileTransactionPlanV1,
        mut committed_results: Vec<CommittedFileResultV1>,
        committed_at_ms: EpochMillisV1,
    ) -> Result<Self, GovernanceErrorCodeV1> {
        plan.validate()?;
        committed_results.sort_by(|left, right| left.path.cmp(&right.path));
        if committed_results.len() != plan.mutations.len()
            || !plan
                .mutations
                .iter()
                .zip(&committed_results)
                .all(|(planned, result)| {
                    planned.frozen.path == result.path
                        && planned.planned_result_hash == result.result_hash
                })
        {
            return Err(GovernanceErrorCodeV1::TransactionInvalid);
        }
        let result_set_hash = ContentHashV1::digest(
            &serde_json::to_vec(&committed_results)
                .expect("transaction result serialization is infallible"),
        );
        Ok(Self {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            project_id: plan.project_id.clone(),
            session_id: plan.session_id.clone(),
            turn_id: plan.turn_id.clone(),
            transaction_hash: plan.transaction_hash.clone(),
            committed_results,
            result_set_hash,
            recovery: RecoveryDispositionV1::NotRequired,
            committed_at_ms,
        })
    }
}
