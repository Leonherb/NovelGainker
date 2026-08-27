use crate::chapter::{ChapterLedgerCandidateV1, ChapterProposalV1};
use crate::chapter_patch::{
    ChapterDiffHunkV1, ChapterDiffLineKindV1, ChapterDiffLineV1, ChapterDiffSummaryV1,
};
use crate::story_context::sha256_hex;
use novelgainker_validation::validate_source;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use uuid::Uuid;

pub const CHAPTER_LIFECYCLE_SCHEMA_VERSION: u16 = 1;
pub const STORY_LEDGER_RELATIVE_PATH: &str = "materials/000.story-ledger.md";
const PENDING_HEADING: &str = "## Ledger Pending";
const PENDING_PREFIX: &str = "> novelgainker-ledger-pending-v1: ";
const APPLIED_PREFIX: &str = "<!-- novelgainker-ledger-applied-v1:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChapterLifecycleStatusV1 {
    Draft,
    Accepted,
    Final,
}

impl ChapterLifecycleStatusV1 {
    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Accepted => "accepted",
            Self::Final => "final",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerPendingRecordV1 {
    pub schema_version: u16,
    pub pending_id: String,
    pub chapter_id: String,
    pub chapter_number: u32,
    pub chapter_title: String,
    pub chapter_relative_path: String,
    pub accepted_status: ChapterLifecycleStatusV1,
    pub source_chapter_hash: String,
    pub proposal_hash: String,
    pub source_manifest_hash: String,
    pub request_hash: String,
    pub candidates: Vec<ChapterLedgerCandidateV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LedgerPendingResolutionV1 {
    Pending,
    Applied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerPendingStatusV1 {
    pub record: LedgerPendingRecordV1,
    pub resolution: LedgerPendingResolutionV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterStatusPreviewV1 {
    pub schema_version: u16,
    pub preview_hash: String,
    pub target_relative_path: String,
    pub expected_source_hash: String,
    pub proposed_content_hash: String,
    pub previous_status: ChapterLifecycleStatusV1,
    pub next_status: ChapterLifecycleStatusV1,
    pub created_pending_id: Option<String>,
    pub hunks: Vec<ChapterDiffHunkV1>,
    pub summary: ChapterDiffSummaryV1,
    pub ready_to_apply: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChapterStatusDocumentV1 {
    pub preview: ChapterStatusPreviewV1,
    pub target_content: String,
}

impl fmt::Debug for ChapterStatusDocumentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChapterStatusDocumentV1")
            .field("preview", &self.preview)
            .field("target_content", &"[REDACTED]")
            .finish()
    }
}

pub struct BuildChapterStatusInputV1<'a> {
    pub target_relative_path: &'a str,
    pub current_content: &'a str,
    pub current_content_hash: &'a str,
    pub current_content_has_utf8_bom: bool,
    pub next_status: ChapterLifecycleStatusV1,
    pub proposal: Option<&'a ChapterProposalV1>,
    pub pending_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerUpdatePreviewV1 {
    pub schema_version: u16,
    pub preview_hash: String,
    pub target_relative_path: String,
    pub chapter_relative_path: String,
    pub expected_source_hash: String,
    pub proposed_content_hash: String,
    pub chapter_content_hash: String,
    pub pending_ids: Vec<String>,
    pub hunks: Vec<ChapterDiffHunkV1>,
    pub summary: ChapterDiffSummaryV1,
    pub ready_to_apply: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct LedgerUpdateDocumentV1 {
    pub preview: LedgerUpdatePreviewV1,
    pub target_content: String,
}

impl fmt::Debug for LedgerUpdateDocumentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LedgerUpdateDocumentV1")
            .field("preview", &self.preview)
            .field("target_content", &"[REDACTED]")
            .finish()
    }
}

pub struct BuildLedgerUpdateInputV1<'a> {
    pub chapter_relative_path: &'a str,
    pub chapter_content: &'a str,
    pub chapter_content_hash: &'a str,
    pub chapter_content_has_utf8_bom: bool,
    pub ledger_content: &'a str,
    pub ledger_content_hash: &'a str,
    pub ledger_content_has_utf8_bom: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterLifecycleError {
    InvalidTarget,
    InvalidChapter,
    InvalidLedger,
    SourceHashMismatch,
    TransitionForbidden,
    ProposalRequired,
    ProposalBindingMismatch,
    PendingInvalid,
    NoPending,
    InvalidHash,
}

impl ChapterLifecycleError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidTarget => "chapter.lifecycle_target_invalid",
            Self::InvalidChapter => "chapter.lifecycle_source_invalid",
            Self::InvalidLedger => "ledger.source_invalid",
            Self::SourceHashMismatch => "chapter.lifecycle_source_hash_mismatch",
            Self::TransitionForbidden => "chapter.lifecycle_transition_forbidden",
            Self::ProposalRequired => "chapter.lifecycle_proposal_required",
            Self::ProposalBindingMismatch => "chapter.lifecycle_proposal_binding_mismatch",
            Self::PendingInvalid => "ledger.pending_invalid",
            Self::NoPending => "ledger.no_pending_update",
            Self::InvalidHash => "chapter.lifecycle_hash_invalid",
        }
    }
}

#[derive(Debug)]
struct ParsedChapter {
    fields: BTreeMap<String, (String, usize)>,
    lines: Vec<String>,
    trailing_newline: bool,
}

pub fn build_chapter_status_transition(
    input: BuildChapterStatusInputV1<'_>,
) -> Result<ChapterStatusDocumentV1, ChapterLifecycleError> {
    validate_chapter_path(input.target_relative_path)?;
    validate_bound_content(
        input.current_content,
        input.current_content_hash,
        input.current_content_has_utf8_bom,
    )?;
    let parsed = parse_chapter(input.target_relative_path, input.current_content)?;
    let previous_status = parse_status(&scalar(&parsed, "status")?)?;
    let existing_pending = bound_pending_records(&parsed, input.target_relative_path)?;
    if !matches!(
        (previous_status, input.next_status),
        (
            ChapterLifecycleStatusV1::Draft,
            ChapterLifecycleStatusV1::Accepted
        ) | (
            ChapterLifecycleStatusV1::Accepted,
            ChapterLifecycleStatusV1::Final
        )
    ) {
        return Err(ChapterLifecycleError::TransitionForbidden);
    }

    let mut revised = parsed.lines.clone();
    let status_index = parsed.fields["status"].1;
    let old_status = revised[status_index].clone();
    revised[status_index] = format!("status: {}", input.next_status.as_str());
    let mut hunks = vec![replacement_hunk(
        status_index,
        &[old_status],
        &[revised[status_index].clone()],
    )];
    let mut created_pending_id = None;

    if previous_status == ChapterLifecycleStatusV1::Draft {
        let proposal = input
            .proposal
            .ok_or(ChapterLifecycleError::ProposalRequired)?;
        let chapter_number = scalar(&parsed, "chapterNumber")?
            .parse::<u32>()
            .map_err(|_| ChapterLifecycleError::InvalidChapter)?;
        if proposal.target_chapter.relative_path != input.target_relative_path
            || proposal.target_chapter.chapter_number != chapter_number
        {
            return Err(ChapterLifecycleError::ProposalBindingMismatch);
        }
        let mut records = existing_pending;
        let pending_id = format!(
            "ledger_pending_{}",
            input.pending_id.unwrap_or_else(Uuid::new_v4)
        );
        if records.iter().any(|record| record.pending_id == pending_id) {
            return Err(ChapterLifecycleError::PendingInvalid);
        }
        records.push(LedgerPendingRecordV1 {
            schema_version: CHAPTER_LIFECYCLE_SCHEMA_VERSION,
            pending_id: pending_id.clone(),
            chapter_id: scalar(&parsed, "id")?,
            chapter_number,
            chapter_title: scalar(&parsed, "title")?,
            chapter_relative_path: input.target_relative_path.to_owned(),
            accepted_status: ChapterLifecycleStatusV1::Accepted,
            source_chapter_hash: input.current_content_hash.to_owned(),
            proposal_hash: proposal.proposal_hash.clone(),
            source_manifest_hash: proposal.source_manifest_hash.clone(),
            request_hash: proposal.request_hash.clone(),
            candidates: proposal.ledger_candidates.clone(),
        });
        let new_lines = render_pending(&records)?
            .split('\n')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let (start, old_lines) = pending_range(&parsed)?;
        if old_lines.is_empty() {
            if revised.last().is_some_and(|line| !line.is_empty()) {
                revised.push(String::new());
            }
            let insert_at = revised.len();
            revised.extend(new_lines.clone());
            hunks.push(replacement_hunk(insert_at, &[], &new_lines));
        } else {
            revised.splice(start..start + old_lines.len(), new_lines.clone());
            hunks.push(replacement_hunk(start, &old_lines, &new_lines));
        }
        created_pending_id = Some(pending_id);
    } else if existing_pending.is_empty() {
        return Err(ChapterLifecycleError::NoPending);
    }

    let mut target_content = revised.join("\n");
    if parsed.trailing_newline {
        target_content.push('\n');
    }
    if !validate_source(input.target_relative_path, &target_content)
        .diagnostics
        .is_empty()
    {
        return Err(ChapterLifecycleError::InvalidChapter);
    }
    let summary = summarize(&hunks);
    let mut preview = ChapterStatusPreviewV1 {
        schema_version: CHAPTER_LIFECYCLE_SCHEMA_VERSION,
        preview_hash: String::new(),
        target_relative_path: input.target_relative_path.to_owned(),
        expected_source_hash: input.current_content_hash.to_owned(),
        proposed_content_hash: sha256_hex(target_content.as_bytes()),
        previous_status,
        next_status: input.next_status,
        created_pending_id,
        hunks,
        summary,
        ready_to_apply: true,
    };
    preview.preview_hash = status_preview_hash(&preview)?;
    Ok(ChapterStatusDocumentV1 {
        preview,
        target_content,
    })
}

pub fn bind_chapter_status_encoded_hash(
    document: &mut ChapterStatusDocumentV1,
    encoded_hash: &str,
) -> Result<(), ChapterLifecycleError> {
    validate_hash(encoded_hash)?;
    document.preview.proposed_content_hash = encoded_hash.to_owned();
    document.preview.preview_hash = status_preview_hash(&document.preview)?;
    Ok(())
}

pub fn load_ledger_pending_status(
    chapter_path: &str,
    chapter_content: &str,
    ledger_content: &str,
) -> Result<Vec<LedgerPendingStatusV1>, ChapterLifecycleError> {
    let chapter = parse_chapter(chapter_path, chapter_content)?;
    validate_ledger(ledger_content)?;
    let applied = applied_ids(ledger_content)?;
    Ok(bound_pending_records(&chapter, chapter_path)?
        .into_iter()
        .map(|record| LedgerPendingStatusV1 {
            resolution: if applied.contains(&record.pending_id) {
                LedgerPendingResolutionV1::Applied
            } else {
                LedgerPendingResolutionV1::Pending
            },
            record,
        })
        .collect())
}

pub fn load_chapter_lifecycle_status(
    chapter_path: &str,
    chapter_content: &str,
) -> Result<ChapterLifecycleStatusV1, ChapterLifecycleError> {
    validate_chapter_path(chapter_path)?;
    let chapter = parse_chapter(chapter_path, chapter_content)?;
    parse_status(&scalar(&chapter, "status")?)
}

pub fn build_ledger_update(
    input: BuildLedgerUpdateInputV1<'_>,
) -> Result<LedgerUpdateDocumentV1, ChapterLifecycleError> {
    validate_chapter_path(input.chapter_relative_path)?;
    validate_bound_content(
        input.chapter_content,
        input.chapter_content_hash,
        input.chapter_content_has_utf8_bom,
    )?;
    validate_bound_content(
        input.ledger_content,
        input.ledger_content_hash,
        input.ledger_content_has_utf8_bom,
    )?;
    let chapter = parse_chapter(input.chapter_relative_path, input.chapter_content)?;
    if !matches!(
        parse_status(&scalar(&chapter, "status")?)?,
        ChapterLifecycleStatusV1::Accepted | ChapterLifecycleStatusV1::Final
    ) {
        return Err(ChapterLifecycleError::TransitionForbidden);
    }
    validate_ledger(input.ledger_content)?;
    let applied = applied_ids(input.ledger_content)?;
    let unresolved = bound_pending_records(&chapter, input.chapter_relative_path)?
        .into_iter()
        .filter(|record| !applied.contains(&record.pending_id))
        .collect::<Vec<_>>();
    if unresolved.is_empty() {
        return Err(ChapterLifecycleError::NoPending);
    }
    let appended = unresolved
        .iter()
        .map(render_ledger_entry)
        .collect::<Vec<_>>()
        .join("\n\n");
    let normalized = normalize(input.ledger_content);
    let mut target_content = normalized.trim_end_matches('\n').to_owned();
    target_content.push_str("\n\n");
    target_content.push_str(&appended);
    target_content.push('\n');
    if !validate_source(STORY_LEDGER_RELATIVE_PATH, &target_content)
        .diagnostics
        .is_empty()
    {
        return Err(ChapterLifecycleError::InvalidLedger);
    }
    let insert_at = normalized.trim_end_matches('\n').split('\n').count();
    let added = format!("\n{appended}")
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let hunks = vec![replacement_hunk(insert_at, &[], &added)];
    let summary = summarize(&hunks);
    let mut preview = LedgerUpdatePreviewV1 {
        schema_version: CHAPTER_LIFECYCLE_SCHEMA_VERSION,
        preview_hash: String::new(),
        target_relative_path: STORY_LEDGER_RELATIVE_PATH.to_owned(),
        chapter_relative_path: input.chapter_relative_path.to_owned(),
        expected_source_hash: input.ledger_content_hash.to_owned(),
        proposed_content_hash: sha256_hex(target_content.as_bytes()),
        chapter_content_hash: input.chapter_content_hash.to_owned(),
        pending_ids: unresolved
            .iter()
            .map(|item| item.pending_id.clone())
            .collect(),
        hunks,
        summary,
        ready_to_apply: true,
    };
    preview.preview_hash = ledger_preview_hash(&preview)?;
    Ok(LedgerUpdateDocumentV1 {
        preview,
        target_content,
    })
}

pub fn bind_ledger_update_encoded_hash(
    document: &mut LedgerUpdateDocumentV1,
    encoded_hash: &str,
) -> Result<(), ChapterLifecycleError> {
    validate_hash(encoded_hash)?;
    document.preview.proposed_content_hash = encoded_hash.to_owned();
    document.preview.preview_hash = ledger_preview_hash(&document.preview)?;
    Ok(())
}

fn parse_chapter(path: &str, content: &str) -> Result<ParsedChapter, ChapterLifecycleError> {
    let normalized = normalize(content.strip_prefix('\u{feff}').unwrap_or(content));
    if !validate_source(path, &normalized).diagnostics.is_empty() {
        return Err(ChapterLifecycleError::InvalidChapter);
    }
    let trailing_newline = normalized.ends_with('\n');
    let mut lines = normalized
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if trailing_newline {
        lines.pop();
    }
    if lines.first().map(String::as_str) != Some("---") {
        return Err(ChapterLifecycleError::InvalidChapter);
    }
    let end = lines
        .iter()
        .skip(1)
        .position(|line| line == "---")
        .map(|index| index + 1)
        .ok_or(ChapterLifecycleError::InvalidChapter)?;
    let mut fields = BTreeMap::new();
    for (index, line) in lines.iter().enumerate().take(end).skip(1) {
        if line.starts_with(' ')
            || line.starts_with('\t')
            || line.starts_with('-')
            || line.is_empty()
        {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or(ChapterLifecycleError::InvalidChapter)?;
        if key.trim().is_empty()
            || fields
                .insert(key.trim().to_owned(), (value.trim().to_owned(), index))
                .is_some()
        {
            return Err(ChapterLifecycleError::InvalidChapter);
        }
    }
    let parsed = ParsedChapter {
        fields,
        lines,
        trailing_newline,
    };
    if scalar(&parsed, "kind")? != "chapter"
        || scalar(&parsed, "schemaVersion")? != "1"
        || scalar(&parsed, "writingLanguage")? != "zh-CN"
        || scalar(&parsed, "id")?.is_empty()
        || scalar(&parsed, "title")?.is_empty()
        || scalar(&parsed, "chapterNumber")?.parse::<u32>().is_err()
        || parse_status(&scalar(&parsed, "status")?).is_err()
    {
        return Err(ChapterLifecycleError::InvalidChapter);
    }
    Ok(parsed)
}

fn pending_records(
    parsed: &ParsedChapter,
) -> Result<Vec<LedgerPendingRecordV1>, ChapterLifecycleError> {
    let headings = parsed
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (line == PENDING_HEADING).then_some(index))
        .collect::<Vec<_>>();
    if headings.is_empty() {
        return Ok(Vec::new());
    }
    if headings.len() != 1 {
        return Err(ChapterLifecycleError::PendingInvalid);
    }
    let start = headings[0];
    let end = parsed
        .lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(parsed.lines.len());
    let body = parsed.lines[start + 1..end]
        .iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if body.len() != 1 {
        return Err(ChapterLifecycleError::PendingInvalid);
    }
    let json = body[0]
        .strip_prefix(PENDING_PREFIX)
        .ok_or(ChapterLifecycleError::PendingInvalid)?;
    let records = serde_json::from_str::<Vec<LedgerPendingRecordV1>>(json)
        .map_err(|_| ChapterLifecycleError::PendingInvalid)?;
    if records.len() > 64 {
        return Err(ChapterLifecycleError::PendingInvalid);
    }
    let mut ids = BTreeSet::new();
    for record in &records {
        if record.schema_version != 1
            || !is_pending_id(&record.pending_id)
            || record.accepted_status != ChapterLifecycleStatusV1::Accepted
            || record.candidates.len() > 64
            || !ids.insert(record.pending_id.clone())
        {
            return Err(ChapterLifecycleError::PendingInvalid);
        }
        for hash in [
            &record.source_chapter_hash,
            &record.proposal_hash,
            &record.source_manifest_hash,
            &record.request_hash,
        ] {
            validate_hash(hash).map_err(|_| ChapterLifecycleError::PendingInvalid)?;
        }
        for candidate in &record.candidates {
            if !is_bounded_pending_text(&candidate.summary, 1_000)
                || candidate.source_ids.len() > 16
            {
                return Err(ChapterLifecycleError::PendingInvalid);
            }
            let mut source_ids = BTreeSet::new();
            for source_id in &candidate.source_ids {
                if !is_bounded_pending_text(source_id, 256)
                    || !source_ids.insert(source_id.as_str())
                {
                    return Err(ChapterLifecycleError::PendingInvalid);
                }
            }
        }
    }
    Ok(records)
}

fn bound_pending_records(
    parsed: &ParsedChapter,
    chapter_relative_path: &str,
) -> Result<Vec<LedgerPendingRecordV1>, ChapterLifecycleError> {
    let chapter_id = scalar(parsed, "id")?;
    let chapter_number = scalar(parsed, "chapterNumber")?
        .parse::<u32>()
        .map_err(|_| ChapterLifecycleError::InvalidChapter)?;
    let chapter_title = scalar(parsed, "title")?;
    let records = pending_records(parsed)?;
    if records.iter().any(|record| {
        record.chapter_id != chapter_id
            || record.chapter_number != chapter_number
            || record.chapter_title != chapter_title
            || record.chapter_relative_path != chapter_relative_path
    }) {
        return Err(ChapterLifecycleError::PendingInvalid);
    }
    Ok(records)
}

fn is_bounded_pending_text(value: &str, maximum_characters: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.chars().count() <= maximum_characters
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{202a}'
                        | '\u{202b}'
                        | '\u{202c}'
                        | '\u{202d}'
                        | '\u{202e}'
                        | '\u{2066}'
                        | '\u{2067}'
                        | '\u{2068}'
                        | '\u{2069}'
                )
        })
}

fn pending_range(parsed: &ParsedChapter) -> Result<(usize, Vec<String>), ChapterLifecycleError> {
    let Some(start) = parsed.lines.iter().position(|line| line == PENDING_HEADING) else {
        return Ok((parsed.lines.len(), Vec::new()));
    };
    pending_records(parsed)?;
    let end = parsed
        .lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(parsed.lines.len());
    Ok((start, parsed.lines[start..end].to_vec()))
}

fn render_pending(records: &[LedgerPendingRecordV1]) -> Result<String, ChapterLifecycleError> {
    let json = serde_json::to_string(records).map_err(|_| ChapterLifecycleError::PendingInvalid)?;
    Ok(format!("{PENDING_HEADING}\n\n{PENDING_PREFIX}{json}"))
}

fn validate_ledger(content: &str) -> Result<(), ChapterLifecycleError> {
    let normalized = normalize(content.strip_prefix('\u{feff}').unwrap_or(content));
    if normalized.trim().is_empty()
        || !validate_source(STORY_LEDGER_RELATIVE_PATH, &normalized)
            .diagnostics
            .is_empty()
    {
        return Err(ChapterLifecycleError::InvalidLedger);
    }
    Ok(())
}

fn applied_ids(content: &str) -> Result<BTreeSet<String>, ChapterLifecycleError> {
    let mut ids = BTreeSet::new();
    for line in normalize(content).lines() {
        if let Some(id) = line
            .strip_prefix(APPLIED_PREFIX)
            .and_then(|value| value.strip_suffix(" -->"))
        {
            if !is_pending_id(id) || !ids.insert(id.to_owned()) {
                return Err(ChapterLifecycleError::InvalidLedger);
            }
        }
    }
    Ok(ids)
}

fn render_ledger_entry(record: &LedgerPendingRecordV1) -> String {
    let tick = '\u{60}';
    let mut lines = vec![
        format!(
            "## 第 {} 章《{}》状态更新",
            record.chapter_number,
            safe_text(&record.chapter_title)
        ),
        format!("{APPLIED_PREFIX}{} -->", record.pending_id),
        format!("- 章节：{tick}{}{tick}", record.chapter_relative_path),
        format!("- 状态：{tick}{}{tick}", record.accepted_status.as_str()),
        format!(
            "- 章节来源 Hash：{tick}{}{tick}",
            record.source_chapter_hash
        ),
        format!("- 提案 Hash：{tick}{}{tick}", record.proposal_hash),
        format!(
            "- Context Manifest Hash：{tick}{}{tick}",
            record.source_manifest_hash
        ),
        "- 连续性候选：".to_owned(),
    ];
    if record.candidates.is_empty() {
        lines.push("  - 无；本条仅记录章节状态。".to_owned());
    } else {
        for candidate in &record.candidates {
            let sources = if candidate.source_ids.is_empty() {
                "无显式来源".to_owned()
            } else {
                candidate
                    .source_ids
                    .iter()
                    .map(|item| safe_text(item))
                    .collect::<Vec<_>>()
                    .join("、")
            };
            lines.push(format!(
                "  - {}（来源：{}）",
                safe_text(&candidate.summary),
                sources
            ));
        }
    }
    lines.join("\n")
}

fn safe_text(value: &str) -> String {
    value
        .replace(['\r', '\n'], " ")
        .replace('\\', "\\\\")
        .replace('\u{60}', "\\\u{60}")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .trim()
        .to_owned()
}

fn validate_chapter_path(path: &str) -> Result<(), ChapterLifecycleError> {
    let Some(file) = path
        .strip_prefix("chapters/")
        .and_then(|item| item.strip_suffix(".md"))
    else {
        return Err(ChapterLifecycleError::InvalidTarget);
    };
    if file.len() < 3 || !file.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ChapterLifecycleError::InvalidTarget);
    }
    Ok(())
}

fn validate_hash(value: &str) -> Result<(), ChapterLifecycleError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(ChapterLifecycleError::InvalidHash)
    }
}

fn validate_bound_content(
    content: &str,
    hash: &str,
    has_utf8_bom: bool,
) -> Result<(), ChapterLifecycleError> {
    validate_hash(hash)?;
    let mut bytes = Vec::with_capacity(content.len() + usize::from(has_utf8_bom) * 3);
    if has_utf8_bom {
        bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
    }
    bytes.extend_from_slice(content.as_bytes());
    if sha256_hex(&bytes) == hash {
        Ok(())
    } else {
        Err(ChapterLifecycleError::SourceHashMismatch)
    }
}

fn is_pending_id(value: &str) -> bool {
    value
        .strip_prefix("ledger_pending_")
        .and_then(|item| Uuid::parse_str(item).ok())
        .is_some()
}

fn parse_status(value: &str) -> Result<ChapterLifecycleStatusV1, ChapterLifecycleError> {
    match value {
        "draft" => Ok(ChapterLifecycleStatusV1::Draft),
        "accepted" => Ok(ChapterLifecycleStatusV1::Accepted),
        "final" => Ok(ChapterLifecycleStatusV1::Final),
        _ => Err(ChapterLifecycleError::InvalidChapter),
    }
}

fn scalar(parsed: &ParsedChapter, field: &str) -> Result<String, ChapterLifecycleError> {
    let value = parsed
        .fields
        .get(field)
        .map(|item| item.0.trim())
        .filter(|value| !value.is_empty())
        .ok_or(ChapterLifecycleError::InvalidChapter)?;
    if value.starts_with('"') || value.ends_with('"') {
        let inner = value
            .strip_prefix('"')
            .and_then(|item| item.strip_suffix('"'))
            .ok_or(ChapterLifecycleError::InvalidChapter)?;
        return Ok(inner.replace("\\\"", "\"").replace("\\\\", "\\"));
    }
    if value.starts_with('\'') || value.ends_with('\'') {
        let inner = value
            .strip_prefix('\'')
            .and_then(|item| item.strip_suffix('\''))
            .ok_or(ChapterLifecycleError::InvalidChapter)?;
        return Ok(inner.replace("''", "'"));
    }
    Ok(value.to_owned())
}

fn normalize(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

fn replacement_hunk(index: usize, old: &[String], new: &[String]) -> ChapterDiffHunkV1 {
    let mut lines = Vec::with_capacity(old.len() + new.len());
    for (offset, content) in old.iter().enumerate() {
        lines.push(ChapterDiffLineV1 {
            kind: ChapterDiffLineKindV1::Removed,
            old_line_number: Some((index + offset + 1) as u32),
            new_line_number: None,
            content: content.clone(),
        });
    }
    for (offset, content) in new.iter().enumerate() {
        lines.push(ChapterDiffLineV1 {
            kind: ChapterDiffLineKindV1::Added,
            old_line_number: None,
            new_line_number: Some((index + offset + 1) as u32),
            content: content.clone(),
        });
    }
    ChapterDiffHunkV1 {
        old_start: if old.is_empty() {
            0
        } else {
            (index + 1) as u32
        },
        old_line_count: old.len() as u32,
        new_start: (index + 1) as u32,
        new_line_count: new.len() as u32,
        lines,
    }
}

fn summarize(hunks: &[ChapterDiffHunkV1]) -> ChapterDiffSummaryV1 {
    let mut summary = ChapterDiffSummaryV1 {
        added_lines: 0,
        removed_lines: 0,
        unchanged_lines: 0,
    };
    for line in hunks.iter().flat_map(|hunk| hunk.lines.iter()) {
        match line.kind {
            ChapterDiffLineKindV1::Added => summary.added_lines += 1,
            ChapterDiffLineKindV1::Removed => summary.removed_lines += 1,
            ChapterDiffLineKindV1::Context => summary.unchanged_lines += 1,
        }
    }
    summary
}

fn preview_hash<T: Serialize>(preview: &T) -> Result<String, ChapterLifecycleError> {
    serde_json::to_vec(preview)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|_| ChapterLifecycleError::PendingInvalid)
}

fn status_preview_hash(preview: &ChapterStatusPreviewV1) -> Result<String, ChapterLifecycleError> {
    let mut material = preview.clone();
    material.preview_hash.clear();
    preview_hash(&material)
}

fn ledger_preview_hash(preview: &LedgerUpdatePreviewV1) -> Result<String, ChapterLifecycleError> {
    let mut material = preview.clone();
    material.preview_hash.clear();
    preview_hash(&material)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chapter::{ChapterProposalSourceStatusV1, ChapterProposalV1};
    use crate::story_context::{StoryActionV2, StoryReaderMentalModelV1, StoryTargetChapterV2};

    fn chapter(status: &str) -> String {
        format!(
            "---\nid: chapter_11111111-2222-4333-8444-555555555555\ntitle: \"潮痕\"\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 3\nstatus: {status}\n---\n\n# 潮痕\n\n## Draft\n\n新的正文。\n"
        )
    }

    fn ledger() -> String {
        "---\ntitle: 故事状态账本\nwritingLanguage: zh-CN\nstatus: active\n---\n\n# 故事状态账本\n\n在这里记录时间线、角色状态、线索、承诺与回收等连续性事实。\n".to_owned()
    }

    fn proposal() -> ChapterProposalV1 {
        ChapterProposalV1 {
            proposal_version: 1,
            proposal_hash: "a".repeat(64),
            action: StoryActionV2::Create,
            target_chapter: StoryTargetChapterV2 {
                chapter_number: 3,
                chapter_title: "潮痕".to_owned(),
                relative_path: "chapters/003.md".to_owned(),
            },
            title: "潮痕".to_owned(),
            draft_markdown: "新的正文。".to_owned(),
            intent_summary: "推进线索。".to_owned(),
            reader_model_delta: StoryReaderMentalModelV1 {
                reader_knows: String::new(),
                reader_infers: String::new(),
                reader_misreads: String::new(),
                reader_questions: String::new(),
                chapter_promises: String::new(),
                forbidden_reveals: String::new(),
            },
            continuity_claims: Vec::new(),
            ledger_candidates: vec![ChapterLedgerCandidateV1 {
                summary: "潮痕钥匙已经交给阿澈。".to_owned(),
                source_ids: vec!["chapter:003".to_owned()],
            }],
            source_manifest_hash: "b".repeat(64),
            current_manifest_hash: "c".repeat(64),
            request_hash: "d".repeat(64),
            provider_id: "deepseek".to_owned(),
            model_id: "deepseek-chat".to_owned(),
            started_at: 1,
            generated_at: 2,
            finish_reason: "stop".to_owned(),
            response_complete: true,
            source_status: ChapterProposalSourceStatusV1::Current,
            continuity_check_version: Some(1),
            write_enabled: false,
        }
    }

    fn accepted_document() -> ChapterStatusDocumentV1 {
        let content = chapter("draft");
        let hash = sha256_hex(content.as_bytes());
        let proposal = proposal();
        build_chapter_status_transition(BuildChapterStatusInputV1 {
            target_relative_path: "chapters/003.md",
            current_content: &content,
            current_content_hash: &hash,
            current_content_has_utf8_bom: false,
            next_status: ChapterLifecycleStatusV1::Accepted,
            proposal: Some(&proposal),
            pending_id: Some(Uuid::parse_str("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee").unwrap()),
        })
        .unwrap()
    }

    #[test]
    fn ng_506_forbids_draft_to_final_and_requires_an_independent_acceptance() {
        let content = chapter("draft");
        let hash = sha256_hex(content.as_bytes());
        let proposal = proposal();
        let error = build_chapter_status_transition(BuildChapterStatusInputV1 {
            target_relative_path: "chapters/003.md",
            current_content: &content,
            current_content_hash: &hash,
            current_content_has_utf8_bom: false,
            next_status: ChapterLifecycleStatusV1::Final,
            proposal: Some(&proposal),
            pending_id: None,
        })
        .unwrap_err();
        assert_eq!(error, ChapterLifecycleError::TransitionForbidden);
    }

    #[test]
    fn ng_506_acceptance_persists_visible_pending_state_and_supports_bom_hashes() {
        let content = chapter("draft");
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend_from_slice(content.as_bytes());
        let hash = sha256_hex(&bytes);
        let proposal = proposal();
        let accepted = build_chapter_status_transition(BuildChapterStatusInputV1 {
            target_relative_path: "chapters/003.md",
            current_content: &content,
            current_content_hash: &hash,
            current_content_has_utf8_bom: true,
            next_status: ChapterLifecycleStatusV1::Accepted,
            proposal: Some(&proposal),
            pending_id: Some(Uuid::parse_str("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee").unwrap()),
        })
        .unwrap();
        assert!(accepted.target_content.contains("status: accepted"));
        assert!(accepted.target_content.contains(PENDING_HEADING));
        assert!(accepted
            .target_content
            .contains("ledger_pending_aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"));
        assert_eq!(
            load_chapter_lifecycle_status("chapters/003.md", &accepted.target_content).unwrap(),
            ChapterLifecycleStatusV1::Accepted
        );
        let pending =
            load_ledger_pending_status("chapters/003.md", &accepted.target_content, &ledger())
                .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].resolution, LedgerPendingResolutionV1::Pending);
    }

    #[test]
    fn ng_506_ledger_update_is_fixed_separate_and_recovers_from_markdown() {
        let accepted = accepted_document();
        let ledger = ledger();
        let chapter_hash = sha256_hex(accepted.target_content.as_bytes());
        let ledger_hash = sha256_hex(ledger.as_bytes());
        let update = build_ledger_update(BuildLedgerUpdateInputV1 {
            chapter_relative_path: "chapters/003.md",
            chapter_content: &accepted.target_content,
            chapter_content_hash: &chapter_hash,
            chapter_content_has_utf8_bom: false,
            ledger_content: &ledger,
            ledger_content_hash: &ledger_hash,
            ledger_content_has_utf8_bom: false,
        })
        .unwrap();
        assert_eq!(
            update.preview.target_relative_path,
            STORY_LEDGER_RELATIVE_PATH
        );
        assert_eq!(update.preview.pending_ids.len(), 1);
        assert!(update.target_content.contains("潮痕钥匙已经交给阿澈"));
        let recovered = load_ledger_pending_status(
            "chapters/003.md",
            &accepted.target_content,
            &update.target_content,
        )
        .unwrap();
        assert_eq!(recovered[0].resolution, LedgerPendingResolutionV1::Applied);
        assert_eq!(
            build_ledger_update(BuildLedgerUpdateInputV1 {
                chapter_relative_path: "chapters/003.md",
                chapter_content: &accepted.target_content,
                chapter_content_hash: &chapter_hash,
                chapter_content_has_utf8_bom: false,
                ledger_content: &update.target_content,
                ledger_content_hash: &sha256_hex(update.target_content.as_bytes()),
                ledger_content_has_utf8_bom: false,
            })
            .unwrap_err(),
            ChapterLifecycleError::NoPending
        );
    }

    #[test]
    fn ng_506_rejects_pending_records_not_bound_to_the_current_chapter() {
        let accepted = accepted_document();
        let tampered = accepted.target_content.replace(
            "\"chapterRelativePath\":\"chapters/003.md\"",
            "\"chapterRelativePath\":\"chapters/999.md\"",
        );
        assert_eq!(
            load_ledger_pending_status("chapters/003.md", &tampered, &ledger()).unwrap_err(),
            ChapterLifecycleError::PendingInvalid
        );
        assert_eq!(
            build_ledger_update(BuildLedgerUpdateInputV1 {
                chapter_relative_path: "chapters/003.md",
                chapter_content: &tampered,
                chapter_content_hash: &sha256_hex(tampered.as_bytes()),
                chapter_content_has_utf8_bom: false,
                ledger_content: &ledger(),
                ledger_content_hash: &sha256_hex(ledger().as_bytes()),
                ledger_content_has_utf8_bom: false,
            })
            .unwrap_err(),
            ChapterLifecycleError::PendingInvalid
        );
    }

    #[test]
    fn ng_506_final_preserves_pending_and_invalid_ledger_cannot_change_chapter() {
        let accepted = accepted_document();
        let accepted_hash = sha256_hex(accepted.target_content.as_bytes());
        let final_document = build_chapter_status_transition(BuildChapterStatusInputV1 {
            target_relative_path: "chapters/003.md",
            current_content: &accepted.target_content,
            current_content_hash: &accepted_hash,
            current_content_has_utf8_bom: false,
            next_status: ChapterLifecycleStatusV1::Final,
            proposal: None,
            pending_id: None,
        })
        .unwrap();
        assert!(final_document.target_content.contains("status: final"));
        assert!(final_document.target_content.contains("ledger_pending_"));
        let error = build_ledger_update(BuildLedgerUpdateInputV1 {
            chapter_relative_path: "chapters/003.md",
            chapter_content: &final_document.target_content,
            chapter_content_hash: &sha256_hex(final_document.target_content.as_bytes()),
            chapter_content_has_utf8_bom: false,
            ledger_content: "not a ledger",
            ledger_content_hash: &sha256_hex(b"not a ledger"),
            ledger_content_has_utf8_bom: false,
        })
        .unwrap_err();
        assert_eq!(error, ChapterLifecycleError::InvalidLedger);
        assert!(final_document.target_content.contains("status: final"));
    }
}
