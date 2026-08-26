use std::collections::HashMap;
use std::fmt;

use novelgainker_validation::validate_source_bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{workflow_resources, MATERIAL_CREATE_WORKFLOW_ID};

pub const MATERIAL_PROPOSAL_SCHEMA_VERSION: u16 = 1;
pub const MAX_MATERIAL_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_REPAIR_CAPTURE_BYTES: usize = 8 * 1024;
const MAX_MATERIAL_FILENAME_BYTES: usize = 220;
const MAX_MATERIAL_FILENAME_UTF16_UNITS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialTypeV1 {
    Theme,
    Clue,
    Object,
    SceneSeed,
}

impl MaterialTypeV1 {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "主题",
            Self::Clue => "线索",
            Self::Object => "物件",
            Self::SceneSeed => "场景种子",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Clue => "clue",
            Self::Object => "object",
            Self::SceneSeed => "scene_seed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialProposalV1 {
    pub schema_version: u16,
    pub workflow_id: String,
    pub material: MaterialDataV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialDataV1 {
    pub display_name: String,
    pub material_type: MaterialTypeV1,
    pub summary: String,
    pub content: String,
    pub intended_use: String,
    pub canon_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialSourceMetadataV1 {
    pub material_id: String,
    pub display_name: String,
    pub material_type: MaterialTypeV1,
}

#[derive(Clone, PartialEq, Eq)]
pub struct MaterialArtifactV1 {
    pub proposal: MaterialProposalV1,
    pub material_id: String,
    pub relative_path: String,
    pub markdown: String,
}

impl fmt::Debug for MaterialArtifactV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaterialArtifactV1")
            .field("material_id", &self.material_id)
            .field("relative_path", &self.relative_path)
            .field("proposal", &"[REDACTED]")
            .field("markdown", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialOutputError {
    Incomplete,
    ResponseTooLarge,
    InvalidJson,
    InvalidSchema,
    UnsafeText,
    InvalidEntityId,
    InvalidFilename,
    InvalidMarkdown,
}

impl MaterialOutputError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Incomplete => "material.output_incomplete",
            Self::ResponseTooLarge => "material.output_too_large",
            Self::InvalidJson => "material.output_invalid_json",
            Self::InvalidSchema => "material.output_invalid_schema",
            Self::UnsafeText => "material.output_unsafe_text",
            Self::InvalidEntityId => "material.entity_id_invalid",
            Self::InvalidFilename => "material.filename_invalid",
            Self::InvalidMarkdown => "material.markdown_invalid",
        }
    }
}

pub fn parse_material_source_metadata(
    relative_path: &str,
    content: &str,
) -> Result<MaterialSourceMetadataV1, MaterialOutputError> {
    validate_material_target_path(relative_path)?;
    if content.is_empty()
        || content.len() > MAX_MATERIAL_RESPONSE_BYTES
        || validate_source_bytes(relative_path, content.as_bytes())
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.severity == novelgainker_validation::DiagnosticSeverity::Error
            })
    {
        return Err(MaterialOutputError::InvalidMarkdown);
    }
    let header = content
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(header, _)| header))
        .ok_or(MaterialOutputError::InvalidMarkdown)?;
    let mut fields = HashMap::new();
    for line in header.lines() {
        let (key, value) = line
            .split_once(": ")
            .ok_or(MaterialOutputError::InvalidMarkdown)?;
        if fields.insert(key, value).is_some() {
            return Err(MaterialOutputError::InvalidMarkdown);
        }
    }
    if fields.len() != 6
        || fields.get("writingLanguage") != Some(&"zh-CN")
        || fields.get("kind") != Some(&"material")
        || fields.get("schemaVersion") != Some(&"1")
    {
        return Err(MaterialOutputError::InvalidMarkdown);
    }
    let material_id: String = serde_json::from_str(
        fields
            .get("id")
            .ok_or(MaterialOutputError::InvalidMarkdown)?,
    )
    .map_err(|_| MaterialOutputError::InvalidMarkdown)?;
    let display_name: String = serde_json::from_str(
        fields
            .get("title")
            .ok_or(MaterialOutputError::InvalidMarkdown)?,
    )
    .map_err(|_| MaterialOutputError::InvalidMarkdown)?;
    let material_type = match fields.get("materialType").copied() {
        Some("theme") => MaterialTypeV1::Theme,
        Some("clue") => MaterialTypeV1::Clue,
        Some("object") => MaterialTypeV1::Object,
        Some("scene_seed") => MaterialTypeV1::SceneSeed,
        _ => return Err(MaterialOutputError::InvalidMarkdown),
    };
    validate_material_id(&material_id)?;
    validate_plain_text(&display_name, 80)?;
    material_relative_path(&display_name)?;
    Ok(MaterialSourceMetadataV1 {
        material_id,
        display_name,
        material_type,
    })
}

pub fn new_material_id() -> String {
    format!("material_{}", Uuid::new_v4())
}

pub fn parse_and_serialize_material(
    response: &str,
    material_id: &str,
) -> Result<MaterialArtifactV1, MaterialOutputError> {
    validate_material_id(material_id)?;
    let proposal = parse_material_proposal(response)?;
    let relative_path = material_relative_path(&proposal.material.display_name)?;
    let markdown = serialize_material_markdown(&proposal.material, material_id, &relative_path)?;
    Ok(MaterialArtifactV1 {
        proposal,
        material_id: material_id.to_owned(),
        relative_path,
        markdown,
    })
}

pub fn parse_and_serialize_material_replace(
    response: &str,
    material_id: &str,
    target_relative_path: &str,
) -> Result<MaterialArtifactV1, MaterialOutputError> {
    validate_material_id(material_id)?;
    validate_material_target_path(target_relative_path)?;
    let proposal = parse_material_proposal(response)?;
    material_relative_path(&proposal.material.display_name)?;
    let markdown =
        serialize_material_markdown(&proposal.material, material_id, target_relative_path)?;
    Ok(MaterialArtifactV1 {
        proposal,
        material_id: material_id.to_owned(),
        relative_path: target_relative_path.to_owned(),
        markdown,
    })
}

pub fn parse_material_proposal(response: &str) -> Result<MaterialProposalV1, MaterialOutputError> {
    if response.is_empty() || response.len() > MAX_MATERIAL_RESPONSE_BYTES {
        return Err(if response.is_empty() {
            MaterialOutputError::InvalidJson
        } else {
            MaterialOutputError::ResponseTooLarge
        });
    }
    let proposal: MaterialProposalV1 =
        serde_json::from_str(response).map_err(|_| MaterialOutputError::InvalidJson)?;
    if proposal.schema_version != MATERIAL_PROPOSAL_SCHEMA_VERSION
        || proposal.workflow_id != MATERIAL_CREATE_WORKFLOW_ID
    {
        return Err(MaterialOutputError::InvalidSchema);
    }
    validate_material(&proposal)?;
    Ok(proposal)
}

pub fn build_material_repair_prompt(error: MaterialOutputError, malformed: &str) -> String {
    let schema = workflow_resources(MATERIAL_CREATE_WORKFLOW_ID)
        .map(|resources| resources.output_schema)
        .unwrap_or("{}");
    let bounded = bounded_utf8_prefix(malformed, MAX_REPAIR_CAPTURE_BYTES);
    format!(
        "上一次输出未通过本地严格校验。错误代码：{}。\n请只返回一个满足下列 JSON Schema 的 JSON 对象，不要解释，不要使用 Markdown 代码块，不要增加未知字段。\nJSON Schema：\n{}\n上一次不合格输出（仅作修复输入）：\n{}",
        error.code(), schema, bounded
    )
}

fn validate_material(proposal: &MaterialProposalV1) -> Result<(), MaterialOutputError> {
    validate_plain_text(&proposal.material.display_name, 80)?;
    validate_safe_text(&proposal.material.summary, 800, false)?;
    validate_safe_text(&proposal.material.content, 4_000, true)?;
    validate_safe_text(&proposal.material.intended_use, 1_000, true)?;
    validate_safe_text(&proposal.material.canon_reason, 1_000, true)?;
    Ok(())
}

fn validate_plain_text(value: &str, max_characters: usize) -> Result<(), MaterialOutputError> {
    validate_safe_text(value, max_characters, false)
}

fn validate_safe_text(
    value: &str,
    max_characters: usize,
    allow_newlines: bool,
) -> Result<(), MaterialOutputError> {
    let count = value.chars().count();
    if count == 0 || count > max_characters || value.trim() != value {
        return Err(MaterialOutputError::InvalidSchema);
    }
    let lower = value.to_ascii_lowercase();
    if value
        .chars()
        .any(|character| is_unsafe_character(character) && !(allow_newlines && character == '\n'))
        || value.contains('<')
        || value.contains('>')
        || lower.contains("javascript:")
        || lower.contains("file://")
    {
        return Err(MaterialOutputError::UnsafeText);
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

fn validate_material_id(material_id: &str) -> Result<(), MaterialOutputError> {
    let suffix = material_id
        .strip_prefix("material_")
        .ok_or(MaterialOutputError::InvalidEntityId)?;
    let parsed = Uuid::parse_str(suffix).map_err(|_| MaterialOutputError::InvalidEntityId)?;
    if parsed.to_string() != suffix {
        return Err(MaterialOutputError::InvalidEntityId);
    }
    Ok(())
}

fn validate_material_target_path(relative_path: &str) -> Result<(), MaterialOutputError> {
    let file_name = relative_path
        .strip_prefix("materials/")
        .filter(|name| !name.contains('/'))
        .filter(|name| name.ends_with(".md"))
        .ok_or(MaterialOutputError::InvalidFilename)?;
    if file_name.starts_with("000.")
        || file_name.starts_with('.')
        || file_name.len() <= 3
        || relative_path.len() > 240
        || file_name.as_bytes().contains(&92)
        || file_name.contains(':')
        || file_name.chars().any(char::is_control)
    {
        return Err(MaterialOutputError::InvalidFilename);
    }
    Ok(())
}

fn material_relative_path(display_name: &str) -> Result<String, MaterialOutputError> {
    if display_name == "000"
        || display_name.starts_with("000.")
        || display_name.starts_with('.')
        || display_name.ends_with(['.', ' '])
        || display_name == "."
        || display_name == ".."
        || display_name.chars().any(|character| {
            matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        })
        || display_name.len() > MAX_MATERIAL_FILENAME_BYTES
        || display_name.encode_utf16().count() > MAX_MATERIAL_FILENAME_UTF16_UNITS
        || is_windows_reserved_name(display_name)
    {
        return Err(MaterialOutputError::InvalidFilename);
    }
    let relative_path = format!("materials/{display_name}.md");
    validate_material_target_path(&relative_path)?;
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

fn serialize_material_markdown(
    material: &MaterialDataV1,
    material_id: &str,
    relative_path: &str,
) -> Result<String, MaterialOutputError> {
    let resources = workflow_resources(MATERIAL_CREATE_WORKFLOW_ID)
        .ok_or(MaterialOutputError::InvalidMarkdown)?;
    let material_id_yaml =
        serde_json::to_string(material_id).map_err(|_| MaterialOutputError::InvalidMarkdown)?;
    let display_name_yaml = serde_json::to_string(&material.display_name)
        .map_err(|_| MaterialOutputError::InvalidMarkdown)?;
    let markdown = resources
        .template_resource
        .replace("{{materialIdYaml}}", &material_id_yaml)
        .replace("{{displayNameYaml}}", &display_name_yaml)
        .replace("{{displayName}}", &escape_markdown(&material.display_name))
        .replace("{{materialType}}", material.material_type.as_str())
        .replace("{{materialTypeLabel}}", material.material_type.label())
        .replace("{{summary}}", &escape_markdown(&material.summary))
        .replace("{{content}}", &escape_markdown(&material.content))
        .replace("{{intendedUse}}", &escape_markdown(&material.intended_use))
        .replace("{{canonReason}}", &escape_markdown(&material.canon_reason));
    if markdown.contains("{{")
        || markdown.contains('\r')
        || markdown.starts_with('\u{feff}')
        || !markdown.ends_with('\n')
    {
        return Err(MaterialOutputError::InvalidMarkdown);
    }
    let validation = validate_source_bytes(relative_path, markdown.as_bytes());
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(MaterialOutputError::InvalidMarkdown);
    }
    Ok(markdown)
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

    const MATERIAL_ID: &str = "material_40000000-0000-4000-8000-000000000004";

    fn valid_json(name: &str, material_type: MaterialTypeV1) -> String {
        serde_json::json!({
            "schemaVersion": 1,
            "workflowId": MATERIAL_CREATE_WORKFLOW_ID,
            "material": {
                "displayName": name,
                "materialType": material_type,
                "summary": "一枚只在退潮后显出字迹的旧铜钥匙。",
                "content": "钥匙来自封闭多年的潮汐塔。\n字迹会指向下一次异常退潮。",
                "intendedUse": "作为第二幕调查线索，并连接潮汐塔支线。",
                "canonReason": "它遵守既有潮汐规则，同时能够承载可验证的后续回收。"
            }
        })
        .to_string()
    }

    #[test]
    fn four_material_types_are_deterministic_and_rust_owned() {
        for (kind, label) in [
            (MaterialTypeV1::Theme, "主题"),
            (MaterialTypeV1::Clue, "线索"),
            (MaterialTypeV1::Object, "物件"),
            (MaterialTypeV1::SceneSeed, "场景种子"),
        ] {
            let first =
                parse_and_serialize_material(&valid_json("退潮铜钥匙", kind), MATERIAL_ID).unwrap();
            let second =
                parse_and_serialize_material(&valid_json("退潮铜钥匙", kind), MATERIAL_ID).unwrap();
            assert_eq!(first, second);
            assert_eq!(first.relative_path, "materials/退潮铜钥匙.md");
            assert!(first.markdown.contains(&format!("## 素材类型\n\n{label}")));
            assert!(first.markdown.contains(&format!("id: \"{MATERIAL_ID}\"")));
            assert!(first.markdown.contains("kind: material"));
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
        let artifact = parse_and_serialize_material_replace(
            &valid_json("新的显示名", MaterialTypeV1::Clue),
            MATERIAL_ID,
            "materials/原素材.md",
        )
        .unwrap();
        assert_eq!(artifact.material_id, MATERIAL_ID);
        assert_eq!(artifact.relative_path, "materials/原素材.md");
        assert!(artifact
            .markdown
            .contains(r#"id: "material_40000000-0000-4000-8000-000000000004""#));
        assert!(artifact.markdown.contains(r#"title: "新的显示名""#));
        let metadata =
            parse_material_source_metadata(&artifact.relative_path, &artifact.markdown).unwrap();
        assert_eq!(metadata.material_id, MATERIAL_ID);
        assert_eq!(metadata.display_name, "新的显示名");
    }

    #[test]
    fn provider_cannot_supply_identity_path_frontmatter_status_or_write_decision() {
        for (field, value) in [
            ("id", serde_json::json!("material_evil")),
            ("path", serde_json::json!("materials/evil.md")),
            ("frontmatter", serde_json::json!({"kind": "material"})),
            ("status", serde_json::json!("canon")),
            ("write", serde_json::json!(true)),
        ] {
            let mut attacked: serde_json::Value =
                serde_json::from_str(&valid_json("钥匙", MaterialTypeV1::Object)).unwrap();
            attacked[field] = value;
            assert_eq!(
                parse_material_proposal(&attacked.to_string()),
                Err(MaterialOutputError::InvalidJson)
            );
        }
    }

    #[test]
    fn reserved_targets_and_unsafe_content_are_rejected() {
        for target in [
            "materials/000.story-ledger.md",
            "materials/000.other.md",
            "materials/.hidden.md",
            "materials/sub/escape.md",
            "world/错目录.md",
        ] {
            assert_eq!(
                parse_and_serialize_material_replace(
                    &valid_json("安全素材", MaterialTypeV1::Theme),
                    MATERIAL_ID,
                    target,
                ),
                Err(MaterialOutputError::InvalidFilename)
            );
        }
        for name in [
            "000.story-ledger",
            "CON",
            ".隐藏",
            "尾点.",
            "a/b",
            "a\\b",
            "a:b",
        ] {
            assert_eq!(
                parse_and_serialize_material(&valid_json(name, MaterialTypeV1::Theme), MATERIAL_ID),
                Err(MaterialOutputError::InvalidFilename)
            );
        }
        let mut unsafe_text: serde_json::Value =
            serde_json::from_str(&valid_json("钥匙", MaterialTypeV1::Clue)).unwrap();
        unsafe_text["material"]["content"] = serde_json::json!("<script>坏内容</script>");
        assert_eq!(
            parse_material_proposal(&unsafe_text.to_string()),
            Err(MaterialOutputError::UnsafeText)
        );
    }

    #[test]
    fn repair_prompt_is_bounded_and_workflow_specific() {
        let malformed = format!("MATERIAL_CANARY{}尾", "坏".repeat(MAX_REPAIR_CAPTURE_BYTES));
        let prompt = build_material_repair_prompt(MaterialOutputError::InvalidJson, &malformed);
        assert!(prompt.contains("material.output_invalid_json"));
        assert!(prompt.contains("MaterialProposalV1"));
        assert!(prompt.contains("MATERIAL_CANARY"));
        assert!(!prompt.ends_with('尾'));
        assert!(prompt.len() < 32 * 1024);
    }
}
