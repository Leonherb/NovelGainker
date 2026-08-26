use std::collections::{HashMap, HashSet};
use std::fmt;

use novelgainker_validation::{validate_source_bytes, DiagnosticSeverity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{workflow_resources, CHARACTER_CREATE_WORKFLOW_ID};

pub const CHARACTER_PROPOSAL_SCHEMA_VERSION_V2: u16 = 2;
pub const MAX_CHARACTER_V2_RESPONSE_BYTES: usize = 96 * 1024;
pub const MAX_CHARACTER_V2_REPAIR_CAPTURE_BYTES: usize = 8 * 1024;
const MAX_FILENAME_BYTES: usize = 220;
const MAX_FILENAME_UTF16_UNITS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterRelationTypeV2 {
    Ally,
    Rival,
    Family,
    Mentor,
    Romantic,
    Subordinate,
    AssociatedWith,
}

impl CharacterRelationTypeV2 {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ally => "同盟",
            Self::Rival => "对手",
            Self::Family => "亲属",
            Self::Mentor => "导师",
            Self::Romantic => "情感",
            Self::Subordinate => "上下级",
            Self::AssociatedWith => "关联",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRelationV2 {
    pub target_id: String,
    pub relation_type: CharacterRelationTypeV2,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterProposalV2 {
    pub schema_version: u16,
    pub workflow_id: String,
    pub character: CharacterDataV2,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterDataV2 {
    pub display_name: String,
    pub role: String,
    pub summary: String,
    pub traits: Vec<String>,
    pub goal: String,
    pub conflict: String,
    pub motivations: Vec<String>,
    pub secrets: Vec<String>,
    pub plot_functions: Vec<String>,
    pub relationships: Vec<CharacterRelationV2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKindV1 {
    Character,
    Setting,
    Material,
    Plot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEntityReferenceV1 {
    pub entity_id: String,
    pub display_name: String,
    pub relative_path: String,
    pub entity_kind: EntityKindV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSourceMetadata {
    pub character_id: Option<String>,
    pub display_name: String,
    pub schema_version: u16,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CharacterArtifactV2 {
    pub proposal: CharacterProposalV2,
    pub character_id: String,
    pub relative_path: String,
    pub markdown: String,
}

impl fmt::Debug for CharacterArtifactV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CharacterArtifactV2")
            .field("character_id", &self.character_id)
            .field("relative_path", &self.relative_path)
            .field("proposal", &"[REDACTED]")
            .field("markdown", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterV2OutputError {
    Incomplete,
    ResponseTooLarge,
    InvalidJson,
    InvalidSchema,
    UnsafeText,
    InvalidEntityId,
    InvalidReference,
    InvalidFilename,
    InvalidMarkdown,
}

impl CharacterV2OutputError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Incomplete => "character.output_incomplete",
            Self::ResponseTooLarge => "character.output_too_large",
            Self::InvalidJson => "character.output_invalid_json",
            Self::InvalidSchema => "character.output_invalid_schema",
            Self::UnsafeText => "character.output_unsafe_text",
            Self::InvalidEntityId => "workflow.entity_id_missing",
            Self::InvalidReference => "workflow.entity_reference_invalid",
            Self::InvalidFilename => "character.filename_invalid",
            Self::InvalidMarkdown => "character.markdown_invalid",
        }
    }
}

pub fn new_character_id() -> String {
    format!("character_{}", Uuid::new_v4())
}

pub fn parse_character_source_metadata(
    relative_path: &str,
    content: &str,
) -> Result<CharacterSourceMetadata, CharacterV2OutputError> {
    validate_character_target_path(relative_path)?;
    if content.is_empty()
        || content.len() > MAX_CHARACTER_V2_RESPONSE_BYTES
        || validate_source_bytes(relative_path, content.as_bytes())
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(CharacterV2OutputError::InvalidMarkdown);
    }
    let header = content
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(header, _)| header))
        .ok_or(CharacterV2OutputError::InvalidMarkdown)?;
    let mut fields = HashMap::new();
    for line in header.lines() {
        let (key, value) = line
            .split_once(": ")
            .ok_or(CharacterV2OutputError::InvalidMarkdown)?;
        if fields.insert(key, value).is_some() {
            return Err(CharacterV2OutputError::InvalidMarkdown);
        }
    }
    if fields.get("writingLanguage") != Some(&"zh-CN") || fields.get("kind") != Some(&"character") {
        return Err(CharacterV2OutputError::InvalidMarkdown);
    }
    let display_name: String = serde_json::from_str(
        fields
            .get("title")
            .ok_or(CharacterV2OutputError::InvalidMarkdown)?,
    )
    .map_err(|_| CharacterV2OutputError::InvalidMarkdown)?;
    validate_plain_text(&display_name, 80)?;
    match fields.get("schemaVersion").copied() {
        Some("1") if fields.len() == 4 && !fields.contains_key("id") => {
            Ok(CharacterSourceMetadata {
                character_id: None,
                display_name,
                schema_version: 1,
            })
        }
        Some("2") if fields.len() == 5 => {
            let character_id: String = serde_json::from_str(
                fields
                    .get("id")
                    .ok_or(CharacterV2OutputError::InvalidMarkdown)?,
            )
            .map_err(|_| CharacterV2OutputError::InvalidMarkdown)?;
            validate_character_id(&character_id)?;
            Ok(CharacterSourceMetadata {
                character_id: Some(character_id),
                display_name,
                schema_version: 2,
            })
        }
        _ => Err(CharacterV2OutputError::InvalidMarkdown),
    }
}

pub fn parse_and_serialize_character_v2_create(
    response: &str,
    character_id: &str,
    references: &[ResolvedEntityReferenceV1],
) -> Result<CharacterArtifactV2, CharacterV2OutputError> {
    validate_character_id(character_id)?;
    let proposal = parse_character_proposal_v2(response, character_id, references)?;
    let relative_path = character_relative_path(&proposal.character.display_name)?;
    let markdown = serialize_character_v2(
        &proposal.character,
        character_id,
        &relative_path,
        references,
    )?;
    Ok(CharacterArtifactV2 {
        proposal,
        character_id: character_id.to_owned(),
        relative_path,
        markdown,
    })
}

pub fn parse_and_serialize_character_v2_replace(
    response: &str,
    character_id: &str,
    target_relative_path: &str,
    references: &[ResolvedEntityReferenceV1],
) -> Result<CharacterArtifactV2, CharacterV2OutputError> {
    validate_character_id(character_id)?;
    validate_character_target_path(target_relative_path)?;
    let proposal = parse_character_proposal_v2(response, character_id, references)?;
    character_relative_path(&proposal.character.display_name)?;
    let markdown = serialize_character_v2(
        &proposal.character,
        character_id,
        target_relative_path,
        references,
    )?;
    Ok(CharacterArtifactV2 {
        proposal,
        character_id: character_id.to_owned(),
        relative_path: target_relative_path.to_owned(),
        markdown,
    })
}

pub fn parse_character_proposal_v2(
    response: &str,
    character_id: &str,
    references: &[ResolvedEntityReferenceV1],
) -> Result<CharacterProposalV2, CharacterV2OutputError> {
    if response.is_empty() || response.len() > MAX_CHARACTER_V2_RESPONSE_BYTES {
        return Err(if response.is_empty() {
            CharacterV2OutputError::InvalidJson
        } else {
            CharacterV2OutputError::ResponseTooLarge
        });
    }
    validate_character_id(character_id)?;
    let proposal: CharacterProposalV2 =
        serde_json::from_str(response).map_err(|_| CharacterV2OutputError::InvalidJson)?;
    if proposal.schema_version != CHARACTER_PROPOSAL_SCHEMA_VERSION_V2
        || proposal.workflow_id != CHARACTER_CREATE_WORKFLOW_ID
    {
        return Err(CharacterV2OutputError::InvalidSchema);
    }
    validate_character_v2(&proposal, character_id, references)?;
    Ok(proposal)
}

pub fn build_character_v2_repair_prompt(error: CharacterV2OutputError, malformed: &str) -> String {
    let schema = workflow_resources(CHARACTER_CREATE_WORKFLOW_ID)
        .map(|resources| resources.output_schema)
        .unwrap_or("{}");
    let bounded = bounded_utf8_prefix(malformed, MAX_CHARACTER_V2_REPAIR_CAPTURE_BYTES);
    format!(
        "上一次输出未通过人物 v2 本地严格校验。错误代码：{}。\n请只返回一个满足下列 JSON Schema 的 JSON 对象，不要解释，不要使用 Markdown 代码块，不要增加未知字段。关系目标 ID 只能复用原 Prompt 中明确提供的 ID。\nJSON Schema：\n{}\n上一次不合格输出（仅作修复输入）：\n{}",
        error.code(),
        schema,
        bounded
    )
}

fn validate_character_v2(
    proposal: &CharacterProposalV2,
    character_id: &str,
    references: &[ResolvedEntityReferenceV1],
) -> Result<(), CharacterV2OutputError> {
    validate_plain_text(&proposal.character.display_name, 80)?;
    validate_plain_text(&proposal.character.role, 120)?;
    validate_plain_text(&proposal.character.summary, 500)?;
    validate_plain_text(&proposal.character.goal, 1_000)?;
    validate_plain_text(&proposal.character.conflict, 1_000)?;
    validate_plain_text(&proposal.rationale, 1_000)?;
    validate_list(&proposal.character.traits, 1, 12, 80)?;
    validate_list(&proposal.character.motivations, 1, 8, 300)?;
    validate_list(&proposal.character.secrets, 0, 8, 500)?;
    validate_list(&proposal.character.plot_functions, 1, 8, 300)?;
    if proposal.character.relationships.len() > 12 {
        return Err(CharacterV2OutputError::InvalidSchema);
    }

    let mut reference_map = HashMap::with_capacity(references.len());
    for reference in references {
        validate_reference(reference)?;
        if reference.entity_id == character_id
            || reference_map
                .insert(reference.entity_id.as_str(), reference)
                .is_some()
        {
            return Err(CharacterV2OutputError::InvalidReference);
        }
    }
    let mut relationships = HashSet::with_capacity(proposal.character.relationships.len());
    for relationship in &proposal.character.relationships {
        validate_plain_text(&relationship.description, 300)?;
        let reference = reference_map
            .get(relationship.target_id.as_str())
            .ok_or(CharacterV2OutputError::InvalidReference)?;
        if reference.entity_kind != EntityKindV1::Character
            && relationship.relation_type != CharacterRelationTypeV2::AssociatedWith
        {
            return Err(CharacterV2OutputError::InvalidReference);
        }
        if !relationships.insert((relationship.target_id.as_str(), relationship.relation_type)) {
            return Err(CharacterV2OutputError::InvalidSchema);
        }
    }
    Ok(())
}

fn validate_list(
    values: &[String],
    minimum: usize,
    maximum: usize,
    max_characters: usize,
) -> Result<(), CharacterV2OutputError> {
    if values.len() < minimum || values.len() > maximum {
        return Err(CharacterV2OutputError::InvalidSchema);
    }
    let mut unique = HashSet::with_capacity(values.len());
    for value in values {
        validate_plain_text(value, max_characters)?;
        if !unique.insert(value.as_str()) {
            return Err(CharacterV2OutputError::InvalidSchema);
        }
    }
    Ok(())
}

fn validate_reference(reference: &ResolvedEntityReferenceV1) -> Result<(), CharacterV2OutputError> {
    validate_plain_text(&reference.display_name, 120)?;
    validate_reference_path(&reference.relative_path)?;
    let prefix = match reference.entity_kind {
        EntityKindV1::Character => "character_",
        EntityKindV1::Setting => "setting_",
        EntityKindV1::Material => "material_",
        EntityKindV1::Plot => "plot_",
    };
    if !reference.entity_id.starts_with(prefix)
        || Uuid::parse_str(reference.entity_id.strip_prefix(prefix).unwrap_or_default()).is_err()
    {
        return Err(CharacterV2OutputError::InvalidEntityId);
    }
    Ok(())
}

fn validate_character_id(value: &str) -> Result<(), CharacterV2OutputError> {
    if !value.starts_with("character_")
        || Uuid::parse_str(value.strip_prefix("character_").unwrap_or_default()).is_err()
    {
        return Err(CharacterV2OutputError::InvalidEntityId);
    }
    Ok(())
}

fn validate_plain_text(value: &str, maximum: usize) -> Result<(), CharacterV2OutputError> {
    if value.is_empty() || value.trim() != value || value.chars().count() > maximum {
        return Err(CharacterV2OutputError::InvalidSchema);
    }
    if value.chars().any(is_unsafe_character)
        || value.contains('<')
        || value.contains('>')
        || value.to_ascii_lowercase().contains("javascript:")
    {
        return Err(CharacterV2OutputError::UnsafeText);
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

fn character_relative_path(display_name: &str) -> Result<String, CharacterV2OutputError> {
    validate_filename(display_name)?;
    let relative_path = format!("characters/{display_name}.md");
    if relative_path.len() > 240 {
        return Err(CharacterV2OutputError::InvalidFilename);
    }
    Ok(relative_path)
}

fn validate_filename(display_name: &str) -> Result<(), CharacterV2OutputError> {
    if display_name.starts_with('.')
        || display_name.ends_with(['.', ' '])
        || matches!(display_name, "." | "..")
        || display_name
            .chars()
            .any(|value| matches!(value, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        || display_name.len() > MAX_FILENAME_BYTES
        || display_name.encode_utf16().count() > MAX_FILENAME_UTF16_UNITS
        || is_windows_reserved_name(display_name)
    {
        return Err(CharacterV2OutputError::InvalidFilename);
    }
    Ok(())
}

fn validate_character_target_path(path: &str) -> Result<(), CharacterV2OutputError> {
    if !path.strip_prefix("characters/").is_some_and(|tail| {
        !tail.contains('/') && tail.ends_with(".md") && tail.len() > 3 && safe_tail(tail)
    }) {
        return Err(CharacterV2OutputError::InvalidFilename);
    }
    Ok(())
}

fn validate_reference_path(path: &str) -> Result<(), CharacterV2OutputError> {
    let Some((directory, tail)) = path.split_once('/') else {
        return Err(CharacterV2OutputError::InvalidReference);
    };
    if !matches!(directory, "characters" | "world" | "materials" | "plot")
        || (directory == "materials" && tail.starts_with("000."))
        || !tail.ends_with(".md")
        || !safe_tail(tail)
    {
        return Err(CharacterV2OutputError::InvalidReference);
    }
    Ok(())
}

fn safe_tail(tail: &str) -> bool {
    tail.split('/').all(|component| {
        !component.is_empty()
            && !matches!(component, "." | "..")
            && !component.starts_with('.')
            && !component.contains(['\\', ':', '\0', '<', '>'])
            && !component.chars().any(char::is_control)
    })
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

fn serialize_character_v2(
    character: &CharacterDataV2,
    character_id: &str,
    relative_path: &str,
    references: &[ResolvedEntityReferenceV1],
) -> Result<String, CharacterV2OutputError> {
    let resources = workflow_resources(CHARACTER_CREATE_WORKFLOW_ID)
        .ok_or(CharacterV2OutputError::InvalidMarkdown)?;
    let character_id_yaml =
        serde_json::to_string(character_id).map_err(|_| CharacterV2OutputError::InvalidMarkdown)?;
    let display_name_yaml = serde_json::to_string(&character.display_name)
        .map_err(|_| CharacterV2OutputError::InvalidMarkdown)?;
    let reference_map = references
        .iter()
        .map(|reference| (reference.entity_id.as_str(), reference))
        .collect::<HashMap<_, _>>();
    let relationships_markdown = if character.relationships.is_empty() {
        "- 暂无已确认关系".to_owned()
    } else {
        character
            .relationships
            .iter()
            .map(|relationship| {
                let target = reference_map
                    .get(relationship.target_id.as_str())
                    .ok_or(CharacterV2OutputError::InvalidReference)?;
                let link = relative_link_from_character(&target.relative_path)?;
                Ok(format!(
                    "- [{}](<{}>) · {}：{}",
                    escape_markdown(&target.display_name),
                    link,
                    relationship.relation_type.label(),
                    escape_markdown(&relationship.description)
                ))
            })
            .collect::<Result<Vec<_>, CharacterV2OutputError>>()?
            .join("\n")
    };
    let markdown = resources
        .template_resource
        .replace("{{characterIdYaml}}", &character_id_yaml)
        .replace("{{displayNameYaml}}", &display_name_yaml)
        .replace("{{displayName}}", &escape_markdown(&character.display_name))
        .replace("{{role}}", &escape_markdown(&character.role))
        .replace("{{summary}}", &escape_markdown(&character.summary))
        .replace(
            "{{traitsMarkdown}}",
            &markdown_list(&character.traits, "暂无"),
        )
        .replace("{{goal}}", &escape_markdown(&character.goal))
        .replace("{{conflict}}", &escape_markdown(&character.conflict))
        .replace(
            "{{motivationsMarkdown}}",
            &markdown_list(&character.motivations, "暂无"),
        )
        .replace(
            "{{secretsMarkdown}}",
            &markdown_list(&character.secrets, "暂无明确秘密"),
        )
        .replace(
            "{{plotFunctionsMarkdown}}",
            &markdown_list(&character.plot_functions, "暂无"),
        )
        .replace("{{relationshipsMarkdown}}", &relationships_markdown);
    if markdown.contains("{{")
        || markdown.contains('\r')
        || markdown.starts_with('\u{feff}')
        || !markdown.ends_with('\n')
    {
        return Err(CharacterV2OutputError::InvalidMarkdown);
    }
    let validation = validate_source_bytes(relative_path, markdown.as_bytes());
    if !validation.eligible_for_ai_apply || !validation.diagnostics.is_empty() {
        return Err(CharacterV2OutputError::InvalidMarkdown);
    }
    Ok(markdown)
}

fn relative_link_from_character(target: &str) -> Result<String, CharacterV2OutputError> {
    validate_reference_path(target)?;
    Ok(if let Some(tail) = target.strip_prefix("characters/") {
        tail.to_owned()
    } else {
        format!("../{target}")
    })
}

fn markdown_list(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        format!("- {empty}")
    } else {
        values
            .iter()
            .map(|value| format!("- {}", escape_markdown(value)))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn escape_markdown(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '\x60'
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

fn bounded_utf8_prefix(value: &str, maximum: usize) -> &str {
    if value.len() <= maximum {
        return value;
    }
    let mut end = maximum;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    &value[..end]
}
#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    const CHARACTER_ID: &str = "character_10000000-0000-4000-8000-000000000001";
    const OTHER_CHARACTER_ID: &str = "character_20000000-0000-4000-8000-000000000002";
    const SETTING_ID: &str = "setting_30000000-0000-4000-8000-000000000003";

    fn references() -> Vec<ResolvedEntityReferenceV1> {
        vec![
            ResolvedEntityReferenceV1 {
                entity_id: OTHER_CHARACTER_ID.to_owned(),
                display_name: "林遥".to_owned(),
                relative_path: "characters/林遥.md".to_owned(),
                entity_kind: EntityKindV1::Character,
            },
            ResolvedEntityReferenceV1 {
                entity_id: SETTING_ID.to_owned(),
                display_name: "雾城".to_owned(),
                relative_path: "world/雾城.md".to_owned(),
                entity_kind: EntityKindV1::Setting,
            },
        ]
    }

    fn proposal(display_name: &str) -> Value {
        json!({
            "schemaVersion": 2,
            "workflowId": CHARACTER_CREATE_WORKFLOW_ID,
            "character": {
                "displayName": display_name,
                "role": "调查员",
                "summary": "用档案追查失踪案。",
                "traits": ["冷静", "谨慎"],
                "goal": "找出失踪案真相。",
                "conflict": "公开真相会伤害同伴。",
                "motivations": ["保护仍然在场的人"],
                "secrets": ["曾经隐去一页关键记录"],
                "plotFunctions": ["推动调查线索升级"],
                "relationships": [
                    {
                        "targetId": OTHER_CHARACTER_ID,
                        "relationType": "ally",
                        "description": "互相核对调查结论"
                    },
                    {
                        "targetId": SETTING_ID,
                        "relationType": "associated_with",
                        "description": "长期在此查阅旧档"
                    }
                ]
            },
            "rationale": "人物动机、秘密和关系均服务于调查主线。"
        })
    }

    #[test]
    fn creates_one_v2_character_with_rust_owned_id_and_verified_links() {
        let response = serde_json::to_string(&proposal("沈砚")).expect("proposal JSON");
        let artifact =
            parse_and_serialize_character_v2_create(&response, CHARACTER_ID, &references())
                .expect("valid v2 character");

        assert_eq!(artifact.character_id, CHARACTER_ID);
        assert_eq!(artifact.relative_path, "characters/沈砚.md");
        assert!(artifact
            .markdown
            .contains("id: \"character_10000000-0000-4000-8000-000000000001\""));
        assert!(artifact.markdown.contains("[林遥](<林遥.md>) · 同盟"));
        assert!(artifact
            .markdown
            .contains("[雾城](<../world/雾城.md>) · 关联"));
        let metadata = parse_character_source_metadata(&artifact.relative_path, &artifact.markdown)
            .expect("strict v2 metadata");
        assert_eq!(metadata.character_id.as_deref(), Some(CHARACTER_ID));
        assert_eq!(metadata.schema_version, 2);
        let debug = format!("{artifact:?}");
        assert!(!debug.contains("曾经隐去"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn enhance_reuses_v2_id_and_target_path_even_when_display_name_changes() {
        let response = serde_json::to_string(&proposal("沈砚·新称呼")).expect("proposal JSON");
        let artifact = parse_and_serialize_character_v2_replace(
            &response,
            CHARACTER_ID,
            "characters/沈砚.md",
            &references(),
        )
        .expect("valid replace");

        assert_eq!(artifact.character_id, CHARACTER_ID);
        assert_eq!(artifact.relative_path, "characters/沈砚.md");
        assert!(artifact.markdown.contains("title: \"沈砚·新称呼\""));
    }

    #[test]
    fn legacy_v1_is_readable_and_explicit_upgrade_gets_a_new_v2_id() {
        let legacy = "---\ntitle: \"旧人物\"\nwritingLanguage: zh-CN\nkind: character\nschemaVersion: 1\n---\n\n# 旧人物\n";
        let metadata = parse_character_source_metadata("characters/旧人物.md", legacy)
            .expect("legacy character remains readable");
        assert_eq!(metadata.schema_version, 1);
        assert_eq!(metadata.character_id, None);

        let upgraded_id = new_character_id();
        assert_ne!(upgraded_id, CHARACTER_ID);
        validate_character_id(&upgraded_id).expect("Rust-created v2 character ID");
        let response = serde_json::to_string(&proposal("旧人物")).expect("proposal JSON");
        let artifact = parse_and_serialize_character_v2_replace(
            &response,
            &upgraded_id,
            "characters/旧人物.md",
            &references(),
        )
        .expect("explicit v1 upgrade");
        assert_eq!(artifact.relative_path, "characters/旧人物.md");
        assert!(artifact.markdown.contains("schemaVersion: 2"));
        assert!(artifact.markdown.contains(&upgraded_id));
    }

    #[test]
    fn rejects_unselected_duplicate_or_illegal_relationship_targets() {
        let mut unknown = proposal("沈砚");
        unknown["character"]["relationships"][0]["targetId"] =
            json!("character_40000000-0000-4000-8000-000000000004");
        assert_eq!(
            parse_character_proposal_v2(
                &serde_json::to_string(&unknown).expect("JSON"),
                CHARACTER_ID,
                &references(),
            ),
            Err(CharacterV2OutputError::InvalidReference)
        );

        let mut illegal = proposal("沈砚");
        illegal["character"]["relationships"][1]["relationType"] = json!("mentor");
        assert_eq!(
            parse_character_proposal_v2(
                &serde_json::to_string(&illegal).expect("JSON"),
                CHARACTER_ID,
                &references(),
            ),
            Err(CharacterV2OutputError::InvalidReference)
        );

        let mut duplicated_references = references();
        duplicated_references.push(duplicated_references[0].clone());
        assert_eq!(
            parse_character_proposal_v2(
                &serde_json::to_string(&proposal("沈砚")).expect("JSON"),
                CHARACTER_ID,
                &duplicated_references,
            ),
            Err(CharacterV2OutputError::InvalidReference)
        );
    }

    #[test]
    fn rejects_forged_fields_unsafe_names_and_unsafe_replace_paths() {
        let mut forged = proposal("沈砚");
        forged["relativePath"] = json!("characters/伪造.md");
        assert_eq!(
            parse_character_proposal_v2(
                &serde_json::to_string(&forged).expect("JSON"),
                CHARACTER_ID,
                &references(),
            ),
            Err(CharacterV2OutputError::InvalidJson)
        );

        let reserved = serde_json::to_string(&proposal("CON")).expect("JSON");
        assert_eq!(
            parse_and_serialize_character_v2_create(&reserved, CHARACTER_ID, &references()),
            Err(CharacterV2OutputError::InvalidFilename)
        );
        assert_eq!(
            parse_and_serialize_character_v2_replace(
                &serde_json::to_string(&proposal("沈砚")).expect("JSON"),
                CHARACTER_ID,
                "characters/../逃逸.md",
                &references(),
            ),
            Err(CharacterV2OutputError::InvalidFilename)
        );
    }

    #[test]
    fn repair_prompt_is_bounded_and_keeps_the_v2_schema_boundary() {
        let malformed = format!("{}TAIL_MARKER", "错".repeat(5_000));
        let repair =
            build_character_v2_repair_prompt(CharacterV2OutputError::InvalidReference, &malformed);
        assert!(!repair.contains("character.output_invalid_json"));
        assert!(repair.contains("workflow.entity_reference_invalid"));
        assert!(repair.contains("CharacterProposalV2"));
        assert!(!repair.contains("TAIL_MARKER"));
        assert!(repair.len() < malformed.len() + 10_000);
    }
}
