use crate::chapter::ChapterProposalV1;
use crate::continuity::StoryContinuityReportV1;
use crate::story_context::{sha256_hex, StoryActionV2};
use novelgainker_validation::validate_source;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use uuid::Uuid;

pub const CHAPTER_PATCH_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChapterPatchOperationV1 {
    Create,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChapterDiffLineKindV1 {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDiffLineV1 {
    pub kind: ChapterDiffLineKindV1,
    pub old_line_number: Option<u32>,
    pub new_line_number: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDiffHunkV1 {
    pub old_start: u32,
    pub old_line_count: u32,
    pub new_start: u32,
    pub new_line_count: u32,
    pub lines: Vec<ChapterDiffLineV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDiffSummaryV1 {
    pub added_lines: u32,
    pub removed_lines: u32,
    pub unchanged_lines: u32,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPatchPreviewV1 {
    pub schema_version: u16,
    pub preview_hash: String,
    pub proposal_hash: String,
    pub continuity_report_hash: String,
    pub source_manifest_hash: String,
    pub current_manifest_hash: String,
    pub request_hash: String,
    pub action: StoryActionV2,
    pub target_relative_path: String,
    pub operation: ChapterPatchOperationV1,
    pub expected_source_hash: Option<String>,
    pub proposed_content_hash: String,
    pub chapter_id: String,
    pub previous_status: Option<String>,
    pub next_status: String,
    pub accepted_or_final_unlock_confirmed: bool,
    pub hunks: Vec<ChapterDiffHunkV1>,
    pub summary: ChapterDiffSummaryV1,
    pub ready_to_apply: bool,
}

impl fmt::Debug for ChapterPatchPreviewV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChapterPatchPreviewV1")
            .field("schema_version", &self.schema_version)
            .field("preview_hash", &self.preview_hash)
            .field("proposal_hash", &self.proposal_hash)
            .field("continuity_report_hash", &self.continuity_report_hash)
            .field("target_relative_path", &self.target_relative_path)
            .field("operation", &self.operation)
            .field("expected_source_hash", &self.expected_source_hash)
            .field("proposed_content_hash", &self.proposed_content_hash)
            .field("chapter_id", &self.chapter_id)
            .field("previous_status", &self.previous_status)
            .field("next_status", &self.next_status)
            .field(
                "accepted_or_final_unlock_confirmed",
                &self.accepted_or_final_unlock_confirmed,
            )
            .field("hunk_count", &self.hunks.len())
            .field("summary", &self.summary)
            .field("ready_to_apply", &self.ready_to_apply)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChapterPatchDocumentV1 {
    pub preview: ChapterPatchPreviewV1,
    pub target_content: String,
}

impl fmt::Debug for ChapterPatchDocumentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChapterPatchDocumentV1")
            .field("preview", &self.preview)
            .field("target_content", &"[REDACTED]")
            .finish()
    }
}

pub struct BuildChapterPatchInputV1<'a> {
    pub proposal: &'a ChapterProposalV1,
    pub continuity_report: &'a StoryContinuityReportV1,
    pub current_content: Option<&'a str>,
    pub current_content_hash: Option<&'a str>,
    pub accepted_or_final_unlock_confirmed: bool,
    pub create_chapter_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterPatchError {
    BindingMismatch,
    ContinuityNotReady,
    InvalidTarget,
    TargetAlreadyExists,
    TargetMissing,
    SourceHashMismatch,
    InvalidChapter,
    LockedStatus,
    InvalidProposal,
}

impl ChapterPatchError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::BindingMismatch => "chapter.patch_binding_mismatch",
            Self::ContinuityNotReady => "chapter.continuity_not_ready",
            Self::InvalidTarget => "chapter.target_invalid",
            Self::TargetAlreadyExists => "chapter.target_exists",
            Self::TargetMissing => "chapter.target_missing",
            Self::SourceHashMismatch => "chapter.source_hash_mismatch",
            Self::InvalidChapter => "chapter.source_invalid",
            Self::LockedStatus => "chapter.status_unlock_required",
            Self::InvalidProposal => "chapter.proposal_invalid",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewHashMaterial<'a> {
    schema_version: u16,
    proposal_hash: &'a str,
    continuity_report_hash: &'a str,
    source_manifest_hash: &'a str,
    current_manifest_hash: &'a str,
    request_hash: &'a str,
    action: StoryActionV2,
    target_relative_path: &'a str,
    operation: ChapterPatchOperationV1,
    expected_source_hash: &'a Option<String>,
    proposed_content_hash: &'a str,
    chapter_id: &'a str,
    previous_status: &'a Option<String>,
    next_status: &'a str,
    accepted_or_final_unlock_confirmed: bool,
    hunks: &'a [ChapterDiffHunkV1],
    summary: &'a ChapterDiffSummaryV1,
}

#[derive(Debug)]
struct ParsedChapter {
    fields: BTreeMap<String, (String, usize)>,
    lines: Vec<String>,
    frontmatter_end: usize,
    draft_heading: usize,
    draft_end: usize,
    trailing_newline: bool,
}

pub fn build_chapter_patch(
    input: BuildChapterPatchInputV1<'_>,
) -> Result<ChapterPatchDocumentV1, ChapterPatchError> {
    validate_bindings(input.proposal, input.continuity_report)?;
    let target = &input.proposal.target_chapter;
    let expected_path = format!("chapters/{:03}.md", target.chapter_number);
    if !(1..=999_999).contains(&target.chapter_number)
        || target.relative_path != expected_path
        || target.chapter_title.trim().is_empty()
    {
        return Err(ChapterPatchError::InvalidTarget);
    }

    let (target_content, chapter_id, previous_status, operation, expected_source_hash, hunks) =
        match input.proposal.action {
            StoryActionV2::Create => {
                if input.current_content.is_some() || input.current_content_hash.is_some() {
                    return Err(ChapterPatchError::TargetAlreadyExists);
                }
                let chapter_id = format!(
                    "chapter_{}",
                    input.create_chapter_id.unwrap_or_else(Uuid::new_v4)
                );
                let content = render_new_chapter(input.proposal, &chapter_id)?;
                let hunks = create_hunks(&content);
                (
                    content,
                    chapter_id,
                    None,
                    ChapterPatchOperationV1::Create,
                    None,
                    hunks,
                )
            }
            StoryActionV2::Revise => {
                if input.continuity_report.current_target_hash.as_deref()
                    != input.current_content_hash
                {
                    return Err(ChapterPatchError::SourceHashMismatch);
                }
                let current = input
                    .current_content
                    .ok_or(ChapterPatchError::TargetMissing)?;
                let current_hash = input
                    .current_content_hash
                    .ok_or(ChapterPatchError::TargetMissing)?;
                let parsed = parse_chapter(&target.relative_path, current)?;
                validate_existing_fields(&parsed, target.chapter_number)?;
                let chapter_id = required_scalar(&parsed, "id")?;
                let previous_status = required_scalar(&parsed, "status")?;
                if matches!(previous_status.as_str(), "accepted" | "final")
                    && !input.accepted_or_final_unlock_confirmed
                {
                    return Err(ChapterPatchError::LockedStatus);
                }
                let (content, hunks) = revise_chapter(&parsed, input.proposal, &previous_status)?;
                (
                    content,
                    chapter_id,
                    Some(previous_status),
                    ChapterPatchOperationV1::Replace,
                    Some(current_hash.to_owned()),
                    hunks,
                )
            }
        };

    let report = validate_source(&target.relative_path, &target_content);
    if !report.diagnostics.is_empty() {
        return Err(ChapterPatchError::InvalidChapter);
    }
    let proposed_content_hash = sha256_hex(target_content.as_bytes());
    let summary = summarize_hunks(&hunks);
    let next_status = "draft".to_owned();
    let material = PreviewHashMaterial {
        schema_version: CHAPTER_PATCH_SCHEMA_VERSION,
        proposal_hash: &input.proposal.proposal_hash,
        continuity_report_hash: &input.continuity_report.report_hash,
        source_manifest_hash: &input.proposal.source_manifest_hash,
        current_manifest_hash: &input.proposal.current_manifest_hash,
        request_hash: &input.proposal.request_hash,
        action: input.proposal.action,
        target_relative_path: &target.relative_path,
        operation,
        expected_source_hash: &expected_source_hash,
        proposed_content_hash: &proposed_content_hash,
        chapter_id: &chapter_id,
        previous_status: &previous_status,
        next_status: &next_status,
        accepted_or_final_unlock_confirmed: input.accepted_or_final_unlock_confirmed,
        hunks: &hunks,
        summary: &summary,
    };
    let preview_hash =
        sha256_hex(&serde_json::to_vec(&material).map_err(|_| ChapterPatchError::InvalidProposal)?);
    Ok(ChapterPatchDocumentV1 {
        preview: ChapterPatchPreviewV1 {
            schema_version: CHAPTER_PATCH_SCHEMA_VERSION,
            preview_hash,
            proposal_hash: input.proposal.proposal_hash.clone(),
            continuity_report_hash: input.continuity_report.report_hash.clone(),
            source_manifest_hash: input.proposal.source_manifest_hash.clone(),
            current_manifest_hash: input.proposal.current_manifest_hash.clone(),
            request_hash: input.proposal.request_hash.clone(),
            action: input.proposal.action,
            target_relative_path: target.relative_path.clone(),
            operation,
            expected_source_hash,
            proposed_content_hash,
            chapter_id,
            previous_status,
            next_status,
            accepted_or_final_unlock_confirmed: input.accepted_or_final_unlock_confirmed,
            hunks,
            summary,
            ready_to_apply: true,
        },
        target_content,
    })
}

pub fn bind_chapter_patch_encoded_hash(
    document: &mut ChapterPatchDocumentV1,
    encoded_content_hash: &str,
) -> Result<(), ChapterPatchError> {
    if encoded_content_hash.len() != 64
        || !encoded_content_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ChapterPatchError::InvalidProposal);
    }
    document.preview.proposed_content_hash = encoded_content_hash.to_owned();
    document.preview.preview_hash = calculate_preview_hash(&document.preview)?;
    Ok(())
}

fn calculate_preview_hash(preview: &ChapterPatchPreviewV1) -> Result<String, ChapterPatchError> {
    let material = PreviewHashMaterial {
        schema_version: preview.schema_version,
        proposal_hash: &preview.proposal_hash,
        continuity_report_hash: &preview.continuity_report_hash,
        source_manifest_hash: &preview.source_manifest_hash,
        current_manifest_hash: &preview.current_manifest_hash,
        request_hash: &preview.request_hash,
        action: preview.action,
        target_relative_path: &preview.target_relative_path,
        operation: preview.operation,
        expected_source_hash: &preview.expected_source_hash,
        proposed_content_hash: &preview.proposed_content_hash,
        chapter_id: &preview.chapter_id,
        previous_status: &preview.previous_status,
        next_status: &preview.next_status,
        accepted_or_final_unlock_confirmed: preview.accepted_or_final_unlock_confirmed,
        hunks: &preview.hunks,
        summary: &preview.summary,
    };
    Ok(sha256_hex(
        &serde_json::to_vec(&material).map_err(|_| ChapterPatchError::InvalidProposal)?,
    ))
}

fn validate_bindings(
    proposal: &ChapterProposalV1,
    report: &StoryContinuityReportV1,
) -> Result<(), ChapterPatchError> {
    if report.proposal_hash != proposal.proposal_hash
        || report.source_manifest_hash != proposal.source_manifest_hash
        || report.current_manifest_hash.as_deref() != Some(proposal.current_manifest_hash.as_str())
        || report.current_target_hash != report.generation_target_hash
    {
        return Err(ChapterPatchError::BindingMismatch);
    }
    if !report.review_ready
        || report.summary.blocking > 0
        || !report.warnings_confirmed
        || !report.unknowns_confirmed
    {
        return Err(ChapterPatchError::ContinuityNotReady);
    }
    Ok(())
}

fn render_new_chapter(
    proposal: &ChapterProposalV1,
    chapter_id: &str,
) -> Result<String, ChapterPatchError> {
    let title = yaml_double_quoted_scalar(&proposal.title)?;
    if proposal.draft_markdown.trim().is_empty() {
        return Err(ChapterPatchError::InvalidProposal);
    }
    Ok(format!(
        "---\nid: {chapter_id}\ntitle: {title}\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: {}\nstatus: draft\n---\n\n# {}\n\n## Draft\n\n{}\n",
        proposal.target_chapter.chapter_number, proposal.title, proposal.draft_markdown
    ))
}

fn yaml_double_quoted_scalar(value: &str) -> Result<String, ChapterPatchError> {
    if value.trim() != value || value.is_empty() || value.contains('\n') || value.contains('\r') {
        return Err(ChapterPatchError::InvalidProposal);
    }
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

fn normalize_newlines(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

fn parse_chapter(relative_path: &str, content: &str) -> Result<ParsedChapter, ChapterPatchError> {
    let normalized = normalize_newlines(content.strip_prefix('\u{feff}').unwrap_or(content));
    if !validate_source(relative_path, &normalized)
        .diagnostics
        .is_empty()
    {
        return Err(ChapterPatchError::InvalidChapter);
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
        return Err(ChapterPatchError::InvalidChapter);
    }
    let frontmatter_end = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (line == "---").then_some(index))
        .ok_or(ChapterPatchError::InvalidChapter)?;
    let mut fields = BTreeMap::new();
    for (index, line) in lines.iter().enumerate().take(frontmatter_end).skip(1) {
        if line.starts_with(' ')
            || line.starts_with('\t')
            || line.starts_with('-')
            || line.trim().is_empty()
        {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or(ChapterPatchError::InvalidChapter)?;
        let key = key.trim();
        if key.is_empty()
            || fields
                .insert(key.to_owned(), (value.trim().to_owned(), index))
                .is_some()
        {
            return Err(ChapterPatchError::InvalidChapter);
        }
    }
    let headings = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (line == "## Draft").then_some(index))
        .collect::<Vec<_>>();
    if headings.len() != 1 || headings[0] <= frontmatter_end {
        return Err(ChapterPatchError::InvalidChapter);
    }
    let draft_heading = headings[0];
    let draft_end = lines
        .iter()
        .enumerate()
        .skip(draft_heading + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(lines.len());
    Ok(ParsedChapter {
        fields,
        lines,
        frontmatter_end,
        draft_heading,
        draft_end,
        trailing_newline,
    })
}

fn required_scalar(parsed: &ParsedChapter, field: &str) -> Result<String, ChapterPatchError> {
    let value = parsed
        .fields
        .get(field)
        .map(|(value, _)| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or(ChapterPatchError::InvalidChapter)?;
    if value.starts_with('"') || value.ends_with('"') {
        let Some(inner) = value
            .strip_prefix('"')
            .and_then(|item| item.strip_suffix('"'))
        else {
            return Err(ChapterPatchError::InvalidChapter);
        };
        let decoded = inner.replace("\\\"", "\"").replace("\\\\", "\\");
        return (!decoded.trim().is_empty())
            .then_some(decoded)
            .ok_or(ChapterPatchError::InvalidChapter);
    }
    if value.starts_with('\'') || value.ends_with('\'') {
        let Some(inner) = value
            .strip_prefix('\'')
            .and_then(|item| item.strip_suffix('\''))
        else {
            return Err(ChapterPatchError::InvalidChapter);
        };
        let decoded = inner.replace("''", "'");
        return (!decoded.trim().is_empty())
            .then_some(decoded)
            .ok_or(ChapterPatchError::InvalidChapter);
    }
    Ok(value.to_owned())
}

fn validate_existing_fields(
    parsed: &ParsedChapter,
    chapter_number: u32,
) -> Result<(), ChapterPatchError> {
    let id = required_scalar(parsed, "id")?;
    let title = required_scalar(parsed, "title")?;
    let kind = required_scalar(parsed, "kind")?;
    let schema_version = required_scalar(parsed, "schemaVersion")?;
    let writing_language = required_scalar(parsed, "writingLanguage")?;
    let number = required_scalar(parsed, "chapterNumber")?;
    let status = required_scalar(parsed, "status")?;
    if id.trim().is_empty()
        || title.trim().is_empty()
        || kind != "chapter"
        || schema_version != "1"
        || writing_language != "zh-CN"
        || number.parse::<u32>().ok() != Some(chapter_number)
        || !matches!(status.as_str(), "draft" | "accepted" | "final")
        || parsed.frontmatter_end == 0
    {
        return Err(ChapterPatchError::InvalidChapter);
    }
    Ok(())
}

fn revise_chapter(
    parsed: &ParsedChapter,
    proposal: &ChapterProposalV1,
    previous_status: &str,
) -> Result<(String, Vec<ChapterDiffHunkV1>), ChapterPatchError> {
    if proposal.draft_markdown.trim().is_empty() {
        return Err(ChapterPatchError::InvalidProposal);
    }
    let mut revised = parsed.lines.clone();
    let mut hunks = Vec::new();
    if previous_status != "draft" {
        let index = parsed
            .fields
            .get("status")
            .map(|(_, index)| *index)
            .ok_or(ChapterPatchError::InvalidChapter)?;
        let old = revised[index].clone();
        revised[index] = "status: draft".to_owned();
        hunks.push(replacement_hunk(index, &[old], &[revised[index].clone()]));
    }

    let draft_start = parsed.draft_heading + 1;
    let old_draft = parsed.lines[draft_start..parsed.draft_end].to_vec();
    let mut new_draft = vec![String::new()];
    new_draft.extend(proposal.draft_markdown.split('\n').map(ToOwned::to_owned));
    new_draft.push(String::new());
    revised.splice(draft_start..parsed.draft_end, new_draft.clone());
    hunks.push(replacement_hunk(draft_start, &old_draft, &new_draft));

    let mut content = revised.join("\n");
    if parsed.trailing_newline || parsed.draft_end == parsed.lines.len() {
        content.push('\n');
    }
    Ok((content, hunks))
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
        old_start: (index + 1) as u32,
        old_line_count: old.len() as u32,
        new_start: (index + 1) as u32,
        new_line_count: new.len() as u32,
        lines,
    }
}

fn create_hunks(content: &str) -> Vec<ChapterDiffHunkV1> {
    let lines = content
        .strip_suffix('\n')
        .unwrap_or(content)
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    vec![ChapterDiffHunkV1 {
        old_start: 0,
        old_line_count: 0,
        new_start: 1,
        new_line_count: lines.len() as u32,
        lines: lines
            .into_iter()
            .enumerate()
            .map(|(index, content)| ChapterDiffLineV1 {
                kind: ChapterDiffLineKindV1::Added,
                old_line_number: None,
                new_line_number: Some((index + 1) as u32),
                content,
            })
            .collect(),
    }]
}

fn summarize_hunks(hunks: &[ChapterDiffHunkV1]) -> ChapterDiffSummaryV1 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chapter::{ChapterProposalSourceStatusV1, ChapterProposalV1};
    use crate::continuity::{ContinuitySummaryV1, StoryContinuityReportV1};
    use crate::story_context::{StoryReaderMentalModelV1, StoryTargetChapterV2};

    fn proposal(action: StoryActionV2) -> ChapterProposalV1 {
        ChapterProposalV1 {
            proposal_version: 1,
            proposal_hash: "p".repeat(64),
            action,
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
            ledger_candidates: Vec::new(),
            source_manifest_hash: "s".repeat(64),
            current_manifest_hash: "c".repeat(64),
            request_hash: "r".repeat(64),
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

    fn report(
        proposal: &ChapterProposalV1,
        target_hash: Option<String>,
    ) -> StoryContinuityReportV1 {
        StoryContinuityReportV1 {
            schema_version: 1,
            report_hash: "q".repeat(64),
            proposal_hash: proposal.proposal_hash.clone(),
            source_manifest_hash: proposal.source_manifest_hash.clone(),
            current_manifest_hash: Some(proposal.current_manifest_hash.clone()),
            generation_target_hash: target_hash.clone(),
            current_target_hash: target_hash,
            checked_at: 3,
            local_only: true,
            results: Vec::new(),
            summary: ContinuitySummaryV1 {
                blocking: 0,
                warning: 0,
                unknown: 0,
                passed: 17,
            },
            acknowledged_warning_rule_ids: Vec::new(),
            acknowledged_unknown_rule_ids: Vec::new(),
            warnings_confirmed: true,
            unknowns_confirmed: true,
            review_ready: true,
            write_enabled: false,
        }
    }

    #[test]
    fn ng_505_create_builds_a_valid_fixed_path_chapter_and_stable_diff() {
        let proposal = proposal(StoryActionV2::Create);
        let report = report(&proposal, None);
        let id = Uuid::parse_str("11111111-2222-4333-8444-555555555555").unwrap();
        let first = build_chapter_patch(BuildChapterPatchInputV1 {
            proposal: &proposal,
            continuity_report: &report,
            current_content: None,
            current_content_hash: None,
            accepted_or_final_unlock_confirmed: false,
            create_chapter_id: Some(id),
        })
        .unwrap();
        let second = build_chapter_patch(BuildChapterPatchInputV1 {
            proposal: &proposal,
            continuity_report: &report,
            current_content: None,
            current_content_hash: None,
            accepted_or_final_unlock_confirmed: false,
            create_chapter_id: Some(id),
        })
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.preview.operation, ChapterPatchOperationV1::Create);
        assert_eq!(first.preview.target_relative_path, "chapters/003.md");
        assert!(first
            .target_content
            .contains("id: chapter_11111111-2222-4333-8444-555555555555"));
        assert!(first.target_content.contains("## Draft\n\n新的正文。"));
        assert!(validate_source("chapters/003.md", &first.target_content)
            .diagnostics
            .is_empty());
        assert!(!format!("{first:?}").contains("新的正文"));
    }

    #[test]
    fn ng_505_revise_preserves_unknown_frontmatter_and_non_draft_sections() {
        let proposal = proposal(StoryActionV2::Revise);
        let current = "---\nid: chapter_old\ntitle: 旧题\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 3\nstatus: draft\ncustomField: 保留\n---\n\n# 旧题\n\n## Draft\n\n旧正文。\n\n## Notes\n\n保留说明。\n";
        let hash = sha256_hex(current.as_bytes());
        let report = report(&proposal, Some(hash.clone()));
        let patch = build_chapter_patch(BuildChapterPatchInputV1 {
            proposal: &proposal,
            continuity_report: &report,
            current_content: Some(current),
            current_content_hash: Some(&hash),
            accepted_or_final_unlock_confirmed: false,
            create_chapter_id: None,
        })
        .unwrap();
        assert_eq!(patch.preview.operation, ChapterPatchOperationV1::Replace);
        assert_eq!(patch.preview.chapter_id, "chapter_old");
        assert!(patch.target_content.contains("customField: 保留"));
        assert!(patch.target_content.contains("## Notes\n\n保留说明。"));
        assert!(!patch.target_content.contains("旧正文。"));
        assert!(patch.target_content.contains("## Draft\n\n新的正文。"));
    }

    #[test]
    fn ng_505_locked_status_requires_unlock_and_returns_to_draft() {
        let proposal = proposal(StoryActionV2::Revise);
        let current = "---\nid: chapter_old\ntitle: 旧题\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 3\nstatus: accepted\n---\n\n## Draft\n\n旧正文。\n";
        let hash = sha256_hex(current.as_bytes());
        let report = report(&proposal, Some(hash.clone()));
        assert_eq!(
            build_chapter_patch(BuildChapterPatchInputV1 {
                proposal: &proposal,
                continuity_report: &report,
                current_content: Some(current),
                current_content_hash: Some(&hash),
                accepted_or_final_unlock_confirmed: false,
                create_chapter_id: None,
            })
            .unwrap_err(),
            ChapterPatchError::LockedStatus
        );
        let patch = build_chapter_patch(BuildChapterPatchInputV1 {
            proposal: &proposal,
            continuity_report: &report,
            current_content: Some(current),
            current_content_hash: Some(&hash),
            accepted_or_final_unlock_confirmed: true,
            create_chapter_id: None,
        })
        .unwrap();
        assert!(patch.target_content.contains("status: draft"));
        assert_eq!(patch.preview.previous_status.as_deref(), Some("accepted"));
        assert_eq!(patch.preview.hunks.len(), 2);
    }

    #[test]
    fn ng_505_create_never_overwrites_and_revise_requires_matching_hash() {
        let create = proposal(StoryActionV2::Create);
        let current = "occupied";
        let hash = sha256_hex(current.as_bytes());
        assert_eq!(
            build_chapter_patch(BuildChapterPatchInputV1 {
                proposal: &create,
                continuity_report: &report(&create, None),
                current_content: Some(current),
                current_content_hash: Some(&hash),
                accepted_or_final_unlock_confirmed: false,
                create_chapter_id: None,
            })
            .unwrap_err(),
            ChapterPatchError::TargetAlreadyExists
        );

        let revise = proposal(StoryActionV2::Revise);
        let target_hash = "f".repeat(64);
        let mismatched_hash = "e".repeat(64);
        assert_eq!(
            build_chapter_patch(BuildChapterPatchInputV1 {
                proposal: &revise,
                continuity_report: &report(&revise, Some(target_hash.clone())),
                current_content: Some(current),
                current_content_hash: Some(&mismatched_hash),
                accepted_or_final_unlock_confirmed: false,
                create_chapter_id: None,
            })
            .unwrap_err(),
            ChapterPatchError::SourceHashMismatch
        );
    }
}
