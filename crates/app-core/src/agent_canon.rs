use crate::agent_anchor::{
    AnchorIntentV1, AnchorSourceBindingV1, AnchorStateV1, AnchorTargetKindV1,
};
use crate::agent_v2::{AgentSessionSnapshotV2, ConversationStatusV2};
use crate::project::ProjectHandle;
use crate::workspace::{read_project_file, sha256_hex};
use crate::AppError;
use novelgainker_agent::v2::{
    CanonFactSourceKindV1, CanonFactSourceV1, CanonFactV1, CanonProposalOperationV1,
    CanonProposalV1, CanonTargetKindV1,
};
use novelgainker_validation::validate_source_bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const AGENT_ROOT: &str = ".novelgainker/agent";
const DRAFT_DIRECTORY: &str = "drafts";
const PROPOSAL_DIRECTORY: &str = "proposals";
const MAX_DRAFT_ARTIFACT_BYTES: usize = 64 * 1024;
const MAX_DRAFT_ARTIFACTS: usize = 64;
const MAX_AGENT_ARTIFACT_STORE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftArtifactStateV1 {
    Complete,
    Discarded,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftArtifactV1 {
    pub schema_version: u16,
    pub artifact_id: String,
    pub project_id: String,
    pub session_id: String,
    pub branch_id: String,
    pub turn_id: String,
    pub anchor_id: String,
    pub target_kind: AnchorTargetKindV1,
    pub source_bindings: Vec<AnchorSourceBindingV1>,
    pub content: String,
    pub content_hash: String,
    pub state: DraftArtifactStateV1,
    pub validated: bool,
    pub created_at_ms: u64,
    pub artifact_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedCanonProposalV1 {
    pub schema_version: u16,
    pub project_id: String,
    pub session_id: String,
    pub branch_id: String,
    pub turn_id: String,
    pub anchor_id: String,
    pub anchor_hash: String,
    pub artifact_id: String,
    pub artifact_hash: String,
    pub proposal: CanonProposalV1,
    pub proposal_hash: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonTargetResolutionV1 {
    Create,
    Extend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedCanonTargetV1 {
    pub entity_id: String,
    pub display_name: String,
    pub relative_path: String,
    pub resolution: CanonTargetResolutionV1,
    pub existing_source_hash: Option<String>,
    pub chapter_number: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedCanonArtifactV1 {
    pub relative_path: String,
    pub markdown: String,
    pub result_hash: String,
}

pub struct MaterializeDraftInputV1<'a> {
    pub snapshot: &'a AgentSessionSnapshotV2,
    pub anchor: &'a AnchorIntentV1,
    pub current_user_message_id: &'a str,
    pub current_user_text: &'a str,
    pub now_ms: u64,
}

pub fn materialize_draft_artifact(
    input: MaterializeDraftInputV1<'_>,
) -> Result<DraftArtifactV1, AppError> {
    if input.anchor.state != AnchorStateV1::Confirmed
        || input.anchor.project_id != input.snapshot.project_id
        || input.anchor.session_id != input.snapshot.session_id.as_deref().unwrap_or_default()
        || input.anchor.branch_id
            != input
                .snapshot
                .active_branch_id
                .as_deref()
                .unwrap_or_default()
        || input.anchor.source_bindings.is_empty()
        || input.anchor.source_bindings.len() > 8
    {
        return Err(canon_error("agent-canon.anchor-invalid"));
    }
    let mut parts = Vec::new();
    for binding in &input.anchor.source_bindings {
        let body = if binding.source_message_id == input.current_user_message_id {
            if sha256_hex(input.current_user_text.as_bytes()) != binding.message_body_hash {
                return Err(canon_error("agent-canon.source-changed"));
            }
            input.current_user_text
        } else {
            let message = input
                .snapshot
                .messages
                .iter()
                .find(|message| message.message_id == binding.source_message_id)
                .ok_or_else(|| canon_error("agent-canon.source-missing"))?;
            if message.status != ConversationStatusV2::Complete
                || message.body_hash != binding.message_body_hash
                || sha256_hex(message.body.as_bytes()) != binding.message_body_hash
            {
                return Err(canon_error("agent-canon.source-changed"));
            }
            message.body.as_str()
        };
        parts.push(bound_content(body, binding)?);
    }
    let content = parts.join("\n\n");
    if content.trim().is_empty() || content.len() > MAX_DRAFT_ARTIFACT_BYTES {
        return Err(canon_error("agent-canon.artifact-invalid"));
    }
    let mut artifact = DraftArtifactV1 {
        schema_version: 1,
        artifact_id: format!("art_{}", Uuid::new_v4().simple()),
        project_id: input.snapshot.project_id.clone(),
        session_id: input.anchor.session_id.clone(),
        branch_id: input.anchor.branch_id.clone(),
        turn_id: input.anchor.turn_id.clone(),
        anchor_id: input.anchor.anchor_id.clone(),
        target_kind: input.anchor.target_kind,
        source_bindings: input.anchor.source_bindings.clone(),
        content_hash: sha256_hex(content.as_bytes()),
        content,
        state: DraftArtifactStateV1::Complete,
        validated: true,
        created_at_ms: input.now_ms,
        artifact_hash: String::new(),
    };
    artifact.artifact_hash = hash_without_field(&artifact, "artifactHash")?;
    Ok(artifact)
}

pub fn persist_draft_artifact(
    handle: &ProjectHandle,
    artifact: &DraftArtifactV1,
) -> Result<(), AppError> {
    if artifact.project_id != handle.project_id()
        || artifact.state != DraftArtifactStateV1::Complete
        || !artifact.validated
        || artifact.content_hash != sha256_hex(artifact.content.as_bytes())
        || artifact.artifact_hash != hash_without_field(artifact, "artifactHash")?
    {
        return Err(canon_error("agent-canon.artifact-invalid"));
    }
    persist_agent_json(handle, DRAFT_DIRECTORY, &artifact.artifact_id, artifact)
}

pub fn bind_canon_proposal(
    handle: &ProjectHandle,
    snapshot: &AgentSessionSnapshotV2,
    anchor: &AnchorIntentV1,
    artifact: &DraftArtifactV1,
    proposal: CanonProposalV1,
    allowed_canon_sources: &[(String, String)],
    now_ms: u64,
) -> Result<PersistedCanonProposalV1, AppError> {
    if artifact.project_id != handle.project_id()
        || artifact.anchor_id != anchor.anchor_id
        || artifact.state != DraftArtifactStateV1::Complete
        || !artifact.validated
        || anchor.project_id != snapshot.project_id
        || anchor.branch_id != snapshot.active_branch_id.as_deref().unwrap_or_default()
        || target_kind(anchor.target_kind) != proposal.target_kind
    {
        return Err(canon_error("agent-canon.proposal-binding-invalid"));
    }
    let mut allowed = allowed_canon_sources.to_vec();
    allowed.push((artifact.artifact_id.clone(), artifact.content_hash.clone()));
    for binding in &artifact.source_bindings {
        allowed.push((
            binding.source_message_id.clone(),
            binding.message_body_hash.clone(),
        ));
    }
    proposal
        .validate(&proposal.request_id, &allowed)
        .map_err(|_| canon_error("agent-canon.proposal-invalid"))?;
    let mut persisted = PersistedCanonProposalV1 {
        schema_version: 1,
        project_id: handle.project_id().to_owned(),
        session_id: anchor.session_id.clone(),
        branch_id: anchor.branch_id.clone(),
        turn_id: anchor.turn_id.clone(),
        anchor_id: anchor.anchor_id.clone(),
        anchor_hash: anchor.anchor_hash.clone(),
        artifact_id: artifact.artifact_id.clone(),
        artifact_hash: artifact.artifact_hash.clone(),
        proposal,
        proposal_hash: String::new(),
        created_at_ms: now_ms,
    };
    persisted.proposal_hash = hash_without_field(&persisted, "proposalHash")?;
    Ok(persisted)
}

pub fn persist_canon_proposal(
    handle: &ProjectHandle,
    proposal: &PersistedCanonProposalV1,
) -> Result<(), AppError> {
    if proposal.project_id != handle.project_id()
        || proposal.proposal_hash != hash_without_field(proposal, "proposalHash")?
    {
        return Err(canon_error("agent-canon.proposal-invalid"));
    }
    persist_agent_json(
        handle,
        PROPOSAL_DIRECTORY,
        &proposal.proposal.proposal_id,
        proposal,
    )
}

pub fn chapter_proposal_from_artifact(
    request_id: String,
    display_name: String,
    artifact: &DraftArtifactV1,
) -> CanonProposalV1 {
    CanonProposalV1 {
        schema_version: 1,
        request_id,
        proposal_id: format!("prop_{}", Uuid::new_v4().simple()),
        target_kind: CanonTargetKindV1::Chapter,
        display_name,
        operation: CanonProposalOperationV1::Create,
        facts: vec![CanonFactV1 {
            field: "draft".to_owned(),
            value: artifact.content.clone(),
            sources: vec![CanonFactSourceV1 {
                kind: CanonFactSourceKindV1::DraftArtifact,
                source_id: artifact.artifact_id.clone(),
                source_hash: artifact.content_hash.clone(),
            }],
        }],
    }
}

pub fn normalize_canon_target(
    handle: &ProjectHandle,
    proposal: &CanonProposalV1,
) -> Result<NormalizedCanonTargetV1, AppError> {
    validate_display_name(&proposal.display_name)?;
    if proposal.target_kind == CanonTargetKindV1::Chapter {
        return normalize_chapter_target(handle, &proposal.display_name);
    }
    let (directory, default_path, id_prefix) = match proposal.target_kind {
        CanonTargetKindV1::Character => (
            "characters",
            format!("characters/{}.md", proposal.display_name),
            "character",
        ),
        CanonTargetKindV1::Setting => (
            "world",
            format!("world/{}.md", proposal.display_name),
            "setting",
        ),
        CanonTargetKindV1::Material => (
            "materials",
            format!("materials/{}.md", proposal.display_name),
            "material",
        ),
        CanonTargetKindV1::Plot => ("plot", "plot/main.md".to_owned(), "plot"),
        CanonTargetKindV1::Chapter => unreachable!(),
    };
    let mut matches = Vec::new();
    for path in markdown_files(&handle.root().join(directory))? {
        let relative = format!(
            "{directory}/{}",
            path.file_name().unwrap().to_string_lossy()
        );
        let document = read_project_file(handle, &relative)?;
        if frontmatter_title(&document.content).is_some_and(|title| title == proposal.display_name)
        {
            matches.push((relative, document.content_hash));
        }
    }
    if matches.len() > 1 {
        return Err(canon_error("agent-canon.entity-ambiguous"));
    }
    if let Some((relative_path, content_hash)) = matches.pop() {
        return Ok(NormalizedCanonTargetV1 {
            entity_id: frontmatter_id(&read_project_file(handle, &relative_path)?.content)
                .unwrap_or_else(|| format!("{id_prefix}_{}", Uuid::new_v4())),
            display_name: proposal.display_name.clone(),
            relative_path,
            resolution: CanonTargetResolutionV1::Extend,
            existing_source_hash: Some(content_hash),
            chapter_number: None,
        });
    }
    if handle.root().join(&default_path).exists() {
        return Err(canon_error("agent-canon.path-collision"));
    }
    Ok(NormalizedCanonTargetV1 {
        entity_id: format!("{id_prefix}_{}", Uuid::new_v4()),
        display_name: proposal.display_name.clone(),
        relative_path: default_path,
        resolution: CanonTargetResolutionV1::Create,
        existing_source_hash: None,
        chapter_number: None,
    })
}

pub fn serialize_canon_artifact(
    target: &NormalizedCanonTargetV1,
    proposal: &CanonProposalV1,
) -> Result<SerializedCanonArtifactV1, AppError> {
    if target.resolution != CanonTargetResolutionV1::Create {
        return Err(canon_error("agent-canon.extend-requires-patch"));
    }
    let title = yaml_string(&target.display_name);
    let facts = proposal
        .facts
        .iter()
        .map(|fact| format!("- **{}**：{}", fact.field.trim(), fact.value.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    let markdown = match proposal.target_kind {
        CanonTargetKindV1::Character => format!(
            "---\nid: {}\ntitle: {title}\nwritingLanguage: zh-CN\nkind: character\nschemaVersion: 2\n---\n\n# {}\n\n## 已确认事实\n\n{facts}\n",
            target.entity_id, target.display_name
        ),
        CanonTargetKindV1::Setting => format!(
            "---\nid: {}\ntitle: {title}\nwritingLanguage: zh-CN\nkind: setting\nschemaVersion: 1\nsettingType: world\n---\n\n# {}\n\n## 已确认事实\n\n{facts}\n",
            target.entity_id, target.display_name
        ),
        CanonTargetKindV1::Material => format!(
            "---\nid: {}\ntitle: {title}\nkind: material\nschemaVersion: 1\nwritingLanguage: zh-CN\nmaterialType: note\n---\n\n# {}\n\n## 已确认事实\n\n{facts}\n",
            target.entity_id, target.display_name
        ),
        CanonTargetKindV1::Plot => format!(
            "---\nid: {}\ntitle: {title}\nkind: plot\nschemaVersion: 1\nwritingLanguage: zh-CN\n---\n\n# {}\n\n## 已确认事实\n\n{facts}\n",
            target.entity_id, target.display_name
        ),
        CanonTargetKindV1::Chapter => {
            let chapter_number = target
                .chapter_number
                .ok_or_else(|| canon_error("agent-canon.chapter-target-invalid"))?;
            let draft = proposal
                .facts
                .iter()
                .find(|fact| fact.field == "draft")
                .map(|fact| fact.value.trim())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| canon_error("agent-canon.chapter-draft-invalid"))?;
            format!(
                "---\nid: {}\ntitle: {title}\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: {chapter_number}\nstatus: draft\n---\n\n# {}\n\n## Draft\n\n{draft}\n",
                target.entity_id, target.display_name
            )
        }
    };
    let report = validate_source_bytes(&target.relative_path, markdown.as_bytes());
    if !report.diagnostics.is_empty() {
        return Err(canon_error("agent-canon.serializer-invalid"));
    }
    Ok(SerializedCanonArtifactV1 {
        relative_path: target.relative_path.clone(),
        result_hash: sha256_hex(markdown.as_bytes()),
        markdown,
    })
}

fn normalize_chapter_target(
    handle: &ProjectHandle,
    display_name: &str,
) -> Result<NormalizedCanonTargetV1, AppError> {
    let mut highest = 0u32;
    let generic_title = matches!(display_name.trim(), "新章节" | "章节" | "下一章");
    for path in markdown_files(&handle.root().join("chapters"))? {
        let relative = format!("chapters/{}", path.file_name().unwrap().to_string_lossy());
        let document = read_project_file(handle, &relative)?;
        if !generic_title
            && frontmatter_title(&document.content).is_some_and(|title| title == display_name)
        {
            return Err(canon_error("agent-canon.chapter-collision"));
        }
        highest = highest.max(frontmatter_u32(&document.content, "chapterNumber").unwrap_or(0));
    }
    let chapter_number = highest.saturating_add(1);
    if chapter_number == 0 || chapter_number > 999_999 {
        return Err(canon_error("agent-canon.chapter-limit"));
    }
    let relative_path = format!("chapters/{chapter_number:03}.md");
    if handle.root().join(&relative_path).exists() {
        return Err(canon_error("agent-canon.chapter-collision"));
    }
    let display_name = if generic_title {
        format!("第{chapter_number}章")
    } else {
        display_name.to_owned()
    };
    Ok(NormalizedCanonTargetV1 {
        entity_id: format!("chapter_{}", Uuid::new_v4()),
        display_name,
        relative_path,
        resolution: CanonTargetResolutionV1::Create,
        existing_source_hash: None,
        chapter_number: Some(chapter_number),
    })
}

pub fn discard_persisted_candidate(
    handle: &ProjectHandle,
    artifact_id: &str,
    proposal_id: &str,
) -> Result<(), AppError> {
    validate_component(artifact_id)?;
    validate_component(proposal_id)?;
    for (directory, id) in [
        (DRAFT_DIRECTORY, artifact_id),
        (PROPOSAL_DIRECTORY, proposal_id),
    ] {
        let path = handle
            .root()
            .join(AGENT_ROOT)
            .join(directory)
            .join(format!("{id}.json"));
        if !path.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| canon_error("agent-canon.store-unavailable"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(canon_error("agent-canon.store-unavailable"));
        }
        fs::remove_file(path).map_err(|_| canon_error("agent-canon.store-unavailable"))?;
    }
    Ok(())
}

fn bound_content<'a>(body: &'a str, binding: &AnchorSourceBindingV1) -> Result<&'a str, AppError> {
    match (
        binding.start_utf8_byte,
        binding.end_utf8_byte,
        binding.selection_hash.as_ref(),
    ) {
        (None, None, None) => Ok(body),
        (Some(start), Some(end), Some(hash)) => {
            let start = start as usize;
            let end = end as usize;
            if start >= end
                || end > body.len()
                || !body.is_char_boundary(start)
                || !body.is_char_boundary(end)
                || sha256_hex(&body.as_bytes()[start..end]) != *hash
            {
                return Err(canon_error("agent-canon.selection-invalid"));
            }
            Ok(&body[start..end])
        }
        _ => Err(canon_error("agent-canon.selection-invalid")),
    }
}

fn persist_agent_json<T: Serialize>(
    handle: &ProjectHandle,
    directory: &str,
    id: &str,
    value: &T,
) -> Result<(), AppError> {
    validate_component(id)?;
    let root = handle.root().join(AGENT_ROOT);
    let target_directory = root.join(directory);
    fs::create_dir_all(&target_directory)
        .map_err(|_| canon_error("agent-canon.store-unavailable"))?;
    for path in [&root, &target_directory] {
        if fs::symlink_metadata(path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(true)
        {
            return Err(canon_error("agent-canon.store-unavailable"));
        }
    }
    let files = fs::read_dir(&target_directory)
        .map_err(|_| canon_error("agent-canon.store-unavailable"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    if files.len() >= MAX_DRAFT_ARTIFACTS {
        return Err(canon_error("agent-canon.quota-exceeded"));
    }
    let existing_bytes = store_bytes(&root)?;
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|_| canon_error("agent-canon.store-invalid"))?;
    if existing_bytes.saturating_add(bytes.len() as u64) > MAX_AGENT_ARTIFACT_STORE_BYTES {
        return Err(canon_error("agent-canon.quota-exceeded"));
    }
    let target = target_directory.join(format!("{id}.json"));
    if target.exists() {
        return Err(canon_error("agent-canon.id-reused"));
    }
    let temporary = target_directory.join(format!(".{id}.{}.tmp", Uuid::new_v4().simple()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| canon_error("agent-canon.store-unavailable"))?;
    let result = file
        .write_all(&bytes)
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&temporary, &target));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(canon_error("agent-canon.store-unavailable"));
    }
    Ok(())
}

fn markdown_files(directory: &Path) -> Result<Vec<PathBuf>, AppError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    if fs::symlink_metadata(directory)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(true)
    {
        return Err(canon_error("agent-canon.target-unavailable"));
    }
    let mut files = fs::read_dir(directory)
        .map_err(|_| canon_error("agent-canon.target-unavailable"))?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && entry.path().extension().and_then(|value| value.to_str()) == Some("md")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn store_bytes(root: &Path) -> Result<u64, AppError> {
    let mut total = 0u64;
    for directory in [DRAFT_DIRECTORY, PROPOSAL_DIRECTORY] {
        let path = root.join(directory);
        if !path.exists() {
            continue;
        }
        for entry in fs::read_dir(path)
            .map_err(|_| canon_error("agent-canon.store-unavailable"))?
            .filter_map(Result::ok)
        {
            total = total.saturating_add(entry.metadata().map(|value| value.len()).unwrap_or(0));
        }
    }
    Ok(total)
}

fn hash_without_field<T: Serialize>(value: &T, field: &str) -> Result<String, AppError> {
    let mut value =
        serde_json::to_value(value).map_err(|_| canon_error("agent-canon.hash-invalid"))?;
    value
        .as_object_mut()
        .ok_or_else(|| canon_error("agent-canon.hash-invalid"))?
        .insert(field.to_owned(), serde_json::Value::String(String::new()));
    let bytes = serde_json::to_vec(&value).map_err(|_| canon_error("agent-canon.hash-invalid"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn target_kind(kind: AnchorTargetKindV1) -> CanonTargetKindV1 {
    match kind {
        AnchorTargetKindV1::Character => CanonTargetKindV1::Character,
        AnchorTargetKindV1::Setting => CanonTargetKindV1::Setting,
        AnchorTargetKindV1::Material => CanonTargetKindV1::Material,
        AnchorTargetKindV1::Plot => CanonTargetKindV1::Plot,
        AnchorTargetKindV1::Chapter => CanonTargetKindV1::Chapter,
    }
}

fn validate_display_name(value: &str) -> Result<(), AppError> {
    if value.trim().is_empty()
        || value.chars().count() > 120
        || value.starts_with('.')
        || value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
        })
    {
        Err(canon_error("agent-canon.display-name-invalid"))
    } else {
        Ok(())
    }
}

fn validate_component(value: &str) -> Result<(), AppError> {
    if value.len() >= 12
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Ok(())
    } else {
        Err(canon_error("agent-canon.id-invalid"))
    }
}

fn frontmatter_value<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    let mut lines = content.lines();
    if lines.next()? != "---" {
        return None;
    }
    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix(&format!("{key}:")) {
            return Some(value.trim().trim_matches('"'));
        }
    }
    None
}

fn frontmatter_title(content: &str) -> Option<&str> {
    frontmatter_value(content, "title")
}

fn frontmatter_id(content: &str) -> Option<String> {
    frontmatter_value(content, "id").map(str::to_owned)
}

fn frontmatter_u32(content: &str, key: &str) -> Option<u32> {
    frontmatter_value(content, key)?.parse().ok()
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).expect("plain display name serializes")
}

fn canon_error(code: &str) -> AppError {
    AppError::agent_contract(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_anchor::{resolve_anchor_intent, AnchorResolutionV1, ResolveAnchorInputV1};
    use crate::agent_v2::{
        load_agent_session, AgentHistoryBranchV2, AgentHistoryHashStatusV2, ConversationMessageV2,
    };
    use crate::project::{create_project, inspect_project_root, ProjectScanMode};

    struct TestProject {
        root: PathBuf,
        handle: ProjectHandle,
    }

    impl TestProject {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("novelgainker-agent-canon-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
            let handle = create_project(&inspection).unwrap();
            Self { root, handle }
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            if self.root.parent() == Some(std::env::temp_dir().as_path())
                && self
                    .root
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.starts_with("novelgainker-agent-canon-"))
            {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    fn snapshot() -> AgentSessionSnapshotV2 {
        AgentSessionSnapshotV2 {
            schema_version: 2,
            project_id: "placeholder".to_owned(),
            history_consent_required: false,
            branch_selection_required: false,
            active_branch_id: Some("brn_12345678".to_owned()),
            session_id: Some("ses_12345678".to_owned()),
            branches: vec![AgentHistoryBranchV2 {
                branch_id: "brn_12345678".to_owned(),
                session_id: "ses_12345678".to_owned(),
                last_turn_at_ms: None,
                message_count: 0,
                hash_status: AgentHistoryHashStatusV2::Empty,
            }],
            messages: Vec::<ConversationMessageV2>::new(),
            memory: None,
            draft_quota: 0,
            write_policy: novelgainker_agent::contracts::AuthorizationPolicyV1::AutoApplySafe,
            write_policy_hash: Some("a".repeat(64)),
            context_marks: crate::agent_marks::ContextMarkStoreV1 {
                schema_version: 1,
                project_id: "project-test".to_owned(),
                marks: Vec::new(),
                store_hash: "a".repeat(64),
            },
        }
    }

    #[test]
    fn materializes_only_hash_bound_visible_content() {
        let mut snapshot = snapshot();
        let text = "主角就叫林舟";
        let AnchorResolutionV1::Ready { anchor } = resolve_anchor_intent(ResolveAnchorInputV1 {
            snapshot: &snapshot,
            turn_id: "trn_87654321",
            current_user_message_id: "msg_87654321",
            user_text: text,
            reference: None,
            now_ms: 2,
        })
        .unwrap() else {
            panic!("anchor expected");
        };
        snapshot.project_id = anchor.project_id.clone();
        let artifact = materialize_draft_artifact(MaterializeDraftInputV1 {
            snapshot: &snapshot,
            anchor: &anchor,
            current_user_message_id: "msg_87654321",
            current_user_text: text,
            now_ms: 3,
        })
        .unwrap();
        assert_eq!(artifact.content, text);
        assert!(artifact.validated);
        assert_eq!(artifact.artifact_hash.len(), 64);
    }

    #[test]
    fn normalizes_and_serializes_host_owned_character_target() {
        let project = TestProject::new();
        let proposal = CanonProposalV1 {
            schema_version: 1,
            request_id: "req_12345678".to_owned(),
            proposal_id: "prop_12345678".to_owned(),
            target_kind: CanonTargetKindV1::Character,
            display_name: "林舟".to_owned(),
            operation: CanonProposalOperationV1::Create,
            facts: vec![CanonFactV1 {
                field: "姓名".to_owned(),
                value: "林舟".to_owned(),
                sources: vec![CanonFactSourceV1 {
                    kind: CanonFactSourceKindV1::VisibleMessage,
                    source_id: "msg_12345678".to_owned(),
                    source_hash: "a".repeat(64),
                }],
            }],
        };
        let target = normalize_canon_target(&project.handle, &proposal).unwrap();
        assert_eq!(target.relative_path, "characters/林舟.md");
        let artifact = serialize_canon_artifact(&target, &proposal).unwrap();
        assert!(artifact.markdown.contains("kind: character"));
        assert!(artifact.markdown.contains("## 已确认事实"));
        assert!(
            validate_source_bytes(&artifact.relative_path, artifact.markdown.as_bytes())
                .diagnostics
                .is_empty()
        );
    }

    #[test]
    fn chapter_serializer_keeps_visible_artifact_exact_and_draft_only() {
        let project = TestProject::new();
        let artifact = DraftArtifactV1 {
            schema_version: 1,
            artifact_id: "art_12345678".to_owned(),
            project_id: project.handle.project_id().to_owned(),
            session_id: "ses_12345678".to_owned(),
            branch_id: "brn_12345678".to_owned(),
            turn_id: "trn_12345678".to_owned(),
            anchor_id: "anc_12345678".to_owned(),
            target_kind: AnchorTargetKindV1::Chapter,
            source_bindings: Vec::new(),
            content: "林舟推开门。".to_owned(),
            content_hash: sha256_hex("林舟推开门。".as_bytes()),
            state: DraftArtifactStateV1::Complete,
            validated: true,
            created_at_ms: 1,
            artifact_hash: String::new(),
        };
        let proposal = chapter_proposal_from_artifact(
            "req_12345678".to_owned(),
            "第一章 潮门".to_owned(),
            &artifact,
        );
        let target = normalize_canon_target(&project.handle, &proposal).unwrap();
        let serialized = serialize_canon_artifact(&target, &proposal).unwrap();
        assert!(serialized.markdown.contains("status: draft"));
        assert!(serialized.markdown.contains("\n林舟推开门。\n"));
        assert!(!serialized.markdown.contains("status: accepted"));
    }

    #[test]
    fn agent_metadata_is_persisted_outside_canon_and_survives_reload() {
        let project = TestProject::new();
        let mut snapshot = load_agent_session(&project.handle, true, 1).unwrap();
        let text = "主角就叫林舟";
        let AnchorResolutionV1::Ready { anchor } = resolve_anchor_intent(ResolveAnchorInputV1 {
            snapshot: &snapshot,
            turn_id: "trn_87654321",
            current_user_message_id: "msg_87654321",
            user_text: text,
            reference: None,
            now_ms: 2,
        })
        .unwrap() else {
            panic!("anchor expected");
        };
        snapshot.project_id = project.handle.project_id().to_owned();
        let mut artifact = materialize_draft_artifact(MaterializeDraftInputV1 {
            snapshot: &snapshot,
            anchor: &AnchorIntentV1 {
                project_id: project.handle.project_id().to_owned(),
                ..*anchor
            },
            current_user_message_id: "msg_87654321",
            current_user_text: text,
            now_ms: 3,
        })
        .unwrap();
        artifact.project_id = project.handle.project_id().to_owned();
        artifact.artifact_hash = hash_without_field(&artifact, "artifactHash").unwrap();
        persist_draft_artifact(&project.handle, &artifact).unwrap();
        assert!(project
            .root
            .join(".novelgainker/agent/drafts")
            .join(format!("{}.json", artifact.artifact_id))
            .is_file());
        assert!(!project.root.join("characters/林舟.md").exists());
    }

    #[test]
    fn consecutive_generic_chapter_targets_receive_unique_host_titles() {
        let project = TestProject::new();
        let proposal = CanonProposalV1 {
            schema_version: 1,
            request_id: "req_12345678".to_owned(),
            proposal_id: "prop_12345678".to_owned(),
            target_kind: CanonTargetKindV1::Chapter,
            display_name: "新章节".to_owned(),
            operation: CanonProposalOperationV1::Create,
            facts: vec![CanonFactV1 {
                field: "正文".to_owned(),
                value: "林舟推开门。".to_owned(),
                sources: vec![CanonFactSourceV1 {
                    kind: CanonFactSourceKindV1::VisibleMessage,
                    source_id: "msg_12345678".to_owned(),
                    source_hash: "a".repeat(64),
                }],
            }],
        };
        let first = normalize_canon_target(&project.handle, &proposal).unwrap();
        assert_eq!(first.display_name, "第1章");
        fs::write(
            project.root.join("chapters/001.md"),
            "---\nid: chapter-1\ntitle: 第1章\nchapterNumber: 1\nstatus: draft\n---\n# 第1章\n\n正文\n",
        )
        .unwrap();
        let second = normalize_canon_target(&project.handle, &proposal).unwrap();
        assert_eq!(second.relative_path, "chapters/002.md");
        assert_eq!(second.display_name, "第2章");
    }

    #[test]
    fn discarding_a_candidate_removes_only_agent_metadata() {
        let project = TestProject::new();
        let artifact_id = "art_12345678";
        let proposal_id = "prop_12345678";
        let drafts = project.root.join(AGENT_ROOT).join(DRAFT_DIRECTORY);
        let proposals = project.root.join(AGENT_ROOT).join(PROPOSAL_DIRECTORY);
        fs::create_dir_all(&drafts).unwrap();
        fs::create_dir_all(&proposals).unwrap();
        fs::write(drafts.join(format!("{artifact_id}.json")), b"{}").unwrap();
        fs::write(proposals.join(format!("{proposal_id}.json")), b"{}").unwrap();
        fs::write(project.root.join("project.md"), "# Canon remains\n").unwrap();
        discard_persisted_candidate(&project.handle, artifact_id, proposal_id).unwrap();
        assert!(!drafts.join(format!("{artifact_id}.json")).exists());
        assert!(!proposals.join(format!("{proposal_id}.json")).exists());
        assert!(project.root.join("project.md").is_file());
    }
}
