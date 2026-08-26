use std::fmt;

use novelgainker_validation::validate_source_bytes;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::registry;

pub const CANON_PROPOSAL_SCHEMA_VERSION: u16 = 1;
pub const MAX_PROPOSAL_BYTES: usize = 1024 * 1024;
const MAX_DIFF_MATRIX_CELLS: usize = 1_000_000;
const ABSENT_SOURCE: &str = "absent";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonFileOperationKind {
    Create,
    Replace,
}

impl CanonFileOperationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Replace => "replace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonFileOperationV1 {
    pub kind: CanonFileOperationKind,
    pub relative_path: String,
    pub expected_source_hash: String,
    pub proposed_content_hash: String,
    pub proposed_byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposalValidationSummaryV1 {
    pub strict: bool,
    pub eligible_for_ai_apply: bool,
    pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonProposalV1 {
    pub schema_version: u16,
    pub proposal_id: String,
    pub workflow_id: String,
    pub workflow_version: u16,
    pub pack_hash: String,
    pub run_id: String,
    pub provider_id: String,
    pub preset_version: u16,
    pub model: String,
    pub manifest_hash: String,
    pub operation: CanonFileOperationV1,
    pub validation_summary: ProposalValidationSummaryV1,
    pub warning_codes: Vec<String>,
    pub proposal_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalDiffLineKind {
    Context,
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposalDiffLineV1 {
    pub kind: ProposalDiffLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonProposalViewV1 {
    pub schema_version: u16,
    pub proposal_id: String,
    pub proposal_hash: String,
    pub workflow_id: String,
    pub workflow_version: u16,
    pub pack_hash: String,
    pub provider_id: String,
    pub preset_version: u16,
    pub model: String,
    pub manifest_hash: String,
    pub operation: CanonFileOperationV1,
    pub run_id: String,
    pub validation_summary: ProposalValidationSummaryV1,
    pub warning_codes: Vec<String>,
    pub full_content: String,
    pub line_diff: Vec<ProposalDiffLineV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildCanonProposalInput {
    pub proposal_id: String,
    pub workflow_id: String,
    pub workflow_version: u16,
    pub pack_hash: String,
    pub run_id: String,
    pub provider_id: String,
    pub preset_version: u16,
    pub model: String,
    pub manifest_hash: String,
    pub operation_kind: CanonFileOperationKind,
    pub relative_path: String,
    pub expected_source_hash: Option<String>,
    pub original_content: Option<String>,
    pub proposed_content: String,
    pub warning_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalError {
    InvalidIdentity,
    InvalidBinding,
    InvalidPath,
    InvalidSourceState,
    InvalidContent,
    ValidationFailed,
    IntegrityMismatch,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RetainedCanonProposal {
    metadata: CanonProposalV1,
    proposed_content: String,
    line_diff: Vec<ProposalDiffLineV1>,
}

impl fmt::Debug for RetainedCanonProposal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedCanonProposal")
            .field("proposal_id", &self.metadata.proposal_id)
            .field("proposal_hash", &self.metadata.proposal_hash)
            .field("operation", &self.metadata.operation)
            .field("proposed_content", &"[REDACTED]")
            .field("line_diff", &"[REDACTED]")
            .finish()
    }
}

impl RetainedCanonProposal {
    pub fn metadata(&self) -> &CanonProposalV1 {
        &self.metadata
    }

    pub fn proposed_bytes(&self) -> &[u8] {
        self.proposed_content.as_bytes()
    }

    pub fn view(&self) -> CanonProposalViewV1 {
        CanonProposalViewV1 {
            schema_version: self.metadata.schema_version,
            proposal_id: self.metadata.proposal_id.clone(),
            proposal_hash: self.metadata.proposal_hash.clone(),
            workflow_id: self.metadata.workflow_id.clone(),
            workflow_version: self.metadata.workflow_version,
            pack_hash: self.metadata.pack_hash.clone(),
            provider_id: self.metadata.provider_id.clone(),
            preset_version: self.metadata.preset_version,
            model: self.metadata.model.clone(),
            manifest_hash: self.metadata.manifest_hash.clone(),
            operation: self.metadata.operation.clone(),
            validation_summary: self.metadata.validation_summary.clone(),
            run_id: self.metadata.run_id.clone(),
            warning_codes: self.metadata.warning_codes.clone(),
            full_content: self.proposed_content.clone(),
            line_diff: self.line_diff.clone(),
        }
    }

    pub fn verify_integrity(&self) -> Result<(), ProposalError> {
        if sha256_hex(self.proposed_content.as_bytes())
            != self.metadata.operation.proposed_content_hash
            || self.proposed_content.len() as u64 != self.metadata.operation.proposed_byte_count
            || proposal_hash(&self.metadata)? != self.metadata.proposal_hash
        {
            return Err(ProposalError::IntegrityMismatch);
        }
        Ok(())
    }
}

pub fn build_canon_proposal(
    mut input: BuildCanonProposalInput,
) -> Result<RetainedCanonProposal, ProposalError> {
    validate_identity(&input)?;
    validate_safe_relative_path(&input.relative_path)?;
    validate_code_list(&mut input.warning_codes)?;
    if input.proposed_content.is_empty()
        || input.proposed_content.len() > MAX_PROPOSAL_BYTES
        || input.proposed_content.contains('\r')
        || input.proposed_content.starts_with('\u{feff}')
        || !input.proposed_content.ends_with('\n')
    {
        return Err(ProposalError::InvalidContent);
    }

    let expected_source_hash = match input.operation_kind {
        CanonFileOperationKind::Create => {
            if input.expected_source_hash.is_some() || input.original_content.is_some() {
                return Err(ProposalError::InvalidSourceState);
            }
            ABSENT_SOURCE.to_owned()
        }
        CanonFileOperationKind::Replace => {
            let expected = input
                .expected_source_hash
                .as_deref()
                .filter(|value| is_sha256(value))
                .ok_or(ProposalError::InvalidSourceState)?;
            let original = input
                .original_content
                .as_deref()
                .ok_or(ProposalError::InvalidSourceState)?;
            if sha256_hex(original.as_bytes()) != expected {
                return Err(ProposalError::InvalidSourceState);
            }
            expected.to_owned()
        }
    };

    let validation = validate_source_bytes(&input.relative_path, input.proposed_content.as_bytes());
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(ProposalError::ValidationFailed);
    }
    let proposed_content_hash = validation.content_hash;
    let validation_summary = ProposalValidationSummaryV1 {
        strict: true,
        eligible_for_ai_apply: true,
        diagnostic_codes: Vec::new(),
    };
    let operation = CanonFileOperationV1 {
        kind: input.operation_kind,
        relative_path: input.relative_path,
        expected_source_hash,
        proposed_content_hash,
        proposed_byte_count: input.proposed_content.len() as u64,
    };
    let mut metadata = CanonProposalV1 {
        schema_version: CANON_PROPOSAL_SCHEMA_VERSION,
        proposal_id: input.proposal_id,
        workflow_id: input.workflow_id,
        workflow_version: input.workflow_version,
        pack_hash: input.pack_hash,
        run_id: input.run_id,
        provider_id: input.provider_id,
        preset_version: input.preset_version,
        model: input.model,
        manifest_hash: input.manifest_hash,
        operation,
        validation_summary,
        warning_codes: input.warning_codes,
        proposal_hash: String::new(),
    };
    metadata.proposal_hash = proposal_hash(&metadata)?;
    let line_diff = match input.operation_kind {
        CanonFileOperationKind::Create => Vec::new(),
        CanonFileOperationKind::Replace => line_diff(
            input.original_content.as_deref().unwrap_or_default(),
            &input.proposed_content,
        ),
    };
    let retained = RetainedCanonProposal {
        metadata,
        proposed_content: input.proposed_content,
        line_diff,
    };
    retained.verify_integrity()?;
    Ok(retained)
}

fn validate_identity(input: &BuildCanonProposalInput) -> Result<(), ProposalError> {
    if Uuid::parse_str(&input.proposal_id).is_err() || Uuid::parse_str(&input.run_id).is_err() {
        return Err(ProposalError::InvalidIdentity);
    }
    let workflow = registry()
        .workflows
        .iter()
        .find(|workflow| workflow.id == input.workflow_id)
        .ok_or(ProposalError::InvalidBinding)?;
    let provider_valid = matches!(
        (
            input.provider_id.as_str(),
            input.preset_version,
            input.model.as_str()
        ),
        ("deepseek", 1, "deepseek-v4-pro") | ("openai", 1, "gpt-5.6-terra")
    );
    if workflow.version != input.workflow_version
        || input.pack_hash != registry().pack_hash
        || !provider_valid
        || !is_sha256(&input.manifest_hash)
    {
        return Err(ProposalError::InvalidBinding);
    }
    Ok(())
}

fn validate_safe_relative_path(relative_path: &str) -> Result<(), ProposalError> {
    let valid = !relative_path.is_empty()
        && relative_path.len() <= 240
        && relative_path.ends_with(".md")
        && !relative_path.starts_with('/')
        && !relative_path.contains(['\\', ':', '\0'])
        && relative_path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.starts_with('.')
                && !component.chars().any(char::is_control)
        });
    if valid {
        Ok(())
    } else {
        Err(ProposalError::InvalidPath)
    }
}

fn validate_code_list(codes: &mut Vec<String>) -> Result<(), ProposalError> {
    if codes.len() > 32
        || codes.iter().any(|code| {
            code.is_empty()
                || code.len() > 80
                || !code.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
                })
        })
    {
        return Err(ProposalError::InvalidBinding);
    }
    codes.sort();
    codes.dedup();
    Ok(())
}

fn proposal_hash(metadata: &CanonProposalV1) -> Result<String, ProposalError> {
    let mut payload = serde_json::to_value(metadata).map_err(|_| ProposalError::InvalidBinding)?;
    let Some(object) = payload.as_object_mut() else {
        return Err(ProposalError::InvalidBinding);
    };
    object.remove("proposalHash");
    let canonical = canonical_json(&payload)?;
    Ok(sha256_hex(canonical.as_bytes()))
}

pub fn canonical_json(value: &Value) -> Result<String, ProposalError> {
    fn ordered(value: &Value) -> Value {
        match value {
            Value::Array(items) => Value::Array(items.iter().map(ordered).collect()),
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort();
                let mut sorted = Map::new();
                for key in keys {
                    sorted.insert(key.clone(), ordered(&object[key]));
                }
                Value::Object(sorted)
            }
            other => other.clone(),
        }
    }
    serde_json::to_string(&ordered(value)).map_err(|_| ProposalError::InvalidBinding)
}

fn line_diff(original: &str, proposed: &str) -> Vec<ProposalDiffLineV1> {
    let old = original.lines().collect::<Vec<_>>();
    let new = proposed.lines().collect::<Vec<_>>();
    if old.len().saturating_mul(new.len()) > MAX_DIFF_MATRIX_CELLS {
        return coarse_line_diff(&old, &new);
    }
    let columns = new.len() + 1;
    let mut matrix = vec![0_u32; (old.len() + 1).saturating_mul(columns)];
    for old_index in (0..old.len()).rev() {
        for new_index in (0..new.len()).rev() {
            let index = old_index * columns + new_index;
            matrix[index] = if old[old_index] == new[new_index] {
                matrix[(old_index + 1) * columns + new_index + 1].saturating_add(1)
            } else {
                matrix[(old_index + 1) * columns + new_index]
                    .max(matrix[old_index * columns + new_index + 1])
            };
        }
    }
    let mut output = Vec::new();
    let (mut old_index, mut new_index) = (0_usize, 0_usize);
    while old_index < old.len() || new_index < new.len() {
        if old_index < old.len() && new_index < new.len() && old[old_index] == new[new_index] {
            output.push(diff_line(
                ProposalDiffLineKind::Context,
                Some(old_index + 1),
                Some(new_index + 1),
                old[old_index],
            ));
            old_index += 1;
            new_index += 1;
        } else if new_index < new.len()
            && (old_index == old.len()
                || matrix[old_index * columns + new_index + 1]
                    >= matrix[(old_index + 1) * columns + new_index])
        {
            output.push(diff_line(
                ProposalDiffLineKind::Add,
                None,
                Some(new_index + 1),
                new[new_index],
            ));
            new_index += 1;
        } else {
            output.push(diff_line(
                ProposalDiffLineKind::Remove,
                Some(old_index + 1),
                None,
                old[old_index],
            ));
            old_index += 1;
        }
    }
    output
}

fn coarse_line_diff(old: &[&str], new: &[&str]) -> Vec<ProposalDiffLineV1> {
    let prefix = old
        .iter()
        .zip(new.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let mut output = Vec::new();
    for (index, content) in old.iter().take(prefix).enumerate() {
        output.push(diff_line(
            ProposalDiffLineKind::Context,
            Some(index + 1),
            Some(index + 1),
            content,
        ));
    }
    for (offset, content) in old[prefix..old.len().saturating_sub(suffix)]
        .iter()
        .enumerate()
    {
        output.push(diff_line(
            ProposalDiffLineKind::Remove,
            Some(prefix + offset + 1),
            None,
            content,
        ));
    }
    for (offset, content) in new[prefix..new.len().saturating_sub(suffix)]
        .iter()
        .enumerate()
    {
        output.push(diff_line(
            ProposalDiffLineKind::Add,
            None,
            Some(prefix + offset + 1),
            content,
        ));
    }
    for offset in 0..suffix {
        let old_index = old.len() - suffix + offset;
        let new_index = new.len() - suffix + offset;
        output.push(diff_line(
            ProposalDiffLineKind::Context,
            Some(old_index + 1),
            Some(new_index + 1),
            old[old_index],
        ));
    }
    output
}

fn diff_line(
    kind: ProposalDiffLineKind,
    old_line: Option<usize>,
    new_line: Option<usize>,
    content: &str,
) -> ProposalDiffLineV1 {
    ProposalDiffLineV1 {
        kind,
        old_line: old_line.and_then(|line| u32::try_from(line).ok()),
        new_line: new_line.and_then(|line| u32::try_from(line).ok()),
        content: content.to_owned(),
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CHARACTER_CREATE_WORKFLOW_ID;

    fn markdown(title: &str, body: &str) -> String {
        format!("---\ntitle: {title}\nwritingLanguage: zh-CN\n---\n\n# {title}\n\n{body}\n")
    }

    fn input(kind: CanonFileOperationKind) -> BuildCanonProposalInput {
        let original = markdown("旧人物", "旧内容");
        BuildCanonProposalInput {
            proposal_id: "30000000-0000-4000-8000-000000000003".to_owned(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: registry().pack_hash.to_owned(),
            run_id: "10000000-0000-4000-8000-000000000001".to_owned(),
            provider_id: "deepseek".to_owned(),
            preset_version: 1,
            model: "deepseek-v4-pro".to_owned(),
            manifest_hash: "a".repeat(64),
            operation_kind: kind,
            relative_path: "characters/林遥.md".to_owned(),
            expected_source_hash: (kind == CanonFileOperationKind::Replace)
                .then(|| sha256_hex(original.as_bytes())),
            original_content: (kind == CanonFileOperationKind::Replace).then_some(original),
            proposed_content: markdown("林遥", "新内容"),
            warning_codes: vec!["review.relationships".to_owned()],
        }
    }

    #[test]
    fn canonical_json_and_hash_are_stable_and_cover_every_safety_binding() {
        let proposal = build_canon_proposal(input(CanonFileOperationKind::Create)).unwrap();
        assert_eq!(
            proposal.metadata().proposal_hash,
            "1453f5902b069906c292f814e8892123d61427538b2ef70b23f430635fc1de0e"
        );
        let original_hash = proposal.metadata().proposal_hash.clone();
        for mutate in [
            |value: &mut BuildCanonProposalInput| value.manifest_hash = "b".repeat(64),
            |value: &mut BuildCanonProposalInput| value.model = "gpt-5.6-terra".to_owned(),
            |value: &mut BuildCanonProposalInput| value.proposed_content.push_str("变化\n"),
            |value: &mut BuildCanonProposalInput| value.warning_codes.clear(),
        ] {
            let mut changed = input(CanonFileOperationKind::Create);
            mutate(&mut changed);
            if changed.model == "gpt-5.6-terra" {
                changed.provider_id = "openai".to_owned();
            }
            assert_ne!(
                build_canon_proposal(changed)
                    .unwrap()
                    .metadata()
                    .proposal_hash,
                original_hash
            );
        }
    }

    #[test]
    fn create_has_full_preview_and_replace_has_numbered_line_diff() {
        let create = build_canon_proposal(input(CanonFileOperationKind::Create)).unwrap();
        assert_eq!(create.view().full_content, create.proposed_content);
        assert!(create.view().line_diff.is_empty());

        let replace = build_canon_proposal(input(CanonFileOperationKind::Replace)).unwrap();
        assert!(replace
            .view()
            .line_diff
            .iter()
            .any(|line| line.kind == ProposalDiffLineKind::Remove && line.old_line.is_some()));
        assert!(replace
            .view()
            .line_diff
            .iter()
            .any(|line| line.kind == ProposalDiffLineKind::Add && line.new_line.is_some()));
    }

    #[test]
    fn rejects_path_attacks_invalid_source_state_and_unvalidated_content() {
        for path in [
            "../escape.md",
            "characters\\escape.md",
            "C:/escape.md",
            ".hidden/a.md",
        ] {
            let mut attacked = input(CanonFileOperationKind::Create);
            attacked.relative_path = path.to_owned();
            assert_eq!(
                build_canon_proposal(attacked),
                Err(ProposalError::InvalidPath)
            );
        }
        let mut forged_replace = input(CanonFileOperationKind::Replace);
        forged_replace.expected_source_hash = Some("0".repeat(64));
        assert_eq!(
            build_canon_proposal(forged_replace),
            Err(ProposalError::InvalidSourceState)
        );
        let mut invalid = input(CanonFileOperationKind::Create);
        invalid.proposed_content = "# no frontmatter\n".to_owned();
        assert_eq!(
            build_canon_proposal(invalid),
            Err(ProposalError::ValidationFailed)
        );
    }
}
