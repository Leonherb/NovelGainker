use crate::agent_canon::{NormalizedCanonTargetV1, PersistedCanonProposalV1};
use crate::AppError;
use novelgainker_agent::v2::CanonFactV1;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKindV1 {
    Identity,
    FieldValue,
    Relationship,
    Timeline,
    WorldRule,
    PlotThread,
    ChapterState,
    SourceHash,
    Semantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictConfidenceV1 {
    ConfirmedConflict,
    ClarificationRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonEvidenceV1 {
    pub kind: ConflictKindV1,
    pub proposition: String,
    pub value: String,
    pub source_id: String,
    pub source_hash: String,
    pub relative_path: String,
    pub heading_path: Vec<String>,
    pub confidence: ConflictConfidenceV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConflictItemV1 {
    pub conflict_id: String,
    pub kind: ConflictKindV1,
    pub confidence: ConflictConfidenceV1,
    pub proposition: String,
    pub existing_value: String,
    pub candidate_value: String,
    pub existing_source_id: String,
    pub existing_source_hash: String,
    pub candidate_source_ids: Vec<String>,
    pub relative_path: String,
    pub heading_path: Vec<String>,
    pub visible_explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConflictSetV1 {
    pub schema_version: u16,
    pub conflict_set_id: String,
    pub project_id: String,
    pub session_id: String,
    pub branch_id: String,
    pub turn_id: String,
    pub proposal_id: String,
    pub proposal_hash: String,
    pub conflicts: Vec<ConflictItemV1>,
    pub requires_resolution: bool,
    pub created_at_ms: u64,
    pub conflict_set_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolutionChoiceV1 {
    KeepExisting,
    AdoptCandidate,
    MergeManual,
    SaveAsAlternative,
    CancelChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolutionOutcomeV1 {
    CandidateDiscarded,
    CandidateRequiresConfirmation,
    ManualConversationRequired,
    AlternativeRequiresRenormalization,
    ChangeCancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolutionResultV1 {
    pub conflict_set_id: String,
    pub conflict_set_hash: String,
    pub choice: ConflictResolutionChoiceV1,
    pub outcome: ConflictResolutionOutcomeV1,
    pub requires_write_confirmation: bool,
    pub consumed: bool,
}

pub fn detect_conflicts(
    persisted: &PersistedCanonProposalV1,
    target: &NormalizedCanonTargetV1,
    current_source_hash: Option<&str>,
    evidence: &[CanonEvidenceV1],
    now_ms: u64,
) -> Result<Option<ConflictSetV1>, AppError> {
    if target.relative_path.starts_with(".novelgainker/")
        || persisted.proposal.proposal_id.is_empty()
        || persisted.proposal_hash.len() != 64
    {
        return Err(conflict_error("agent-conflict.input-invalid"));
    }
    let mut conflicts = Vec::new();
    if let (Some(expected), Some(current)) =
        (target.existing_source_hash.as_deref(), current_source_hash)
    {
        if expected != current {
            conflicts.push(ConflictItemV1 {
                conflict_id: format!("conf_{}", Uuid::new_v4().simple()),
                kind: ConflictKindV1::SourceHash,
                confidence: ConflictConfidenceV1::ConfirmedConflict,
                proposition: "source_hash".to_owned(),
                existing_value: expected.to_owned(),
                candidate_value: current.to_owned(),
                existing_source_id: "src_hash_guard".to_owned(),
                existing_source_hash: current.to_owned(),
                candidate_source_ids: proposal_source_ids(&persisted.proposal.facts),
                relative_path: target.relative_path.clone(),
                heading_path: Vec::new(),
                visible_explanation: "目标文档在计划后发生变化，需要重新读取并确认。".to_owned(),
            });
        }
    }
    for fact in &persisted.proposal.facts {
        for existing in evidence
            .iter()
            .filter(|existing| same_proposition(&existing.proposition, &fact.field))
        {
            if compatible(&existing.value, &fact.value) {
                continue;
            }
            conflicts.push(ConflictItemV1 {
                conflict_id: format!("conf_{}", Uuid::new_v4().simple()),
                kind: existing.kind,
                confidence: existing.confidence,
                proposition: existing.proposition.clone(),
                existing_value: existing.value.clone(),
                candidate_value: fact.value.clone(),
                existing_source_id: existing.source_id.clone(),
                existing_source_hash: existing.source_hash.clone(),
                candidate_source_ids: fact
                    .sources
                    .iter()
                    .map(|source| source.source_id.clone())
                    .collect(),
                relative_path: existing.relative_path.clone(),
                heading_path: existing.heading_path.clone(),
                visible_explanation: explanation(existing.kind).to_owned(),
            });
        }
    }
    if conflicts.is_empty() {
        return Ok(None);
    }
    conflicts.sort_by(|left, right| {
        (
            left.relative_path.as_str(),
            left.proposition.as_str(),
            left.conflict_id.as_str(),
        )
            .cmp(&(
                right.relative_path.as_str(),
                right.proposition.as_str(),
                right.conflict_id.as_str(),
            ))
    });
    let mut set = ConflictSetV1 {
        schema_version: 1,
        conflict_set_id: format!("cset_{}", Uuid::new_v4().simple()),
        project_id: persisted.project_id.clone(),
        session_id: persisted.session_id.clone(),
        branch_id: persisted.branch_id.clone(),
        turn_id: persisted.turn_id.clone(),
        proposal_id: persisted.proposal.proposal_id.clone(),
        proposal_hash: persisted.proposal_hash.clone(),
        requires_resolution: conflicts
            .iter()
            .any(|conflict| conflict.confidence == ConflictConfidenceV1::ConfirmedConflict),
        conflicts,
        created_at_ms: now_ms,
        conflict_set_hash: String::new(),
    };
    set.conflict_set_hash = hash_set(&set)?;
    Ok(Some(set))
}

pub fn resolve_conflict_set(
    set: &ConflictSetV1,
    expected_hash: &str,
    choice: ConflictResolutionChoiceV1,
    confirmed: bool,
) -> Result<ConflictResolutionResultV1, AppError> {
    if !confirmed
        || set.conflict_set_hash != expected_hash
        || set.conflict_set_hash != hash_set(set)?
        || !set.requires_resolution
    {
        return Err(conflict_error("agent-conflict.binding-invalid"));
    }
    let (outcome, requires_write_confirmation) = match choice {
        ConflictResolutionChoiceV1::KeepExisting => {
            (ConflictResolutionOutcomeV1::CandidateDiscarded, false)
        }
        ConflictResolutionChoiceV1::AdoptCandidate => (
            ConflictResolutionOutcomeV1::CandidateRequiresConfirmation,
            true,
        ),
        ConflictResolutionChoiceV1::MergeManual => (
            ConflictResolutionOutcomeV1::ManualConversationRequired,
            true,
        ),
        ConflictResolutionChoiceV1::SaveAsAlternative => (
            ConflictResolutionOutcomeV1::AlternativeRequiresRenormalization,
            true,
        ),
        ConflictResolutionChoiceV1::CancelChange => {
            (ConflictResolutionOutcomeV1::ChangeCancelled, false)
        }
    };
    Ok(ConflictResolutionResultV1 {
        conflict_set_id: set.conflict_set_id.clone(),
        conflict_set_hash: set.conflict_set_hash.clone(),
        choice,
        outcome,
        requires_write_confirmation,
        consumed: true,
    })
}

pub fn evidence_from_markdown_facts(
    relative_path: &str,
    source_id: &str,
    source_hash: &str,
    content: &str,
) -> Vec<CanonEvidenceV1> {
    content
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("- **")?;
            let (field, value) = rest.split_once("**：")?;
            let value = value.trim();
            if field.trim().is_empty() || value.is_empty() {
                return None;
            }
            Some(CanonEvidenceV1 {
                kind: classify_kind(field),
                proposition: field.trim().to_owned(),
                value: value.to_owned(),
                source_id: source_id.to_owned(),
                source_hash: source_hash.to_owned(),
                relative_path: relative_path.to_owned(),
                heading_path: vec!["已确认事实".to_owned()],
                confidence: if field.contains("可能") || field.contains("待定") {
                    ConflictConfidenceV1::ClarificationRequired
                } else {
                    ConflictConfidenceV1::ConfirmedConflict
                },
            })
        })
        .collect()
}

fn classify_kind(field: &str) -> ConflictKindV1 {
    if field.contains("身份") || field.contains("姓名") || field.contains("别名") {
        ConflictKindV1::Identity
    } else if field.contains("关系") {
        ConflictKindV1::Relationship
    } else if field.contains("时间") || field.contains("年代") {
        ConflictKindV1::Timeline
    } else if field.contains("规则") || field.contains("限制") {
        ConflictKindV1::WorldRule
    } else if field.contains("主线") || field.contains("支线") || field.contains("剧情") {
        ConflictKindV1::PlotThread
    } else if field.contains("章节状态") {
        ConflictKindV1::ChapterState
    } else if field.contains("语义") || field.contains("含义") {
        ConflictKindV1::Semantic
    } else {
        ConflictKindV1::FieldValue
    }
}

fn same_proposition(existing: &str, candidate: &str) -> bool {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
            .to_lowercase()
    };
    normalize(existing) == normalize(candidate)
}

fn compatible(existing: &str, candidate: &str) -> bool {
    let existing = existing.trim();
    let candidate = candidate.trim();
    existing == candidate || existing.contains(candidate) || candidate.contains(existing)
}

fn proposal_source_ids(facts: &[CanonFactV1]) -> Vec<String> {
    let mut ids = facts
        .iter()
        .flat_map(|fact| fact.sources.iter())
        .map(|source| source.source_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn explanation(kind: ConflictKindV1) -> &'static str {
    match kind {
        ConflictKindV1::Identity => "候选身份与现有 Canon 身份不能同时成立。",
        ConflictKindV1::FieldValue => "候选字段值与现有 Canon 值不同。",
        ConflictKindV1::Relationship => "候选人物关系与现有关系不能同时成立。",
        ConflictKindV1::Timeline => "候选时间点与现有时间线冲突。",
        ConflictKindV1::WorldRule => "候选规则与现有世界规则冲突。",
        ConflictKindV1::PlotThread => "候选剧情走向与现有剧情线冲突。",
        ConflictKindV1::ChapterState => "目标章节状态不允许当前变更。",
        ConflictKindV1::SourceHash => "目标来源 Hash 已发生变化。",
        ConflictKindV1::Semantic => "候选含义与现有 Canon 命题不兼容。",
    }
}

fn hash_set(set: &ConflictSetV1) -> Result<String, AppError> {
    let mut canonical = set.clone();
    canonical.conflict_set_hash.clear();
    serde_json::to_vec(&canonical)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|_| conflict_error("agent-conflict.hash-invalid"))
}

fn conflict_error(code: &str) -> AppError {
    AppError::agent_contract(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_canon::CanonTargetResolutionV1;
    use novelgainker_agent::v2::{
        CanonFactSourceKindV1, CanonFactSourceV1, CanonProposalOperationV1, CanonProposalV1,
        CanonTargetKindV1,
    };

    fn persisted(kind: ConflictKindV1) -> (PersistedCanonProposalV1, CanonEvidenceV1) {
        let field = match kind {
            ConflictKindV1::Identity => "身份",
            ConflictKindV1::FieldValue => "年龄",
            ConflictKindV1::Relationship => "关系",
            ConflictKindV1::Timeline => "时间",
            ConflictKindV1::WorldRule => "规则",
            ConflictKindV1::PlotThread => "主线",
            ConflictKindV1::ChapterState => "章节状态",
            ConflictKindV1::SourceHash => "来源",
            ConflictKindV1::Semantic => "语义",
        };
        let proposal = CanonProposalV1 {
            schema_version: 1,
            request_id: "req_12345678".to_owned(),
            proposal_id: "prop_12345678".to_owned(),
            target_kind: CanonTargetKindV1::Character,
            display_name: "林舟".to_owned(),
            operation: CanonProposalOperationV1::Extend,
            facts: vec![CanonFactV1 {
                field: field.to_owned(),
                value: "候选值".to_owned(),
                sources: vec![CanonFactSourceV1 {
                    kind: CanonFactSourceKindV1::VisibleMessage,
                    source_id: "msg_12345678".to_owned(),
                    source_hash: "a".repeat(64),
                }],
            }],
        };
        (
            PersistedCanonProposalV1 {
                schema_version: 1,
                project_id: "project".to_owned(),
                session_id: "ses_12345678".to_owned(),
                branch_id: "brn_12345678".to_owned(),
                turn_id: "trn_12345678".to_owned(),
                anchor_id: "anc_12345678".to_owned(),
                anchor_hash: "b".repeat(64),
                artifact_id: "art_12345678".to_owned(),
                artifact_hash: "c".repeat(64),
                proposal,
                proposal_hash: "d".repeat(64),
                created_at_ms: 1,
            },
            CanonEvidenceV1 {
                kind,
                proposition: field.to_owned(),
                value: "现有值".to_owned(),
                source_id: "src_12345678".to_owned(),
                source_hash: "e".repeat(64),
                relative_path: "characters/林舟.md".to_owned(),
                heading_path: vec!["已确认事实".to_owned()],
                confidence: ConflictConfidenceV1::ConfirmedConflict,
            },
        )
    }

    fn target() -> NormalizedCanonTargetV1 {
        NormalizedCanonTargetV1 {
            entity_id: "character_12345678".to_owned(),
            display_name: "林舟".to_owned(),
            relative_path: "characters/林舟.md".to_owned(),
            resolution: CanonTargetResolutionV1::Extend,
            existing_source_hash: Some("f".repeat(64)),
            chapter_number: None,
        }
    }

    #[test]
    fn all_nine_conflict_kinds_are_deterministic_and_blocking() {
        for kind in [
            ConflictKindV1::Identity,
            ConflictKindV1::FieldValue,
            ConflictKindV1::Relationship,
            ConflictKindV1::Timeline,
            ConflictKindV1::WorldRule,
            ConflictKindV1::PlotThread,
            ConflictKindV1::ChapterState,
            ConflictKindV1::SourceHash,
            ConflictKindV1::Semantic,
        ] {
            let (persisted, evidence) = persisted(kind);
            let current = if kind == ConflictKindV1::SourceHash {
                Some("0".repeat(64))
            } else {
                Some("f".repeat(64))
            };
            let set = detect_conflicts(&persisted, &target(), current.as_deref(), &[evidence], 2)
                .unwrap()
                .unwrap();
            assert!(set.requires_resolution);
            assert!(set.conflicts.iter().any(|item| item.kind == kind));
        }
    }

    #[test]
    fn compatible_fact_does_not_create_false_conflict() {
        let (mut persisted, mut evidence) = persisted(ConflictKindV1::WorldRule);
        persisted.proposal.facts[0].value = "现有值".to_owned();
        evidence.value = "现有值".to_owned();
        assert!(
            detect_conflicts(&persisted, &target(), Some(&"f".repeat(64)), &[evidence], 2)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn five_resolutions_are_hash_bound_and_conflict_changes_always_confirm() {
        let (persisted, evidence) = persisted(ConflictKindV1::Identity);
        let set = detect_conflicts(&persisted, &target(), Some(&"f".repeat(64)), &[evidence], 2)
            .unwrap()
            .unwrap();
        for choice in [
            ConflictResolutionChoiceV1::KeepExisting,
            ConflictResolutionChoiceV1::AdoptCandidate,
            ConflictResolutionChoiceV1::MergeManual,
            ConflictResolutionChoiceV1::SaveAsAlternative,
            ConflictResolutionChoiceV1::CancelChange,
        ] {
            let result = resolve_conflict_set(&set, &set.conflict_set_hash, choice, true).unwrap();
            assert!(result.consumed);
            if matches!(
                choice,
                ConflictResolutionChoiceV1::AdoptCandidate
                    | ConflictResolutionChoiceV1::MergeManual
                    | ConflictResolutionChoiceV1::SaveAsAlternative
            ) {
                assert!(result.requires_write_confirmation);
            }
        }
        assert_eq!(
            resolve_conflict_set(
                &set,
                &"0".repeat(64),
                ConflictResolutionChoiceV1::AdoptCandidate,
                true
            )
            .unwrap_err()
            .code,
            "agent-conflict.binding-invalid"
        );
    }
}
