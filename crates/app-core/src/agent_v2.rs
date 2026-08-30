use crate::agent_marks::{load_context_marks, ContextMarkStoreV1};
use crate::project::ProjectHandle;
use crate::AppError;
use novelgainker_agent::contracts::AuthorizationPolicyV1;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const AGENT_SCHEMA_VERSION: u16 = 2;
const AGENT_ROOT: &str = ".novelgainker/agent";
const MANIFEST_FILE: &str = "manifest.json";
const MAX_HISTORY_LINE_BYTES: usize = 8 * 1024 * 1024;
const MAX_MESSAGE_BODY_CHARACTERS: usize = 32_768;
const MAX_SOURCE_REFS_PER_TURN: usize = 48;
const MAX_SOURCE_EXCERPT_CHARACTERS: usize = 16_384;
const MAX_CONTEXT_FILES: usize = 5_000;
const MAX_CONTEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MEMORY_COMPACT_MESSAGE_THRESHOLD: u64 = 48;
const MEMORY_COMPACT_CHARACTER_THRESHOLD: u64 = 96_000;
// A handful of shared bi/tri-grams is not sufficient evidence for a Canon source. Exact
// phrase and entity matches score at least 48; the floor prevents unrelated long documents
// from surfacing merely because common fragments overlap.
const MIN_CONTEXT_MATCH_SCORE: u32 = 48;
const CANON_DIRECTORIES: &[&str] = &[
    "characters",
    "materials",
    "macguffins",
    "plot",
    "outlines",
    "world",
    "style",
    "chapters",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHistoryHashStatusV2 {
    Valid,
    Empty,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationMessageV2 {
    pub schema_version: u16,
    pub message_id: String,
    pub response_id: Option<String>,
    pub turn_id: String,
    pub role: ConversationRoleV2,
    pub body: String,
    pub body_hash: String,
    pub source_ids: Vec<String>,
    pub status: ConversationStatusV2,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversationRoleV2 {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversationStatusV2 {
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRefV2 {
    pub source_id: String,
    pub relative_path: String,
    pub entity: Option<String>,
    pub field_path: Option<String>,
    pub heading_path: Vec<String>,
    pub content_hash: String,
    pub excerpt_hash: String,
    pub excerpt: String,
    pub truncated: bool,
    pub untrusted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryRecordV2 {
    pub schema_version: u16,
    pub project_id: String,
    pub branch_id: String,
    pub session_id: String,
    pub sequence: u64,
    pub turn_id: String,
    pub user_message: ConversationMessageV2,
    pub assistant_messages: Vec<ConversationMessageV2>,
    pub selected_skill_ids: Vec<String>,
    pub source_refs: Vec<SourceRefV2>,
    pub terminal_state: String,
    pub created_at_ms: u64,
    pub previous_hash: Option<String>,
    pub record_hash: String,
}

#[derive(Debug, Clone)]
pub struct RetainedSourceBindingV2 {
    pub session_id: String,
    pub branch_id: String,
    pub source: SourceRefV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHistoryBranchV2 {
    pub branch_id: String,
    pub session_id: String,
    pub last_turn_at_ms: Option<u64>,
    pub message_count: u64,
    pub hash_status: AgentHistoryHashStatusV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestV2 {
    pub schema_version: u16,
    pub project_id: String,
    pub active_branch_id: Option<String>,
    pub known_branch_ids: Vec<String>,
    pub draft_quota: u32,
    #[serde(default = "default_authorization_policy")]
    pub write_policy: AuthorizationPolicyV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionMemoryV2 {
    pub schema_version: u16,
    pub project_id: String,
    pub session_id: String,
    pub branch_id: String,
    pub completed_turns: u64,
    pub visible_message_count: u64,
    pub visible_character_count: u64,
    pub summary: String,
    pub compacted_through_turn_id: Option<String>,
    pub last_turn_id: Option<String>,
    pub last_record_hash: Option<String>,
    pub needs_compaction: bool,
    pub memory_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionSnapshotV2 {
    pub schema_version: u16,
    pub project_id: String,
    pub history_consent_required: bool,
    pub branch_selection_required: bool,
    pub active_branch_id: Option<String>,
    pub session_id: Option<String>,
    pub branches: Vec<AgentHistoryBranchV2>,
    pub messages: Vec<ConversationMessageV2>,
    pub memory: Option<SessionMemoryV2>,
    pub draft_quota: u32,
    pub write_policy: AuthorizationPolicyV1,
    pub write_policy_hash: Option<String>,
    pub context_marks: ContextMarkStoreV1,
}

fn default_authorization_policy() -> AuthorizationPolicyV1 {
    AuthorizationPolicyV1::AutoApplySafe
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHistoryClearScopeV2 {
    Session,
    AllHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHistoryClearResultV2 {
    pub scope: AgentHistoryClearScopeV2,
    pub cleared_branch_count: u32,
    pub cleared_message_count: u64,
    pub snapshot: AgentSessionSnapshotV2,
}

#[derive(Debug, Clone)]
struct ValidatedBranch {
    summary: AgentHistoryBranchV2,
    records: Vec<HistoryRecordV2>,
}

pub fn agent_store_exists(handle: &ProjectHandle) -> bool {
    handle.root().join(AGENT_ROOT).join(MANIFEST_FILE).is_file()
}

pub fn last_completed_skill_ids(handle: &ProjectHandle) -> Result<Vec<String>, AppError> {
    if !agent_store_exists(handle) {
        return Ok(Vec::new());
    }
    let manifest = read_manifest(handle)?;
    let Some(branch_id) = manifest.active_branch_id else {
        return Ok(Vec::new());
    };
    let branch = read_valid_history(handle, &branch_id)?;
    Ok(branch
        .records
        .iter()
        .rev()
        .find(|record| {
            record.terminal_state == "completed" && !record.selected_skill_ids.is_empty()
        })
        .map(|record| record.selected_skill_ids.clone())
        .unwrap_or_default())
}

pub fn retained_source_bindings(
    handle: &ProjectHandle,
    maximum: usize,
) -> Result<Vec<RetainedSourceBindingV2>, AppError> {
    if !agent_store_exists(handle) || maximum == 0 {
        return Ok(Vec::new());
    }
    let manifest = read_manifest(handle)?;
    let mut branch_ids = manifest.known_branch_ids;
    branch_ids.sort();
    let mut seen = BTreeSet::new();
    let mut bindings = Vec::new();
    for branch_id in branch_ids {
        let branch = read_valid_history(handle, &branch_id)?;
        for record in branch.records.iter().rev() {
            for source in record.source_refs.iter().rev() {
                let key = format!(
                    "{}\0{}\0{}",
                    source.relative_path,
                    source.field_path.as_deref().unwrap_or_default(),
                    source.heading_path.join("\0")
                );
                if source.untrusted && seen.insert(key) {
                    bindings.push(RetainedSourceBindingV2 {
                        session_id: record.session_id.clone(),
                        branch_id: record.branch_id.clone(),
                        source: source.clone(),
                    });
                    if bindings.len() == maximum.min(64) {
                        return Ok(bindings);
                    }
                }
            }
        }
    }
    Ok(bindings)
}

pub fn source_ref_by_id(handle: &ProjectHandle, source_id: &str) -> Result<SourceRefV2, AppError> {
    validate_opaque_id(source_id, "agent.source_invalid")?;
    if !source_id.starts_with("src_") || !agent_store_exists(handle) {
        return Err(agent_error("agent.source_invalid"));
    }
    let manifest = read_manifest(handle)?;
    let branch_id = manifest
        .active_branch_id
        .ok_or_else(|| agent_error("agent.session_not_ready"))?;
    read_valid_history(handle, &branch_id)?
        .records
        .iter()
        .rev()
        .flat_map(|record| record.source_refs.iter())
        .find(|source| source.source_id == source_id)
        .cloned()
        .ok_or_else(|| agent_error("agent.source_invalid"))
}

pub fn load_agent_session(
    handle: &ProjectHandle,
    history_consent: bool,
    now_ms: u64,
) -> Result<AgentSessionSnapshotV2, AppError> {
    validate_project_root(handle)?;
    let manifest_path = agent_root(handle)?.join(MANIFEST_FILE);
    if !manifest_path.exists() {
        if !history_consent {
            return Ok(AgentSessionSnapshotV2 {
                schema_version: AGENT_SCHEMA_VERSION,
                project_id: handle.project_id().to_owned(),
                history_consent_required: true,
                branch_selection_required: false,
                active_branch_id: None,
                session_id: None,
                branches: Vec::new(),
                messages: Vec::new(),
                memory: None,
                draft_quota: 0,
                write_policy: AuthorizationPolicyV1::AutoApplySafe,
                write_policy_hash: None,
                context_marks: load_context_marks(handle)?,
            });
        }
        initialize_agent_store(handle, now_ms)?;
    }

    let manifest = read_manifest(handle)?;
    if manifest.project_id != handle.project_id()
        || manifest.schema_version != AGENT_SCHEMA_VERSION
        || manifest.draft_quota != 0
    {
        return Err(agent_error("agent.manifest_invalid"));
    }
    let branches = scan_history_branches(handle)?;
    let current_ids = branches
        .iter()
        .map(|branch| branch.summary.branch_id.clone())
        .collect::<BTreeSet<_>>();
    let known_ids = manifest
        .known_branch_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let active_is_valid = manifest.active_branch_id.as_ref().is_some_and(|active| {
        branches.iter().any(|branch| {
            branch.summary.branch_id == *active
                && branch.summary.hash_status != AgentHistoryHashStatusV2::Invalid
        })
    });
    let branch_selection_required = branches.len() > 1
        && (current_ids != known_ids || !active_is_valid)
        || branches
            .iter()
            .any(|branch| branch.summary.hash_status == AgentHistoryHashStatusV2::Invalid);
    let active = if branch_selection_required {
        None
    } else {
        manifest.active_branch_id.as_ref().and_then(|active| {
            branches
                .iter()
                .find(|branch| branch.summary.branch_id == *active)
        })
    };
    let messages = active
        .map(|branch| visible_messages(&branch.records))
        .unwrap_or_default();
    let memory = active
        .map(|branch| read_memory(handle, &branch.summary.session_id))
        .transpose()?
        .flatten();
    let active_policy_hash = active
        .map(|branch| write_policy_hash(handle, &branch.summary.session_id, manifest.write_policy))
        .transpose()?;
    Ok(AgentSessionSnapshotV2 {
        schema_version: AGENT_SCHEMA_VERSION,
        project_id: handle.project_id().to_owned(),
        history_consent_required: false,
        branch_selection_required,
        active_branch_id: active.map(|branch| branch.summary.branch_id.clone()),
        session_id: active.map(|branch| branch.summary.session_id.clone()),
        branches: branches.into_iter().map(|branch| branch.summary).collect(),
        messages,
        memory,
        draft_quota: 0,
        write_policy: manifest.write_policy,
        write_policy_hash: active_policy_hash,
        context_marks: load_context_marks(handle)?,
    })
}

pub fn resolve_history_branch(
    handle: &ProjectHandle,
    branch_id: &str,
) -> Result<AgentSessionSnapshotV2, AppError> {
    validate_opaque_id(branch_id, "agent.branch_invalid")?;
    let branches = scan_history_branches(handle)?;
    let selected = branches
        .iter()
        .find(|branch| branch.summary.branch_id == branch_id)
        .filter(|branch| branch.summary.hash_status != AgentHistoryHashStatusV2::Invalid)
        .ok_or_else(|| agent_error("agent.branch_invalid"))?;
    let mut manifest = read_manifest(handle)?;
    manifest.active_branch_id = Some(branch_id.to_owned());
    manifest.known_branch_ids = branches
        .iter()
        .map(|branch| branch.summary.branch_id.clone())
        .collect();
    write_json_atomic(&agent_root(handle)?.join(MANIFEST_FILE), &manifest)?;
    Ok(AgentSessionSnapshotV2 {
        schema_version: AGENT_SCHEMA_VERSION,
        project_id: handle.project_id().to_owned(),
        history_consent_required: false,
        branch_selection_required: false,
        active_branch_id: Some(branch_id.to_owned()),
        session_id: Some(selected.summary.session_id.clone()),
        branches: branches
            .iter()
            .map(|branch| branch.summary.clone())
            .collect(),
        messages: visible_messages(&selected.records),
        memory: read_memory(handle, &selected.summary.session_id)?,
        draft_quota: 0,
        write_policy: manifest.write_policy,
        write_policy_hash: Some(write_policy_hash(
            handle,
            &selected.summary.session_id,
            manifest.write_policy,
        )?),
        context_marks: load_context_marks(handle)?,
    })
}

pub struct CompletedTurnInputV2<'a> {
    pub turn_id: &'a str,
    pub user_message_id: Option<&'a str>,
    pub user_content: &'a str,
    pub assistant_content: &'a str,
    pub selected_skill_ids: Vec<String>,
    pub source_refs: Vec<SourceRefV2>,
    pub terminal_state: &'a str,
    pub now_ms: u64,
}

pub fn append_completed_turn(
    handle: &ProjectHandle,
    input: CompletedTurnInputV2<'_>,
) -> Result<AgentSessionSnapshotV2, AppError> {
    let CompletedTurnInputV2 {
        turn_id,
        user_message_id,
        user_content,
        assistant_content,
        selected_skill_ids,
        source_refs,
        terminal_state,
        now_ms,
    } = input;
    validate_opaque_id(turn_id, "agent.turn_invalid")?;
    if let Some(message_id) = user_message_id {
        validate_opaque_id(message_id, "agent.message_invalid")?;
        if !message_id.starts_with("msg_") {
            return Err(agent_error("agent.message_invalid"));
        }
    }
    if user_content.trim().is_empty()
        || assistant_content.trim().is_empty()
        || user_content.chars().count() > MAX_MESSAGE_BODY_CHARACTERS
        || assistant_content.chars().count() > MAX_MESSAGE_BODY_CHARACTERS
        || !matches!(terminal_state, "completed" | "cancelled" | "failed")
    {
        return Err(agent_error("agent.message_invalid"));
    }
    let mut selected_skills = BTreeSet::new();
    if selected_skill_ids.len() > novelgainker_agent::v2::MAX_SELECTED_SKILLS_V2
        || selected_skill_ids.iter().any(|skill_id| {
            !novelgainker_agent::SKILL_IDS.contains(&skill_id.as_str())
                || !selected_skills.insert(skill_id.as_str())
        })
    {
        return Err(agent_error("agent.skill_selection_invalid"));
    }
    let mut source_ids = BTreeSet::new();
    if source_refs.len() > MAX_SOURCE_REFS_PER_TURN
        || source_refs.iter().any(|source| {
            validate_opaque_id(&source.source_id, "agent.source_invalid").is_err()
                || !source.source_id.starts_with("src_")
                || !source_ids.insert(source.source_id.as_str())
                || source.relative_path.len() > 512
                || validate_relative_source_path(&source.relative_path).is_err()
                || source
                    .entity
                    .as_ref()
                    .is_some_and(|value| value.len() > 256)
                || source
                    .field_path
                    .as_ref()
                    .is_some_and(|value| value.len() > 256)
                || source.heading_path.len() > 16
                || source.heading_path.iter().any(|value| value.len() > 256)
                || validate_hash(&source.content_hash).is_err()
                || validate_hash(&source.excerpt_hash).is_err()
                || source.excerpt_hash != sha256_hex(source.excerpt.as_bytes())
                || source.excerpt.chars().count() > MAX_SOURCE_EXCERPT_CHARACTERS
                || !source.untrusted
        })
    {
        return Err(agent_error("agent.source_invalid"));
    }
    let snapshot = load_agent_session(handle, false, now_ms)?;
    if snapshot.history_consent_required || snapshot.branch_selection_required {
        return Err(agent_error("agent.session_not_ready"));
    }
    let branch_id = snapshot
        .active_branch_id
        .ok_or_else(|| agent_error("agent.session_not_ready"))?;
    let session_id = snapshot
        .session_id
        .ok_or_else(|| agent_error("agent.session_not_ready"))?;
    let branch = read_valid_history(handle, &branch_id)?;
    let previous_hash = branch
        .records
        .last()
        .map(|record| record.record_hash.clone());
    let sequence = u64::try_from(branch.records.len()).unwrap_or(u64::MAX) + 1;
    let mut record = HistoryRecordV2 {
        schema_version: AGENT_SCHEMA_VERSION,
        project_id: handle.project_id().to_owned(),
        branch_id: branch_id.clone(),
        session_id: session_id.clone(),
        sequence,
        turn_id: turn_id.to_owned(),
        user_message: ConversationMessageV2 {
            schema_version: AGENT_SCHEMA_VERSION,
            message_id: user_message_id
                .map(str::to_owned)
                .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple())),
            response_id: None,
            turn_id: turn_id.to_owned(),
            role: ConversationRoleV2::User,
            body: user_content.to_owned(),
            body_hash: sha256_hex(user_content.as_bytes()),
            source_ids: Vec::new(),
            status: ConversationStatusV2::Complete,
            created_at_ms: now_ms,
        },
        assistant_messages: vec![ConversationMessageV2 {
            schema_version: AGENT_SCHEMA_VERSION,
            message_id: format!("msg_{}", Uuid::new_v4().simple()),
            response_id: Some(format!("rsp_{}", Uuid::new_v4().simple())),
            turn_id: turn_id.to_owned(),
            role: ConversationRoleV2::Assistant,
            body: assistant_content.to_owned(),
            body_hash: sha256_hex(assistant_content.as_bytes()),
            source_ids: source_refs
                .iter()
                .map(|source| source.source_id.clone())
                .collect(),
            status: match terminal_state {
                "cancelled" => ConversationStatusV2::Cancelled,
                "failed" => ConversationStatusV2::Failed,
                _ => ConversationStatusV2::Complete,
            },
            created_at_ms: now_ms,
        }],
        selected_skill_ids,
        source_refs,
        terminal_state: terminal_state.to_owned(),
        created_at_ms: now_ms,
        previous_hash,
        record_hash: String::new(),
    };
    record.record_hash = history_record_hash(&record)?;
    append_json_line_atomic(handle, &branch_id, &record)?;
    update_memory_after_turn(handle, &record)?;
    load_agent_session(handle, false, now_ms)
}

pub fn clear_agent_history(
    handle: &ProjectHandle,
    scope: AgentHistoryClearScopeV2,
    now_ms: u64,
) -> Result<AgentHistoryClearResultV2, AppError> {
    let before = load_agent_session(handle, false, now_ms)?;
    if before.history_consent_required {
        return Err(agent_error("agent.session_not_ready"));
    }
    let selected_branch = before.active_branch_id.clone();
    let selected_session = before.session_id.clone();
    let targets = match scope {
        AgentHistoryClearScopeV2::Session => selected_branch.into_iter().collect::<Vec<_>>(),
        AgentHistoryClearScopeV2::AllHistory => before
            .branches
            .iter()
            .map(|branch| branch.branch_id.clone())
            .collect(),
    };
    for branch_id in &targets {
        remove_known_file(&history_path(handle, branch_id)?)?;
    }
    if scope == AgentHistoryClearScopeV2::AllHistory {
        for branch in &before.branches {
            remove_known_file(&memory_path(handle, &branch.session_id)?)?;
        }
    } else if let Some(session_id) = selected_session {
        remove_known_file(&memory_path(handle, &session_id)?)?;
    }
    let mut manifest = read_manifest(handle)?;
    let (branch_id, session_id) = fresh_session_ids(now_ms);
    create_empty_session_files(handle, &branch_id, &session_id)?;
    manifest.active_branch_id = Some(branch_id.clone());
    manifest
        .known_branch_ids
        .retain(|known| !targets.contains(known));
    manifest.known_branch_ids.push(branch_id);
    if scope == AgentHistoryClearScopeV2::AllHistory {
        manifest.known_branch_ids = vec![manifest
            .active_branch_id
            .clone()
            .expect("active branch was just assigned")];
    }
    write_json_atomic(&agent_root(handle)?.join(MANIFEST_FILE), &manifest)?;
    let cleared_message_count = if scope == AgentHistoryClearScopeV2::AllHistory {
        before
            .branches
            .iter()
            .map(|branch| branch.message_count)
            .sum()
    } else {
        before.messages.len() as u64
    };
    Ok(AgentHistoryClearResultV2 {
        scope,
        cleared_branch_count: targets.len() as u32,
        cleared_message_count,
        snapshot: load_agent_session(handle, false, now_ms)?,
    })
}

pub fn persist_agent_write_policy(
    handle: &ProjectHandle,
    expected_policy_hash: &str,
    policy: AuthorizationPolicyV1,
    now_ms: u64,
) -> Result<AgentSessionSnapshotV2, AppError> {
    validate_hash(expected_policy_hash)?;
    let session = load_agent_session(handle, false, now_ms)?;
    let session_id = session
        .session_id
        .as_deref()
        .ok_or_else(|| agent_error("agent.session_not_ready"))?;
    if session.write_policy_hash.as_deref() != Some(expected_policy_hash) {
        return Err(agent_error("agent-write-policy.hash-changed"));
    }
    let mut manifest = read_manifest(handle)?;
    manifest.write_policy = policy;
    write_json_atomic(&agent_root(handle)?.join(MANIFEST_FILE), &manifest)?;
    let mut refreshed = load_agent_session(handle, false, now_ms)?;
    refreshed.write_policy_hash = Some(write_policy_hash(handle, session_id, policy)?);
    Ok(refreshed)
}

fn write_policy_hash(
    handle: &ProjectHandle,
    session_id: &str,
    policy: AuthorizationPolicyV1,
) -> Result<String, AppError> {
    serde_json::to_vec(&(2u16, handle.project_id(), session_id, policy))
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|_| agent_error("agent-write-policy.hash-invalid"))
}

pub fn compact_memory(
    handle: &ProjectHandle,
    session_id: &str,
    expected_memory_hash: &str,
    compacted_summary: &str,
    compacted_through_turn_id: &str,
) -> Result<SessionMemoryV2, AppError> {
    validate_opaque_id(session_id, "agent.session_invalid")?;
    validate_hash(expected_memory_hash)?;
    let mut memory =
        read_memory(handle, session_id)?.ok_or_else(|| agent_error("agent.memory_not_found"))?;
    if memory.memory_hash != expected_memory_hash {
        return Err(agent_error("agent.memory_hash_changed"));
    }
    validate_opaque_id(compacted_through_turn_id, "agent.turn_invalid")?;
    memory.summary = compacted_summary.trim().chars().take(16_000).collect();
    memory.compacted_through_turn_id = Some(compacted_through_turn_id.to_owned());
    memory.needs_compaction = false;
    memory.memory_hash = memory_hash(&memory)?;
    write_json_atomic(&memory_path(handle, session_id)?, &memory)?;
    Ok(memory)
}

pub fn memory_for_compaction(
    handle: &ProjectHandle,
    session_id: &str,
    expected_memory_hash: &str,
) -> Result<SessionMemoryV2, AppError> {
    validate_hash(expected_memory_hash)?;
    let memory =
        read_memory(handle, session_id)?.ok_or_else(|| agent_error("agent.memory_not_found"))?;
    if memory.memory_hash != expected_memory_hash {
        return Err(agent_error("agent.memory_hash_changed"));
    }
    if !memory.needs_compaction {
        return Err(agent_error("agent.memory_compaction_not_needed"));
    }
    Ok(memory)
}

fn initialize_agent_store(handle: &ProjectHandle, now_ms: u64) -> Result<(), AppError> {
    let root = agent_root(handle)?;
    ensure_directory(&root)?;
    ensure_directory(&root.join("history"))?;
    ensure_directory(&root.join("memory"))?;
    ensure_directory(&root.join("drafts"))?;
    let (branch_id, session_id) = fresh_session_ids(now_ms);
    create_empty_session_files(handle, &branch_id, &session_id)?;
    write_json_atomic(
        &root.join(MANIFEST_FILE),
        &AgentManifestV2 {
            schema_version: AGENT_SCHEMA_VERSION,
            project_id: handle.project_id().to_owned(),
            active_branch_id: Some(branch_id.clone()),
            known_branch_ids: vec![branch_id],
            draft_quota: 0,
            write_policy: AuthorizationPolicyV1::AutoApplySafe,
        },
    )
}

fn fresh_session_ids(now_ms: u64) -> (String, String) {
    let suffix = Uuid::new_v4().simple().to_string();
    (
        format!("brn_{now_ms}_{}", &suffix[..12]),
        format!("ses_{now_ms}_{}", &suffix[12..24]),
    )
}

fn create_empty_session_files(
    handle: &ProjectHandle,
    branch_id: &str,
    session_id: &str,
) -> Result<(), AppError> {
    write_bytes_atomic(&history_path(handle, branch_id)?, b"")?;
    let mut memory = SessionMemoryV2 {
        schema_version: AGENT_SCHEMA_VERSION,
        project_id: handle.project_id().to_owned(),
        session_id: session_id.to_owned(),
        branch_id: branch_id.to_owned(),
        completed_turns: 0,
        visible_message_count: 0,
        visible_character_count: 0,
        summary: String::new(),
        compacted_through_turn_id: None,
        last_turn_id: None,
        last_record_hash: None,
        needs_compaction: false,
        memory_hash: String::new(),
    };
    memory.memory_hash = memory_hash(&memory)?;
    write_json_atomic(&memory_path(handle, session_id)?, &memory)
}

fn update_memory_after_turn(
    handle: &ProjectHandle,
    record: &HistoryRecordV2,
) -> Result<(), AppError> {
    let mut memory = read_memory(handle, &record.session_id)?
        .ok_or_else(|| agent_error("agent.memory_not_found"))?;
    memory.completed_turns += 1;
    memory.visible_message_count += 2;
    memory.visible_character_count += (record.user_message.body.chars().count()
        + record
            .assistant_messages
            .iter()
            .map(|message| message.body.chars().count())
            .sum::<usize>()) as u64;
    memory.last_turn_id = Some(record.turn_id.clone());
    memory.last_record_hash = Some(record.record_hash.clone());
    let delta = format!(
        "T{} 用户：{}\nAI：{}",
        record.sequence,
        tail_chars(&record.user_message.body, 480),
        tail_chars(
            &record
                .assistant_messages
                .iter()
                .map(|message| message.body.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            960,
        )
    );
    if !memory.summary.is_empty() {
        memory.summary.push_str("\n\n");
    }
    memory.summary.push_str(&delta);
    if memory.summary.chars().count() > 24_000 {
        memory.summary = tail_chars(&memory.summary, 24_000);
    }
    memory.needs_compaction = memory.visible_message_count >= MEMORY_COMPACT_MESSAGE_THRESHOLD
        || memory.visible_character_count >= MEMORY_COMPACT_CHARACTER_THRESHOLD;
    memory.memory_hash = memory_hash(&memory)?;
    write_json_atomic(&memory_path(handle, &record.session_id)?, &memory)
}

fn read_manifest(handle: &ProjectHandle) -> Result<AgentManifestV2, AppError> {
    read_json(
        &agent_root(handle)?.join(MANIFEST_FILE),
        "agent.manifest_invalid",
    )
}

fn read_memory(
    handle: &ProjectHandle,
    session_id: &str,
) -> Result<Option<SessionMemoryV2>, AppError> {
    validate_opaque_id(session_id, "agent.session_invalid")?;
    let path = memory_path(handle, session_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let memory: SessionMemoryV2 = read_json(&path, "agent.memory_invalid")?;
    if memory.project_id != handle.project_id()
        || memory.schema_version != AGENT_SCHEMA_VERSION
        || memory.session_id != session_id
        || memory.memory_hash != memory_hash(&memory)?
    {
        return Err(agent_error("agent.memory_invalid"));
    }
    Ok(Some(memory))
}

fn scan_history_branches(handle: &ProjectHandle) -> Result<Vec<ValidatedBranch>, AppError> {
    let directory = agent_root(handle)?.join("history");
    validate_existing_directory(&directory)?;
    let mut branches = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|_| agent_error("agent.history_unavailable"))? {
        let entry = entry.map_err(|_| agent_error("agent.history_unavailable"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| agent_error("agent.history_unavailable"))?;
        if is_link_or_reparse(&metadata) || !metadata.is_file() {
            return Err(agent_error("agent.history_boundary"));
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(branch_id) = name.strip_suffix(".jsonl") else {
            return Err(agent_error("agent.history_unknown_file"));
        };
        validate_opaque_id(branch_id, "agent.branch_invalid")?;
        match read_valid_history(handle, branch_id) {
            Ok(branch) => branches.push(branch),
            Err(_) => branches.push(ValidatedBranch {
                summary: AgentHistoryBranchV2 {
                    branch_id: branch_id.to_owned(),
                    session_id: "unavailable".to_owned(),
                    last_turn_at_ms: None,
                    message_count: 0,
                    hash_status: AgentHistoryHashStatusV2::Invalid,
                },
                records: Vec::new(),
            }),
        }
    }
    branches.sort_by(|left, right| left.summary.branch_id.cmp(&right.summary.branch_id));
    Ok(branches)
}

fn read_valid_history(
    handle: &ProjectHandle,
    branch_id: &str,
) -> Result<ValidatedBranch, AppError> {
    validate_opaque_id(branch_id, "agent.branch_invalid")?;
    let path = history_path(handle, branch_id)?;
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|_| agent_error("agent.history_unavailable"))?;
    let mut records = Vec::new();
    let mut expected_previous: Option<String> = None;
    let mut expected_session: Option<String> = None;
    for (index, line) in BufReader::new(file).split(b'\n').enumerate() {
        let line = line.map_err(|_| agent_error("agent.history_invalid"))?;
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_HISTORY_LINE_BYTES {
            return Err(agent_error("agent.history_invalid"));
        }
        let record: HistoryRecordV2 =
            serde_json::from_slice(&line).map_err(|_| agent_error("agent.history_invalid"))?;
        let expected_sequence =
            u64::try_from(records.len()).map_err(|_| agent_error("agent.history_invalid"))? + 1;
        if record.schema_version != AGENT_SCHEMA_VERSION
            || record.branch_id != branch_id
            || record.sequence != expected_sequence
            || record.previous_hash != expected_previous
            || record.record_hash != history_record_hash(&record)?
            || record.user_message.role != ConversationRoleV2::User
            || record.project_id != handle.project_id()
            || record.user_message.turn_id != record.turn_id
            || record.user_message.body_hash != sha256_hex(record.user_message.body.as_bytes())
            || record.assistant_messages.is_empty()
            || record.assistant_messages.iter().any(|message| {
                message.role != ConversationRoleV2::Assistant
                    || message.turn_id != record.turn_id
                    || message.body_hash != sha256_hex(message.body.as_bytes())
            })
        {
            return Err(agent_error("agent.history_invalid"));
        }
        if let Some(session_id) = &expected_session {
            if session_id != &record.session_id {
                return Err(agent_error("agent.history_invalid"));
            }
        } else {
            expected_session = Some(record.session_id.clone());
        }
        if index > usize::try_from(u32::MAX).unwrap_or(usize::MAX) {
            return Err(agent_error("agent.history_invalid"));
        }
        expected_previous = Some(record.record_hash.clone());
        records.push(record);
    }
    let memory_session =
        expected_session.unwrap_or_else(|| empty_branch_session_id(handle, branch_id));
    Ok(ValidatedBranch {
        summary: AgentHistoryBranchV2 {
            branch_id: branch_id.to_owned(),
            session_id: memory_session,
            last_turn_at_ms: records.last().map(|record| record.created_at_ms),
            message_count: (records.len() * 2) as u64,
            hash_status: if records.is_empty() {
                AgentHistoryHashStatusV2::Empty
            } else {
                AgentHistoryHashStatusV2::Valid
            },
        },
        records,
    })
}

fn empty_branch_session_id(handle: &ProjectHandle, branch_id: &str) -> String {
    let memory_directory = match agent_root(handle) {
        Ok(root) => root.join("memory"),
        Err(_) => return "unavailable".to_owned(),
    };
    let Ok(entries) = fs::read_dir(memory_directory) else {
        return "unavailable".to_owned();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(memory) = serde_json::from_slice::<SessionMemoryV2>(&bytes) else {
            continue;
        };
        if memory.branch_id == branch_id {
            return memory.session_id;
        }
    }
    "unavailable".to_owned()
}

fn append_json_line_atomic(
    handle: &ProjectHandle,
    branch_id: &str,
    record: &HistoryRecordV2,
) -> Result<(), AppError> {
    let branch = read_valid_history(handle, branch_id)?;
    let mut bytes = Vec::new();
    for existing in branch.records {
        serde_json::to_writer(&mut bytes, &existing)
            .map_err(|_| agent_error("agent.history_serialize"))?;
        bytes.push(b'\n');
    }
    serde_json::to_writer(&mut bytes, record)
        .map_err(|_| agent_error("agent.history_serialize"))?;
    bytes.push(b'\n');
    write_bytes_atomic(&history_path(handle, branch_id)?, &bytes)
}

fn visible_messages(records: &[HistoryRecordV2]) -> Vec<ConversationMessageV2> {
    records
        .iter()
        .flat_map(|record| {
            std::iter::once(record.user_message.clone())
                .chain(record.assistant_messages.iter().cloned())
        })
        .collect()
}

fn history_record_hash(record: &HistoryRecordV2) -> Result<String, AppError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct HashInput<'a> {
        schema_version: u16,
        project_id: &'a str,
        branch_id: &'a str,
        session_id: &'a str,
        sequence: u64,
        turn_id: &'a str,
        user_message: &'a ConversationMessageV2,
        assistant_messages: &'a [ConversationMessageV2],
        selected_skill_ids: &'a [String],
        source_refs: &'a [SourceRefV2],
        terminal_state: &'a str,
        created_at_ms: u64,
        previous_hash: &'a Option<String>,
    }
    let bytes = serde_json::to_vec(&HashInput {
        schema_version: record.schema_version,
        project_id: &record.project_id,
        branch_id: &record.branch_id,
        session_id: &record.session_id,
        sequence: record.sequence,
        turn_id: &record.turn_id,
        user_message: &record.user_message,
        assistant_messages: &record.assistant_messages,
        selected_skill_ids: &record.selected_skill_ids,
        source_refs: &record.source_refs,
        terminal_state: &record.terminal_state,
        created_at_ms: record.created_at_ms,
        previous_hash: &record.previous_hash,
    })
    .map_err(|_| agent_error("agent.history_serialize"))?;
    Ok(sha256_hex(&bytes))
}

fn memory_hash(memory: &SessionMemoryV2) -> Result<String, AppError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct HashInput<'a> {
        schema_version: u16,
        project_id: &'a str,
        session_id: &'a str,
        branch_id: &'a str,
        completed_turns: u64,
        visible_message_count: u64,
        visible_character_count: u64,
        summary: &'a str,
        compacted_through_turn_id: &'a Option<String>,
        last_turn_id: &'a Option<String>,
        last_record_hash: &'a Option<String>,
        needs_compaction: bool,
    }
    let bytes = serde_json::to_vec(&HashInput {
        schema_version: memory.schema_version,
        project_id: &memory.project_id,
        session_id: &memory.session_id,
        branch_id: &memory.branch_id,
        completed_turns: memory.completed_turns,
        visible_message_count: memory.visible_message_count,
        visible_character_count: memory.visible_character_count,
        summary: &memory.summary,
        compacted_through_turn_id: &memory.compacted_through_turn_id,
        last_turn_id: &memory.last_turn_id,
        last_record_hash: &memory.last_record_hash,
        needs_compaction: memory.needs_compaction,
    })
    .map_err(|_| agent_error("agent.memory_serialize"))?;
    Ok(sha256_hex(&bytes))
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    error_code: &'static str,
) -> Result<T, AppError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| agent_error(error_code))?;
    if is_link_or_reparse(&metadata)
        || !metadata.is_file()
        || metadata.len() > MAX_HISTORY_LINE_BYTES as u64
    {
        return Err(agent_error(error_code));
    }
    let bytes = fs::read(path).map_err(|_| agent_error(error_code))?;
    serde_json::from_slice(&bytes).map_err(|_| agent_error(error_code))
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), AppError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|_| agent_error("agent.persistence_serialize"))?;
    bytes.push(b'\n');
    write_bytes_atomic(path, &bytes)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or_else(|| agent_error("agent.persistence_boundary"))?;
    validate_existing_directory(parent)?;
    if path.exists() {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| agent_error("agent.persistence_unavailable"))?;
        if is_link_or_reparse(&metadata) || !metadata.is_file() {
            return Err(agent_error("agent.persistence_boundary"));
        }
    }
    let temp = parent.join(format!(".agent-v2-{}.tmp", Uuid::new_v4().simple()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|_| agent_error("agent.persistence_unavailable"))?;
    let write_result = (|| {
        file.write_all(bytes)
            .map_err(|_| agent_error("agent.persistence_unavailable"))?;
        file.sync_all()
            .map_err(|_| agent_error("agent.persistence_unavailable"))
    })();
    drop(file);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = atomic_replace(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

#[cfg(windows)]
fn atomic_replace(temporary_path: &Path, target_path: &Path) -> Result<(), AppError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let temporary = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(agent_error("agent.persistence_atomic_replace"))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(temporary_path: &Path, target_path: &Path) -> Result<(), AppError> {
    fs::rename(temporary_path, target_path)
        .map_err(|_| agent_error("agent.persistence_atomic_replace"))
}

fn remove_known_file(path: &Path) -> Result<(), AppError> {
    if !path.exists() {
        return Ok(());
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| agent_error("agent.persistence_unavailable"))?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(agent_error("agent.persistence_boundary"));
    }
    fs::remove_file(path).map_err(|_| agent_error("agent.persistence_unavailable"))
}

fn ensure_directory(path: &Path) -> Result<(), AppError> {
    if path.exists() {
        return validate_existing_directory(path);
    }
    fs::create_dir(path).map_err(|_| agent_error("agent.persistence_unavailable"))
}

fn validate_existing_directory(path: &Path) -> Result<(), AppError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| agent_error("agent.persistence_unavailable"))?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(agent_error("agent.persistence_boundary"));
    }
    Ok(())
}

fn validate_project_root(handle: &ProjectHandle) -> Result<(), AppError> {
    let root = handle.root();
    if !root.is_absolute() {
        return Err(agent_error("agent.project_boundary"));
    }
    validate_existing_directory(root)
}

fn agent_root(handle: &ProjectHandle) -> Result<PathBuf, AppError> {
    validate_project_root(handle)?;
    Ok(handle.root().join(AGENT_ROOT))
}

fn history_path(handle: &ProjectHandle, branch_id: &str) -> Result<PathBuf, AppError> {
    validate_opaque_id(branch_id, "agent.branch_invalid")?;
    Ok(agent_root(handle)?
        .join("history")
        .join(format!("{branch_id}.jsonl")))
}

fn memory_path(handle: &ProjectHandle, session_id: &str) -> Result<PathBuf, AppError> {
    validate_opaque_id(session_id, "agent.session_invalid")?;
    Ok(agent_root(handle)?
        .join("memory")
        .join(format!("{session_id}.json")))
}

fn validate_opaque_id(value: &str, code: &'static str) -> Result<(), AppError> {
    if value.len() < 8
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(agent_error(code));
    }
    Ok(())
}

fn validate_hash(value: &str) -> Result<(), AppError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(agent_error("agent.hash_invalid"))
    }
}

fn agent_error(code: &'static str) -> AppError {
    AppError::agent_contract(code)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn tail_chars(value: &str, limit: usize) -> String {
    let length = value.chars().count();
    value.chars().skip(length.saturating_sub(limit)).collect()
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextIndexStatsV2 {
    pub document_count: u32,
    pub character_count: u64,
    pub index_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextQueryResultV2 {
    pub query: novelgainker_agent::v2::ContextQueryV2,
    pub sources: Vec<SourceRefV2>,
}

#[derive(Debug, Clone)]
pub struct ProjectContextIndexV2 {
    project_id: String,
    documents: Vec<IndexedDocumentV2>,
    stats: ContextIndexStatsV2,
}

#[derive(Debug, Clone)]
struct IndexedDocumentV2 {
    relative_path: String,
    entity: Option<String>,
    field_paths: Vec<String>,
    heading_paths: Vec<Vec<String>>,
    content: String,
    normalized: String,
    content_hash: String,
    untrusted: bool,
}

impl ProjectContextIndexV2 {
    pub fn build(handle: &ProjectHandle) -> Result<Self, AppError> {
        validate_project_root(handle)?;
        let mut paths = Vec::new();
        collect_root_markdown_paths(handle.root(), &mut paths)?;
        for directory in CANON_DIRECTORIES {
            let root = handle.root().join(directory);
            if root.exists() {
                collect_canon_markdown_paths(handle.root(), &root, 0, &mut paths)?;
            }
        }
        paths.sort();
        paths.dedup();
        if paths.len() > MAX_CONTEXT_FILES {
            return Err(agent_error("agent.context_index_too_large"));
        }
        let mut documents = Vec::with_capacity(paths.len());
        let mut character_count = 0_u64;
        let mut aggregate = Sha256::new();
        for relative_path in paths {
            let absolute = safe_canon_file(handle.root(), &relative_path)?;
            let metadata = fs::symlink_metadata(&absolute)
                .map_err(|_| agent_error("agent.context_source_unavailable"))?;
            if metadata.len() > MAX_CONTEXT_FILE_BYTES {
                return Err(agent_error("agent.context_source_too_large"));
            }
            let bytes =
                fs::read(&absolute).map_err(|_| agent_error("agent.context_source_unavailable"))?;
            let content = String::from_utf8(bytes)
                .map_err(|_| agent_error("agent.context_source_encoding"))?;
            let content_hash = sha256_hex(content.as_bytes());
            aggregate.update(relative_path.as_bytes());
            aggregate.update([0]);
            aggregate.update(content_hash.as_bytes());
            aggregate.update([0]);
            character_count = character_count.saturating_add(content.chars().count() as u64);
            let (entity, field_paths, heading_paths) =
                extract_document_anchors(&relative_path, &content);
            documents.push(IndexedDocumentV2 {
                relative_path,
                entity,
                field_paths,
                heading_paths,
                normalized: normalize_search_text(&content),
                content,
                content_hash,
                untrusted: true,
            });
        }
        let stats = ContextIndexStatsV2 {
            document_count: documents.len() as u32,
            character_count,
            index_hash: sha256_hex(&aggregate.finalize()),
        };
        Ok(Self {
            project_id: handle.project_id().to_owned(),
            documents,
            stats,
        })
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn stats(&self) -> &ContextIndexStatsV2 {
        &self.stats
    }

    pub fn query(
        &self,
        turn_id: &str,
        queries: &[novelgainker_agent::v2::ContextQueryV2],
        maximum_sources_per_query: usize,
    ) -> Result<Vec<ContextQueryResultV2>, AppError> {
        validate_opaque_id(turn_id, "agent.turn_invalid")?;
        if queries.is_empty() || queries.len() > novelgainker_agent::v2::MAX_CONTEXT_QUERIES_V2 {
            return Err(agent_error("agent.context_query_invalid"));
        }
        let maximum_sources = maximum_sources_per_query.clamp(1, 16);
        let mut results = Vec::with_capacity(queries.len());
        for query in queries {
            let query_text = query.terms.join(" ");
            let normalized_queries = query
                .terms
                .iter()
                .map(|term| normalize_search_text(term))
                .filter(|term| !term.is_empty())
                .collect::<Vec<_>>();
            if normalized_queries.is_empty() || query.purpose.trim().is_empty() {
                return Err(agent_error("agent.context_query_invalid"));
            }
            let query_terms = normalized_queries
                .iter()
                .flat_map(|query| search_terms(query))
                .collect::<BTreeSet<_>>();
            let mut ranked = self
                .documents
                .iter()
                .filter_map(|document| {
                    let score = context_match_score(document, &normalized_queries, &query_terms);
                    (score >= MIN_CONTEXT_MATCH_SCORE).then_some((score, document))
                })
                .collect::<Vec<_>>();
            ranked.sort_by(|(left_score, left), (right_score, right)| {
                right_score
                    .cmp(left_score)
                    .then_with(|| left.relative_path.cmp(&right.relative_path))
            });
            let mut sources = Vec::new();
            for (_, document) in ranked.into_iter().take(maximum_sources) {
                let excerpt = context_excerpt(&document.content, &query_text, 2_048);
                let truncated = excerpt.chars().count() < document.content.chars().count();
                let source_seed = format!(
                    "{turn_id}:{}:{}",
                    document.relative_path, document.content_hash
                );
                sources.push(SourceRefV2 {
                    source_id: format!("src_{}", &sha256_hex(source_seed.as_bytes())[..24]),
                    relative_path: document.relative_path.clone(),
                    entity: document.entity.clone(),
                    field_path: document.field_paths.first().cloned(),
                    heading_path: best_heading_path(document, &normalized_queries),
                    content_hash: document.content_hash.clone(),
                    excerpt_hash: sha256_hex(excerpt.as_bytes()),
                    excerpt,
                    truncated,
                    untrusted: document.untrusted,
                });
            }
            results.push(ContextQueryResultV2 {
                query: query.clone(),
                sources,
            });
        }
        Ok(results)
    }

    pub fn sources_for_relative_paths(
        &self,
        turn_id: &str,
        relative_paths: &[String],
        maximum_sources: usize,
    ) -> Result<Vec<SourceRefV2>, AppError> {
        validate_opaque_id(turn_id, "agent.turn_invalid")?;
        let limit = maximum_sources.clamp(1, 16);
        let mut sources = Vec::new();
        let mut seen = BTreeSet::new();
        for relative_path in relative_paths.iter().take(limit) {
            if !seen.insert(relative_path.as_str()) {
                continue;
            }
            let Some(document) = self
                .documents
                .iter()
                .find(|document| document.relative_path == *relative_path)
            else {
                continue;
            };
            let excerpt = document.content.chars().take(2_048).collect::<String>();
            let source_seed = format!(
                "{turn_id}:marked:{}:{}",
                document.relative_path, document.content_hash
            );
            sources.push(SourceRefV2 {
                source_id: format!("src_{}", &sha256_hex(source_seed.as_bytes())[..24]),
                relative_path: document.relative_path.clone(),
                entity: document.entity.clone(),
                field_path: document.field_paths.first().cloned(),
                heading_path: document.heading_paths.first().cloned().unwrap_or_default(),
                content_hash: document.content_hash.clone(),
                excerpt_hash: sha256_hex(excerpt.as_bytes()),
                truncated: excerpt.chars().count() < document.content.chars().count(),
                excerpt,
                untrusted: document.untrusted,
            });
        }
        Ok(sources)
    }

    pub fn verify_unchanged(&self, handle: &ProjectHandle) -> Result<(), AppError> {
        let current = Self::build(handle)?;
        if current.stats.index_hash != self.stats.index_hash {
            return Err(agent_error("agent.context_hash_changed"));
        }
        Ok(())
    }
}

fn collect_root_markdown_paths(root: &Path, output: &mut Vec<String>) -> Result<(), AppError> {
    for entry in fs::read_dir(root).map_err(|_| agent_error("agent.context_index_unavailable"))? {
        let entry = entry.map_err(|_| agent_error("agent.context_index_unavailable"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| agent_error("agent.context_index_unavailable"))?;
        if is_link_or_reparse(&metadata) {
            return Err(agent_error("agent.context_source_boundary"));
        }
        if metadata.is_file()
            && entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                output.push(name.replace('\\', "/"));
            }
        }
    }
    Ok(())
}

fn collect_canon_markdown_paths(
    project_root: &Path,
    directory: &Path,
    depth: usize,
    output: &mut Vec<String>,
) -> Result<(), AppError> {
    if depth > 16 {
        return Err(agent_error("agent.context_index_too_deep"));
    }
    let directory_metadata = fs::symlink_metadata(directory)
        .map_err(|_| agent_error("agent.context_index_unavailable"))?;
    if is_link_or_reparse(&directory_metadata) || !directory_metadata.is_dir() {
        return Err(agent_error("agent.context_source_boundary"));
    }
    for entry in
        fs::read_dir(directory).map_err(|_| agent_error("agent.context_index_unavailable"))?
    {
        let entry = entry.map_err(|_| agent_error("agent.context_index_unavailable"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| agent_error("agent.context_index_unavailable"))?;
        if is_link_or_reparse(&metadata) {
            return Err(agent_error("agent.context_source_boundary"));
        }
        if metadata.is_dir() {
            collect_canon_markdown_paths(project_root, &path, depth + 1, output)?;
        } else if metadata.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        {
            if output.len() >= MAX_CONTEXT_FILES {
                return Err(agent_error("agent.context_index_too_large"));
            }
            let relative = path
                .strip_prefix(project_root)
                .map_err(|_| agent_error("agent.context_source_boundary"))?
                .to_string_lossy()
                .replace('\\', "/");
            if relative.starts_with(".novelgainker/") {
                return Err(agent_error("agent.context_source_boundary"));
            }
            output.push(relative);
        }
    }
    Ok(())
}

fn safe_canon_file(root: &Path, relative_path: &str) -> Result<PathBuf, AppError> {
    validate_relative_source_path(relative_path)?;
    let relative = Path::new(relative_path);
    let absolute = root.join(relative);
    let canonical = absolute
        .canonicalize()
        .map_err(|_| agent_error("agent.context_source_unavailable"))?;
    if !canonical.starts_with(root) {
        return Err(agent_error("agent.context_source_boundary"));
    }
    Ok(canonical)
}

fn validate_relative_source_path(relative_path: &str) -> Result<(), AppError> {
    let relative = Path::new(relative_path);
    if relative_path.is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || relative_path
            .replace('\\', "/")
            .starts_with(".novelgainker/")
        || relative_path.contains(':')
    {
        return Err(agent_error("agent.context_source_boundary"));
    }
    Ok(())
}

fn extract_document_anchors(
    relative_path: &str,
    content: &str,
) -> (Option<String>, Vec<String>, Vec<Vec<String>>) {
    let mut frontmatter = false;
    let mut frontmatter_ended = false;
    let mut entity = None;
    let mut field_paths = Vec::new();
    let mut heading_stack = Vec::<String>::new();
    let mut heading_paths = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if index == 0 && trimmed == "---" {
            frontmatter = true;
            continue;
        }
        if frontmatter && !frontmatter_ended {
            if trimmed == "---" {
                frontmatter_ended = true;
                continue;
            }
            if let Some((key, value)) = trimmed.split_once(':') {
                let key = key.trim();
                let value = value.trim().trim_matches(['"', '\'']);
                if !key.is_empty() {
                    field_paths.push(key.to_owned());
                }
                if entity.is_none()
                    && matches!(key, "name" | "title" | "id" | "display_name")
                    && !value.is_empty()
                {
                    entity = Some(value.to_owned());
                }
            }
            continue;
        }
        let level = trimmed
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if level > 0 && level <= 6 && trimmed.chars().nth(level) == Some(' ') {
            let heading = trimmed[level + 1..].trim().to_owned();
            heading_stack.truncate(level.saturating_sub(1));
            heading_stack.push(heading);
            heading_paths.push(heading_stack.clone());
        }
    }
    if entity.is_none() {
        entity = Path::new(relative_path)
            .file_stem()
            .and_then(|value| value.to_str())
            .map(str::to_owned);
    }
    (entity, field_paths, heading_paths)
}

fn normalize_search_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace() && !character.is_ascii_punctuation())
        .flat_map(char::to_lowercase)
        .collect()
}

fn search_terms(normalized: &str) -> BTreeSet<String> {
    let characters = normalized.chars().collect::<Vec<_>>();
    let mut terms = BTreeSet::new();
    for width in [2_usize, 3, 4] {
        for window in characters.windows(width) {
            terms.insert(window.iter().collect());
        }
    }
    if terms.is_empty() && !normalized.is_empty() {
        terms.insert(normalized.to_owned());
    }
    terms
}

fn context_match_score(
    document: &IndexedDocumentV2,
    normalized_queries: &[String],
    terms: &BTreeSet<String>,
) -> u32 {
    let mut score = if normalized_queries
        .iter()
        .any(|query| document.normalized.contains(query))
    {
        100
    } else {
        0
    };
    for term in terms {
        if document.normalized.contains(term) {
            score += term.chars().count() as u32;
        }
    }
    let path = normalize_search_text(&document.relative_path);
    if terms.iter().any(|term| path.contains(term)) {
        score += 24;
    }
    if document.entity.as_ref().is_some_and(|entity| {
        let normalized_entity = normalize_search_text(entity);
        normalized_queries
            .iter()
            .any(|query| query.contains(&normalized_entity) || normalized_entity.contains(query))
    }) {
        score += 48;
    }
    score
}

fn context_excerpt(content: &str, query: &str, maximum_chars: usize) -> String {
    let total = content.chars().count();
    if total <= maximum_chars {
        return content.to_owned();
    }
    let normalized_query = query.trim();
    let byte_position = content.find(normalized_query).unwrap_or(0);
    let char_position = content[..byte_position].chars().count();
    let start = char_position.saturating_sub(maximum_chars / 4);
    content.chars().skip(start).take(maximum_chars).collect()
}

fn best_heading_path(document: &IndexedDocumentV2, normalized_queries: &[String]) -> Vec<String> {
    document
        .heading_paths
        .iter()
        .find(|path| {
            path.iter().any(|heading| {
                let normalized_heading = normalize_search_text(heading);
                normalized_queries.iter().any(|query| {
                    query.contains(&normalized_heading) || normalized_heading.contains(query)
                })
            })
        })
        .cloned()
        .or_else(|| document.heading_paths.first().cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{create_project, inspect_project_root, ProjectScanMode};
    use std::path::{Path, PathBuf};

    struct TestProject {
        root: PathBuf,
        handle: ProjectHandle,
    }

    impl TestProject {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("novelgainker-agent-v2-{label}-{}", Uuid::new_v4()));
            fs::create_dir(&root).expect("test root must be created");
            let inspection = inspect_project_root(&root, ProjectScanMode::Create)
                .expect("test root must inspect");
            let handle = create_project(&inspection).expect("test project must be created");
            Self { root, handle }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let temp = std::env::temp_dir();
            let verified = self.root.parent() == Some(temp.as_path())
                && self
                    .root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("novelgainker-agent-v2-"));
            if verified {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    #[test]
    fn requires_privacy_consent_before_creating_project_history() {
        let project = TestProject::new("privacy");
        let before = load_agent_session(&project.handle, false, 100)
            .expect("session preview must be available");
        assert!(before.history_consent_required);
        assert!(!project.path().join(AGENT_ROOT).exists());

        let created =
            load_agent_session(&project.handle, true, 101).expect("consent must initialize");
        assert!(!created.history_consent_required);
        assert_eq!(created.draft_quota, 0);
        assert!(project
            .path()
            .join(AGENT_ROOT)
            .join(MANIFEST_FILE)
            .is_file());
    }

    #[test]
    fn restores_visible_messages_and_incremental_memory_with_hash_chain() {
        let project = TestProject::new("history");
        load_agent_session(&project.handle, true, 100).expect("session must initialize");
        let updated = append_completed_turn(
            &project.handle,
            CompletedTurnInputV2 {
                turn_id: "trn_12345678",
                user_message_id: None,
                user_content: "林舟是谁？",
                assistant_content: "林舟是雾港的守灯人。",
                selected_skill_ids: vec!["create-character".to_owned()],
                source_refs: Vec::new(),
                terminal_state: "completed",
                now_ms: 200,
            },
        )
        .expect("turn must persist");
        assert_eq!(updated.messages.len(), 2);
        let memory = updated.memory.expect("memory must persist");
        assert_eq!(memory.completed_turns, 1);
        assert_eq!(memory.visible_message_count, 2);
        assert!(!memory.memory_hash.is_empty());

        let reopened =
            load_agent_session(&project.handle, false, 300).expect("history must reopen");
        assert_eq!(reopened.messages, updated.messages);
        assert_eq!(
            reopened.branches[0].hash_status,
            AgentHistoryHashStatusV2::Valid
        );
        assert_eq!(
            last_completed_skill_ids(&project.handle).expect("skill continuity metadata"),
            vec!["create-character".to_owned()]
        );
    }

    #[test]
    fn skill_continuity_ignores_failed_empty_selection_without_exposing_message_text() {
        let project = TestProject::new("skill-continuity");
        load_agent_session(&project.handle, true, 100).expect("session must initialize");
        append_completed_turn(
            &project.handle,
            CompletedTurnInputV2 {
                turn_id: "trn_12345678",
                user_message_id: None,
                user_content: "创建人物",
                assistant_content: "人物讨论完成。",
                selected_skill_ids: vec!["create-character".to_owned()],
                source_refs: Vec::new(),
                terminal_state: "completed",
                now_ms: 200,
            },
        )
        .expect("completed turn must persist");
        append_completed_turn(
            &project.handle,
            CompletedTurnInputV2 {
                turn_id: "trn_87654321",
                user_message_id: None,
                user_content: "重试",
                assistant_content: "本次回复未完成，请重试。",
                selected_skill_ids: Vec::new(),
                source_refs: Vec::new(),
                terminal_state: "failed",
                now_ms: 300,
            },
        )
        .expect("failed turn must persist");

        assert_eq!(
            last_completed_skill_ids(&project.handle).expect("skill continuity metadata"),
            vec!["create-character".to_owned()]
        );
    }

    #[test]
    fn isolates_corrupt_history_and_never_exposes_record_body_in_error() {
        let project = TestProject::new("corrupt");
        let session =
            load_agent_session(&project.handle, true, 100).expect("session must initialize");
        append_completed_turn(
            &project.handle,
            CompletedTurnInputV2 {
                turn_id: "trn_12345678",
                user_message_id: None,
                user_content: "私人原稿内容",
                assistant_content: "只显示给作者",
                selected_skill_ids: vec!["create-character".to_owned()],
                source_refs: Vec::new(),
                terminal_state: "completed",
                now_ms: 200,
            },
        )
        .expect("turn must persist");
        let branch = session.active_branch_id.expect("branch must exist");
        fs::write(
            history_path(&project.handle, &branch).expect("path must resolve"),
            b"{\"private\":\"secret manuscript\"}\n",
        )
        .expect("fixture must corrupt");
        let snapshot =
            load_agent_session(&project.handle, false, 300).expect("corruption must be isolated");
        assert!(snapshot.branch_selection_required);
        assert!(snapshot.messages.is_empty());
        assert_eq!(
            snapshot.branches[0].hash_status,
            AgentHistoryHashStatusV2::Invalid
        );
    }

    #[test]
    fn detects_sync_fork_and_requires_explicit_branch_selection() {
        let project = TestProject::new("fork");
        let initial =
            load_agent_session(&project.handle, true, 100).expect("session must initialize");
        let (fork_branch, fork_session) = fresh_session_ids(200);
        create_empty_session_files(&project.handle, &fork_branch, &fork_session)
            .expect("fork fixture must be valid");

        let forked = load_agent_session(&project.handle, false, 300).expect("fork must be listed");
        assert!(forked.branch_selection_required);
        assert_eq!(forked.branches.len(), 2);
        assert!(forked.active_branch_id.is_none());

        let selected = resolve_history_branch(
            &project.handle,
            initial.active_branch_id.as_deref().expect("initial branch"),
        )
        .expect("user selection must resolve");
        assert!(!selected.branch_selection_required);
        assert_eq!(selected.branches.len(), 2);
    }

    #[test]
    fn context_index_reads_only_canon_and_returns_hash_bound_sources() {
        let project = TestProject::new("context");
        fs::write(
            project.path().join("characters/lin-zhou.md"),
            "---\nname: 林舟\naliases: [守灯人]\n---\n# 林舟\n林舟守护雾港灯塔。\n",
        )
        .expect("character fixture must write");
        fs::write(
            project.path().join("world/fog-harbor.md"),
            "---\ntitle: 雾港规则\n---\n# 潮汐规则\n午夜退潮时旧码头才会出现。\n",
        )
        .expect("world fixture must write");
        load_agent_session(&project.handle, true, 100).expect("agent store must initialize");
        fs::write(
            project.path().join(AGENT_ROOT).join("private.md"),
            "# 私人聊天\n不应进入 Canon 索引",
        )
        .expect("agent fixture must write");

        let index = ProjectContextIndexV2::build(&project.handle).expect("index must build");
        assert_eq!(index.stats().document_count, 4);
        let results = index
            .query(
                "trn_12345678",
                &[novelgainker_agent::v2::ContextQueryV2 {
                    kind: novelgainker_agent::v2::ContextQueryKindV2::EntityOrRule,
                    terms: vec!["林舟".to_owned(), "守灯人".to_owned()],
                    known_entity_ids: Vec::new(),
                    known_source_ids: Vec::new(),
                    purpose: "回答人物身份".to_owned(),
                }],
                4,
            )
            .expect("query must succeed");
        assert_eq!(
            results[0].sources[0].relative_path,
            "characters/lin-zhou.md"
        );
        assert_eq!(results[0].sources[0].entity.as_deref(), Some("林舟"));
        assert_eq!(results[0].sources[0].content_hash.len(), 64);
        assert!(results[0]
            .sources
            .iter()
            .all(|source| !source.relative_path.starts_with(".novelgainker/agent")));
    }

    #[test]
    fn context_query_treats_multiple_terms_as_semantic_alternatives() {
        let project = TestProject::new("context-multiple-terms");
        fs::write(
            project.path().join("project.md"),
            "---\ntitle: 广东之王的日常\n---\n# 广东之王的日常\n",
        )
        .expect("project fixture must write");

        let index = ProjectContextIndexV2::build(&project.handle).expect("index must build");
        let results = index
            .query(
                "trn_12345678",
                &[novelgainker_agent::v2::ContextQueryV2 {
                    kind: novelgainker_agent::v2::ContextQueryKindV2::FullText,
                    terms: vec![
                        "岭南".to_owned(),
                        "流放书生".to_owned(),
                        "广东之王".to_owned(),
                    ],
                    known_entity_ids: Vec::new(),
                    known_source_ids: Vec::new(),
                    purpose: "继续现有故事".to_owned(),
                }],
                4,
            )
            .expect("query must succeed");

        assert!(results[0]
            .sources
            .iter()
            .any(|source| source.relative_path == "project.md"));
    }

    #[test]
    fn marked_context_sources_preserve_host_priority_order() {
        let project = TestProject::new("marked-priority");
        fs::write(
            project.path().join("characters/lin-zhou.md"),
            "---\nname: 林舟\n---\n# 林舟\n守灯人。\n",
        )
        .expect("character fixture must write");
        fs::write(
            project.path().join("world/fog-harbor.md"),
            "---\ntitle: 雾港\n---\n# 雾港\n午夜退潮。\n",
        )
        .expect("world fixture must write");
        let index = ProjectContextIndexV2::build(&project.handle).expect("index must build");
        let sources = index
            .sources_for_relative_paths(
                "trn_12345678",
                &[
                    "world/fog-harbor.md".to_owned(),
                    "characters/lin-zhou.md".to_owned(),
                ],
                16,
            )
            .expect("marked sources must resolve");
        assert_eq!(sources[0].relative_path, "world/fog-harbor.md");
        assert_eq!(sources[1].relative_path, "characters/lin-zhou.md");
    }

    #[test]
    fn portable_write_policy_survives_session_reload_and_rejects_stale_hash() {
        let project = TestProject::new("write-policy");
        let initial =
            load_agent_session(&project.handle, true, 100).expect("session must initialize");
        let initial_hash = initial.write_policy_hash.expect("policy hash must exist");
        let updated = persist_agent_write_policy(
            &project.handle,
            &initial_hash,
            AuthorizationPolicyV1::ReviewEveryWrite,
            200,
        )
        .expect("policy update must persist");
        assert_eq!(
            updated.write_policy,
            AuthorizationPolicyV1::ReviewEveryWrite
        );
        let reopened =
            load_agent_session(&project.handle, false, 300).expect("session must reopen");
        assert_eq!(
            reopened.write_policy,
            AuthorizationPolicyV1::ReviewEveryWrite
        );
        assert_eq!(reopened.write_policy_hash, updated.write_policy_hash);
        assert_eq!(
            persist_agent_write_policy(
                &project.handle,
                &initial_hash,
                AuthorizationPolicyV1::AutoApplySafe,
                400,
            )
            .unwrap_err()
            .code,
            "agent-write-policy.hash-changed"
        );
    }

    #[test]
    fn clearing_session_preserves_canon_tree() {
        let project = TestProject::new("clear");
        fs::write(project.path().join("chapters/001.md"), "# 第一章\n正文")
            .expect("chapter fixture must write");
        let canon_before =
            sha256_hex(&fs::read(project.path().join("chapters/001.md")).expect("chapter read"));
        load_agent_session(&project.handle, true, 100).expect("session must initialize");
        append_completed_turn(
            &project.handle,
            CompletedTurnInputV2 {
                turn_id: "trn_12345678",
                user_message_id: None,
                user_content: "继续",
                assistant_content: "我们继续。",
                selected_skill_ids: vec!["novel-story-telling".to_owned()],
                source_refs: Vec::new(),
                terminal_state: "completed",
                now_ms: 200,
            },
        )
        .expect("turn must persist");
        let cleared = clear_agent_history(&project.handle, AgentHistoryClearScopeV2::Session, 300)
            .expect("session clear must succeed");
        assert_eq!(cleared.cleared_message_count, 2);
        assert!(cleared.snapshot.messages.is_empty());
        let canon_after =
            sha256_hex(&fs::read(project.path().join("chapters/001.md")).expect("chapter read"));
        assert_eq!(canon_before, canon_after);
    }
}
