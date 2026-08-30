use crate::project::ProjectHandle;
use crate::workspace::{read_project_file, resolve_scoped_target, MAX_EDITABLE_FILE_BYTES};
use crate::AppError;
use novelgainker_agent::action_contracts::{RegisteredToolIdV1, ToolRequestV1};
use novelgainker_agent::authorization::{
    authorize_turn, AuthorizationEffectV1, GovernedActionV1, RiskLevelV1, ValidatedMutationShapeV1,
    WriteToolClassificationV1,
};
use novelgainker_agent::contracts::{AuthorizationPolicyV1, ContentHashV1};
use novelgainker_agent::persistence_contracts::reject_sensitive_text;
use novelgainker_agent::runtime_registry::write_path_allowed_for_skills;
use novelgainker_agent::transaction_contracts::{
    FrozenFileStateV1, MultiFileTransactionPlanV1, PlannedFileMutationV1,
};
use novelgainker_validation::validate_source_bytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;

pub const AGENT_PATCH_FORMAT_V1: &str = "novelgainker.patch-v1";
const MAX_EXACT_EDITS: usize = 32;
const MAX_DIFF_MATRIX_CELLS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWriteOperationV1 {
    Create,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentDiffLineKindV1 {
    Context,
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDiffLineV1 {
    pub kind: AgentDiffLineKindV1,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWriteFilePreviewV1 {
    pub call_id: String,
    pub tool_id: String,
    pub relative_path: String,
    pub operation: AgentWriteOperationV1,
    pub mutation_shape: ValidatedMutationShapeV1,
    pub action: GovernedActionV1,
    pub risk: RiskLevelV1,
    pub expected_source_hash: Option<String>,
    pub result_hash: String,
    pub result_bytes: u32,
    pub full_content: String,
    pub line_diff: Vec<AgentDiffLineV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWritePreviewV1 {
    pub schema_version: u16,
    pub pending_id: String,
    pub policy: AuthorizationPolicyV1,
    pub authorization_effect: AuthorizationEffectV1,
    pub requires_confirmation: bool,
    pub can_confirm: bool,
    pub transaction_hash: String,
    pub action_plan_hash: String,
    pub source_set_hash: String,
    pub policy_snapshot_hash: String,
    pub highest_risk: RiskLevelV1,
    pub files: Vec<AgentWriteFilePreviewV1>,
    pub project_write_count: u16,
}

#[derive(Clone)]
pub struct PreparedAgentMutationV1 {
    pub request: ToolRequestV1,
    pub operation: AgentWriteOperationV1,
    pub source_bytes: Option<Vec<u8>>,
    pub target_bytes: Vec<u8>,
    pub classification: WriteToolClassificationV1,
}

impl fmt::Debug for PreparedAgentMutationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedAgentMutationV1")
            .field("path", &self.classification.path.as_str())
            .field("operation", &self.operation)
            .field("source_bytes", &"[REDACTED]")
            .field("target_bytes", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct PreparedAgentWriteV1 {
    pub preview: AgentWritePreviewV1,
    pub plan: MultiFileTransactionPlanV1,
    pub mutations: Vec<PreparedAgentMutationV1>,
}

impl fmt::Debug for PreparedAgentWriteV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedAgentWriteV1")
            .field("pending_id", &self.preview.pending_id)
            .field("transaction_hash", &self.preview.transaction_hash)
            .field("file_count", &self.mutations.len())
            .finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PatchEnvelopeV1 {
    schema_version: u16,
    format: String,
    operation: PatchOperationKindV1,
    edits: Option<Vec<ExactEditV1>>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PatchOperationKindV1 {
    ReplaceExact,
    ReplaceWhole,
    ReplaceWholeChapter,
    PromoteChapterState,
    UpdateStoryLedger,
    ModifyCriticalMetadata,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExactEditV1 {
    find: String,
    replace: String,
}

pub fn prepare_write_batch(
    handle: &ProjectHandle,
    pending_id: String,
    requests: &[ToolRequestV1],
    skill_ids: &[String],
    policy: AuthorizationPolicyV1,
) -> Result<PreparedAgentWriteV1, AppError> {
    if requests.is_empty()
        || requests.len()
            > novelgainker_agent::contracts::MAX_MODIFIED_PROJECT_FILES_PER_TURN as usize
        || !pending_id.starts_with("pnd_")
    {
        return Err(write_error("agent-write.batch-invalid"));
    }
    let skill_refs = skill_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let mut paths = BTreeSet::new();
    let mut prepared = Vec::with_capacity(requests.len());
    let mut planned = Vec::with_capacity(requests.len());
    let mut previews = Vec::with_capacity(requests.len());

    for request in requests {
        request
            .validate()
            .map_err(|error| write_error(error.stable_code()))?;
        if !request.tool_id.is_write() {
            return Err(write_error("agent-write.read-tool-in-write-batch"));
        }
        let path = request
            .arguments
            .path
            .as_ref()
            .ok_or_else(|| write_error("agent-write.path-required"))?;
        if !paths.insert(path.as_str().to_owned())
            || !write_path_allowed_for_skills(&skill_refs, path.as_str())
            || is_forbidden_target(path.as_str())
        {
            return Err(write_error("agent-write.path-policy-blocked"));
        }
        let mutation = prepare_mutation(handle, request)?;
        let result_hash = ContentHashV1::digest(&mutation.target_bytes);
        let estimated_undo_bytes = u32::try_from(
            mutation
                .source_bytes
                .as_ref()
                .map_or(1usize, |source| source.len().max(1)),
        )
        .map_err(|_| write_error("agent-write.undo-limit"))?;
        planned.push(PlannedFileMutationV1 {
            frozen: FrozenFileStateV1 {
                path: mutation.classification.path.clone(),
                source_hash: mutation
                    .source_bytes
                    .as_ref()
                    .map(|source| ContentHashV1::digest(source)),
                target_must_be_absent: mutation.source_bytes.is_none(),
            },
            classification: mutation.classification.clone(),
            planned_result_hash: result_hash.clone(),
            estimated_undo_bytes,
        });
        let original = mutation
            .source_bytes
            .as_ref()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .unwrap_or_default();
        let target = std::str::from_utf8(&mutation.target_bytes)
            .map_err(|_| write_error("agent-write.invalid-utf8"))?;
        previews.push(AgentWriteFilePreviewV1 {
            call_id: request.call_id.as_str().to_owned(),
            tool_id: tool_name(request.tool_id).to_owned(),
            relative_path: path.as_str().to_owned(),
            operation: mutation.operation,
            mutation_shape: mutation.classification.mutation_shape,
            action: mutation.classification.action,
            risk: mutation.classification.risk,
            expected_source_hash: mutation
                .source_bytes
                .as_ref()
                .map(|source| ContentHashV1::digest(source).as_str().to_owned()),
            result_hash: result_hash.as_str().to_owned(),
            result_bytes: mutation.target_bytes.len().min(u32::MAX as usize) as u32,
            full_content: target.to_owned(),
            line_diff: line_diff(original, target),
        });
        prepared.push(mutation);
    }

    let binding = &requests[0].binding;
    if requests.iter().any(|request| request.binding != *binding) {
        return Err(write_error("agent-write.binding-mismatch"));
    }
    let policy_snapshot_hash = ContentHashV1::digest(
        &serde_json::to_vec(&(policy, binding)).expect("policy serialization is infallible"),
    );
    let plan = MultiFileTransactionPlanV1::build(
        binding.project_id.clone(),
        binding.session_id.clone(),
        binding.turn_id.clone(),
        policy_snapshot_hash.clone(),
        planned,
    )
    .map_err(|error| write_error(&error.to_string()))?;
    let actions = prepared
        .iter()
        .map(|mutation| mutation.classification.action)
        .collect::<Vec<_>>();
    let authorization =
        authorize_turn(policy, &actions).map_err(|error| write_error(&error.to_string()))?;
    let highest_risk = prepared
        .iter()
        .map(|mutation| mutation.classification.risk)
        .max()
        .unwrap_or(RiskLevelV1::ForbiddenV1);
    let requires_confirmation =
        authorization.transaction_effect == AuthorizationEffectV1::AwaitConfirmation;
    let can_confirm = authorization.transaction_effect != AuthorizationEffectV1::Block;
    let preview = AgentWritePreviewV1 {
        schema_version: 1,
        pending_id,
        policy,
        authorization_effect: authorization.transaction_effect,
        requires_confirmation,
        can_confirm,
        transaction_hash: plan.transaction_hash.as_str().to_owned(),
        action_plan_hash: plan.action_plan_hash.as_str().to_owned(),
        source_set_hash: plan.source_set_hash.as_str().to_owned(),
        policy_snapshot_hash: policy_snapshot_hash.as_str().to_owned(),
        highest_risk,
        files: previews,
        project_write_count: 0,
    };
    Ok(PreparedAgentWriteV1 {
        preview,
        plan,
        mutations: prepared,
    })
}

fn prepare_mutation(
    handle: &ProjectHandle,
    request: &ToolRequestV1,
) -> Result<PreparedAgentMutationV1, AppError> {
    let path = request
        .arguments
        .path
        .as_ref()
        .ok_or_else(|| write_error("agent-write.path-required"))?;
    let target_path = resolve_scoped_target(handle.root(), path.as_str())?;
    let (operation, source_bytes, target_bytes, mutation_shape) = match request.tool_id {
        RegisteredToolIdV1::ProjectCreateMarkdown => {
            match fs::symlink_metadata(&target_path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) => return Err(write_error("agent-write.target-exists")),
                Err(_) => return Err(write_error("agent-write.target-unreadable")),
            }
            let content = request
                .arguments
                .content
                .as_ref()
                .ok_or_else(|| write_error("agent-write.content-required"))?;
            let shape = if path.as_str().starts_with("chapters/") {
                ValidatedMutationShapeV1::CreateDraftChapter
            } else {
                ValidatedMutationShapeV1::CreateEntityMarkdown
            };
            (
                AgentWriteOperationV1::Create,
                None,
                content.as_bytes().to_vec(),
                shape,
            )
        }
        RegisteredToolIdV1::ProjectPatchMarkdown => {
            let source = read_project_file(handle, path.as_str())?;
            let expected = request
                .arguments
                .expected_source_hash
                .as_ref()
                .ok_or_else(|| write_error("agent-write.source-hash-required"))?;
            if source.content_hash != expected.as_str() {
                return Err(write_error("agent-write.source-changed"));
            }
            let patch = request
                .arguments
                .patch
                .as_ref()
                .ok_or_else(|| write_error("agent-write.patch-required"))?;
            let envelope: PatchEnvelopeV1 = serde_json::from_str(patch)
                .map_err(|_| write_error("agent-write.patch-invalid"))?;
            if envelope.schema_version != 1 || envelope.format != AGENT_PATCH_FORMAT_V1 {
                return Err(write_error("agent-write.patch-invalid"));
            }
            let (target, shape) = apply_patch_envelope(path.as_str(), &source.content, envelope)?;
            (
                AgentWriteOperationV1::Replace,
                Some(source.content.into_bytes()),
                target.into_bytes(),
                shape,
            )
        }
        _ => return Err(write_error("agent-write.tool-policy-blocked")),
    };

    validate_target_content(path.as_str(), &target_bytes)?;
    if source_bytes.as_deref() == Some(target_bytes.as_slice()) {
        return Err(write_error("agent-write.no-change"));
    }
    let classification = WriteToolClassificationV1::build(request, mutation_shape)
        .map_err(|error| write_error(&error.to_string()))?;
    classification
        .validate()
        .map_err(|error| write_error(&error.to_string()))?;
    if classification.risk == RiskLevelV1::ForbiddenV1 {
        return Err(write_error("agent-write.forbidden-v1"));
    }
    Ok(PreparedAgentMutationV1 {
        request: request.clone(),
        operation,
        source_bytes,
        target_bytes,
        classification,
    })
}

fn apply_patch_envelope(
    path: &str,
    source: &str,
    envelope: PatchEnvelopeV1,
) -> Result<(String, ValidatedMutationShapeV1), AppError> {
    match envelope.operation {
        PatchOperationKindV1::ReplaceExact if envelope.content.is_none() => {
            let edits = envelope
                .edits
                .ok_or_else(|| write_error("agent-write.patch-edits-required"))?;
            if edits.is_empty() || edits.len() > MAX_EXACT_EDITS {
                return Err(write_error("agent-write.patch-edit-count"));
            }
            let replaces_whole_document = edits.len() == 1 && edits[0].find == source;
            let mut output = source.to_owned();
            for edit in edits {
                if edit.find.is_empty() || edit.find == edit.replace {
                    return Err(write_error("agent-write.patch-edit-invalid"));
                }
                if output.match_indices(&edit.find).count() != 1 {
                    return Err(write_error("agent-write.patch-match-not-unique"));
                }
                output = output.replacen(&edit.find, &edit.replace, 1);
            }
            Ok((
                output,
                if replaces_whole_document {
                    if path.starts_with("chapters/") {
                        ValidatedMutationShapeV1::ReplaceWholeChapter
                    } else {
                        ValidatedMutationShapeV1::ReplaceWholeFile
                    }
                } else {
                    ValidatedMutationShapeV1::PatchHunks
                },
            ))
        }
        PatchOperationKindV1::ReplaceWhole if envelope.edits.is_none() => Ok((
            envelope
                .content
                .ok_or_else(|| write_error("agent-write.patch-content-required"))?,
            if path.starts_with("chapters/") {
                ValidatedMutationShapeV1::ReplaceWholeChapter
            } else {
                ValidatedMutationShapeV1::ReplaceWholeFile
            },
        )),
        PatchOperationKindV1::ReplaceWholeChapter
            if path.starts_with("chapters/") && envelope.edits.is_none() =>
        {
            Ok((
                envelope
                    .content
                    .ok_or_else(|| write_error("agent-write.patch-content-required"))?,
                ValidatedMutationShapeV1::ReplaceWholeChapter,
            ))
        }
        PatchOperationKindV1::PromoteChapterState
            if path.starts_with("chapters/") && envelope.edits.is_none() =>
        {
            Ok((
                envelope
                    .content
                    .ok_or_else(|| write_error("agent-write.patch-content-required"))?,
                ValidatedMutationShapeV1::PromoteChapterState,
            ))
        }
        PatchOperationKindV1::UpdateStoryLedger
            if path.eq_ignore_ascii_case("materials/000.story-ledger.md")
                && envelope.edits.is_none() =>
        {
            Ok((
                envelope
                    .content
                    .ok_or_else(|| write_error("agent-write.patch-content-required"))?,
                ValidatedMutationShapeV1::UpdateStoryLedger,
            ))
        }
        PatchOperationKindV1::ModifyCriticalMetadata
            if path.eq_ignore_ascii_case("project.md") && envelope.edits.is_none() =>
        {
            Ok((
                envelope
                    .content
                    .ok_or_else(|| write_error("agent-write.patch-content-required"))?,
                ValidatedMutationShapeV1::ModifyCriticalMetadata,
            ))
        }
        _ => Err(write_error("agent-write.patch-shape-path-mismatch")),
    }
}

fn validate_target_content(path: &str, bytes: &[u8]) -> Result<(), AppError> {
    if bytes.is_empty()
        || bytes.len() > MAX_EDITABLE_FILE_BYTES as usize
        || bytes.starts_with(&[0xef, 0xbb, 0xbf])
        || bytes.contains(&b'\r')
        || !bytes.ends_with(b"\n")
    {
        return Err(write_error("agent-write.content-invalid"));
    }
    let content =
        std::str::from_utf8(bytes).map_err(|_| write_error("agent-write.invalid-utf8"))?;
    reject_sensitive_text(content)
        .map_err(|_| write_error("agent-write.sensitive-content-forbidden"))?;
    let validation = validate_source_bytes(path, bytes);
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(write_error("agent-write.markdown-invalid"));
    }
    Ok(())
}

fn is_forbidden_target(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower == "agents.md"
        || lower == "claude.md"
        || lower.starts_with('.')
        || lower.starts_with("published/")
        || !lower.ends_with(".md")
}

fn tool_name(tool: RegisteredToolIdV1) -> &'static str {
    match tool {
        RegisteredToolIdV1::ProjectCreateMarkdown => "project.create_markdown",
        RegisteredToolIdV1::ProjectPatchMarkdown => "project.patch_markdown",
        _ => "unknown",
    }
}

fn line_diff(original: &str, proposed: &str) -> Vec<AgentDiffLineV1> {
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
    let (mut old_index, mut new_index) = (0usize, 0usize);
    while old_index < old.len() || new_index < new.len() {
        if old_index < old.len() && new_index < new.len() && old[old_index] == new[new_index] {
            output.push(diff_line(
                AgentDiffLineKindV1::Context,
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
                AgentDiffLineKindV1::Add,
                None,
                Some(new_index + 1),
                new[new_index],
            ));
            new_index += 1;
        } else {
            output.push(diff_line(
                AgentDiffLineKindV1::Remove,
                Some(old_index + 1),
                None,
                old[old_index],
            ));
            old_index += 1;
        }
    }
    output
}

fn coarse_line_diff(old: &[&str], new: &[&str]) -> Vec<AgentDiffLineV1> {
    let mut output = old
        .iter()
        .enumerate()
        .map(|(index, content)| {
            diff_line(AgentDiffLineKindV1::Remove, Some(index + 1), None, content)
        })
        .collect::<Vec<_>>();
    output.extend(new.iter().enumerate().map(|(index, content)| {
        diff_line(AgentDiffLineKindV1::Add, None, Some(index + 1), content)
    }));
    output
}

fn diff_line(
    kind: AgentDiffLineKindV1,
    old_line: Option<usize>,
    new_line: Option<usize>,
    content: &str,
) -> AgentDiffLineV1 {
    AgentDiffLineV1 {
        kind,
        old_line: old_line.map(|value| value.min(u32::MAX as usize) as u32),
        new_line: new_line.map(|value| value.min(u32::MAX as usize) as u32),
        content: content.to_owned(),
    }
}

fn write_error(code: &str) -> AppError {
    AppError::agent_contract(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{create_project, inspect_project_root, ProjectScanMode};
    use novelgainker_agent::action_contracts::{
        ProjectRelativePathV1, ProjectToolBindingV1, ToolArgumentsV1, ToolCallIdV1,
        AGENT_ACTION_SCHEMA_VERSION,
    };
    use novelgainker_agent::contracts::{ProjectIdV1, SessionIdV1, TurnIdV1};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fixture() -> (PathBuf, ProjectHandle) {
        let root =
            std::env::temp_dir().join(format!("novelgainker-am2-preview-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
        let handle = create_project(&inspection).unwrap();
        fs::write(
            root.join("characters/hero.md"),
            "---\ntitle: 阿澜\nwritingLanguage: zh-CN\n---\n\n旧动机。\n",
        )
        .unwrap();
        (root, handle)
    }

    fn binding() -> ProjectToolBindingV1 {
        ProjectToolBindingV1 {
            project_id: ProjectIdV1::parse("prj_fixture0001").unwrap(),
            session_id: SessionIdV1::parse("ses_fixture0001").unwrap(),
            turn_id: TurnIdV1::parse("trn_fixture0001").unwrap(),
        }
    }

    fn request(tool_id: RegisteredToolIdV1, path: &str, content: Option<String>) -> ToolRequestV1 {
        ToolRequestV1 {
            schema_version: AGENT_ACTION_SCHEMA_VERSION,
            binding: binding(),
            serial_ordinal: 0,
            call_id: ToolCallIdV1::parse(format!("call_{}", Uuid::new_v4().simple())).unwrap(),
            tool_id,
            arguments: ToolArgumentsV1 {
                path: Some(ProjectRelativePathV1::parse(path).unwrap()),
                directory: None,
                query: None,
                content: if tool_id == RegisteredToolIdV1::ProjectCreateMarkdown {
                    content.clone()
                } else {
                    None
                },
                patch: if tool_id == RegisteredToolIdV1::ProjectPatchMarkdown {
                    content
                } else {
                    None
                },
                expected_source_hash: None,
            },
            purpose: "更新人物".to_owned(),
        }
    }

    #[test]
    fn create_and_exact_patch_build_zero_write_previews() {
        let (root, handle) = fixture();
        let create = request(
            RegisteredToolIdV1::ProjectCreateMarkdown,
            "characters/new.md",
            Some("---\ntitle: 新人物\nwritingLanguage: zh-CN\n---\n\n新人物。\n".to_owned()),
        );
        let prepared = prepare_write_batch(
            &handle,
            "pnd_fixture0001".to_owned(),
            &[create],
            &["create-character".to_owned()],
            AuthorizationPolicyV1::AutoApplySafe,
        )
        .unwrap();
        assert_eq!(prepared.preview.project_write_count, 0);
        assert!(!prepared.preview.requires_confirmation);
        assert_eq!(prepared.preview.highest_risk, RiskLevelV1::WriteSafe);
        assert!(!root.join("characters/new.md").exists());

        let source = fs::read(root.join("characters/hero.md")).unwrap();
        let patch = serde_json::json!({
            "schemaVersion": 1,
            "format": AGENT_PATCH_FORMAT_V1,
            "operation": "replace_exact",
            "edits": [{"find": "旧动机。", "replace": "寻找潮门。"}]
        })
        .to_string();
        let mut patch_request = request(
            RegisteredToolIdV1::ProjectPatchMarkdown,
            "characters/hero.md",
            Some(patch),
        );
        patch_request.arguments.expected_source_hash = Some(ContentHashV1::digest(&source));
        let prepared = prepare_write_batch(
            &handle,
            "pnd_fixture0002".to_owned(),
            &[patch_request],
            &["create-character".to_owned()],
            AuthorizationPolicyV1::ReviewEveryWrite,
        )
        .unwrap();
        assert!(prepared.preview.requires_confirmation);
        assert!(prepared.preview.files[0]
            .line_diff
            .iter()
            .any(|line| line.kind == AgentDiffLineKindV1::Add));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn high_risk_and_negative_matrix_are_enforced_before_write() {
        let (root, handle) = fixture();
        let source = fs::read(root.join("characters/hero.md")).unwrap();
        let whole = serde_json::json!({
            "schemaVersion": 1,
            "format": AGENT_PATCH_FORMAT_V1,
            "operation": "replace_whole",
            "content": "---\ntitle: 阿澜\nwritingLanguage: zh-CN\n---\n\n整文件。\n"
        })
        .to_string();
        let mut whole_request = request(
            RegisteredToolIdV1::ProjectPatchMarkdown,
            "characters/hero.md",
            Some(whole),
        );
        whole_request.arguments.expected_source_hash = Some(ContentHashV1::digest(&source));
        let prepared = prepare_write_batch(
            &handle,
            "pnd_fixture0003".to_owned(),
            &[whole_request],
            &["create-character".to_owned()],
            AuthorizationPolicyV1::AutoApplySafe,
        )
        .unwrap();
        assert!(prepared.preview.requires_confirmation);
        assert_eq!(prepared.preview.highest_risk, RiskLevelV1::AlwaysConfirm);

        let forbidden = request(
            RegisteredToolIdV1::ProjectCreateMarkdown,
            "world/new.md",
            Some("---\ntitle: 世界\nwritingLanguage: zh-CN\n---\n\n世界。\n".to_owned()),
        );
        assert!(prepare_write_batch(
            &handle,
            "pnd_fixture0004".to_owned(),
            &[forbidden],
            &["create-character".to_owned()],
            AuthorizationPolicyV1::AutoApplySafe,
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
