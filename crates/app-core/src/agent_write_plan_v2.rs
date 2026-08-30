use crate::agent_canon::{
    normalize_canon_target, serialize_canon_artifact, CanonTargetResolutionV1,
    NormalizedCanonTargetV1, PersistedCanonProposalV1, SerializedCanonArtifactV1,
};
use crate::agent_write::{prepare_write_batch, AgentWritePreviewV1, PreparedAgentWriteV1};
use crate::app_state::agent_project_id;
use crate::project::ProjectHandle;
use crate::workspace::read_project_file;
use crate::AppError;
use novelgainker_agent::action_contracts::{
    ProjectRelativePathV1, ProjectToolBindingV1, RegisteredToolIdV1, ToolArgumentsV1, ToolCallIdV1,
    ToolRequestV1, AGENT_ACTION_SCHEMA_VERSION,
};
use novelgainker_agent::contracts::{
    AuthorizationPolicyV1, ContentHashV1, ProjectIdV1, SessionIdV1, TurnIdV1,
};
use novelgainker_agent::v2::CanonTargetKindV1;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePlanV2 {
    pub schema_version: u16,
    pub pending_id: String,
    pub project_id: String,
    pub session_id: String,
    pub branch_id: String,
    pub turn_id: String,
    pub proposal_id: String,
    pub artifact_id: String,
    pub proposal_hash: String,
    pub source_binding_hash: String,
    pub conflict_set_hash: Option<String>,
    pub policy: AuthorizationPolicyV1,
    pub policy_hash: String,
    pub preview: AgentWritePreviewV1,
    pub plan_hash: String,
}

#[derive(Clone)]
pub struct PreparedWritePlanV2 {
    pub plan: WritePlanV2,
    pub target: NormalizedCanonTargetV1,
    pub serialized: SerializedCanonArtifactV1,
    pub prepared: PreparedAgentWriteV1,
}

impl std::fmt::Debug for PreparedWritePlanV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedWritePlanV2")
            .field("pending_id", &self.plan.pending_id)
            .field("plan_hash", &self.plan.plan_hash)
            .field("relative_path", &self.target.relative_path)
            .finish()
    }
}

pub struct BuildWritePlanInputV2<'a> {
    pub handle: &'a ProjectHandle,
    pub persisted: &'a PersistedCanonProposalV1,
    pub policy: AuthorizationPolicyV1,
    pub conflict_set_hash: Option<&'a str>,
}

pub fn build_write_plan_v2(
    input: BuildWritePlanInputV2<'_>,
) -> Result<PreparedWritePlanV2, AppError> {
    if input.persisted.project_id != input.handle.project_id()
        || input.persisted.proposal_hash.len() != 64
        || input.conflict_set_hash.is_some_and(|hash| !is_hash(hash))
    {
        return Err(plan_error("agent-write-plan.binding-invalid"));
    }
    let target = normalize_canon_target(input.handle, &input.persisted.proposal)?;
    let (serialized, tool_id, arguments) = match target.resolution {
        CanonTargetResolutionV1::Create => {
            let serialized = serialize_canon_artifact(&target, &input.persisted.proposal)?;
            let arguments = ToolArgumentsV1 {
                path: Some(
                    ProjectRelativePathV1::parse(serialized.relative_path.clone())
                        .map_err(|_| plan_error("agent-write-plan.path-invalid"))?,
                ),
                directory: None,
                query: None,
                content: Some(serialized.markdown.clone()),
                patch: None,
                expected_source_hash: None,
            };
            (
                serialized,
                RegisteredToolIdV1::ProjectCreateMarkdown,
                arguments,
            )
        }
        CanonTargetResolutionV1::Extend => {
            let current = read_project_file(input.handle, &target.relative_path)?;
            if Some(current.content_hash.as_str()) != target.existing_source_hash.as_deref() {
                return Err(plan_error("agent-write-plan.source-changed"));
            }
            let (appended, find, replace) = extension_markdown(
                &target.relative_path,
                &current.content,
                &input.persisted.proposal,
            )?;
            let serialized = SerializedCanonArtifactV1 {
                relative_path: target.relative_path.clone(),
                result_hash: crate::workspace::sha256_hex(appended.as_bytes()),
                markdown: appended.clone(),
            };
            let patch = serde_json::json!({
                "schemaVersion": 1,
                "format": crate::agent_write::AGENT_PATCH_FORMAT_V1,
                "operation": "replace_exact",
                "edits": [{
                    "find": find,
                    "replace": replace
                }]
            })
            .to_string();
            let arguments = ToolArgumentsV1 {
                path: Some(
                    ProjectRelativePathV1::parse(target.relative_path.clone())
                        .map_err(|_| plan_error("agent-write-plan.path-invalid"))?,
                ),
                directory: None,
                query: None,
                content: None,
                patch: Some(patch),
                expected_source_hash: Some(
                    ContentHashV1::parse(current.content_hash)
                        .map_err(|_| plan_error("agent-write-plan.source-invalid"))?,
                ),
            };
            (
                serialized,
                RegisteredToolIdV1::ProjectPatchMarkdown,
                arguments,
            )
        }
    };
    let binding = ProjectToolBindingV1 {
        project_id: ProjectIdV1::parse(agent_project_id(input.handle.project_id())?)
            .map_err(|_| plan_error("agent-write-plan.project-invalid"))?,
        session_id: SessionIdV1::parse(input.persisted.session_id.clone())
            .map_err(|_| plan_error("agent-write-plan.session-invalid"))?,
        turn_id: TurnIdV1::parse(input.persisted.turn_id.clone())
            .map_err(|_| plan_error("agent-write-plan.turn-invalid"))?,
    };
    let request = ToolRequestV1 {
        schema_version: AGENT_ACTION_SCHEMA_VERSION,
        binding,
        serial_ordinal: 0,
        call_id: ToolCallIdV1::parse(format!("call_{}", Uuid::new_v4().simple()))
            .map_err(|_| plan_error("agent-write-plan.call-invalid"))?,
        tool_id,
        arguments,
        purpose: "将作者确认的可见 Conversation Anchor 提交为 Canon".to_owned(),
    };
    let pending_id = format!("pnd_{}", Uuid::new_v4().simple());
    let skill_ids = vec![skill_for_target(input.persisted.proposal.target_kind).to_owned()];
    let prepared = prepare_write_batch(
        input.handle,
        pending_id.clone(),
        &[request],
        &skill_ids,
        input.policy,
    )?;
    if prepared.mutations.is_empty() || prepared.mutations.len() > 8 {
        return Err(plan_error("agent-write-plan.transaction-invalid"));
    }
    let source_binding_hash = hash_json(&(
        &input.persisted.anchor_hash,
        &input.persisted.artifact_hash,
        &input.persisted.proposal_hash,
        input.conflict_set_hash,
    ))?;
    let policy_hash = prepared.preview.policy_snapshot_hash.clone();
    let mut plan = WritePlanV2 {
        schema_version: 2,
        pending_id,
        project_id: input.persisted.project_id.clone(),
        session_id: input.persisted.session_id.clone(),
        branch_id: input.persisted.branch_id.clone(),
        turn_id: input.persisted.turn_id.clone(),
        proposal_id: input.persisted.proposal.proposal_id.clone(),
        artifact_id: input.persisted.artifact_id.clone(),
        proposal_hash: input.persisted.proposal_hash.clone(),
        source_binding_hash,
        conflict_set_hash: input.conflict_set_hash.map(str::to_owned),
        policy: input.policy,
        policy_hash,
        preview: prepared.preview.clone(),
        plan_hash: String::new(),
    };
    plan.plan_hash = hash_plan(&plan)?;
    Ok(PreparedWritePlanV2 {
        plan,
        target,
        serialized,
        prepared,
    })
}

pub fn validate_write_confirmation(
    prepared: &PreparedWritePlanV2,
    pending_id: &str,
    plan_hash: &str,
    source_binding_hash: &str,
    policy_hash: &str,
    confirmed: bool,
) -> Result<(), AppError> {
    if !confirmed
        || prepared.plan.pending_id != pending_id
        || prepared.plan.plan_hash != plan_hash
        || prepared.plan.source_binding_hash != source_binding_hash
        || prepared.plan.policy_hash != policy_hash
        || prepared.plan.plan_hash != hash_plan(&prepared.plan)?
        || prepared.prepared.preview.transaction_hash
            != prepared.prepared.plan.transaction_hash.as_str()
    {
        return Err(plan_error("agent-write-plan.confirmation-invalid"));
    }
    Ok(())
}

pub fn policy_hash(
    project_id: &str,
    session_id: &str,
    policy: AuthorizationPolicyV1,
) -> Result<String, AppError> {
    hash_json(&(2u16, project_id, session_id, policy))
}

fn extension_markdown(
    relative_path: &str,
    current: &str,
    proposal: &novelgainker_agent::v2::CanonProposalV1,
) -> Result<(String, String, String), AppError> {
    let facts = proposal
        .facts
        .iter()
        .map(|fact| format!("- **{}**：{}", fact.field.trim(), fact.value.trim()))
        .collect::<Vec<_>>();
    if facts.is_empty() || current.len().saturating_add(facts.join("\n").len()) > 4 * 1024 * 1024 {
        return Err(plan_error("agent-write-plan.extension-invalid"));
    }
    let addition = if current.contains("\n## 已确认事实\n") {
        format!("\n{}\n", facts.join("\n"))
    } else {
        format!("\n## 已确认事实\n\n{}\n", facts.join("\n"))
    };
    let find = unique_trailing_patch_anchor(current)?;
    let replace = format!("{find}{addition}");
    let output = format!("{current}{addition}");
    let report = novelgainker_validation::validate_source_bytes(relative_path, output.as_bytes());
    if !report.diagnostics.is_empty() {
        return Err(plan_error("agent-write-plan.extension-invalid"));
    }
    Ok((output, find, replace))
}

fn unique_trailing_patch_anchor(current: &str) -> Result<String, AppError> {
    if current.is_empty() || !current.ends_with('\n') {
        return Err(plan_error("agent-write-plan.extension-invalid"));
    }
    let mut starts = current
        .match_indices('\n')
        .map(|(index, _)| index + 1)
        .filter(|index| *index < current.len())
        .collect::<Vec<_>>();
    starts.insert(0, 0);
    for start in starts.into_iter().rev() {
        let candidate = &current[start..];
        if !candidate.is_empty() && current.match_indices(candidate).count() == 1 {
            return Ok(candidate.to_owned());
        }
    }
    Err(plan_error("agent-write-plan.extension-invalid"))
}

fn skill_for_target(target: CanonTargetKindV1) -> &'static str {
    match target {
        CanonTargetKindV1::Character => "create-character",
        CanonTargetKindV1::Setting => "create-setting",
        CanonTargetKindV1::Material => "create-material",
        CanonTargetKindV1::Plot => "create-plot",
        CanonTargetKindV1::Chapter => "novel-story-telling",
    }
}

fn hash_plan(plan: &WritePlanV2) -> Result<String, AppError> {
    let mut canonical = plan.clone();
    canonical.plan_hash.clear();
    hash_json(&canonical)
}

fn hash_json<T: Serialize>(value: &T) -> Result<String, AppError> {
    serde_json::to_vec(value)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|_| plan_error("agent-write-plan.hash-invalid"))
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn plan_error(code: &str) -> AppError {
    AppError::agent_contract(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{create_project, inspect_project_root, ProjectScanMode};
    use novelgainker_agent::v2::{
        CanonFactSourceKindV1, CanonFactSourceV1, CanonFactV1, CanonProposalOperationV1,
        CanonProposalV1,
    };
    use std::fs;
    use std::path::PathBuf;

    struct TestProject {
        root: PathBuf,
        handle: ProjectHandle,
    }

    impl TestProject {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("novelgainker-write-plan-v2-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
            let handle = create_project(&inspection).unwrap();
            Self { root, handle }
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            if self.root.parent() == Some(std::env::temp_dir().as_path()) {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    fn persisted(
        project: &TestProject,
        policy_target: CanonTargetKindV1,
    ) -> PersistedCanonProposalV1 {
        let display_name = if policy_target == CanonTargetKindV1::Chapter {
            "第一章 潮门"
        } else {
            "林舟"
        };
        let fact = if policy_target == CanonTargetKindV1::Chapter {
            ("draft", "林舟推开潮门。")
        } else {
            ("身份", "守灯人")
        };
        PersistedCanonProposalV1 {
            schema_version: 1,
            project_id: project.handle.project_id().to_owned(),
            session_id: "ses_12345678".to_owned(),
            branch_id: "brn_12345678".to_owned(),
            turn_id: "trn_12345678".to_owned(),
            anchor_id: "anc_12345678".to_owned(),
            anchor_hash: "a".repeat(64),
            artifact_id: "art_12345678".to_owned(),
            artifact_hash: "b".repeat(64),
            proposal: CanonProposalV1 {
                schema_version: 1,
                request_id: "req_12345678".to_owned(),
                proposal_id: "prop_12345678".to_owned(),
                target_kind: policy_target,
                display_name: display_name.to_owned(),
                operation: CanonProposalOperationV1::Create,
                facts: vec![CanonFactV1 {
                    field: fact.0.to_owned(),
                    value: fact.1.to_owned(),
                    sources: vec![CanonFactSourceV1 {
                        kind: CanonFactSourceKindV1::DraftArtifact,
                        source_id: "art_12345678".to_owned(),
                        source_hash: "c".repeat(64),
                    }],
                }],
            },
            proposal_hash: "d".repeat(64),
            created_at_ms: 1,
        }
    }

    #[test]
    fn builds_hash_bound_create_plan_without_writing() {
        let project = TestProject::new();
        let persisted = persisted(&project, CanonTargetKindV1::Character);
        let prepared = build_write_plan_v2(BuildWritePlanInputV2 {
            handle: &project.handle,
            persisted: &persisted,
            policy: AuthorizationPolicyV1::AutoApplySafe,
            conflict_set_hash: None,
        })
        .unwrap();
        assert_eq!(prepared.plan.schema_version, 2);
        assert_eq!(prepared.plan.preview.files.len(), 1);
        assert!(!project.root.join("characters/林舟.md").exists());
        validate_write_confirmation(
            &prepared,
            &prepared.plan.pending_id,
            &prepared.plan.plan_hash,
            &prepared.plan.source_binding_hash,
            &prepared.plan.policy_hash,
            true,
        )
        .unwrap();
    }

    #[test]
    fn policy_matrix_is_inherited_from_frozen_write_kernel() {
        let project = TestProject::new();
        let persisted = persisted(&project, CanonTargetKindV1::Character);
        let auto = build_write_plan_v2(BuildWritePlanInputV2 {
            handle: &project.handle,
            persisted: &persisted,
            policy: AuthorizationPolicyV1::AutoApplySafe,
            conflict_set_hash: None,
        })
        .unwrap();
        assert!(!auto.plan.preview.requires_confirmation);
        let review = build_write_plan_v2(BuildWritePlanInputV2 {
            handle: &project.handle,
            persisted: &persisted,
            policy: AuthorizationPolicyV1::ReviewEveryWrite,
            conflict_set_hash: None,
        })
        .unwrap();
        assert!(review.plan.preview.requires_confirmation);
        let blocked = build_write_plan_v2(BuildWritePlanInputV2 {
            handle: &project.handle,
            persisted: &persisted,
            policy: AuthorizationPolicyV1::PlanOnly,
            conflict_set_hash: None,
        })
        .unwrap();
        assert!(!blocked.plan.preview.can_confirm);
    }

    #[test]
    fn chapter_plan_is_new_draft_and_composition_free() {
        let project = TestProject::new();
        let persisted = persisted(&project, CanonTargetKindV1::Chapter);
        let prepared = build_write_plan_v2(BuildWritePlanInputV2 {
            handle: &project.handle,
            persisted: &persisted,
            policy: AuthorizationPolicyV1::AutoApplySafe,
            conflict_set_hash: None,
        })
        .unwrap();
        assert!(prepared.serialized.markdown.contains("status: draft"));
        assert!(prepared.serialized.markdown.contains("林舟推开潮门。"));
        assert_eq!(
            prepared.plan.preview.files[0].relative_path,
            "chapters/001.md"
        );
    }
}
