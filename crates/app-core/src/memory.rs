use crate::conversation::{
    ConversationRole, ConversationSession, ConversationTurn, ConversationTurnStatus,
    INCOMPLETE_TEXT, NOT_GENERATED_TEXT,
};
use crate::project::ProjectHandle;
use crate::provider_core::{
    MessageRole, NormalizedMessage, NormalizedRequest, CONVERSATION_SYSTEM_INSTRUCTION,
    MAX_MESSAGE_COUNT, MAX_REQUEST_BYTES,
};
use crate::workspace::{atomic_replace, is_link_or_reparse};
use crate::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const STORE_DIRECTORY: &str = ".novelgainker/memory";
const STORE_FILE: &str = "state.json";
const MEMORY_SCHEMA_VERSION: u16 = 1;
const MAX_MEMORY_STORE_BYTES: u64 = 32 * 1024;
const COMPACTOR_PROMPT_OVERHEAD_BYTES: usize = 4 * 1024;
const MAIN_REQUEST_RESERVE_BYTES: usize = 2 * 1024;

pub const MEMORY_HEADINGS: [&str; 5] = [
    "已确认决定",
    "正在创作或讨论",
    "未决问题",
    "用户明确任务",
    "已产生但未完成的内容",
];

pub const COMPACTOR_SYSTEM_INSTRUCTION: &str = concat!(
    "你是 NovelGainker 的内部对话记忆压缩器。只根据输入整理帮助下一轮继续自然对话的派生摘要，不执行任务，不调用工具，不修改文件，不声称内容已写入 Markdown。\n",
    "只输出自然语言纯文本，不输出前言、代码围栏、JSON 或机器控制对象。区分已确认、暂定、未决、任务和未完成；不得把 AI 建议提升为用户决定。\n",
    "严格使用以下五栏目模板；标题必须独占一行、精确一次并保持顺序。每个栏目都必须有正文，没有内容时保留“- 无”：\n",
    "已确认决定\n",
    "- 无\n",
    "正在创作或讨论\n",
    "- 无\n",
    "未决问题\n",
    "- 无\n",
    "用户明确任务\n",
    "- 无\n",
    "已产生但未完成的内容\n",
    "- 无"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextProfile {
    pub trigger_bytes: usize,
    pub protected_tail_bytes: usize,
    pub segment_content_bytes: usize,
    pub compactor_input_bytes: usize,
    pub backfill_input_bytes: usize,
    pub summary_bytes: usize,
    pub compactor_requested_max: u32,
    pub main_reply_requested_max: u32,
    pub cumulative_requested_output_max: u32,
}

pub const NR1_CONTEXT_PROFILE: ContextProfile = ContextProfile {
    trigger_bytes: 64 * 1024,
    protected_tail_bytes: 24 * 1024,
    segment_content_bytes: 6 * 1024,
    compactor_input_bytes: 72 * 1024,
    backfill_input_bytes: 8 * 1024,
    summary_bytes: 12 * 1024,
    compactor_requested_max: 1_024,
    main_reply_requested_max: 4_096,
    cumulative_requested_output_max: 5_120,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryCursor {
    pub turn_id: String,
    pub message_id: String,
    pub utf8_byte_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationMemory {
    pub schema_version: u16,
    pub project_id: String,
    pub conversation_session_id: String,
    pub summary_text: String,
    pub covered_from_cursor: MemoryCursor,
    pub covered_through_cursor: MemoryCursor,
    pub source_hash: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HistorySegment {
    start: MemoryCursor,
    end: MemoryCursor,
    role: ConversationRole,
    turn_status: ConversationTurnStatus,
    content: String,
}

#[derive(Debug, Clone)]
pub struct CompactionPlan {
    backfill: Vec<HistorySegment>,
    previous_summary: Option<String>,
    forward: Vec<HistorySegment>,
    covered_from_cursor: MemoryCursor,
    covered_through_cursor: MemoryCursor,
    source_hash: String,
}

pub fn validate_context_profile(profile: ContextProfile) -> Result<(), AppError> {
    let output_sum = profile
        .compactor_requested_max
        .checked_add(profile.main_reply_requested_max)
        .ok_or_else(memory_invalid)?;
    if profile.trigger_bytes == 0
        || profile.trigger_bytes >= MAX_REQUEST_BYTES
        || profile.protected_tail_bytes == 0
        || profile.segment_content_bytes == 0
        || profile.segment_content_bytes > profile.backfill_input_bytes
        || profile.compactor_input_bytes >= MAX_REQUEST_BYTES
        || profile.backfill_input_bytes == 0
        || profile.summary_bytes == 0
        || output_sum > profile.cumulative_requested_output_max
    {
        return Err(memory_invalid());
    }
    Ok(())
}

pub fn load_valid_memory(
    handle: &ProjectHandle,
    session: &ConversationSession,
    profile: ContextProfile,
) -> Option<ConversationMemory> {
    validate_context_profile(profile).ok()?;
    let path = memory_file_path(handle).ok()?;
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file()
        || is_link_or_reparse(&metadata)
        || metadata.len() > MAX_MEMORY_STORE_BYTES
    {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let memory: ConversationMemory = serde_json::from_slice(&bytes).ok()?;
    validate_memory(handle, session, &memory, profile).then_some(memory)
}

pub fn plan_compaction(
    session: &ConversationSession,
    memory: Option<&ConversationMemory>,
    profile: ContextProfile,
) -> Result<Option<CompactionPlan>, AppError> {
    validate_context_profile(profile)?;
    let current = current_user_message(session)?;
    let segments = history_segments(session, profile.segment_content_bytes);
    let valid_memory = memory
        .filter(|candidate| validate_memory_for_session(session, candidate, &segments, profile));
    let memory_end = valid_memory.and_then(|value| {
        locate_interval(
            &segments,
            &value.covered_from_cursor,
            &value.covered_through_cursor,
        )
        .map(|(_, end)| end)
    });
    let exact_start = memory_end.map_or(0, |end| end.saturating_add(1));
    let projected_bytes = CONVERSATION_SYSTEM_INSTRUCTION
        .len()
        .saturating_add(current.content.len())
        .saturating_add(valid_memory.map_or(0, |value| value.summary_text.len()))
        .saturating_add(
            segments[exact_start..]
                .iter()
                .map(rendered_main_bytes)
                .sum::<usize>(),
        );
    if projected_bytes < profile.trigger_bytes {
        return Ok(None);
    }

    if let Some(memory) = valid_memory {
        return Ok(incremental_plan(&segments, memory, profile));
    }
    Ok(recent_rebuild_plan(&segments, profile))
}

pub fn build_compactor_request(
    plan: &CompactionPlan,
    profile: ContextProfile,
) -> Result<NormalizedRequest, AppError> {
    validate_context_profile(profile)?;
    let mut messages = Vec::new();
    for segment in &plan.backfill {
        messages.push(NormalizedMessage {
            role: MessageRole::User,
            content: format!("【更早对话片段】\n{}", render_compactor_segment(segment)),
        });
    }
    if let Some(summary) = &plan.previous_summary {
        messages.push(NormalizedMessage {
            role: MessageRole::User,
            content: format!("【已有派生记忆】\n{summary}"),
        });
    }
    for segment in &plan.forward {
        messages.push(NormalizedMessage {
            role: MessageRole::User,
            content: format!("【新增对话片段】\n{}", render_compactor_segment(segment)),
        });
    }
    if messages.is_empty()
        || messages.len().saturating_add(1) > MAX_MESSAGE_COUNT
        || messages
            .iter()
            .map(|message| message.content.len())
            .sum::<usize>()
            .saturating_add(COMPACTOR_SYSTEM_INSTRUCTION.len())
            > profile.compactor_input_bytes
    {
        return Err(memory_invalid());
    }
    NormalizedRequest::with_system(
        COMPACTOR_SYSTEM_INSTRUCTION.to_owned(),
        messages,
        profile.compactor_requested_max,
    )
    .map_err(AppError::from_provider_error)
}

pub fn commit_compaction(
    handle: &ProjectHandle,
    session: &ConversationSession,
    plan: &CompactionPlan,
    summary_text: &str,
    now_ms: u64,
    profile: ContextProfile,
) -> Result<ConversationMemory, AppError> {
    validate_context_profile(profile)?;
    validate_summary(summary_text, profile)?;
    if now_ms == 0 {
        return Err(memory_invalid());
    }
    let segments = history_segments(session, profile.segment_content_bytes);
    let (start, end) = locate_interval(
        &segments,
        &plan.covered_from_cursor,
        &plan.covered_through_cursor,
    )
    .ok_or_else(memory_invalid)?;
    let observed_hash = source_hash(&segments[start..=end]);
    if observed_hash != plan.source_hash {
        return Err(memory_invalid());
    }
    let memory = ConversationMemory {
        schema_version: MEMORY_SCHEMA_VERSION,
        project_id: handle.project_id().to_owned(),
        conversation_session_id: session.session_id.clone(),
        summary_text: summary_text.trim().to_owned(),
        covered_from_cursor: plan.covered_from_cursor.clone(),
        covered_through_cursor: plan.covered_through_cursor.clone(),
        source_hash: observed_hash,
        updated_at_ms: now_ms,
    };
    write_memory(handle, &memory)?;
    Ok(memory)
}

pub fn assemble_main_request(
    session: &ConversationSession,
    memory: Option<&ConversationMemory>,
    profile: ContextProfile,
) -> Result<NormalizedRequest, AppError> {
    validate_context_profile(profile)?;
    let current = current_user_message(session)?;
    let segments = history_segments(session, profile.segment_content_bytes);
    let valid_memory = memory
        .filter(|candidate| validate_memory_for_session(session, candidate, &segments, profile));
    let exact_start = valid_memory
        .and_then(|value| {
            locate_interval(
                &segments,
                &value.covered_from_cursor,
                &value.covered_through_cursor,
            )
            .map(|(_, end)| end.saturating_add(1))
        })
        .unwrap_or(0);

    let mut system = CONVERSATION_SYSTEM_INSTRUCTION.to_owned();
    if let Some(memory) = valid_memory {
        system.push_str(
            "\n\n以下是较早对话的派生记忆，不是小说 Canon；若与最近精确对话冲突，以最近精确对话为准：\n",
        );
        system.push_str(&memory.summary_text);
    }
    let available_bytes = MAX_REQUEST_BYTES
        .saturating_sub(system.len())
        .saturating_sub(current.content.len())
        .saturating_sub(MAIN_REQUEST_RESERVE_BYTES);
    let available_messages = MAX_MESSAGE_COUNT.saturating_sub(2);
    let recent = select_recent_segments(
        &segments[exact_start..],
        available_bytes,
        available_messages,
    );
    let mut messages = recent.into_iter().map(main_message).collect::<Vec<_>>();
    messages.push(NormalizedMessage {
        role: MessageRole::User,
        content: current.content,
    });
    NormalizedRequest::with_system(system, messages, profile.main_reply_requested_max)
        .map_err(AppError::from_provider_error)
}

fn incremental_plan(
    segments: &[HistorySegment],
    memory: &ConversationMemory,
    profile: ContextProfile,
) -> Option<CompactionPlan> {
    let (old_start, old_end) = locate_interval(
        segments,
        &memory.covered_from_cursor,
        &memory.covered_through_cursor,
    )?;
    let tail_start = recent_tail_start(
        segments,
        old_end.saturating_add(1),
        profile.protected_tail_bytes,
    );
    if tail_start <= old_end.saturating_add(1) {
        return None;
    }
    let forward = segments[old_end + 1..tail_start].to_vec();
    let backfill = if old_start == 0 {
        Vec::new()
    } else {
        select_backfill(&segments[..old_start], profile.backfill_input_bytes)
    };
    let input_bytes = memory
        .summary_text
        .len()
        .saturating_add(COMPACTOR_PROMPT_OVERHEAD_BYTES)
        .saturating_add(backfill.iter().map(rendered_compactor_bytes).sum::<usize>())
        .saturating_add(forward.iter().map(rendered_compactor_bytes).sum::<usize>());
    if input_bytes > profile.compactor_input_bytes || (old_start > 0 && backfill.is_empty()) {
        return None;
    }
    let new_start = if backfill.is_empty() {
        old_start
    } else {
        old_start - backfill.len()
    };
    let new_end = tail_start - 1;
    Some(CompactionPlan {
        backfill,
        previous_summary: Some(memory.summary_text.clone()),
        forward,
        covered_from_cursor: segments[new_start].start.clone(),
        covered_through_cursor: segments[new_end].end.clone(),
        source_hash: source_hash(&segments[new_start..=new_end]),
    })
}

fn recent_rebuild_plan(
    segments: &[HistorySegment],
    profile: ContextProfile,
) -> Option<CompactionPlan> {
    let tail_start = recent_tail_start(segments, 0, profile.protected_tail_bytes);
    if tail_start == 0 {
        return None;
    }
    let mut start = tail_start;
    let mut used = COMPACTOR_PROMPT_OVERHEAD_BYTES;
    while start > 0 {
        let candidate = &segments[start - 1];
        let bytes = rendered_compactor_bytes(candidate);
        if used.saturating_add(bytes) > profile.compactor_input_bytes {
            break;
        }
        used += bytes;
        start -= 1;
    }
    if start == tail_start {
        return None;
    }
    let forward = segments[start..tail_start].to_vec();
    Some(CompactionPlan {
        backfill: Vec::new(),
        previous_summary: None,
        forward,
        covered_from_cursor: segments[start].start.clone(),
        covered_through_cursor: segments[tail_start - 1].end.clone(),
        source_hash: source_hash(&segments[start..tail_start]),
    })
}

fn recent_tail_start(segments: &[HistorySegment], minimum: usize, budget: usize) -> usize {
    let mut start = segments.len();
    let mut used = 0usize;
    let mut count = 0usize;
    while start > minimum && count < MAX_MESSAGE_COUNT.saturating_sub(2) {
        let bytes = rendered_main_bytes(&segments[start - 1]);
        if used.saturating_add(bytes) > budget {
            break;
        }
        used += bytes;
        count += 1;
        start -= 1;
    }
    start
}

fn select_backfill(segments: &[HistorySegment], budget: usize) -> Vec<HistorySegment> {
    let mut start = segments.len();
    let mut used = 0usize;
    while start > 0 {
        let bytes = rendered_compactor_bytes(&segments[start - 1]);
        if used.saturating_add(bytes) > budget {
            break;
        }
        used += bytes;
        start -= 1;
    }
    segments[start..].to_vec()
}

fn select_recent_segments(
    segments: &[HistorySegment],
    byte_budget: usize,
    message_budget: usize,
) -> Vec<HistorySegment> {
    let mut start = segments.len();
    let mut used = 0usize;
    let mut count = 0usize;
    while start > 0 && count < message_budget {
        let bytes = rendered_main_bytes(&segments[start - 1]);
        if used.saturating_add(bytes) > byte_budget {
            break;
        }
        used += bytes;
        count += 1;
        start -= 1;
    }
    segments[start..].to_vec()
}

fn current_user_message(
    session: &ConversationSession,
) -> Result<crate::conversation::ConversationMessage, AppError> {
    let turn = session.turns.last().ok_or_else(memory_invalid)?;
    if turn.status != ConversationTurnStatus::Running || turn.messages.len() != 1 {
        return Err(memory_invalid());
    }
    let message = turn.messages.first().ok_or_else(memory_invalid)?;
    if message.role != ConversationRole::User {
        return Err(memory_invalid());
    }
    Ok(message.clone())
}

fn history_segments(
    session: &ConversationSession,
    maximum_segment_bytes: usize,
) -> Vec<HistorySegment> {
    let mut output = Vec::new();
    for turn in &session.turns {
        if turn.status == ConversationTurnStatus::Running {
            continue;
        }
        for message in &turn.messages {
            let Some(content) = semantic_content(turn, message.role, &message.content) else {
                continue;
            };
            let mut start = 0usize;
            while start < content.len() {
                let end = utf8_chunk_end(content, start, maximum_segment_bytes);
                if end <= start {
                    break;
                }
                output.push(HistorySegment {
                    start: MemoryCursor {
                        turn_id: turn.turn_id.clone(),
                        message_id: message.message_id.clone(),
                        utf8_byte_offset: start as u64,
                    },
                    end: MemoryCursor {
                        turn_id: turn.turn_id.clone(),
                        message_id: message.message_id.clone(),
                        utf8_byte_offset: end as u64,
                    },
                    role: message.role,
                    turn_status: turn.status,
                    content: content[start..end].to_owned(),
                });
                start = end;
            }
        }
    }
    output
}

fn semantic_content<'a>(
    turn: &ConversationTurn,
    role: ConversationRole,
    content: &'a str,
) -> Option<&'a str> {
    if role == ConversationRole::User {
        return Some(content);
    }
    match turn.status {
        ConversationTurnStatus::NotGenerated => None,
        ConversationTurnStatus::Failed | ConversationTurnStatus::Cancelled => {
            let actual = content
                .strip_suffix(INCOMPLETE_TEXT)
                .unwrap_or(content)
                .trim_end();
            (!actual.is_empty() && actual != NOT_GENERATED_TEXT).then_some(actual)
        }
        ConversationTurnStatus::Completed => Some(content),
        ConversationTurnStatus::Running => None,
    }
}

fn utf8_chunk_end(content: &str, start: usize, maximum_bytes: usize) -> usize {
    let mut end = start.saturating_add(maximum_bytes).min(content.len());
    while end > start && !content.is_char_boundary(end) {
        end -= 1;
    }
    if end == start {
        content[start..]
            .char_indices()
            .nth(1)
            .map_or(content.len(), |(offset, _)| start + offset)
    } else {
        end
    }
}

fn main_message(segment: HistorySegment) -> NormalizedMessage {
    let role = match segment.role {
        ConversationRole::User => MessageRole::User,
        ConversationRole::Assistant => MessageRole::Assistant,
    };
    let content = if segment.role == ConversationRole::Assistant
        && matches!(
            segment.turn_status,
            ConversationTurnStatus::Failed | ConversationTurnStatus::Cancelled
        ) {
        format!("【上一条回复未完成】\n{}", segment.content)
    } else {
        segment.content
    };
    NormalizedMessage { role, content }
}

fn render_compactor_segment(segment: &HistorySegment) -> String {
    let speaker = match segment.role {
        ConversationRole::User => "用户",
        ConversationRole::Assistant => "AI",
    };
    let state = match segment.turn_status {
        ConversationTurnStatus::Completed => "已完成",
        ConversationTurnStatus::Failed => "失败但有实际内容",
        ConversationTurnStatus::Cancelled => "已取消但有实际内容",
        ConversationTurnStatus::NotGenerated => "未生成",
        ConversationTurnStatus::Running => "运行中",
    };
    format!("{speaker}（{state}）：{}", segment.content)
}

fn rendered_compactor_bytes(segment: &HistorySegment) -> usize {
    render_compactor_segment(segment).len().saturating_add(64)
}

fn rendered_main_bytes(segment: &HistorySegment) -> usize {
    segment.content.len().saturating_add(64)
}

fn locate_interval(
    segments: &[HistorySegment],
    from: &MemoryCursor,
    through: &MemoryCursor,
) -> Option<(usize, usize)> {
    let start = segments.iter().position(|segment| &segment.start == from)?;
    let end = segments
        .iter()
        .position(|segment| &segment.end == through)?;
    (start <= end).then_some((start, end))
}

fn source_hash(segments: &[HistorySegment]) -> String {
    let mut hasher = Sha256::new();
    for segment in segments {
        update_hash_field(&mut hasher, segment.start.turn_id.as_bytes());
        update_hash_field(&mut hasher, segment.start.message_id.as_bytes());
        hasher.update(segment.start.utf8_byte_offset.to_le_bytes());
        hasher.update(segment.end.utf8_byte_offset.to_le_bytes());
        hasher.update([match segment.role {
            ConversationRole::User => 0,
            ConversationRole::Assistant => 1,
        }]);
        hasher.update([match segment.turn_status {
            ConversationTurnStatus::Running => 0,
            ConversationTurnStatus::Completed => 1,
            ConversationTurnStatus::Cancelled => 2,
            ConversationTurnStatus::Failed => 3,
            ConversationTurnStatus::NotGenerated => 4,
        }]);
        update_hash_field(&mut hasher, segment.content.as_bytes());
    }
    let mut output = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn update_hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn validate_memory(
    handle: &ProjectHandle,
    session: &ConversationSession,
    memory: &ConversationMemory,
    profile: ContextProfile,
) -> bool {
    memory.project_id == handle.project_id()
        && validate_memory_for_session(
            session,
            memory,
            &history_segments(session, profile.segment_content_bytes),
            profile,
        )
}

fn validate_memory_for_session(
    session: &ConversationSession,
    memory: &ConversationMemory,
    segments: &[HistorySegment],
    profile: ContextProfile,
) -> bool {
    if memory.schema_version != MEMORY_SCHEMA_VERSION
        || memory.conversation_session_id != session.session_id
        || memory.updated_at_ms == 0
        || !is_sha256(&memory.source_hash)
        || validate_summary(&memory.summary_text, profile).is_err()
    {
        return false;
    }
    let Some((start, end)) = locate_interval(
        segments,
        &memory.covered_from_cursor,
        &memory.covered_through_cursor,
    ) else {
        return false;
    };
    source_hash(&segments[start..=end]) == memory.source_hash
}

fn validate_summary(summary: &str, profile: ContextProfile) -> Result<(), AppError> {
    if summary.len() > profile.summary_bytes {
        return Err(memory_invalid());
    }
    let normalized = summary.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() || trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Err(memory_invalid());
    }

    let lines = trimmed.lines().collect::<Vec<_>>();
    if lines.first().copied() != Some(MEMORY_HEADINGS[0])
        || lines.iter().any(|line| {
            let candidate = line.trim_start();
            candidate.starts_with("\x60\x60\x60") || candidate.starts_with("~~~")
        })
    {
        return Err(memory_invalid());
    }

    let mut positions = [0usize; MEMORY_HEADINGS.len()];
    for (heading_index, heading) in MEMORY_HEADINGS.iter().enumerate() {
        let matches = lines
            .iter()
            .enumerate()
            .filter_map(|(line_index, line)| (*line == *heading).then_some(line_index))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(memory_invalid());
        }
        positions[heading_index] = matches[0];
    }
    if positions[0] != 0 || positions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(memory_invalid());
    }
    for (index, start) in positions.iter().copied().enumerate() {
        let end = positions.get(index + 1).copied().unwrap_or(lines.len());
        if lines[start + 1..end]
            .iter()
            .all(|line| line.trim().is_empty())
        {
            return Err(memory_invalid());
        }
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn write_memory(handle: &ProjectHandle, memory: &ConversationMemory) -> Result<(), AppError> {
    let target = ensure_memory_file_path(handle)?;
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(memory_boundary());
        }
    }
    let bytes = serde_json::to_vec_pretty(memory).map_err(|_| memory_invalid())?;
    if bytes.len() as u64 > MAX_MEMORY_STORE_BYTES {
        return Err(memory_invalid());
    }
    let parent = target.parent().ok_or_else(memory_boundary)?;
    let temporary = parent.join(format!(".state-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| memory_unavailable())?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|_| memory_unavailable())?;
        drop(file);
        atomic_replace(&temporary, &target).map_err(|_| memory_unavailable())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn memory_file_path(handle: &ProjectHandle) -> Result<PathBuf, AppError> {
    let root = handle.root().join(STORE_DIRECTORY);
    let metadata = fs::symlink_metadata(&root).map_err(|_| memory_unavailable())?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(memory_boundary());
    }
    Ok(root.join(STORE_FILE))
}

fn ensure_memory_file_path(handle: &ProjectHandle) -> Result<PathBuf, AppError> {
    let state_root = handle.root().join(".novelgainker");
    ensure_plain_directory(&state_root)?;
    let memory_root = handle.root().join(STORE_DIRECTORY);
    ensure_plain_directory(&memory_root)?;
    let canonical_project = handle
        .root()
        .canonicalize()
        .map_err(|_| memory_boundary())?;
    let canonical_memory = memory_root.canonicalize().map_err(|_| memory_boundary())?;
    if !canonical_memory.starts_with(&canonical_project) {
        return Err(memory_boundary());
    }
    Ok(memory_root.join(STORE_FILE))
}

fn ensure_plain_directory(path: &Path) -> Result<(), AppError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => Ok(()),
        Ok(_) => Err(memory_boundary()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|_| memory_unavailable())?;
            let metadata = fs::symlink_metadata(path).map_err(|_| memory_unavailable())?;
            if metadata.is_dir() && !is_link_or_reparse(&metadata) {
                Ok(())
            } else {
                Err(memory_boundary())
            }
        }
        Err(_) => Err(memory_unavailable()),
    }
}

fn memory_invalid() -> AppError {
    AppError::runtime_unavailable().with_diagnostic("memory-state-invalid")
}

fn memory_unavailable() -> AppError {
    AppError::runtime_unavailable().with_diagnostic("memory-state-unavailable")
}

fn memory_boundary() -> AppError {
    AppError::runtime_unavailable().with_diagnostic("memory-state-boundary")
}

#[cfg(test)]
mod tests {
    use super::{
        assemble_main_request, commit_compaction, history_segments, load_valid_memory,
        locate_interval, plan_compaction, rendered_main_bytes, select_recent_segments, source_hash,
        validate_context_profile, validate_summary, ContextProfile, COMPACTOR_SYSTEM_INSTRUCTION,
        MEMORY_HEADINGS, NR1_CONTEXT_PROFILE,
    };
    use crate::conversation::{
        finish_conversation_turn, read_conversation, start_conversation_turn, ConversationRole,
        ConversationTerminalKind, INCOMPLETE_TEXT, NOT_GENERATED_TEXT,
    };
    use crate::credential_vault::ProviderId;
    use crate::project::{inspect_project_root, open_project, ProjectScanMode};
    use crate::provider_core::{MAX_MESSAGE_COUNT, MAX_REQUEST_BYTES};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct Fixture {
        root: PathBuf,
        handle: crate::project::ProjectHandle,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("novelgainker-nr101-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("project.md"), "# 小说\n").unwrap();
            let inspection = inspect_project_root(&root, ProjectScanMode::Open).unwrap();
            let handle = open_project(&inspection, true).unwrap();
            Self { root, handle }
        }

        fn complete_turn(&self, user: &str, assistant: &str, now: u64) {
            let started = start_conversation_turn(
                &self.handle,
                ProviderId::Openai,
                "gpt-5.6-terra",
                user,
                now,
            )
            .unwrap();
            finish_conversation_turn(
                &self.handle,
                &started.turn_id,
                assistant,
                ConversationTerminalKind::Completed,
                None,
                now + 1,
            )
            .unwrap();
        }

        fn start_turn(&self, user: &str, now: u64) {
            start_conversation_turn(&self.handle, ProviderId::Openai, "gpt-5.6-terra", user, now)
                .unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn small_profile() -> ContextProfile {
        ContextProfile {
            trigger_bytes: 900,
            protected_tail_bytes: 240,
            segment_content_bytes: 128,
            compactor_input_bytes: 8_192,
            backfill_input_bytes: 256,
            summary_bytes: 1_024,
            compactor_requested_max: 128,
            main_reply_requested_max: 512,
            cumulative_requested_output_max: 640,
        }
    }

    fn valid_summary(label: &str) -> String {
        format!(
            "{}\n- {label}\n{}\n- 正在讨论\n{}\n- 无\n{}\n- 继续\n{}\n- 无",
            MEMORY_HEADINGS[0],
            MEMORY_HEADINGS[1],
            MEMORY_HEADINGS[2],
            MEMORY_HEADINGS[3],
            MEMORY_HEADINGS[4],
        )
    }

    #[test]
    fn summary_validation_enforces_exact_ordered_unique_nonempty_sections() {
        let profile = small_profile();
        let valid = valid_summary("已确认");
        validate_summary(&valid, profile).unwrap();
        validate_summary(&valid.replace('\n', "\r\n"), profile).unwrap();
        validate_summary(&valid.replace('\n', "\r"), profile).unwrap();

        let structured_term = ["Control", "JSON"].join(" ");
        let prose_with_structured_words = format!(
            "{}\n- 决策正文包含 JSON 片段 {{\"mode\":\"draft\"}}\n{}\n- 正文提到“已确认决定”和 {structured_term}，但它们不是标题行\n{}\n- 数组示例 [1, 2]\n{}\n- 继续\n{}\n- 无",
            MEMORY_HEADINGS[0],
            MEMORY_HEADINGS[1],
            MEMORY_HEADINGS[2],
            MEMORY_HEADINGS[3],
            MEMORY_HEADINGS[4],
        );
        validate_summary(&prose_with_structured_words, profile).unwrap();

        for heading in MEMORY_HEADINGS {
            assert_eq!(
                COMPACTOR_SYSTEM_INSTRUCTION
                    .lines()
                    .filter(|line| *line == heading)
                    .count(),
                1,
                "Compactor prompt must contain each exact heading line once"
            );
        }

        let missing_heading = valid.replacen(&format!("{}\n- 无\n", MEMORY_HEADINGS[2]), "", 1);
        let duplicate_heading = format!("{valid}\n{}\n- 重复", MEMORY_HEADINGS[0]);
        let out_of_order = format!(
            "{}\n- 已确认\n{}\n- 顺序错误\n{}\n- 正在讨论\n{}\n- 继续\n{}\n- 无",
            MEMORY_HEADINGS[0],
            MEMORY_HEADINGS[2],
            MEMORY_HEADINGS[1],
            MEMORY_HEADINGS[3],
            MEMORY_HEADINGS[4],
        );
        let leading_text = format!("以下是摘要\n{valid}");
        let empty_section = format!(
            "{}\n{}\n- 正在讨论\n{}\n- 无\n{}\n- 继续\n{}\n- 无",
            MEMORY_HEADINGS[0],
            MEMORY_HEADINGS[1],
            MEMORY_HEADINGS[2],
            MEMORY_HEADINGS[3],
            MEMORY_HEADINGS[4],
        );
        let embedded_only = valid.replacen(
            MEMORY_HEADINGS[2],
            "正文提到了未决问题，但这一整行不是标题",
            1,
        );
        let fence = char::from(96);
        let backtick_fence = valid.replacen(
            "- 正在讨论",
            &format!("{fence}{fence}{fence}text\n内容\n{fence}{fence}{fence}"),
            1,
        );
        let tilde_fence = valid.replacen("- 正在讨论", "~~~text\n内容\n~~~", 1);
        let oversized = valid_summary(&"甲".repeat(profile.summary_bytes));

        for invalid in [
            String::new(),
            "   \r\n".to_owned(),
            r#"{"type":"control","sections":["已确认决定"]}"#.to_owned(),
            r#"[{"heading":"已确认决定"}]"#.to_owned(),
            missing_heading,
            duplicate_heading,
            out_of_order,
            leading_text,
            empty_section,
            embedded_only,
            backtick_fence,
            tilde_fence,
            oversized,
        ] {
            assert!(
                validate_summary(&invalid, profile).is_err(),
                "invalid summary unexpectedly passed: {invalid:?}"
            );
        }
    }

    #[test]
    fn invalid_summary_never_creates_or_replaces_memory() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fixture.complete_turn(
                &format!("用户决定 {index} {}", "甲".repeat(40)),
                &format!("回复 {index} {}", "乙".repeat(40)),
                10 + index * 10,
            );
        }
        fixture.start_turn("继续", 200);
        let session = read_conversation(&fixture.handle).unwrap();
        let plan = plan_compaction(&session, None, small_profile())
            .unwrap()
            .expect("history should compact");
        let memory_path = fixture.root.join(".novelgainker/memory/state.json");

        assert!(commit_compaction(
            &fixture.handle,
            &session,
            &plan,
            r#"{"control":"not-natural-language"}"#,
            201,
            small_profile(),
        )
        .is_err());
        assert!(!memory_path.exists());

        commit_compaction(
            &fixture.handle,
            &session,
            &plan,
            &valid_summary("有效摘要"),
            202,
            small_profile(),
        )
        .unwrap();
        let memory_before = fs::read(&memory_path).unwrap();
        assert!(commit_compaction(
            &fixture.handle,
            &session,
            &plan,
            "已确认决定\n- 只有一个栏目",
            203,
            small_profile(),
        )
        .is_err());
        assert_eq!(fs::read(memory_path).unwrap(), memory_before);
    }

    #[test]
    fn frozen_profile_is_locally_enforceable() {
        validate_context_profile(NR1_CONTEXT_PROFILE).unwrap();
        assert_eq!(
            NR1_CONTEXT_PROFILE.compactor_requested_max
                + NR1_CONTEXT_PROFILE.main_reply_requested_max,
            NR1_CONTEXT_PROFILE.cumulative_requested_output_max
        );
    }

    #[test]
    fn memory_round_trip_preserves_history_and_markdown_bytes() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fixture.complete_turn(
                &format!("用户决定 {index} {}", "甲".repeat(40)),
                &format!("回复 {index} {}", "乙".repeat(40)),
                10 + index * 10,
            );
        }
        fixture.start_turn("继续", 200);
        let session = read_conversation(&fixture.handle).unwrap();
        let conversation_before =
            fs::read(fixture.root.join(".novelgainker/conversation/session.json")).unwrap();
        let markdown_before = fs::read(fixture.root.join("project.md")).unwrap();
        let plan = plan_compaction(&session, None, small_profile())
            .unwrap()
            .expect("long history should compact");
        let memory = commit_compaction(
            &fixture.handle,
            &session,
            &plan,
            &valid_summary("已确认"),
            300,
            small_profile(),
        )
        .unwrap();
        assert_eq!(
            load_valid_memory(&fixture.handle, &session, small_profile()),
            Some(memory)
        );
        assert_eq!(
            fs::read(fixture.root.join(".novelgainker/conversation/session.json")).unwrap(),
            conversation_before
        );
        assert_eq!(
            fs::read(fixture.root.join("project.md")).unwrap(),
            markdown_before
        );
    }

    #[test]
    fn corrupt_or_mismatched_memory_is_a_cache_miss() {
        let fixture = Fixture::new();
        fixture.complete_turn("决定", &"内容".repeat(200), 10);
        fixture.start_turn("继续", 30);
        let session = read_conversation(&fixture.handle).unwrap();
        let plan = plan_compaction(&session, None, small_profile())
            .unwrap()
            .expect("history should compact");
        let memory = commit_compaction(
            &fixture.handle,
            &session,
            &plan,
            &valid_summary("决定"),
            40,
            small_profile(),
        )
        .unwrap();
        let mut invalid_memory = memory;
        invalid_memory.source_hash = "0".repeat(64);
        assert!(
            plan_compaction(&session, Some(&invalid_memory), small_profile())
                .unwrap()
                .is_some(),
            "invalid Memory must continue to use recent rebuild"
        );
        let path = fixture.root.join(".novelgainker/memory/state.json");
        fs::write(&path, b"not-json").unwrap();
        assert!(load_valid_memory(&fixture.handle, &session, small_profile()).is_none());
    }

    #[test]
    fn oversized_assistant_messages_are_segmented_without_changing_history() {
        let fixture = Fixture::new();
        let long_text = "长".repeat(18_000);
        fixture.complete_turn("生成长回复", &long_text, 10);
        fixture.start_turn("继续", 30);
        let session = read_conversation(&fixture.handle).unwrap();
        let segments = history_segments(&session, NR1_CONTEXT_PROFILE.segment_content_bytes);
        let assistant_segments = segments
            .iter()
            .filter(|segment| segment.content.starts_with('长'))
            .collect::<Vec<_>>();
        assert!(assistant_segments.len() > 1);
        assert!(assistant_segments
            .iter()
            .all(|segment| segment.content.len() <= NR1_CONTEXT_PROFILE.segment_content_bytes));
        assert_eq!(session.turns[0].messages[1].content, long_text);
    }

    #[test]
    fn main_request_keeps_current_user_exactly_once() {
        let fixture = Fixture::new();
        fixture.complete_turn("前文", "已有内容", 10);
        fixture.start_turn("唯一当前消息", 30);
        let session = read_conversation(&fixture.handle).unwrap();
        let request = assemble_main_request(&session, None, NR1_CONTEXT_PROFILE).unwrap();
        assert_eq!(
            request
                .messages
                .iter()
                .filter(|message| message.content == "唯一当前消息")
                .count(),
            1
        );
        assert_eq!(request.messages.last().unwrap().content, "唯一当前消息");
    }
    #[test]
    fn threshold_is_byte_based_and_does_not_use_story_keywords() {
        let fixture = Fixture::new();
        fixture.complete_turn("普通闲聊", &"没有小说关键词".repeat(50), 10);
        fixture.start_turn("继续聊", 30);
        let session = read_conversation(&fixture.handle).unwrap();
        let mut below = small_profile();
        below.trigger_bytes = 32 * 1024;
        assert!(plan_compaction(&session, None, below).unwrap().is_none());
        assert!(plan_compaction(&session, None, small_profile())
            .unwrap()
            .is_some());
    }

    #[test]
    fn failed_cancelled_and_not_generated_turns_have_exact_semantic_inputs() {
        let fixture = Fixture::new();
        let failed = start_conversation_turn(
            &fixture.handle,
            ProviderId::Openai,
            "gpt-5.6-terra",
            "失败请求",
            10,
        )
        .unwrap();
        finish_conversation_turn(
            &fixture.handle,
            &failed.turn_id,
            "真实半段",
            ConversationTerminalKind::Failed,
            Some("provider.protocol"),
            11,
        )
        .unwrap();
        let cancelled = start_conversation_turn(
            &fixture.handle,
            ProviderId::Openai,
            "gpt-5.6-terra",
            "取消请求",
            20,
        )
        .unwrap();
        finish_conversation_turn(
            &fixture.handle,
            &cancelled.turn_id,
            "取消前内容",
            ConversationTerminalKind::Cancelled,
            Some("provider.cancelled"),
            21,
        )
        .unwrap();
        let empty = start_conversation_turn(
            &fixture.handle,
            ProviderId::Openai,
            "gpt-5.6-terra",
            "零文本请求",
            30,
        )
        .unwrap();
        finish_conversation_turn(
            &fixture.handle,
            &empty.turn_id,
            "",
            ConversationTerminalKind::Failed,
            Some("provider.protocol"),
            31,
        )
        .unwrap();
        fixture.start_turn("当前请求", 40);
        let session = read_conversation(&fixture.handle).unwrap();
        let rendered = history_segments(&session, 1024)
            .into_iter()
            .map(|segment| segment.content)
            .collect::<Vec<_>>();
        for expected in [
            "失败请求",
            "真实半段",
            "取消请求",
            "取消前内容",
            "零文本请求",
        ] {
            assert!(rendered.iter().any(|content| content == expected));
        }
        assert!(!rendered.iter().any(|content| content == NOT_GENERATED_TEXT));
        assert!(!rendered
            .iter()
            .any(|content| content.contains(INCOMPLETE_TEXT)));
        assert!(!rendered.iter().any(|content| content == "当前请求"));
    }

    #[test]
    fn repeated_normal_compaction_backfills_at_least_one_segment_without_gaps() {
        let fixture = Fixture::new();
        for index in 0..60 {
            fixture.complete_turn(
                &format!("早期决定 {index} {}", "甲".repeat(40)),
                &format!("早期回复 {index} {}", "乙".repeat(40)),
                10 + index * 10,
            );
        }
        fixture.start_turn("开始近期重建", 700);
        let session = read_conversation(&fixture.handle).unwrap();
        let first_plan = plan_compaction(&session, None, small_profile())
            .unwrap()
            .expect("recent rebuild must be available");
        let first_memory = commit_compaction(
            &fixture.handle,
            &session,
            &first_plan,
            &valid_summary("近期重建"),
            701,
            small_profile(),
        )
        .unwrap();
        let first_segments = history_segments(&session, small_profile().segment_content_bytes);
        let (first_start, first_end) = locate_interval(
            &first_segments,
            &first_memory.covered_from_cursor,
            &first_memory.covered_through_cursor,
        )
        .unwrap();
        assert!(first_start > 0, "fixture must retain an older backlog");
        assert!(first_end + 1 < first_segments.len());

        let memory_path = fixture.root.join(".novelgainker/memory/state.json");
        let memory_before_no_forward = fs::read(&memory_path).unwrap();
        let mut no_forward_profile = small_profile();
        no_forward_profile.trigger_bytes = 1;
        no_forward_profile.protected_tail_bytes = 64 * 1024;
        assert!(
            plan_compaction(&session, Some(&first_memory), no_forward_profile)
                .unwrap()
                .is_none(),
            "valid Memory without compressible forward input must not rebuild"
        );
        assert_eq!(fs::read(&memory_path).unwrap(), memory_before_no_forward);

        finish_conversation_turn(
            &fixture.handle,
            session.turns.last().unwrap().turn_id.as_str(),
            "近期回复",
            ConversationTerminalKind::Completed,
            None,
            702,
        )
        .unwrap();
        for index in 0..8 {
            fixture.complete_turn(
                &format!("新增决定 {index} {}", "丙".repeat(30)),
                &format!("新增回复 {index} {}", "丁".repeat(30)),
                710 + index * 10,
            );
        }
        fixture.start_turn("再次触发", 900);
        let next_session = read_conversation(&fixture.handle).unwrap();
        let memory_before_budget_block = fs::read(&memory_path).unwrap();
        let mut insufficient_backfill_profile = small_profile();
        insufficient_backfill_profile.trigger_bytes = 1;
        insufficient_backfill_profile.backfill_input_bytes =
            insufficient_backfill_profile.segment_content_bytes;
        assert!(
            plan_compaction(
                &next_session,
                Some(&first_memory),
                insufficient_backfill_profile,
            )
            .unwrap()
            .is_none(),
            "valid Memory without minimum Backfill budget must not rebuild"
        );
        assert_eq!(fs::read(&memory_path).unwrap(), memory_before_budget_block);

        let next_plan = plan_compaction(&next_session, Some(&first_memory), small_profile())
            .unwrap()
            .expect("incremental compaction must be available");
        assert!(!next_plan.backfill.is_empty());
        assert!(!next_plan.forward.is_empty());
        let next_memory = commit_compaction(
            &fixture.handle,
            &next_session,
            &next_plan,
            &valid_summary("增量与回填"),
            901,
            small_profile(),
        )
        .unwrap();
        let next_segments = history_segments(&next_session, small_profile().segment_content_bytes);
        let (next_start, next_end) = locate_interval(
            &next_segments,
            &next_memory.covered_from_cursor,
            &next_memory.covered_through_cursor,
        )
        .unwrap();
        assert!(next_start < first_start);
        assert!(next_end > first_end);
        assert_eq!(
            source_hash(&next_segments[next_start..=next_end]),
            next_memory.source_hash
        );
    }

    #[test]
    fn ten_thousand_visible_messages_keep_provider_input_bounded() {
        use crate::conversation::{
            ConversationMessage, ConversationSession, ConversationTurn, ConversationTurnStatus,
        };

        let mut session = ConversationSession {
            schema_version: 1,
            project_id: "project-scale".to_owned(),
            session_id: "session-scale".to_owned(),
            turns: Vec::with_capacity(5_000),
        };
        for index in 0..5_000u64 {
            session.turns.push(ConversationTurn {
                turn_id: format!("turn-{index:05}"),
                provider_id: ProviderId::Openai,
                model: "gpt-5.6-terra".to_owned(),
                status: ConversationTurnStatus::Completed,
                started_at_ms: index * 2 + 1,
                updated_at_ms: index * 2 + 2,
                messages: vec![
                    ConversationMessage {
                        message_id: format!("user-{index:05}"),
                        role: ConversationRole::User,
                        content: format!("用户消息 {index:05}"),
                        created_at_ms: index * 2 + 1,
                    },
                    ConversationMessage {
                        message_id: format!("assistant-{index:05}"),
                        role: ConversationRole::Assistant,
                        content: format!("助手回复 {index:05}"),
                        created_at_ms: index * 2 + 2,
                    },
                ],
                error_code: None,
            });
        }
        let segments = history_segments(&session, NR1_CONTEXT_PROFILE.segment_content_bytes);
        assert_eq!(segments.len(), 10_000);
        let selected = select_recent_segments(
            &segments,
            MAX_REQUEST_BYTES - 2 * 1024,
            MAX_MESSAGE_COUNT - 2,
        );
        assert!(selected.len() <= MAX_MESSAGE_COUNT - 2);
        assert!(selected.len() < segments.len());
        assert!(selected.iter().map(rendered_main_bytes).sum::<usize>() < MAX_REQUEST_BYTES);
    }
}
