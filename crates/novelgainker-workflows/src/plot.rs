use std::collections::{HashMap, HashSet};
use std::fmt;

use novelgainker_validation::validate_source_bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{workflow_resources, PLOT_CREATE_WORKFLOW_ID};

pub const PLOT_PROPOSAL_SCHEMA_VERSION: u16 = 1;
pub const MAX_PLOT_RESPONSE_BYTES: usize = 96 * 1024;
pub const MAX_REPAIR_CAPTURE_BYTES: usize = 8 * 1024;
pub const MAIN_PLOT_RELATIVE_PATH: &str = "plot/main.md";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlotProposalV1 {
    pub schema_version: u16,
    pub workflow_id: String,
    pub plot: PlotDataV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlotDataV1 {
    pub title: String,
    pub promise: String,
    pub pressure: String,
    pub turns: Vec<String>,
    pub reveals: Vec<String>,
    pub payoffs: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotSourceMetadataV1 {
    pub plot_id: String,
    pub title: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PlotArtifactV1 {
    pub proposal: PlotProposalV1,
    pub plot_id: String,
    pub relative_path: String,
    pub markdown: String,
}

impl fmt::Debug for PlotArtifactV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlotArtifactV1")
            .field("plot_id", &self.plot_id)
            .field("relative_path", &self.relative_path)
            .field("proposal", &"[REDACTED]")
            .field("markdown", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotOutputError {
    Incomplete,
    ResponseTooLarge,
    InvalidJson,
    InvalidSchema,
    UnsafeText,
    InvalidEntityId,
    InvalidTarget,
    InvalidMarkdown,
}

impl PlotOutputError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Incomplete => "plot.output_incomplete",
            Self::ResponseTooLarge => "plot.output_too_large",
            Self::InvalidJson => "plot.output_invalid_json",
            Self::InvalidSchema => "plot.output_invalid_schema",
            Self::UnsafeText => "plot.output_unsafe_text",
            Self::InvalidEntityId => "plot.entity_id_invalid",
            Self::InvalidTarget => "plot.target_invalid",
            Self::InvalidMarkdown => "plot.markdown_invalid",
        }
    }
}

pub fn parse_plot_source_metadata(
    relative_path: &str,
    content: &str,
) -> Result<PlotSourceMetadataV1, PlotOutputError> {
    validate_plot_target(relative_path)?;
    if content.is_empty()
        || content.len() > MAX_PLOT_RESPONSE_BYTES
        || validate_source_bytes(relative_path, content.as_bytes())
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.severity == novelgainker_validation::DiagnosticSeverity::Error
            })
    {
        return Err(PlotOutputError::InvalidMarkdown);
    }
    let header = content
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(header, _)| header))
        .ok_or(PlotOutputError::InvalidMarkdown)?;
    let mut fields = HashMap::new();
    for line in header.lines() {
        let (key, value) = line
            .split_once(": ")
            .ok_or(PlotOutputError::InvalidMarkdown)?;
        if fields.insert(key, value).is_some() {
            return Err(PlotOutputError::InvalidMarkdown);
        }
    }
    if fields.len() != 5
        || fields.get("writingLanguage") != Some(&"zh-CN")
        || fields.get("kind") != Some(&"plot")
        || fields.get("schemaVersion") != Some(&"1")
    {
        return Err(PlotOutputError::InvalidMarkdown);
    }
    let plot_id: String =
        serde_json::from_str(fields.get("id").ok_or(PlotOutputError::InvalidMarkdown)?)
            .map_err(|_| PlotOutputError::InvalidMarkdown)?;
    let title: String = serde_json::from_str(
        fields
            .get("title")
            .ok_or(PlotOutputError::InvalidMarkdown)?,
    )
    .map_err(|_| PlotOutputError::InvalidMarkdown)?;
    validate_plot_id(&plot_id)?;
    validate_plain_text(&title, 120)?;
    Ok(PlotSourceMetadataV1 { plot_id, title })
}

pub fn new_plot_id() -> String {
    format!("plot_{}", Uuid::new_v4())
}

pub fn parse_and_serialize_plot(
    response: &str,
    plot_id: &str,
) -> Result<PlotArtifactV1, PlotOutputError> {
    validate_plot_id(plot_id)?;
    let proposal = parse_plot_proposal(response)?;
    let markdown = serialize_plot_markdown(&proposal.plot, plot_id)?;
    Ok(PlotArtifactV1 {
        proposal,
        plot_id: plot_id.to_owned(),
        relative_path: MAIN_PLOT_RELATIVE_PATH.to_owned(),
        markdown,
    })
}

pub fn parse_and_serialize_plot_replace(
    response: &str,
    plot_id: &str,
    target_relative_path: &str,
) -> Result<PlotArtifactV1, PlotOutputError> {
    validate_plot_id(plot_id)?;
    validate_plot_target(target_relative_path)?;
    let proposal = parse_plot_proposal(response)?;
    let markdown = serialize_plot_markdown(&proposal.plot, plot_id)?;
    Ok(PlotArtifactV1 {
        proposal,
        plot_id: plot_id.to_owned(),
        relative_path: MAIN_PLOT_RELATIVE_PATH.to_owned(),
        markdown,
    })
}

pub fn parse_plot_proposal(response: &str) -> Result<PlotProposalV1, PlotOutputError> {
    if response.is_empty() || response.len() > MAX_PLOT_RESPONSE_BYTES {
        return Err(if response.is_empty() {
            PlotOutputError::InvalidJson
        } else {
            PlotOutputError::ResponseTooLarge
        });
    }
    let proposal: PlotProposalV1 =
        serde_json::from_str(response).map_err(|_| PlotOutputError::InvalidJson)?;
    if proposal.schema_version != PLOT_PROPOSAL_SCHEMA_VERSION
        || proposal.workflow_id != PLOT_CREATE_WORKFLOW_ID
    {
        return Err(PlotOutputError::InvalidSchema);
    }
    validate_plot(&proposal)?;
    Ok(proposal)
}

pub fn build_plot_repair_prompt(error: PlotOutputError, malformed: &str) -> String {
    let schema = workflow_resources(PLOT_CREATE_WORKFLOW_ID)
        .map(|resources| resources.output_schema)
        .unwrap_or("{}");
    let bounded = bounded_utf8_prefix(malformed, MAX_REPAIR_CAPTURE_BYTES);
    format!(
        "上一次输出未通过本地严格校验。错误代码：{}。\n请只返回一个满足下列 JSON Schema 的 JSON 对象，不要解释，不要使用 Markdown 代码块，不要增加未知字段。\nJSON Schema：\n{}\n上一次不合格输出（仅作修复输入）：\n{}",
        error.code(), schema, bounded
    )
}

fn validate_plot(proposal: &PlotProposalV1) -> Result<(), PlotOutputError> {
    validate_plain_text(&proposal.plot.title, 120)?;
    validate_plain_text(&proposal.plot.promise, 2_000)?;
    validate_plain_text(&proposal.plot.pressure, 2_000)?;
    validate_unique_list(&proposal.plot.turns, 3, 12, 1_000)?;
    validate_unique_list(&proposal.plot.reveals, 1, 8, 1_000)?;
    validate_unique_list(&proposal.plot.payoffs, 1, 8, 1_000)?;
    validate_unique_list(&proposal.plot.open_questions, 0, 8, 1_000)?;
    Ok(())
}

fn validate_unique_list(
    values: &[String],
    minimum: usize,
    maximum: usize,
    max_characters: usize,
) -> Result<(), PlotOutputError> {
    if values.len() < minimum || values.len() > maximum {
        return Err(PlotOutputError::InvalidSchema);
    }
    let mut unique = HashSet::with_capacity(values.len());
    for value in values {
        validate_plain_text(value, max_characters)?;
        if !unique.insert(value.as_str()) {
            return Err(PlotOutputError::InvalidSchema);
        }
    }
    Ok(())
}

fn validate_plain_text(value: &str, max_characters: usize) -> Result<(), PlotOutputError> {
    let count = value.chars().count();
    if count == 0 || count > max_characters || value.trim() != value {
        return Err(PlotOutputError::InvalidSchema);
    }
    let lower = value.to_ascii_lowercase();
    if value.chars().any(is_unsafe_character)
        || value.contains('<')
        || value.contains('>')
        || lower.contains("javascript:")
        || lower.contains("file://")
    {
        return Err(PlotOutputError::UnsafeText);
    }
    Ok(())
}

fn is_unsafe_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2069}'
                | '\u{feff}'
                | '\u{fffd}'
        )
        || (character as u32 & 0xffff == 0xffff)
        || (character as u32 & 0xffff == 0xfffe)
}

fn validate_plot_id(plot_id: &str) -> Result<(), PlotOutputError> {
    let suffix = plot_id
        .strip_prefix("plot_")
        .ok_or(PlotOutputError::InvalidEntityId)?;
    let parsed = Uuid::parse_str(suffix).map_err(|_| PlotOutputError::InvalidEntityId)?;
    if parsed.to_string() != suffix {
        return Err(PlotOutputError::InvalidEntityId);
    }
    Ok(())
}

fn validate_plot_target(relative_path: &str) -> Result<(), PlotOutputError> {
    if relative_path != MAIN_PLOT_RELATIVE_PATH {
        return Err(PlotOutputError::InvalidTarget);
    }
    Ok(())
}

fn serialize_plot_markdown(plot: &PlotDataV1, plot_id: &str) -> Result<String, PlotOutputError> {
    let resources =
        workflow_resources(PLOT_CREATE_WORKFLOW_ID).ok_or(PlotOutputError::InvalidMarkdown)?;
    let plot_id_yaml =
        serde_json::to_string(plot_id).map_err(|_| PlotOutputError::InvalidMarkdown)?;
    let title_yaml =
        serde_json::to_string(&plot.title).map_err(|_| PlotOutputError::InvalidMarkdown)?;
    let open_questions = if plot.open_questions.is_empty() {
        "- 暂无".to_owned()
    } else {
        markdown_list(&plot.open_questions)
    };
    let markdown = resources
        .template_resource
        .replace("{{plotIdYaml}}", &plot_id_yaml)
        .replace("{{titleYaml}}", &title_yaml)
        .replace("{{title}}", &escape_markdown(&plot.title))
        .replace("{{promise}}", &escape_markdown(&plot.promise))
        .replace("{{pressure}}", &escape_markdown(&plot.pressure))
        .replace("{{turns}}", &markdown_ordered_list(&plot.turns))
        .replace("{{reveals}}", &markdown_ordered_list(&plot.reveals))
        .replace("{{payoffs}}", &markdown_ordered_list(&plot.payoffs))
        .replace("{{openQuestions}}", &open_questions);
    if markdown.contains("{{")
        || markdown.contains('\r')
        || markdown.starts_with('\u{feff}')
        || !markdown.ends_with('\n')
    {
        return Err(PlotOutputError::InvalidMarkdown);
    }
    let validation = validate_source_bytes(MAIN_PLOT_RELATIVE_PATH, markdown.as_bytes());
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(PlotOutputError::InvalidMarkdown);
    }
    Ok(markdown)
}

fn markdown_ordered_list(values: &[String]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| format!("{}. {}", index + 1, escape_markdown(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn markdown_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("- {}", escape_markdown(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_markdown(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '`'
                | '*'
                | '_'
                | '{'
                | '}'
                | '['
                | ']'
                | '('
                | ')'
                | '#'
                | '+'
                | '-'
                | '.'
                | '!'
                | '|'
                | '~'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn bounded_utf8_prefix(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    &value[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLOT_ID: &str = "plot_50000000-0000-4000-8000-000000000005";

    fn valid_json(title: &str) -> String {
        serde_json::json!({
            "schemaVersion": 1,
            "workflowId": PLOT_CREATE_WORKFLOW_ID,
            "plot": {
                "title": title,
                "promise": "林遥必须查明潮汐塔为何会提前鸣钟，否则雾城将失去最后一条安全航道。",
                "pressure": "每次异常退潮都会缩短调查时间，并让城市管理者更接近封锁真相。",
                "turns": [
                    "林遥在退潮后的塔基发现被删除的第四次钟声记录。",
                    "管理者宣布潮汐塔永久封闭，迫使林遥转向地下航道。",
                    "旧钥匙打开的并非塔门，而是保存真实历法的档案室。"
                ],
                "reveals": [
                    "所谓自然异常其实来自管理者人为改写潮汐历法。",
                    "林遥的导师曾参与改写，但留下钥匙作为纠错手段。"
                ],
                "payoffs": [
                    "第一幕出现的旧钥匙最终打开真实历法档案。",
                    "反复出现的第四次钟声成为公开真相的证据。"
                ],
                "openQuestions": [
                    "管理者为何宁愿牺牲安全航道也要隐藏真实历法？"
                ]
            }
        })
        .to_string()
    }

    #[test]
    fn fixed_target_sections_order_and_rust_identity_are_deterministic() {
        let first = parse_and_serialize_plot(&valid_json("雾城潮汐主线"), PLOT_ID).unwrap();
        let second = parse_and_serialize_plot(&valid_json("雾城潮汐主线"), PLOT_ID).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.relative_path, MAIN_PLOT_RELATIVE_PATH);
        assert!(first.markdown.contains(&format!("id: \"{PLOT_ID}\"")));
        assert!(first.markdown.contains("kind: plot"));
        assert!(first.markdown.contains("## 故事承诺"));
        assert!(first.markdown.contains("## 核心压力"));
        assert!(first.markdown.contains("## 转折"));
        assert!(first.markdown.contains("## 揭示"));
        assert!(first.markdown.contains("## 回收"));
        assert!(first.markdown.contains("## 未决问题"));
        assert!(first.markdown.contains("1. 林遥在退潮后的塔基"));
        assert!(first.markdown.ends_with('\n'));
        assert!(
            validate_source_bytes(&first.relative_path, first.markdown.as_bytes())
                .diagnostics
                .is_empty()
        );
    }

    #[test]
    fn explicit_replace_reuses_original_id_and_fixed_path() {
        let artifact = parse_and_serialize_plot_replace(
            &valid_json("修订主线"),
            PLOT_ID,
            MAIN_PLOT_RELATIVE_PATH,
        )
        .unwrap();
        assert_eq!(artifact.plot_id, PLOT_ID);
        assert_eq!(artifact.relative_path, MAIN_PLOT_RELATIVE_PATH);
        assert!(artifact.markdown.contains(r#"title: "修订主线""#));
        let metadata =
            parse_plot_source_metadata(&artifact.relative_path, &artifact.markdown).unwrap();
        assert_eq!(metadata.plot_id, PLOT_ID);
        assert_eq!(metadata.title, "修订主线");
    }

    #[test]
    fn provider_cannot_supply_identity_path_markdown_tool_or_operation() {
        for (field, value) in [
            ("id", serde_json::json!("plot_evil")),
            ("path", serde_json::json!("plot/evil.md")),
            ("frontmatter", serde_json::json!({"kind": "plot"})),
            ("markdown", serde_json::json!("# 已写入")),
            ("tool", serde_json::json!("source.create_plot")),
            ("operation", serde_json::json!("replace")),
        ] {
            let mut attacked: serde_json::Value =
                serde_json::from_str(&valid_json("主线")).unwrap();
            attacked[field] = value;
            assert_eq!(
                parse_plot_proposal(&attacked.to_string()),
                Err(PlotOutputError::InvalidJson)
            );
        }
    }

    #[test]
    fn list_limits_duplicates_unsafe_text_and_non_main_targets_are_rejected() {
        let mut duplicate: serde_json::Value = serde_json::from_str(&valid_json("主线")).unwrap();
        duplicate["plot"]["turns"] = serde_json::json!(["重复", "重复", "第三个"]);
        assert_eq!(
            parse_plot_proposal(&duplicate.to_string()),
            Err(PlotOutputError::InvalidSchema)
        );

        let mut too_short: serde_json::Value = serde_json::from_str(&valid_json("主线")).unwrap();
        too_short["plot"]["turns"] = serde_json::json!(["一个", "两个"]);
        assert_eq!(
            parse_plot_proposal(&too_short.to_string()),
            Err(PlotOutputError::InvalidSchema)
        );

        let mut unsafe_text: serde_json::Value = serde_json::from_str(&valid_json("主线")).unwrap();
        unsafe_text["plot"]["reveals"] = serde_json::json!(["<script>泄漏</script>"]);
        assert_eq!(
            parse_plot_proposal(&unsafe_text.to_string()),
            Err(PlotOutputError::UnsafeText)
        );

        for target in [
            "plot/side.md",
            "plot/../main.md",
            "outline/main.md",
            "plot/Main.md",
        ] {
            assert_eq!(
                parse_and_serialize_plot_replace(&valid_json("主线"), PLOT_ID, target),
                Err(PlotOutputError::InvalidTarget)
            );
            assert_eq!(
                parse_plot_source_metadata(target, "not markdown"),
                Err(PlotOutputError::InvalidTarget)
            );
        }
    }

    #[test]
    fn repair_prompt_is_bounded_and_workflow_specific() {
        let malformed = format!("PLOT_CANARY{}尾", "坏".repeat(MAX_REPAIR_CAPTURE_BYTES));
        let prompt = build_plot_repair_prompt(PlotOutputError::InvalidJson, &malformed);
        assert!(prompt.contains("plot.output_invalid_json"));
        assert!(prompt.contains("PlotProposalV1"));
        assert!(prompt.contains("PLOT_CANARY"));
        assert!(!prompt.ends_with('尾'));
        assert!(prompt.len() < 32 * 1024);
    }
}
