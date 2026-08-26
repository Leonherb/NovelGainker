use std::collections::HashSet;
use std::fmt;

use novelgainker_validation::validate_source_bytes;
use serde::{Deserialize, Serialize};

use crate::CHARACTER_CREATE_WORKFLOW_ID;

pub const CHARACTER_PROPOSAL_SCHEMA_VERSION: u16 = 1;
pub const MAX_CHARACTER_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_REPAIR_CAPTURE_BYTES: usize = 8 * 1024;
const MAX_CHARACTER_FILENAME_BYTES: usize = 220;
const MAX_CHARACTER_FILENAME_UTF16_UNITS: usize = 200;

const LEGACY_CHARACTER_OUTPUT_SCHEMA_V1: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "CharacterProposalV1",
  "type": "object",
  "additionalProperties": false,
  "required": ["schemaVersion", "workflowId", "character", "rationale"],
  "properties": {
    "schemaVersion": {"type": "integer", "const": 1},
    "workflowId": {"type": "string", "const": "novelgainker.character-create"},
    "character": {
      "type": "object",
      "additionalProperties": false,
      "required": ["displayName", "role", "summary", "traits", "goal", "conflict"],
      "properties": {
        "displayName": {"type": "string", "minLength": 1, "maxLength": 80},
        "role": {"type": "string", "minLength": 1, "maxLength": 120},
        "summary": {"type": "string", "minLength": 1, "maxLength": 500},
        "traits": {"type": "array", "minItems": 1, "maxItems": 12, "uniqueItems": true, "items": {"type": "string", "minLength": 1, "maxLength": 80}},
        "goal": {"type": "string", "minLength": 1, "maxLength": 1000},
        "conflict": {"type": "string", "minLength": 1, "maxLength": 1000}
      }
    },
    "rationale": {"type": "string", "minLength": 1, "maxLength": 1000}
  }
}"#;
const LEGACY_CHARACTER_TEMPLATE_V1: &str = r#"---
title: {{displayNameYaml}}
writingLanguage: zh-CN
kind: character
schemaVersion: 1
---

# {{displayName}}

## 角色定位

{{role}}。{{summary}}

## 核心特征

{{traitsMarkdown}}

## 目标

{{goal}}

## 冲突

{{conflict}}
"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterProposalV1 {
    pub schema_version: u16,
    pub workflow_id: String,
    pub character: CharacterDataV1,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterDataV1 {
    pub display_name: String,
    pub role: String,
    pub summary: String,
    pub traits: Vec<String>,
    pub goal: String,
    pub conflict: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CharacterArtifactV1 {
    pub proposal: CharacterProposalV1,
    pub relative_path: String,
    pub markdown: String,
}

impl fmt::Debug for CharacterArtifactV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CharacterArtifactV1")
            .field("relative_path", &self.relative_path)
            .field("proposal", &"[REDACTED]")
            .field("markdown", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterOutputError {
    Incomplete,
    ResponseTooLarge,
    InvalidJson,
    InvalidSchema,
    UnsafeText,
    InvalidFilename,
    InvalidMarkdown,
}

impl CharacterOutputError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Incomplete => "character.output_incomplete",
            Self::ResponseTooLarge => "character.output_too_large",
            Self::InvalidJson => "character.output_invalid_json",
            Self::InvalidSchema => "character.output_invalid_schema",
            Self::UnsafeText => "character.output_unsafe_text",
            Self::InvalidFilename => "character.filename_invalid",
            Self::InvalidMarkdown => "character.markdown_invalid",
        }
    }
}

pub fn parse_and_serialize_character(
    response: &str,
) -> Result<CharacterArtifactV1, CharacterOutputError> {
    let proposal = parse_character_proposal(response)?;
    let relative_path = character_relative_path(&proposal.character.display_name)?;
    let markdown = serialize_character_markdown(&proposal.character, &relative_path)?;
    Ok(CharacterArtifactV1 {
        proposal,
        relative_path,
        markdown,
    })
}

pub fn parse_character_proposal(
    response: &str,
) -> Result<CharacterProposalV1, CharacterOutputError> {
    if response.is_empty() || response.len() > MAX_CHARACTER_RESPONSE_BYTES {
        return Err(if response.is_empty() {
            CharacterOutputError::InvalidJson
        } else {
            CharacterOutputError::ResponseTooLarge
        });
    }
    let proposal: CharacterProposalV1 =
        serde_json::from_str(response).map_err(|_| CharacterOutputError::InvalidJson)?;
    if proposal.schema_version != CHARACTER_PROPOSAL_SCHEMA_VERSION
        || proposal.workflow_id != CHARACTER_CREATE_WORKFLOW_ID
    {
        return Err(CharacterOutputError::InvalidSchema);
    }
    validate_character(&proposal)?;
    Ok(proposal)
}

pub fn build_character_repair_prompt(error: CharacterOutputError, malformed: &str) -> String {
    let schema = LEGACY_CHARACTER_OUTPUT_SCHEMA_V1;
    let bounded = bounded_utf8_prefix(malformed, MAX_REPAIR_CAPTURE_BYTES);
    format!(
        "上一次输出未通过本地严格校验。错误代码：{}。\n请只返回一个满足下列 JSON Schema 的 JSON 对象，不要解释，不要使用 Markdown 代码块，不要增加未知字段。\nJSON Schema：\n{}\n上一次不合格输出（仅作修复输入）：\n{}",
        error.code(), schema, bounded
    )
}

fn validate_character(proposal: &CharacterProposalV1) -> Result<(), CharacterOutputError> {
    validate_plain_text(&proposal.character.display_name, 80)?;
    validate_plain_text(&proposal.character.role, 120)?;
    validate_plain_text(&proposal.character.summary, 500)?;
    validate_plain_text(&proposal.character.goal, 1000)?;
    validate_plain_text(&proposal.character.conflict, 1000)?;
    validate_plain_text(&proposal.rationale, 1000)?;
    if proposal.character.traits.is_empty() || proposal.character.traits.len() > 12 {
        return Err(CharacterOutputError::InvalidSchema);
    }
    let mut traits = HashSet::with_capacity(proposal.character.traits.len());
    for trait_text in &proposal.character.traits {
        validate_plain_text(trait_text, 80)?;
        if !traits.insert(trait_text.as_str()) {
            return Err(CharacterOutputError::InvalidSchema);
        }
    }
    Ok(())
}

fn validate_plain_text(value: &str, max_characters: usize) -> Result<(), CharacterOutputError> {
    let count = value.chars().count();
    if count == 0 || count > max_characters || value.trim() != value {
        return Err(CharacterOutputError::InvalidSchema);
    }
    if value.chars().any(is_unsafe_character)
        || value.contains('<')
        || value.contains('>')
        || value.to_ascii_lowercase().contains("javascript:")
    {
        return Err(CharacterOutputError::UnsafeText);
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

fn character_relative_path(display_name: &str) -> Result<String, CharacterOutputError> {
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
        || display_name.len() > MAX_CHARACTER_FILENAME_BYTES
        || display_name.encode_utf16().count() > MAX_CHARACTER_FILENAME_UTF16_UNITS
        || is_windows_reserved_name(display_name)
    {
        return Err(CharacterOutputError::InvalidFilename);
    }
    let relative_path = format!("characters/{display_name}.md");
    if relative_path.len() > 240 {
        return Err(CharacterOutputError::InvalidFilename);
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

fn serialize_character_markdown(
    character: &CharacterDataV1,
    relative_path: &str,
) -> Result<String, CharacterOutputError> {
    let display_name_yaml = serde_json::to_string(&character.display_name)
        .map_err(|_| CharacterOutputError::InvalidMarkdown)?;
    let traits_markdown = character
        .traits
        .iter()
        .map(|value| format!("- {}", escape_markdown(value)))
        .collect::<Vec<_>>()
        .join("\n");
    let markdown = LEGACY_CHARACTER_TEMPLATE_V1
        .replace("{{displayNameYaml}}", &display_name_yaml)
        .replace("{{displayName}}", &escape_markdown(&character.display_name))
        .replace("{{role}}", &escape_markdown(&character.role))
        .replace("{{summary}}", &escape_markdown(&character.summary))
        .replace("{{traitsMarkdown}}", &traits_markdown)
        .replace("{{goal}}", &escape_markdown(&character.goal))
        .replace("{{conflict}}", &escape_markdown(&character.conflict));
    if markdown.contains("{{")
        || markdown.contains('\r')
        || markdown.starts_with('\u{feff}')
        || !markdown.ends_with('\n')
    {
        return Err(CharacterOutputError::InvalidMarkdown);
    }
    let validation = validate_source_bytes(relative_path, markdown.as_bytes());
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(CharacterOutputError::InvalidMarkdown);
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

    fn valid_json(name: &str) -> String {
        serde_json::json!({
            "schemaVersion": 1,
            "workflowId": CHARACTER_CREATE_WORKFLOW_ID,
            "character": {
                "displayName": name,
                "role": "主角",
                "summary": "在边城调查旧案的年轻档案员",
                "traits": ["克制", "敏锐"],
                "goal": "找出失踪案背后的真相",
                "conflict": "真相会伤害她唯一信任的亲人"
            },
            "rationale": "人物目标与冲突能够推动主线。"
        })
        .to_string()
    }

    #[test]
    fn strict_json_becomes_deterministic_safe_chinese_markdown() {
        let first = parse_and_serialize_character(&valid_json("林遥")).unwrap();
        let second = parse_and_serialize_character(&valid_json("林遥")).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.relative_path, "characters/林遥.md");
        assert!(first.markdown.starts_with("---\ntitle: \"林遥\"\n"));
        assert!(first.markdown.contains("writingLanguage: zh-CN"));
        assert!(first.markdown.contains("## 核心特征\n\n- 克制\n- 敏锐"));
        assert!(first.markdown.ends_with('\n'));
        assert!(!first.markdown.contains('\r'));
        assert_eq!(
            validate_source_bytes(&first.relative_path, first.markdown.as_bytes()).diagnostics,
            Vec::new()
        );
    }

    #[test]
    fn unknown_fields_wrong_constants_duplicates_and_unsafe_text_are_rejected() {
        let mut unknown: serde_json::Value = serde_json::from_str(&valid_json("林遥")).unwrap();
        unknown["path"] = serde_json::json!("characters/evil.md");
        assert_eq!(
            parse_character_proposal(&unknown.to_string()),
            Err(CharacterOutputError::InvalidJson)
        );

        let mut wrong: serde_json::Value = serde_json::from_str(&valid_json("林遥")).unwrap();
        wrong["schemaVersion"] = serde_json::json!(2);
        assert_eq!(
            parse_character_proposal(&wrong.to_string()),
            Err(CharacterOutputError::InvalidSchema)
        );

        let mut duplicate: serde_json::Value = serde_json::from_str(&valid_json("林遥")).unwrap();
        duplicate["character"]["traits"] = serde_json::json!(["克制", "克制"]);
        assert_eq!(
            parse_character_proposal(&duplicate.to_string()),
            Err(CharacterOutputError::InvalidSchema)
        );

        let mut unsafe_value: serde_json::Value =
            serde_json::from_str(&valid_json("林遥")).unwrap();
        unsafe_value["character"]["summary"] = serde_json::json!("<script>坏内容</script>");
        assert_eq!(
            parse_character_proposal(&unsafe_value.to_string()),
            Err(CharacterOutputError::UnsafeText)
        );
    }

    #[test]
    fn windows_reserved_separators_hidden_and_overlong_names_are_rejected() {
        for name in [
            "CON", "con.txt", "LPT9", ".隐藏", "尾点.", "a/b", "a\\b", "a:b",
        ] {
            assert_eq!(
                parse_and_serialize_character(&valid_json(name)),
                Err(CharacterOutputError::InvalidFilename),
                "name {name:?} must be rejected"
            );
        }
        let overlong = "界".repeat(80);
        assert_eq!(
            parse_and_serialize_character(&valid_json(&overlong)),
            Err(CharacterOutputError::InvalidFilename)
        );
    }

    #[test]
    fn markdown_metacharacters_are_escaped_without_changing_the_local_filename() {
        let artifact = parse_and_serialize_character(&valid_json("林#遥")).unwrap();
        assert_eq!(artifact.relative_path, "characters/林#遥.md");
        assert!(artifact.markdown.contains("# 林\\#遥"));
        assert!(artifact.markdown.contains("title: \"林#遥\""));
    }

    #[test]
    fn repair_prompt_is_bounded_fixed_and_contains_only_a_stable_error_code() {
        let malformed = format!("CANARY{}尾", "坏".repeat(MAX_REPAIR_CAPTURE_BYTES));
        let prompt = build_character_repair_prompt(CharacterOutputError::InvalidJson, &malformed);
        assert!(prompt.contains("character.output_invalid_json"));
        assert!(prompt.contains("CharacterProposalV1"));
        assert!(prompt.contains("CANARY"));
        assert!(!prompt.ends_with('尾'));
        assert!(prompt.len() < 32 * 1024);
    }
}
