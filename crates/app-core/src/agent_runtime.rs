use crate::project::ProjectHandle;
use crate::search::search_project_sources;
use crate::workspace::{
    list_project_files, read_project_file, validate_project_draft, ProjectFileKind, ProjectFileNode,
};
use crate::AppError;
use novelgainker_agent::action_contracts::{
    ProjectRelativePathV1, RegisteredToolIdV1, ToolErrorCodeV1, ToolRequestV1, ToolResultDataV1,
    ToolResultStatusV1, ToolResultV1, TruncationReasonV1,
};
use novelgainker_agent::contracts::ContentHashV1;
use novelgainker_agent::persistence_contracts::reject_sensitive_text;
use novelgainker_agent::runtime_registry::{is_am1_read_only_tool, path_allowed_for_skills};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const MAX_AGENT_LIST_ITEMS: usize = 200;
pub const MAX_AGENT_READ_UTF8_BYTES: usize = 8 * 1024;
pub const MAX_AGENT_SEARCH_MATCHES: usize = 24;
pub const MAX_AGENT_TOOL_PAYLOAD_UTF8_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentReadToolExecutionV1 {
    pub result: ToolResultV1,
    pub visible_payload: Value,
    pub source_paths: Vec<String>,
    pub project_write_count: u16,
}

pub fn execute_read_tool(
    handle: &ProjectHandle,
    request: &ToolRequestV1,
    skill_ids: &[String],
) -> Result<AgentReadToolExecutionV1, AppError> {
    request
        .validate()
        .map_err(|error| AppError::agent_contract(error.stable_code()))?;
    if !is_am1_read_only_tool(request.tool_id) {
        return Ok(failed_execution(request, ToolErrorCodeV1::PolicyBlocked));
    }
    let skill_refs = skill_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let requested_path = request
        .arguments
        .path
        .as_ref()
        .or(request.arguments.directory.as_ref())
        .map(ProjectRelativePathV1::as_str)
        .ok_or_else(|| AppError::agent_contract("agent-runtime.path-required"))?;
    if !path_allowed_for_skills(&skill_refs, requested_path) {
        return Ok(failed_execution(request, ToolErrorCodeV1::PolicyBlocked));
    }

    match request.tool_id {
        RegisteredToolIdV1::ProjectListMarkdown => execute_list(handle, request, requested_path),
        RegisteredToolIdV1::ProjectReadMarkdown => execute_read(handle, request, requested_path),
        RegisteredToolIdV1::ProjectSearchMarkdown => {
            execute_search(handle, request, requested_path)
        }
        RegisteredToolIdV1::ProjectValidateMarkdown => {
            execute_validate(handle, request, requested_path)
        }
        RegisteredToolIdV1::ProjectCreateMarkdown | RegisteredToolIdV1::ProjectPatchMarkdown => {
            Ok(failed_execution(request, ToolErrorCodeV1::PolicyBlocked))
        }
    }
}

fn execute_list(
    handle: &ProjectHandle,
    request: &ToolRequestV1,
    directory: &str,
) -> Result<AgentReadToolExecutionV1, AppError> {
    let index = list_project_files(handle)?;
    let mut paths = Vec::new();
    collect_markdown_paths(&index.nodes, directory, &mut paths);
    paths.sort();
    let truncated = paths.len() > MAX_AGENT_LIST_ITEMS;
    paths.truncate(MAX_AGENT_LIST_ITEMS);
    let payload = bounded_payload(json!({ "paths": paths }), MAX_AGENT_TOOL_PAYLOAD_UTF8_BYTES);
    success_execution(
        request,
        payload.value,
        paths,
        truncated || payload.truncated,
        if truncated {
            Some(TruncationReasonV1::ItemLimit)
        } else if payload.truncated {
            Some(TruncationReasonV1::ByteLimit)
        } else {
            None
        },
    )
}

fn execute_read(
    handle: &ProjectHandle,
    request: &ToolRequestV1,
    path: &str,
) -> Result<AgentReadToolExecutionV1, AppError> {
    let document = read_project_file(handle, path)?;
    if document.kind != ProjectFileKind::Markdown {
        return Ok(failed_execution(request, ToolErrorCodeV1::PolicyBlocked));
    }
    let redacted = redact_sensitive_lines(&document.content);
    let (content, truncated) = truncate_utf8(&redacted, MAX_AGENT_READ_UTF8_BYTES);
    let payload = json!({
        "relativePath": document.relative_path,
        "content": content,
        "contentHash": document.content_hash,
        "sourceBytes": document.size_bytes,
        "truncated": truncated
    });
    success_execution(
        request,
        payload,
        vec![document.relative_path],
        truncated,
        truncated.then_some(TruncationReasonV1::ByteLimit),
    )
}

fn execute_search(
    handle: &ProjectHandle,
    request: &ToolRequestV1,
    directory: &str,
) -> Result<AgentReadToolExecutionV1, AppError> {
    let query = request
        .arguments
        .query
        .as_deref()
        .ok_or_else(|| AppError::agent_contract("agent-runtime.query-required"))?;
    let report = search_project_sources(handle, query, false)?;
    let prefix = format!("{directory}/");
    let mut matches = report
        .matches
        .into_iter()
        .filter(|item| item.relative_path == directory || item.relative_path.starts_with(&prefix))
        .take(MAX_AGENT_SEARCH_MATCHES + 1)
        .collect::<Vec<_>>();
    let item_truncated = matches.len() > MAX_AGENT_SEARCH_MATCHES;
    matches.truncate(MAX_AGENT_SEARCH_MATCHES);
    for item in &mut matches {
        item.snippet = redact_sensitive_lines(&item.snippet);
    }
    let source_paths = matches
        .iter()
        .map(|item| item.relative_path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let payload = bounded_payload(
        json!({
            "matches": matches,
            "scannedFileCount": report.scanned_file_count,
            "skippedFileCount": report.skipped_file_count,
            "truncated": report.truncated || item_truncated
        }),
        MAX_AGENT_TOOL_PAYLOAD_UTF8_BYTES,
    );
    let truncated = report.truncated || item_truncated || payload.truncated;
    success_execution(
        request,
        payload.value,
        source_paths,
        truncated,
        truncated.then_some(if payload.truncated {
            TruncationReasonV1::ByteLimit
        } else {
            TruncationReasonV1::ItemLimit
        }),
    )
}

fn execute_validate(
    handle: &ProjectHandle,
    request: &ToolRequestV1,
    path: &str,
) -> Result<AgentReadToolExecutionV1, AppError> {
    let document = read_project_file(handle, path)?;
    if document.kind != ProjectFileKind::Markdown {
        return Ok(failed_execution(request, ToolErrorCodeV1::PolicyBlocked));
    }
    let report = validate_project_draft(handle, path, &document.content)?;
    success_execution(
        request,
        json!({
            "relativePath": path,
            "contentHash": document.content_hash,
            "validation": report
        }),
        vec![path.to_owned()],
        false,
        None,
    )
}

fn success_execution(
    request: &ToolRequestV1,
    visible_payload: Value,
    source_paths: Vec<String>,
    truncated: bool,
    truncation_reason: Option<TruncationReasonV1>,
) -> Result<AgentReadToolExecutionV1, AppError> {
    let affected_paths = source_paths
        .iter()
        .map(|path| ProjectRelativePathV1::parse(path.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::agent_contract(error.stable_code()))?;
    let data = ToolResultDataV1 {
        item_count: u16::try_from(source_paths.len()).unwrap_or(u16::MAX),
        content_hash: ContentHashV1::digest(
            &serde_json::to_vec(&visible_payload)
                .expect("tool payload serialization is infallible"),
        ),
        affected_paths,
    };
    let result = ToolResultV1::build(
        request,
        ToolResultStatusV1::Succeeded,
        None,
        Some(data),
        truncated,
        truncation_reason,
    )
    .map_err(|error| AppError::agent_contract(error.stable_code()))?;
    Ok(AgentReadToolExecutionV1 {
        result,
        visible_payload,
        source_paths,
        project_write_count: 0,
    })
}

fn failed_execution(
    request: &ToolRequestV1,
    error_code: ToolErrorCodeV1,
) -> AgentReadToolExecutionV1 {
    AgentReadToolExecutionV1 {
        result: ToolResultV1::build(
            request,
            ToolResultStatusV1::Failed,
            Some(error_code),
            None,
            false,
            None,
        )
        .expect("policy-blocked tool result must satisfy the frozen contract"),
        visible_payload: json!({ "error": "agent-runtime.policy-blocked" }),
        source_paths: Vec::new(),
        project_write_count: 0,
    }
}

fn collect_markdown_paths(nodes: &[ProjectFileNode], directory: &str, output: &mut Vec<String>) {
    let prefix = format!("{directory}/");
    for node in nodes {
        if node.kind == ProjectFileKind::Markdown
            && (node.relative_path == directory || node.relative_path.starts_with(&prefix))
        {
            output.push(node.relative_path.clone());
        }
        collect_markdown_paths(&node.children, directory, output);
    }
}

fn redact_sensitive_lines(value: &str) -> String {
    value
        .lines()
        .map(|line| {
            if reject_sensitive_text(line).is_ok() {
                line
            } else {
                "[敏感内容已脱敏]"
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_utf8(value: &str, maximum_bytes: usize) -> (String, bool) {
    if value.len() <= maximum_bytes {
        return (value.to_owned(), false);
    }
    let mut end = maximum_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    (value[..end].to_owned(), true)
}

struct BoundedPayload {
    value: Value,
    truncated: bool,
}

fn bounded_payload(value: Value, maximum_bytes: usize) -> BoundedPayload {
    if serde_json::to_vec(&value).is_ok_and(|bytes| bytes.len() <= maximum_bytes) {
        BoundedPayload {
            value,
            truncated: false,
        }
    } else {
        BoundedPayload {
            value: json!({ "truncated": true, "reason": "byte-limit" }),
            truncated: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{create_project, inspect_project_root, ProjectScanMode};
    use novelgainker_agent::action_contracts::{
        ProjectToolBindingV1, ToolArgumentsV1, ToolCallIdV1, AGENT_ACTION_SCHEMA_VERSION,
    };
    use novelgainker_agent::contracts::{ProjectIdV1, SessionIdV1, TurnIdV1};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fixture() -> (PathBuf, ProjectHandle) {
        let root = std::env::temp_dir().join(format!("novelgainker-am1-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
        let handle = create_project(&inspection).unwrap();
        fs::write(
            root.join("characters/hero.md"),
            "---\nname: 阿澜\n---\n\n阿澜寻找潮门。\n",
        )
        .unwrap();
        fs::write(
            root.join("world/harbor.md"),
            "---\nname: 潮港\n---\n\n潮港禁止在夜间开门。\n",
        )
        .unwrap();
        (root, handle)
    }

    fn request(tool_id: RegisteredToolIdV1, path: &str, query: Option<&str>) -> ToolRequestV1 {
        ToolRequestV1 {
            schema_version: AGENT_ACTION_SCHEMA_VERSION,
            binding: ProjectToolBindingV1 {
                project_id: ProjectIdV1::parse("prj_fixture0001").unwrap(),
                session_id: SessionIdV1::parse("ses_fixture0001").unwrap(),
                turn_id: TurnIdV1::parse("trn_fixture0001").unwrap(),
            },
            serial_ordinal: 0,
            call_id: ToolCallIdV1::parse("call_fixture0001").unwrap(),
            tool_id,
            arguments: ToolArgumentsV1 {
                path: matches!(
                    tool_id,
                    RegisteredToolIdV1::ProjectReadMarkdown
                        | RegisteredToolIdV1::ProjectValidateMarkdown
                )
                .then(|| ProjectRelativePathV1::parse(path).unwrap()),
                directory: matches!(
                    tool_id,
                    RegisteredToolIdV1::ProjectListMarkdown
                        | RegisteredToolIdV1::ProjectSearchMarkdown
                )
                .then(|| ProjectRelativePathV1::parse(path).unwrap()),
                query: query.map(str::to_owned),
                content: None,
                patch: None,
                expected_source_hash: None,
            },
            purpose: "读取项目事实".to_owned(),
        }
    }

    fn tree_hash(root: &std::path::Path) -> String {
        let mut files = Vec::new();
        fn collect(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect(root, &path, out);
                } else {
                    out.push(path.strip_prefix(root).unwrap().to_path_buf());
                }
            }
        }
        collect(root, root, &mut files);
        files.sort();
        let mut hasher = Sha256::new();
        for relative in files {
            hasher.update(relative.to_string_lossy().as_bytes());
            hasher.update(fs::read(root.join(relative)).unwrap());
        }
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn am1_read_tool_matrix_is_scope_bound_bounded_and_zero_write() {
        let (root, handle) = fixture();
        let before = tree_hash(&root);
        let skills = vec!["create-character".to_owned(), "create-setting".to_owned()];
        for item in [
            request(RegisteredToolIdV1::ProjectListMarkdown, "characters", None),
            request(
                RegisteredToolIdV1::ProjectReadMarkdown,
                "characters/hero.md",
                None,
            ),
            request(
                RegisteredToolIdV1::ProjectSearchMarkdown,
                "world",
                Some("潮港"),
            ),
            request(
                RegisteredToolIdV1::ProjectValidateMarkdown,
                "characters/hero.md",
                None,
            ),
        ] {
            let execution = execute_read_tool(&handle, &item, &skills).unwrap();
            assert_eq!(execution.result.status, ToolResultStatusV1::Succeeded);
            assert_eq!(execution.project_write_count, 0);
        }
        assert_eq!(before, tree_hash(&root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_tools_and_cross_skill_paths_are_policy_blocked() {
        let (root, handle) = fixture();
        let write = ToolRequestV1 {
            arguments: ToolArgumentsV1 {
                path: Some(ProjectRelativePathV1::parse("characters/new.md").unwrap()),
                directory: None,
                query: None,
                content: Some("forbidden".to_owned()),
                patch: None,
                expected_source_hash: None,
            },
            ..request(
                RegisteredToolIdV1::ProjectReadMarkdown,
                "characters/hero.md",
                None,
            )
        };
        let write = ToolRequestV1 {
            tool_id: RegisteredToolIdV1::ProjectCreateMarkdown,
            ..write
        };
        let blocked = execute_read_tool(&handle, &write, &["create-character".to_owned()]).unwrap();
        assert_eq!(
            blocked.result.error_code,
            Some(ToolErrorCodeV1::PolicyBlocked)
        );

        let chapter = request(
            RegisteredToolIdV1::ProjectReadMarkdown,
            "chapters/001.md",
            None,
        );
        let blocked =
            execute_read_tool(&handle, &chapter, &["create-character".to_owned()]).unwrap();
        assert_eq!(
            blocked.result.error_code,
            Some(ToolErrorCodeV1::PolicyBlocked)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sensitive_lines_are_redacted_before_provider_visibility() {
        assert_eq!(
            redact_sensitive_lines("安全行\nAuthorization: Bearer synthetic\n继续"),
            "安全行\n[敏感内容已脱敏]\n继续"
        );
    }
}
