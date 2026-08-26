use std::collections::{HashMap, HashSet};
use std::fmt;

use novelgainker_validation::validate_source_bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{workflow_resources, SETTING_CREATE_WORKFLOW_ID};

pub const SETTING_PROPOSAL_SCHEMA_VERSION: u16 = 1;
pub const MAX_SETTING_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_REPAIR_CAPTURE_BYTES: usize = 8 * 1024;
const MAX_SETTING_FILENAME_BYTES: usize = 220;
const MAX_SETTING_FILENAME_UTF16_UNITS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingTypeV1 {
    World,
    Location,
    Rule,
}

impl SettingTypeV1 {
    pub const fn label(self) -> &'static str {
        match self {
            Self::World => "世界",
            Self::Location => "地点",
            Self::Rule => "规则",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::World => "world",
            Self::Location => "location",
            Self::Rule => "rule",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingProposalV1 {
    pub schema_version: u16,
    pub workflow_id: String,
    pub setting: SettingDataV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingDataV1 {
    pub display_name: String,

    pub setting_type: SettingTypeV1,
    pub summary: String,
    pub atmosphere: String,
    pub rules: Vec<String>,
    pub constraints: Vec<String>,
    pub plot_hooks: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingSourceMetadataV1 {
    pub setting_id: String,
    pub display_name: String,
    pub setting_type: SettingTypeV1,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SettingArtifactV1 {
    pub proposal: SettingProposalV1,
    pub setting_id: String,
    pub relative_path: String,
    pub markdown: String,
}

impl fmt::Debug for SettingArtifactV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SettingArtifactV1")
            .field("setting_id", &self.setting_id)
            .field("relative_path", &self.relative_path)
            .field("proposal", &"[REDACTED]")
            .field("markdown", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingOutputError {
    Incomplete,
    ResponseTooLarge,
    InvalidJson,
    InvalidSchema,
    UnsafeText,
    InvalidEntityId,
    InvalidFilename,
    InvalidMarkdown,
}

impl SettingOutputError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Incomplete => "setting.output_incomplete",
            Self::ResponseTooLarge => "setting.output_too_large",
            Self::InvalidJson => "setting.output_invalid_json",
            Self::InvalidSchema => "setting.output_invalid_schema",
            Self::UnsafeText => "setting.output_unsafe_text",
            Self::InvalidEntityId => "setting.entity_id_invalid",
            Self::InvalidFilename => "setting.filename_invalid",
            Self::InvalidMarkdown => "setting.markdown_invalid",
        }
    }
}

pub fn parse_setting_source_metadata(
    relative_path: &str,
    content: &str,
) -> Result<SettingSourceMetadataV1, SettingOutputError> {
    if content.is_empty()
        || content.len() > MAX_SETTING_RESPONSE_BYTES
        || !relative_path.starts_with("world/")
        || !relative_path.ends_with(".md")
        || validate_source_bytes(relative_path, content.as_bytes())
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.severity == novelgainker_validation::DiagnosticSeverity::Error
            })
    {
        return Err(SettingOutputError::InvalidMarkdown);
    }
    let header = content
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(header, _)| header))
        .ok_or(SettingOutputError::InvalidMarkdown)?;
    let mut fields = HashMap::new();
    for line in header.lines() {
        let (key, value) = line
            .split_once(": ")
            .ok_or(SettingOutputError::InvalidMarkdown)?;
        if fields.insert(key, value).is_some() {
            return Err(SettingOutputError::InvalidMarkdown);
        }
    }
    if fields.len() != 6
        || fields.get("writingLanguage") != Some(&"zh-CN")
        || fields.get("kind") != Some(&"setting")
        || fields.get("schemaVersion") != Some(&"1")
    {
        return Err(SettingOutputError::InvalidMarkdown);
    }
    let setting_id: String = serde_json::from_str(
        fields
            .get("id")
            .ok_or(SettingOutputError::InvalidMarkdown)?,
    )
    .map_err(|_| SettingOutputError::InvalidMarkdown)?;
    let display_name: String = serde_json::from_str(
        fields
            .get("title")
            .ok_or(SettingOutputError::InvalidMarkdown)?,
    )
    .map_err(|_| SettingOutputError::InvalidMarkdown)?;
    let setting_type = match fields.get("settingType").copied() {
        Some("world") => SettingTypeV1::World,
        Some("location") => SettingTypeV1::Location,
        Some("rule") => SettingTypeV1::Rule,
        _ => return Err(SettingOutputError::InvalidMarkdown),
    };
    validate_setting_id(&setting_id)?;
    validate_plain_text(&display_name, 80)?;
    setting_relative_path(&display_name)?;
    Ok(SettingSourceMetadataV1 {
        setting_id,
        display_name,
        setting_type,
    })
}

pub fn new_setting_id() -> String {
    format!("setting_{}", Uuid::new_v4())
}

pub fn parse_and_serialize_setting(
    response: &str,
    setting_id: &str,
) -> Result<SettingArtifactV1, SettingOutputError> {
    validate_setting_id(setting_id)?;
    let proposal = parse_setting_proposal(response)?;
    let relative_path = setting_relative_path(&proposal.setting.display_name)?;
    let markdown = serialize_setting_markdown(&proposal.setting, setting_id, &relative_path)?;
    Ok(SettingArtifactV1 {
        proposal,
        setting_id: setting_id.to_owned(),
        relative_path,
        markdown,
    })
}

pub fn parse_and_serialize_setting_replace(
    response: &str,
    setting_id: &str,
    target_relative_path: &str,
) -> Result<SettingArtifactV1, SettingOutputError> {
    validate_setting_id(setting_id)?;
    if !target_relative_path.starts_with("world/")
        || !target_relative_path.ends_with(".md")
        || target_relative_path.split('/').any(|component| {
            component.is_empty()
                || component == "."
                || component == ".."
                || component.starts_with('.')
                || component.as_bytes().contains(&92)
                || component.contains(':')
                || component.chars().any(char::is_control)
        })
    {
        return Err(SettingOutputError::InvalidFilename);
    }
    let proposal = parse_setting_proposal(response)?;
    setting_relative_path(&proposal.setting.display_name)?;
    let markdown = serialize_setting_markdown(&proposal.setting, setting_id, target_relative_path)?;
    Ok(SettingArtifactV1 {
        proposal,
        setting_id: setting_id.to_owned(),
        relative_path: target_relative_path.to_owned(),
        markdown,
    })
}

pub fn parse_setting_proposal(response: &str) -> Result<SettingProposalV1, SettingOutputError> {
    if response.is_empty() || response.len() > MAX_SETTING_RESPONSE_BYTES {
        return Err(if response.is_empty() {
            SettingOutputError::InvalidJson
        } else {
            SettingOutputError::ResponseTooLarge
        });
    }
    let proposal: SettingProposalV1 =
        serde_json::from_str(response).map_err(|_| SettingOutputError::InvalidJson)?;
    if proposal.schema_version != SETTING_PROPOSAL_SCHEMA_VERSION
        || proposal.workflow_id != SETTING_CREATE_WORKFLOW_ID
    {
        return Err(SettingOutputError::InvalidSchema);
    }
    validate_setting(&proposal)?;
    Ok(proposal)
}

pub fn build_setting_repair_prompt(error: SettingOutputError, malformed: &str) -> String {
    let schema = workflow_resources(SETTING_CREATE_WORKFLOW_ID)
        .map(|resources| resources.output_schema)
        .unwrap_or("{}");
    let bounded = bounded_utf8_prefix(malformed, MAX_REPAIR_CAPTURE_BYTES);
    format!(
        "上一次输出未通过本地严格校验。错误代码：{}。\n请只返回一个满足下列 JSON Schema 的 JSON 对象，不要解释，不要使用 Markdown 代码块，不要增加未知字段。\nJSON Schema：\n{}\n上一次不合格输出（仅作修复输入）：\n{}",
        error.code(), schema, bounded
    )
}

fn validate_setting(proposal: &SettingProposalV1) -> Result<(), SettingOutputError> {
    validate_plain_text(&proposal.setting.display_name, 80)?;
    validate_plain_text(&proposal.setting.summary, 800)?;
    validate_plain_text(&proposal.setting.atmosphere, 500)?;
    validate_unique_list(&proposal.setting.rules, 1, 12, 200)?;
    validate_unique_list(&proposal.setting.constraints, 0, 10, 200)?;
    validate_unique_list(&proposal.setting.plot_hooks, 1, 10, 300)?;
    Ok(())
}

fn validate_unique_list(
    values: &[String],
    minimum: usize,
    maximum: usize,
    max_characters: usize,
) -> Result<(), SettingOutputError> {
    if values.len() < minimum || values.len() > maximum {
        return Err(SettingOutputError::InvalidSchema);
    }
    let mut unique = HashSet::with_capacity(values.len());
    for value in values {
        validate_plain_text(value, max_characters)?;
        if !unique.insert(value.as_str()) {
            return Err(SettingOutputError::InvalidSchema);
        }
    }
    Ok(())
}

fn validate_plain_text(value: &str, max_characters: usize) -> Result<(), SettingOutputError> {
    let count = value.chars().count();
    if count == 0 || count > max_characters || value.trim() != value {
        return Err(SettingOutputError::InvalidSchema);
    }
    let lower = value.to_ascii_lowercase();
    if value.chars().any(is_unsafe_character)
        || value.contains('<')
        || value.contains('>')
        || lower.contains("javascript:")
        || lower.contains("file://")
    {
        return Err(SettingOutputError::UnsafeText);
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

fn validate_setting_id(setting_id: &str) -> Result<(), SettingOutputError> {
    let suffix = setting_id
        .strip_prefix("setting_")
        .ok_or(SettingOutputError::InvalidEntityId)?;
    let parsed = Uuid::parse_str(suffix).map_err(|_| SettingOutputError::InvalidEntityId)?;
    if parsed.to_string() != suffix {
        return Err(SettingOutputError::InvalidEntityId);
    }
    Ok(())
}

fn setting_relative_path(display_name: &str) -> Result<String, SettingOutputError> {
    if display_name.starts_with('.')
        || display_name.ends_with(['.', ' '])
        || display_name == "."
        || display_name == ".."
        || display_name.chars().any(|character| {
            matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        })
        || display_name.len() > MAX_SETTING_FILENAME_BYTES
        || display_name.encode_utf16().count() > MAX_SETTING_FILENAME_UTF16_UNITS
        || is_windows_reserved_name(display_name)
    {
        return Err(SettingOutputError::InvalidFilename);
    }
    let relative_path = format!("world/{display_name}.md");
    if relative_path.len() > 240 {
        return Err(SettingOutputError::InvalidFilename);
    }
    Ok(relative_path)
}

fn is_windows_reserved_name(display_name: &str) -> bool {
    let stem = display_name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

fn serialize_setting_markdown(
    setting: &SettingDataV1,
    setting_id: &str,
    relative_path: &str,
) -> Result<String, SettingOutputError> {
    let resources = workflow_resources(SETTING_CREATE_WORKFLOW_ID)
        .ok_or(SettingOutputError::InvalidMarkdown)?;
    let setting_id_yaml =
        serde_json::to_string(setting_id).map_err(|_| SettingOutputError::InvalidMarkdown)?;
    let display_name_yaml = serde_json::to_string(&setting.display_name)
        .map_err(|_| SettingOutputError::InvalidMarkdown)?;
    let rules_markdown = markdown_list(&setting.rules);
    let constraints_markdown = if setting.constraints.is_empty() {
        "- 无明确限制".to_owned()
    } else {
        markdown_list(&setting.constraints)
    };
    let plot_hooks_markdown = markdown_list(&setting.plot_hooks);
    let markdown = resources
        .template_resource
        .replace("{{settingIdYaml}}", &setting_id_yaml)
        .replace("{{displayNameYaml}}", &display_name_yaml)
        .replace("{{displayName}}", &escape_markdown(&setting.display_name))
        .replace("{{settingType}}", setting.setting_type.as_str())
        .replace("{{settingTypeLabel}}", setting.setting_type.label())
        .replace("{{summary}}", &escape_markdown(&setting.summary))
        .replace("{{atmosphere}}", &escape_markdown(&setting.atmosphere))
        .replace("{{rulesMarkdown}}", &rules_markdown)
        .replace("{{constraintsMarkdown}}", &constraints_markdown)
        .replace("{{plotHooksMarkdown}}", &plot_hooks_markdown);
    if markdown.contains("{{")
        || markdown.contains('\r')
        || markdown.starts_with('\u{feff}')
        || !markdown.ends_with('\n')
    {
        return Err(SettingOutputError::InvalidMarkdown);
    }
    let validation = validate_source_bytes(relative_path, markdown.as_bytes());
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(SettingOutputError::InvalidMarkdown);
    }
    Ok(markdown)
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

    const SETTING_ID: &str = "setting_30000000-0000-4000-8000-000000000003";

    fn valid_json(name: &str, setting_type: SettingTypeV1) -> String {
        serde_json::json!({
            "schemaVersion": 1,
            "workflowId": SETTING_CREATE_WORKFLOW_ID,
            "setting": {
                "displayName": name,
                "settingType": setting_type,
                "summary": "雾港依靠潮汐机械维持城市运转。",
                "atmosphere": "潮湿、克制，旧铜管在夜里低鸣。",
                "rules": ["每日退潮后城门只开启一小时", "潮汐机械不得由单人启动"],
                "constraints": ["机械能源不可凭空补充"],
                "plotHooks": ["主角发现退潮时间正在逐日提前"]
            }
        })
        .to_string()
    }

    #[test]
    fn world_location_and_rule_golden_are_deterministic_and_rust_owned() {
        for (kind, label) in [
            (SettingTypeV1::World, "世界"),
            (SettingTypeV1::Location, "地点"),
            (SettingTypeV1::Rule, "规则"),
        ] {
            let first = parse_and_serialize_setting(&valid_json("雾港", kind), SETTING_ID).unwrap();
            let second =
                parse_and_serialize_setting(&valid_json("雾港", kind), SETTING_ID).unwrap();
            assert_eq!(first, second);
            assert_eq!(first.relative_path, "world/雾港.md");
            assert!(first.markdown.contains(&format!("## 设定类型\n\n{label}")));
            assert!(first.markdown.contains(&format!("id: \"{SETTING_ID}\"")));
            assert!(first.markdown.contains("kind: setting"));
            assert!(first.markdown.ends_with('\n'));
            assert!(!first.markdown.contains('\r'));
            assert!(
                validate_source_bytes(&first.relative_path, first.markdown.as_bytes())
                    .diagnostics
                    .is_empty()
            );
        }
    }

    #[test]
    fn explicit_replace_reuses_original_id_and_path_without_renaming() {
        let artifact = parse_and_serialize_setting_replace(
            &valid_json("新都名称", SettingTypeV1::Location),
            SETTING_ID,
            "world/旧都.md",
        )
        .unwrap();
        assert_eq!(artifact.setting_id, SETTING_ID);
        assert_eq!(artifact.relative_path, "world/旧都.md");
        assert!(artifact
            .markdown
            .contains(r#"id: "setting_30000000-0000-4000-8000-000000000003""#));
        assert!(artifact.markdown.contains(r#"title: "新都名称""#));
        let metadata =
            parse_setting_source_metadata(&artifact.relative_path, &artifact.markdown).unwrap();
        assert_eq!(metadata.setting_id, SETTING_ID);
        assert_eq!(metadata.display_name, "新都名称");

        for unsafe_target in [
            "world/../逃逸.md",
            "world/.隐藏.md",
            "characters/错目录.md",
            "world/目录:文件.md",
        ] {
            assert_eq!(
                parse_and_serialize_setting_replace(
                    &valid_json("安全名称", SettingTypeV1::World),
                    SETTING_ID,
                    unsafe_target,
                ),
                Err(SettingOutputError::InvalidFilename)
            );
        }
    }

    #[test]
    fn provider_cannot_supply_identity_path_markdown_tools_or_unknown_fields() {
        for (field, value) in [
            ("id", serde_json::json!("setting_evil")),
            ("path", serde_json::json!("world/evil.md")),
            ("markdown", serde_json::json!("# evil")),
            ("tool", serde_json::json!("filesystem.write")),
            ("frontmatter", serde_json::json!({"kind": "setting"})),
        ] {
            let mut attacked: serde_json::Value =
                serde_json::from_str(&valid_json("雾港", SettingTypeV1::World)).unwrap();
            attacked[field] = value;
            assert_eq!(
                parse_setting_proposal(&attacked.to_string()),
                Err(SettingOutputError::InvalidJson)
            );
        }
    }

    #[test]
    fn duplicates_unsafe_text_bad_ids_and_unsafe_filenames_are_rejected() {
        let mut duplicate: serde_json::Value =
            serde_json::from_str(&valid_json("雾港", SettingTypeV1::World)).unwrap();
        duplicate["setting"]["rules"] = serde_json::json!(["重复", "重复"]);
        assert_eq!(
            parse_setting_proposal(&duplicate.to_string()),
            Err(SettingOutputError::InvalidSchema)
        );
        let mut unsafe_text: serde_json::Value =
            serde_json::from_str(&valid_json("雾港", SettingTypeV1::World)).unwrap();
        unsafe_text["setting"]["summary"] = serde_json::json!("<script>坏内容</script>");
        assert_eq!(
            parse_setting_proposal(&unsafe_text.to_string()),
            Err(SettingOutputError::UnsafeText)
        );
        assert_eq!(
            parse_and_serialize_setting(
                &valid_json("雾港", SettingTypeV1::World),
                "setting_NOT_A_UUID"
            ),
            Err(SettingOutputError::InvalidEntityId)
        );
        for name in ["CON", ".隐藏", "尾点.", "a/b", "a\\b", "a:b"] {
            assert_eq!(
                parse_and_serialize_setting(&valid_json(name, SettingTypeV1::World), SETTING_ID),
                Err(SettingOutputError::InvalidFilename)
            );
        }
    }

    #[test]
    fn repair_prompt_is_bounded_and_workflow_specific() {
        let malformed = format!("SETTING_CANARY{}尾", "坏".repeat(MAX_REPAIR_CAPTURE_BYTES));
        let prompt = build_setting_repair_prompt(SettingOutputError::InvalidJson, &malformed);
        assert!(prompt.contains("setting.output_invalid_json"));
        assert!(prompt.contains("SettingProposalV1"));
        assert!(prompt.contains("SETTING_CANARY"));
        assert!(!prompt.ends_with('尾'));
        assert!(prompt.len() < 32 * 1024);
    }
}
