use crate::chapter::{ChapterProposalSourceStatusV1, ChapterProposalV1};
use crate::story_context::{
    sha256_hex, StoryActionV2, StoryContextRequestSnapshotV1, StoryContextSourceKindV2,
    StoryReaderMentalModelV1, StoryTargetChapterV2,
};
use novelgainker_validation::validate_source;
use serde::Serialize;

pub const CONTINUITY_CHECK_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContinuitySeverityV1 {
    Blocking,
    Warning,
    Unknown,
    Passed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuitySourceV1 {
    pub source_id: String,
    pub kind: StoryContextSourceKindV2,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuityTargetStateV1 {
    pub available: bool,
    pub exists: bool,
    pub content_hash: Option<String>,
    pub content: Option<String>,
}

impl ContinuityTargetStateV1 {
    pub fn missing() -> Self {
        Self {
            available: true,
            exists: false,
            content_hash: None,
            content: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityRuleResultV1 {
    pub rule_id: String,
    pub severity: ContinuitySeverityV1,
    pub title: String,
    pub explanation: String,
    pub evidence: Vec<String>,
    pub source_ids: Vec<String>,
    pub suggested_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuitySummaryV1 {
    pub blocking: u16,
    pub warning: u16,
    pub unknown: u16,
    pub passed: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryContinuityReportV1 {
    pub schema_version: u16,
    pub report_hash: String,
    pub proposal_hash: String,
    pub source_manifest_hash: String,
    pub current_manifest_hash: Option<String>,
    pub generation_target_hash: Option<String>,
    pub current_target_hash: Option<String>,
    pub checked_at: u64,
    pub local_only: bool,
    pub results: Vec<ContinuityRuleResultV1>,
    pub summary: ContinuitySummaryV1,
    pub acknowledged_warning_rule_ids: Vec<String>,
    pub acknowledged_unknown_rule_ids: Vec<String>,
    pub warnings_confirmed: bool,
    pub unknowns_confirmed: bool,
    pub review_ready: bool,
    pub write_enabled: bool,
}

pub struct ContinuityCheckInputV1<'a> {
    pub proposal: &'a ChapterProposalV1,
    pub expected_action: StoryActionV2,
    pub expected_target: &'a StoryTargetChapterV2,
    pub reader_model: &'a StoryReaderMentalModelV1,
    pub current_snapshot: Option<&'a StoryContextRequestSnapshotV1>,
    pub sources: &'a [ContinuitySourceV1],
    pub generation_target: &'a ContinuityTargetStateV1,
    pub current_target: &'a ContinuityTargetStateV1,
    pub checked_at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportHashMaterial<'a> {
    schema_version: u16,
    proposal_hash: &'a str,
    source_manifest_hash: &'a str,
    current_manifest_hash: &'a Option<String>,
    generation_target_hash: &'a Option<String>,
    current_target_hash: &'a Option<String>,
    results: &'a [ContinuityRuleResultV1],
}

macro_rules! push_result {
    (
        $results:expr,
        $rule_id:expr,
        $severity:expr,
        $title:expr,
        $explanation:expr,
        $evidence:expr,
        $source_ids:expr,
        $suggested_action:expr $(,)?
    ) => {{
        let mut source_ids: Vec<String> = $source_ids;
        source_ids.sort();
        source_ids.dedup();
        $results.push(ContinuityRuleResultV1 {
            rule_id: $rule_id.to_owned(),
            severity: $severity,
            title: $title.to_owned(),
            explanation: $explanation.to_owned(),
            evidence: $evidence,
            source_ids,
            suggested_action: $suggested_action.to_owned(),
        });
    }};
}

pub fn check_story_continuity(input: ContinuityCheckInputV1<'_>) -> StoryContinuityReportV1 {
    let proposal = input.proposal;
    let mut results = Vec::with_capacity(17);
    let current_manifest_hash = input
        .current_snapshot
        .map(|value| value.manifest.aggregate_hash.clone());

    let path_safe = safe_target_path(&proposal.target_chapter.relative_path);
    push_result!(
        &mut results,
        "chapter.target_path_safe",
        bool_severity(path_safe, ContinuitySeverityV1::Blocking),
        "目标路径位于项目内",
        if path_safe {
            "目标是固定 chapters Markdown 相对路径。"
        } else {
            "目标包含绝对路径、父目录或非章节 Markdown 路径。"
        },
        vec![format!("target={}", proposal.target_chapter.relative_path)],
        vec![],
        "返回上下文步骤并重新选择合法章节编号。",
    );

    let expected_path = format!("chapters/{:03}.md", proposal.target_chapter.chapter_number);
    let target_bound = proposal.action == input.expected_action
        && proposal.target_chapter == *input.expected_target
        && proposal.target_chapter.relative_path == expected_path;
    push_result!(
        &mut results,
        "chapter.proposal_target_binding",
        bool_severity(target_bound, ContinuitySeverityV1::Blocking),
        "提案动作与目标绑定",
        if target_bound {
            "提案动作、章节编号、标题和固定路径与已复核上下文一致。"
        } else {
            "提案动作或目标与已复核上下文不一致。"
        },
        vec![
            format!("proposalAction={:?}", proposal.action),
            format!("expectedAction={:?}", input.expected_action),
            format!("expectedPath={expected_path}"),
        ],
        vec![],
        "废弃此提案并从最新上下文重新生成。",
    );

    let response_complete = proposal.response_complete && proposal.finish_reason == "stop";
    push_result!(
        &mut results,
        "chapter.provider_response_complete",
        bool_severity(response_complete, ContinuitySeverityV1::Blocking),
        "Provider 响应完整",
        if response_complete {
            "响应以 stop 正常结束并通过结构化解析。"
        } else {
            "响应未完整结束，不能形成可写候选。"
        },
        vec![
            format!("finishReason={}", proposal.finish_reason),
            format!("responseComplete={}", proposal.response_complete),
        ],
        vec![],
        "重新生成完整提案。",
    );

    let source_current = input.current_snapshot.is_some_and(|snapshot| {
        proposal.source_status == ChapterProposalSourceStatusV1::Current
            && proposal.source_manifest_hash == snapshot.manifest.aggregate_hash
            && proposal.current_manifest_hash == snapshot.manifest.aggregate_hash
            && proposal.request_hash == snapshot.request_hash
    });
    push_result!(
        &mut results,
        "chapter.source_snapshot_fresh",
        bool_severity(source_current, ContinuitySeverityV1::Blocking),
        "生成来源仍为当前版本",
        if source_current {
            "Manifest、Request 与提案绑定 Hash 均未变化。"
        } else {
            "来源、Reader Model 或 Manifest 在生成后发生变化，或当前快照无法重建。"
        },
        vec![
            format!("proposalManifest={}", proposal.source_manifest_hash),
            format!(
                "currentManifest={}",
                current_manifest_hash.as_deref().unwrap_or("unavailable")
            ),
            format!("proposalRequest={}", proposal.request_hash),
        ],
        vec![],
        "回到上下文步骤复核最新来源并重新生成提案。",
    );

    let target_fresh = input.generation_target.available
        && input.current_target.available
        && input.generation_target.exists == input.current_target.exists
        && input.generation_target.content_hash == input.current_target.content_hash;
    push_result!(
        &mut results,
        "chapter.target_snapshot_fresh",
        bool_severity(target_fresh, ContinuitySeverityV1::Blocking),
        "目标文件未在预览后变化",
        if target_fresh {
            "目标文件的存在状态与内容 Hash 未变化。"
        } else {
            "目标文件在生成后变化，或无法可靠读取当前状态。"
        },
        vec![
            format!("generationTarget={}", target_label(input.generation_target)),
            format!("currentTarget={}", target_label(input.current_target)),
        ],
        vec![],
        "重新加载目标并重新生成提案。",
    );

    let target_conflict = match proposal.action {
        StoryActionV2::Create => input.generation_target.exists || input.current_target.exists,
        StoryActionV2::Revise => !input.generation_target.exists || !input.current_target.exists,
    };
    let target_ok = input.current_target.available && !target_conflict;
    push_result!(
        &mut results,
        "chapter.target_action_conflict",
        bool_severity(target_ok, ContinuitySeverityV1::Blocking),
        "创建/修订目标状态有效",
        if !input.current_target.available {
            "无法读取目标文件状态。"
        } else if target_conflict {
            "创建目标已存在，或修订目标不存在。"
        } else {
            "目标存在状态符合创建或修订动作。"
        },
        vec![
            format!("action={:?}", proposal.action),
            format!("targetExists={}", input.current_target.exists),
        ],
        vec![],
        "调整动作或章节编号后重新生成。",
    );

    let candidate = candidate_document(proposal, input.current_target.content.as_deref());
    let validation = validate_source(&proposal.target_chapter.relative_path, &candidate);
    let structure_codes = validation
        .diagnostics
        .iter()
        .filter(|item| item.code.starts_with("chapter.draft_heading"))
        .map(|item| item.code)
        .collect::<Vec<_>>();
    push_result!(
        &mut results,
        "chapter.single_draft_block",
        bool_severity(structure_codes.is_empty(), ContinuitySeverityV1::Blocking),
        "章节只包含一个 Draft 区段",
        if structure_codes.is_empty() {
            "内存候选恰好包含一个 ## Draft。"
        } else {
            "内存候选缺少或重复 ## Draft。"
        },
        if structure_codes.is_empty() {
            vec!["draftBlocks=1".to_owned()]
        } else {
            structure_codes
                .iter()
                .map(|code| format!("diagnostic={code}"))
                .collect()
        },
        vec![],
        "修复目标章节结构后重新检查。",
    );

    let draft_empty = validation
        .diagnostics
        .iter()
        .any(|item| item.code == "chapter.draft_empty")
        || proposal.draft_markdown.trim().is_empty();
    push_result!(
        &mut results,
        "chapter.draft_nonempty",
        if draft_empty {
            ContinuitySeverityV1::Blocking
        } else {
            ContinuitySeverityV1::Passed
        },
        "Draft 正文非空",
        if draft_empty {
            "提案 Draft 没有可写正文。"
        } else {
            "提案 Draft 包含正文。"
        },
        vec![format!("draftBytes={}", proposal.draft_markdown.len())],
        vec![],
        "重新生成或补充章节正文。",
    );

    let frontmatter_codes = validation
        .diagnostics
        .iter()
        .filter(|item| !item.code.starts_with("chapter."))
        .map(|item| item.code)
        .collect::<Vec<_>>();
    push_result!(
        &mut results,
        "chapter.frontmatter_valid",
        bool_severity(frontmatter_codes.is_empty(), ContinuitySeverityV1::Blocking),
        "章节 Frontmatter 有效",
        if frontmatter_codes.is_empty() {
            "内存候选满足简体中文章节元数据约束。"
        } else {
            "目标章节 Frontmatter 无法通过安全校验。"
        },
        if frontmatter_codes.is_empty() {
            vec!["writingLanguage=zh-CN".to_owned()]
        } else {
            frontmatter_codes
                .iter()
                .map(|code| format!("diagnostic={code}"))
                .collect()
        },
        vec![],
        "修复 title、writingLanguage 或 YAML 结构后重新检查。",
    );

    let final_locked = proposal.action == StoryActionV2::Revise
        && input
            .current_target
            .content
            .as_deref()
            .is_some_and(is_final_chapter);
    push_result!(
        &mut results,
        "chapter.final_revision_unlocked",
        if final_locked {
            ContinuitySeverityV1::Blocking
        } else {
            ContinuitySeverityV1::Passed
        },
        "终稿修订锁",
        if final_locked {
            "目标章节状态为 final，当前版本没有解锁授权。"
        } else {
            "目标不是锁定终稿。"
        },
        vec![format!("finalLocked={final_locked}")],
        vec![],
        "先在后续受控流程中显式解锁终稿修订。",
    );

    let outline_ids = source_ids(input.current_snapshot, StoryContextSourceKindV2::Outline);
    let title_match = proposal.title.trim() == input.expected_target.chapter_title.trim();
    push_result!(
        &mut results,
        "story.outline_title_match",
        bool_severity(title_match, ContinuitySeverityV1::Warning),
        "提案标题与目标纲要标题一致",
        if title_match {
            "提案标题与已复核目标标题一致。"
        } else {
            "提案标题偏离目标章节标题，可能是有意改名，也可能是跑题。"
        },
        vec![
            format!("proposalTitle={}", proposal.title),
            format!("targetTitle={}", input.expected_target.chapter_title),
        ],
        outline_ids,
        "确认改名意图，或修改标题后重新生成。",
    );

    let unsourced = proposal
        .continuity_claims
        .iter()
        .filter(|claim| claim.source_ids.is_empty())
        .map(|claim| claim.claim.clone())
        .collect::<Vec<_>>();
    push_result!(
        &mut results,
        "story.new_entity_claims_sourced",
        bool_severity(unsourced.is_empty(), ContinuitySeverityV1::Warning),
        "新人物、地点、物件与规则声明有来源",
        if unsourced.is_empty() {
            "所有连续性声明都附带显式来源 ID，或没有声明。"
        } else {
            "部分连续性声明没有来源，可能引入未建立的新实体或规则。"
        },
        if unsourced.is_empty() {
            vec![format!("claimCount={}", proposal.continuity_claims.len())]
        } else {
            unsourced
                .into_iter()
                .map(|value| format!("unsourced={value}"))
                .collect()
        },
        proposal
            .continuity_claims
            .iter()
            .flat_map(|claim| claim.source_ids.clone())
            .collect(),
        "核对声明并选择对应 Canon 来源。",
    );

    let promise_terms = terms(&input.reader_model.chapter_promises);
    let proposal_text =
        format!("{}\n{}", proposal.intent_summary, proposal.draft_markdown).to_lowercase();
    let progressed = promise_terms.is_empty()
        || promise_terms
            .iter()
            .any(|term| proposal_text.contains(&term.to_lowercase()));
    push_result!(
        &mut results,
        "story.ledger_promises_progress",
        bool_severity(progressed, ContinuitySeverityV1::Warning),
        "既有章节承诺获得推进",
        if progressed {
            "未发现待推进承诺，或正文命中了至少一项承诺线索。"
        } else {
            "Reader Model 中仍有承诺，但正文未命中可解释的文本线索。"
        },
        vec![format!("promiseTerms={}", promise_terms.len())],
        source_ids(input.current_snapshot, StoryContextSourceKindV2::Ledger),
        "确认本章确实暂缓承诺，或补充推进内容。",
    );

    let recent_lengths = input
        .sources
        .iter()
        .filter(|source| source.kind == StoryContextSourceKindV2::ChapterDraft)
        .map(|source| source.content.chars().count())
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    let current_length = proposal.draft_markdown.chars().count();
    let baseline = median(&recent_lengths);
    let length_warning =
        baseline.is_some_and(|value| current_length * 2 < value || current_length > value * 2);
    push_result!(
        &mut results,
        "story.recent_chapter_length",
        if length_warning {
            ContinuitySeverityV1::Warning
        } else {
            ContinuitySeverityV1::Passed
        },
        "篇幅接近近期章节",
        if length_warning {
            "本章字数低于近期中位数的一半或高于两倍。"
        } else {
            "没有足够近期章节，或篇幅处于固定阈值内。"
        },
        vec![
            format!("draftChars={current_length}"),
            format!(
                "recentMedian={}",
                baseline.map_or("unavailable".to_owned(), |value| value.to_string())
            ),
        ],
        source_ids(
            input.current_snapshot,
            StoryContextSourceKindV2::ChapterDraft,
        ),
        "确认篇幅变化是叙事需要，或调整正文。",
    );

    let reveals = terms(&input.reader_model.forbidden_reveals)
        .into_iter()
        .filter(|term| proposal_text.contains(&term.to_lowercase()))
        .collect::<Vec<_>>();
    push_result!(
        &mut results,
        "story.forbidden_reveal_keywords",
        bool_severity(reveals.is_empty(), ContinuitySeverityV1::Warning),
        "禁揭示关键词扫描",
        if reveals.is_empty() {
            "正文未命中 Reader Model 中的禁揭示文本片段。"
        } else {
            "正文命中禁揭示关键词；本地规则不能判断语境是否真的泄露。"
        },
        reveals
            .into_iter()
            .map(|value| format!("matched={value}"))
            .collect(),
        vec![],
        "人工复核命中语境并明确确认。",
    );

    push_result!(
        &mut results,
        "story.reader_knows_regression",
        ContinuitySeverityV1::Unknown,
        "读者已知信息是否倒退",
        "仅靠字符串规则无法可靠判断叙事开篇是否让读者认知倒退。",
        vec![format!(
            "readerKnowsBytes={}",
            input.reader_model.reader_knows.len()
        )],
        vec![],
        "人工比较开篇与 Reader Model 的 readerKnows。",
    );
    push_result!(
        &mut results,
        "story.future_outline_fulfillment",
        ContinuitySeverityV1::Unknown,
        "是否过早兑现未来纲要",
        "本地规则无法可靠区分铺垫、误导与真正兑现。",
        vec![format!(
            "outlineSources={}",
            source_ids(input.current_snapshot, StoryContextSourceKindV2::Outline).len()
        )],
        source_ids(input.current_snapshot, StoryContextSourceKindV2::Outline),
        "人工核对未来章节纲要与本章揭示范围。",
    );

    let summary = summarize(&results);
    let generation_target_hash = input.generation_target.content_hash.clone();
    let current_target_hash = input.current_target.content_hash.clone();
    let hash_material = ReportHashMaterial {
        schema_version: CONTINUITY_CHECK_VERSION,
        proposal_hash: &proposal.proposal_hash,
        source_manifest_hash: &proposal.source_manifest_hash,
        current_manifest_hash: &current_manifest_hash,
        generation_target_hash: &generation_target_hash,
        current_target_hash: &current_target_hash,
        results: &results,
    };
    let report_hash = serde_json::to_vec(&hash_material)
        .map(|value| sha256_hex(&value))
        .unwrap_or_else(|_| "0".repeat(64));
    let warnings_confirmed = summary.warning == 0;
    let unknowns_confirmed = summary.unknown == 0;
    StoryContinuityReportV1 {
        schema_version: CONTINUITY_CHECK_VERSION,
        report_hash,
        proposal_hash: proposal.proposal_hash.clone(),
        source_manifest_hash: proposal.source_manifest_hash.clone(),
        current_manifest_hash,
        generation_target_hash,
        current_target_hash,
        checked_at: input.checked_at,
        local_only: true,
        results,
        summary: summary.clone(),
        acknowledged_warning_rule_ids: Vec::new(),
        acknowledged_unknown_rule_ids: Vec::new(),
        warnings_confirmed,
        unknowns_confirmed,
        review_ready: summary.blocking == 0 && warnings_confirmed && unknowns_confirmed,
        write_enabled: false,
    }
}

pub fn confirm_story_continuity_warnings(
    report: &mut StoryContinuityReportV1,
    warning_rule_ids: &[String],
    unknown_rule_ids: &[String],
) -> bool {
    let expected_warnings = report
        .results
        .iter()
        .filter(|item| item.severity == ContinuitySeverityV1::Warning)
        .map(|item| item.rule_id.clone())
        .collect::<Vec<_>>();
    let expected_unknowns = report
        .results
        .iter()
        .filter(|item| item.severity == ContinuitySeverityV1::Unknown)
        .map(|item| item.rule_id.clone())
        .collect::<Vec<_>>();
    if expected_warnings != warning_rule_ids || expected_unknowns != unknown_rule_ids {
        return false;
    }
    report.acknowledged_warning_rule_ids = expected_warnings;
    report.acknowledged_unknown_rule_ids = expected_unknowns;
    report.warnings_confirmed = true;
    report.unknowns_confirmed = true;
    report.review_ready = report.summary.blocking == 0;
    report.write_enabled = false;
    true
}

fn bool_severity(passed: bool, failed: ContinuitySeverityV1) -> ContinuitySeverityV1 {
    if passed {
        ContinuitySeverityV1::Passed
    } else {
        failed
    }
}
fn target_label(target: &ContinuityTargetStateV1) -> &str {
    target.content_hash.as_deref().unwrap_or(if target.exists {
        "unavailable"
    } else {
        "missing"
    })
}
fn source_ids(
    snapshot: Option<&StoryContextRequestSnapshotV1>,
    kind: StoryContextSourceKindV2,
) -> Vec<String> {
    snapshot
        .into_iter()
        .flat_map(|value| value.manifest.selected_sources.iter())
        .filter(|source| source.kind == kind)
        .map(|source| source.source_id.clone())
        .collect()
}
fn safe_target_path(path: &str) -> bool {
    path.starts_with("chapters/")
        && path.ends_with(".md")
        && !path.contains("..")
        && !path.contains('\\')
        && !path.contains(':')
        && !path.starts_with('/')
}

fn candidate_document(proposal: &ChapterProposalV1, current: Option<&str>) -> String {
    if proposal.action == StoryActionV2::Create {
        return format!("---\nid: chapter-{:03}\ntitle: {}\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: {}\nstatus: draft\n---\n\n# {}\n\n## Draft\n\n{}\n", proposal.target_chapter.chapter_number, proposal.title, proposal.target_chapter.chapter_number, proposal.title, proposal.draft_markdown);
    }
    current
        .map(|content| replace_draft(content, &proposal.draft_markdown))
        .unwrap_or_default()
}
fn replace_draft(content: &str, draft: &str) -> String {
    let lines = content.lines().collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|line| line.trim_end() == "## Draft") else {
        return content.to_owned();
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(lines.len());
    let mut result = lines[..=start].join("\n");
    result.push_str("\n\n");
    result.push_str(draft.trim());
    result.push('\n');
    if end < lines.len() {
        result.push('\n');
        result.push_str(&lines[end..].join("\n"));
        result.push('\n');
    }
    result
}
fn is_final_chapter(content: &str) -> bool {
    content
        .lines()
        .skip(1)
        .take_while(|line| *line != "---")
        .any(|line| line.trim().eq_ignore_ascii_case("status: final"))
}
fn terms(value: &str) -> Vec<String> {
    value
        .lines()
        .flat_map(|line| line.split(['，', '。', '；', ';']))
        .map(str::trim)
        .filter(|item| item.chars().count() >= 2)
        .take(64)
        .map(ToOwned::to_owned)
        .collect()
}
fn median(values: &[usize]) -> Option<usize> {
    let mut values = values.to_vec();
    values.sort_unstable();
    values.get(values.len() / 2).copied()
}
fn summarize(results: &[ContinuityRuleResultV1]) -> ContinuitySummaryV1 {
    let mut value = ContinuitySummaryV1 {
        blocking: 0,
        warning: 0,
        unknown: 0,
        passed: 0,
    };
    for result in results {
        match result.severity {
            ContinuitySeverityV1::Blocking => value.blocking += 1,
            ContinuitySeverityV1::Warning => value.warning += 1,
            ContinuitySeverityV1::Unknown => value.unknown += 1,
            ContinuitySeverityV1::Passed => value.passed += 1,
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chapter::{build_chapter_proposal, ChapterProposalPayloadV1};
    use crate::story_context::{
        build_story_context_manifest, build_story_context_request_snapshot,
        BuildStoryContextInputV2, StoryContextSelectionReasonV2, StoryContextSourceInputV2,
    };
    fn fixture() -> (StoryContextRequestSnapshotV1, ChapterProposalV1) {
        let manifest = build_story_context_manifest(BuildStoryContextInputV2 {
            workflow_id: crate::STORYTELLING_WORKFLOW_ID.to_owned(),
            project_root_identity: "project".to_owned(),
            action: StoryActionV2::Create,
            chapter_number: 2,
            chapter_title: "潮门".to_owned(),
            sources: vec![StoryContextSourceInputV2 {
                source_id: "builtin:protocol".to_owned(),
                kind: StoryContextSourceKindV2::BuiltinProtocol,
                relative_path: None,
                builtin_resource_id: Some("protocol".to_owned()),
                display_name: "协议".to_owned(),
                selection_reason: StoryContextSelectionReasonV2::Required,
                required: true,
                user_deselectable: false,
                priority: 1,
                content: "协议".to_owned(),
                content_hash: sha256_hex("协议".as_bytes()),
                source_byte_count: 6,
                modified_time_ms: 0,
                included: true,
            }],
        })
        .unwrap();
        let snapshot = build_story_context_request_snapshot(
            manifest.clone(),
            manifest.aggregate_hash.clone(),
            StoryReaderMentalModelV1 {
                reader_knows: "潮门存在".to_owned(),
                reader_infers: String::new(),
                reader_misreads: String::new(),
                reader_questions: String::new(),
                chapter_promises: String::new(),
                forbidden_reveals: "真凶".to_owned(),
            },
        )
        .unwrap();
        let proposal = build_chapter_proposal(
            &snapshot,
            ChapterProposalPayloadV1 {
                schema_version: 1,
                title: "潮门".to_owned(),
                draft_markdown: "雨落在潮门上。".to_owned(),
                intent_summary: "推进调查".to_owned(),
                reader_model_delta: StoryReaderMentalModelV1 {
                    reader_knows: String::new(),
                    reader_infers: String::new(),
                    reader_misreads: String::new(),
                    reader_questions: String::new(),
                    chapter_promises: String::new(),
                    forbidden_reveals: String::new(),
                },
                continuity_claims: vec![],
                ledger_candidates: vec![],
            },
            manifest.aggregate_hash.clone(),
            "deepseek".to_owned(),
            "deepseek-v4-pro".to_owned(),
            1,
            2,
            "stop".to_owned(),
        )
        .unwrap();
        (snapshot, proposal)
    }
    #[test]
    fn ng_504_results_are_stably_ordered_and_semantic_uncertainty_is_not_passed() {
        let (snapshot, proposal) = fixture();
        let target = ContinuityTargetStateV1::missing();
        let first = check_story_continuity(ContinuityCheckInputV1 {
            proposal: &proposal,
            expected_action: StoryActionV2::Create,
            expected_target: &snapshot.manifest.target_chapter,
            reader_model: &snapshot.reader_mental_model,
            current_snapshot: Some(&snapshot),
            sources: &[],
            generation_target: &target,
            current_target: &target,
            checked_at: 1,
        });
        let second = check_story_continuity(ContinuityCheckInputV1 {
            proposal: &proposal,
            expected_action: StoryActionV2::Create,
            expected_target: &snapshot.manifest.target_chapter,
            reader_model: &snapshot.reader_mental_model,
            current_snapshot: Some(&snapshot),
            sources: &[],
            generation_target: &target,
            current_target: &target,
            checked_at: 2,
        });
        assert_eq!(first.report_hash, second.report_hash);
        assert_eq!(
            first
                .results
                .iter()
                .map(|item| item.rule_id.as_str())
                .collect::<Vec<_>>(),
            second
                .results
                .iter()
                .map(|item| item.rule_id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(first.summary.unknown, 2);
        assert!(!first.write_enabled);
    }
    #[test]
    fn ng_504_source_stale_and_target_conflict_are_blocking_and_warnings_require_exact_ack() {
        let (snapshot, proposal) = fixture();
        let generated = ContinuityTargetStateV1::missing();
        let current = ContinuityTargetStateV1 {
            available: true,
            exists: true,
            content_hash: Some("a".repeat(64)),
            content: Some("occupied".to_owned()),
        };
        let mut report = check_story_continuity(ContinuityCheckInputV1 {
            proposal: &proposal,
            expected_action: StoryActionV2::Create,
            expected_target: &snapshot.manifest.target_chapter,
            reader_model: &snapshot.reader_mental_model,
            current_snapshot: None,
            sources: &[],
            generation_target: &generated,
            current_target: &current,
            checked_at: 1,
        });
        assert!(report.summary.blocking >= 2);
        assert!(!confirm_story_continuity_warnings(
            &mut report,
            &["forged".to_owned()],
            &[]
        ));
        let warning_ids = report
            .results
            .iter()
            .filter(|item| item.severity == ContinuitySeverityV1::Warning)
            .map(|item| item.rule_id.clone())
            .collect::<Vec<_>>();
        let unknown_ids = report
            .results
            .iter()
            .filter(|item| item.severity == ContinuitySeverityV1::Unknown)
            .map(|item| item.rule_id.clone())
            .collect::<Vec<_>>();
        assert!(confirm_story_continuity_warnings(
            &mut report,
            &warning_ids,
            &unknown_ids
        ));
        assert!(!report.review_ready);
        assert!(!report.write_enabled);
    }
}
