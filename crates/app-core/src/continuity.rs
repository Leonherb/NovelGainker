//! NR-503 mechanical source contracts. These validate provenance and coverage,
//! never the semantic truth of a model's inference or its chosen method.
use crate::canon_change::{CanonEntityKind, ChangeError, ChangeEvidence};
use crate::chapter_contract::ChapterIdentity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterSourceState {
    Current,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChapterDependency {
    pub project_id: String,
    pub transaction_id: String,
    pub revision: u64,
    pub identity: ChapterIdentity,
    pub relative_path: String,
    pub after_hash: String,
    pub read_receipt_id: String,
}

impl ChapterDependency {
    pub fn validate(&self) -> Result<(), ChangeError> {
        for id in [
            &self.project_id,
            &self.transaction_id,
            &self.read_receipt_id,
        ] {
            if !Uuid::parse_str(id).is_ok_and(|uuid| !uuid.is_nil() && uuid.to_string() == *id) {
                return Err(ChangeError::SourceInvalid);
            }
        }
        self.identity
            .validate()
            .map_err(|_| ChangeError::SourceInvalid)?;
        CanonEntityKind::Chapter.validate_path(&self.relative_path)?;
        if self.revision == 0 || !hash(&self.after_hash) {
            return Err(ChangeError::SourceInvalid);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityDimension {
    Character,
    Timeline,
    WorldRule,
    Clue,
    Macguffin,
    PlotThread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    Fact,
    Inference,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    ExaminedSources,
    Partial,
    NotRead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContinuityCoverage {
    pub dimension: ContinuityDimension,
    pub status: CoverageStatus,
    pub read_receipt_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceCitation {
    pub read_receipt_id: String,
    pub relative_path: String,
    pub content_hash: String,
    #[serde(deserialize_with = "required_nullable_identity")]
    pub chapter_id: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub quote: String,
}

fn required_nullable_identity<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContinuityFinding {
    pub dimension: ContinuityDimension,
    pub kind: FindingKind,
    pub statement: String,
    pub citations: Vec<SourceCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContinuityReport {
    pub schema_version: u16,
    pub project_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub coverage: Vec<ContinuityCoverage>,
    pub findings: Vec<ContinuityFinding>,
}

impl ContinuityReport {
    pub fn validate_sources(&self, evidence: &ChangeEvidence) -> Result<(), ChangeError> {
        if self.schema_version != 1
            || !evidence.matches_context(&self.project_id, &self.session_id, &self.turn_id)
            || self.coverage.len() != 6
            || self
                .coverage
                .iter()
                .map(|entry| entry.dimension)
                .collect::<BTreeSet<_>>()
                .len()
                != 6
            || self.findings.len() > 32
            || serde_json::to_vec(self)
                .map_err(|_| ChangeError::InvalidRequest)?
                .len()
                > 32 * 1024
        {
            return Err(ChangeError::InvalidRequest);
        }
        for coverage in &self.coverage {
            if coverage.read_receipt_ids.len() > 8
                || coverage
                    .read_receipt_ids
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != coverage.read_receipt_ids.len()
                || (coverage.status == CoverageStatus::NotRead)
                    != coverage.read_receipt_ids.is_empty()
                || coverage
                    .read_receipt_ids
                    .iter()
                    .any(|id| !evidence.has_delivered_read(id))
            {
                return Err(ChangeError::IncompleteRead);
            }
        }
        for finding in &self.findings {
            if finding.statement.trim().is_empty()
                || finding.statement.len() > 2048
                || finding.citations.is_empty()
                || finding.citations.len() > 8
                || self.coverage.iter().any(|coverage| {
                    coverage.dimension == finding.dimension
                        && coverage.status == CoverageStatus::NotRead
                })
            {
                return Err(ChangeError::SourceInvalid);
            }
            for citation in &finding.citations {
                if !self.coverage.iter().any(|coverage| {
                    coverage.dimension == finding.dimension
                        && coverage
                            .read_receipt_ids
                            .contains(&citation.read_receipt_id)
                }) {
                    return Err(ChangeError::SourceInvalid);
                }
                evidence.validate_citation(citation)?;
            }
        }
        Ok(())
    }
}

fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
