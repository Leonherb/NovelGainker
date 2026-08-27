use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PACK_SCHEMA_VERSION: u16 = 1;
pub const WORKFLOW_PACK_VERSION: &str = "1.0.0";
pub const CHARACTER_WORKFLOW_ID: &str = "novelgainker.character-create";
pub const CHARACTER_WORKFLOW_VERSION: u16 = 2;
pub const CHARACTER_MODE: &str = "interactive-write";
pub const CHARACTER_TOOL: &str = "source.create_character";
pub const CHARACTER_CONFIRMATIONS: [&str; 2] = ["send_context", "create_or_replace_character_file"];
pub const PROJECT_WORKFLOW_ID: &str = "novelgainker.project-create";
pub const PROJECT_WORKFLOW_VERSION: u16 = 1;
pub const PROJECT_MODE: &str = "deterministic-write";
pub const PROJECT_TOOLS: [&str; 2] = ["project.scan", "project.create"];
pub const PROJECT_CONFIRMATIONS: [&str; 1] = ["create_project_scaffold"];
pub const SETTING_WORKFLOW_ID: &str = "novelgainker.setting-create";
pub const SETTING_WORKFLOW_VERSION: u16 = 1;
pub const SETTING_MODE: &str = "interactive-write";
pub const SETTING_TOOL: &str = "source.create_setting";
pub const SETTING_CONFIRMATIONS: [&str; 2] = ["send_context", "create_setting_file"];
pub const MATERIAL_WORKFLOW_ID: &str = "novelgainker.material-create";
pub const MATERIAL_WORKFLOW_VERSION: u16 = 1;
pub const MATERIAL_MODE: &str = "interactive-write";
pub const MATERIAL_TOOL: &str = "source.create_material";
pub const MATERIAL_CONFIRMATIONS: [&str; 2] = ["send_context", "promote_material_to_canon"];
pub const PLOT_WORKFLOW_ID: &str = "novelgainker.plot-create";
pub const PLOT_WORKFLOW_VERSION: u16 = 1;
pub const PLOT_MODE: &str = "interactive-write";
pub const PLOT_TOOL: &str = "source.create_plot";
pub const PLOT_CONFIRMATIONS: [&str; 2] = ["send_context", "create_or_replace_main_plot"];
pub const STORYTELLING_WORKFLOW_ID: &str = "novelgainker.storytelling";
pub const STORYTELLING_WORKFLOW_VERSION: u16 = 1;
pub const STORYTELLING_MODE: &str = "interactive-write";
pub const STORYTELLING_TOOLS: [&str; 8] = [
    "story.build_context",
    "story.preview_request",
    "story.generate_proposal",
    "story.check_continuity",
    "story.preview_chapter_patch",
    "story.apply_chapter_patch",
    "story.transition_chapter_status",
    "story.update_ledger",
];
pub const STORYTELLING_CONFIRMATIONS: [&str; 7] = [
    "select_context",
    "review_reader_model",
    "send_context",
    "review_continuity_warnings",
    "apply_chapter_patch",
    "transition_chapter_status",
    "apply_ledger_update",
];
pub const EXPECTED_RESOURCE_PATHS: [&str; 31] = [
    "character-create/character.template.md",
    "character-create/input.schema.json",
    "character-create/output.schema.json",
    "character-create/workflow.md",
    "material-create/input.schema.json",
    "material-create/material.template.md",
    "material-create/output.schema.json",
    "material-create/workflow.md",
    "pack.json",
    "plot-create/input.schema.json",
    "plot-create/output.schema.json",
    "plot-create/plot.template.md",
    "plot-create/workflow.md",
    "project-create/input.schema.json",
    "project-create/output.schema.json",
    "project-create/project.template.md",
    "project-create/workflow.md",
    "router/deterministic-intents.v1.json",
    "router/router-decision.schema.json",
    "router/workflow.md",
    "setting-create/input.schema.json",
    "setting-create/output.schema.json",
    "setting-create/setting.template.md",
    "setting-create/workflow.md",
    "storytelling/chapter-proposal.output.schema.json",
    "storytelling/chapter.template.md",
    "storytelling/input.schema.json",
    "storytelling/output.schema.json",
    "storytelling/request-preview.input.schema.json",
    "storytelling/request-snapshot.output.schema.json",
    "storytelling/workflow.md",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackManifest {
    pub schema_version: u16,
    pub workflow_pack_version: String,
    pub workflows: Vec<WorkflowManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowManifest {
    pub id: String,
    pub version: u16,
    pub display_name: String,
    pub description: String,
    pub mode: String,
    pub execution_available: bool,
    pub routable: bool,
    pub allowed_tools: Vec<String>,
    pub required_confirmations: Vec<String>,
    pub input_schema_version: u16,
    pub output_schema_version: u16,
    pub workflow_resource: String,
    pub input_schema: String,
    pub output_schema: String,
    pub template_resource: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedWorkflow {
    pub id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPack {
    pub pack_hash: String,
    pub workflows: Vec<ValidatedWorkflow>,
}

fn invalid(code: &str) -> String {
    code.to_owned()
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(invalid("workflow-pack.resource-path-invalid"));
    }
    Ok(())
}

fn resource_text<'a>(
    resources: &'a BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<&'a str, String> {
    validate_relative_path(path)?;
    let bytes = resources
        .get(path)
        .ok_or_else(|| invalid("workflow-pack.resource-missing"))?;
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(invalid("workflow-pack.resource-bom-forbidden"));
    }
    if bytes.contains(&b'\r') {
        return Err(invalid("workflow-pack.resource-line-ending-invalid"));
    }
    std::str::from_utf8(bytes).map_err(|_| invalid("workflow-pack.resource-encoding-invalid"))
}

fn contains_hangul(value: &str) -> bool {
    value.chars().any(|character| {
        matches!(
            character as u32,
            0x1100..=0x11ff | 0x3130..=0x318f | 0xa960..=0xa97f | 0xac00..=0xd7af
        )
    })
}

fn validate_language_and_origin(path: &str, text: &str) -> Result<(), String> {
    if contains_hangul(text) {
        return Err(invalid("workflow-pack.resource-hangul-forbidden"));
    }
    let lower = text.to_ascii_lowercase();
    for forbidden in [
        "skill.md",
        "openai.yaml",
        "agents/openai",
        "plugin cache",
        "plugin-cache",
        "novelist",
        "python",
    ] {
        if lower.contains(forbidden) {
            return Err(format!("workflow-pack.external-runtime-forbidden:{path}"));
        }
    }
    Ok(())
}

fn object<'a>(value: &'a Value, code: &str) -> Result<&'a Map<String, Value>, String> {
    value.as_object().ok_or_else(|| invalid(code))
}

fn exact_string_set(value: &Value, expected: &[&str], code: &str) -> Result<(), String> {
    let values = value.as_array().ok_or_else(|| invalid(code))?;
    let actual = values
        .iter()
        .map(|item| item.as_str().ok_or_else(|| invalid(code)))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected || values.len() != expected.len() {
        return Err(invalid(code));
    }
    Ok(())
}

fn exact_object_keys(
    value: &Map<String, Value>,
    expected: &[&str],
    code: &str,
) -> Result<(), String> {
    let actual = value.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid(code));
    }
    Ok(())
}

fn validate_string_property(
    property: &Value,
    minimum: u64,
    maximum: u64,
    code: &str,
) -> Result<(), String> {
    let property = object(property, code)?;
    if property.get("type").and_then(Value::as_str) != Some("string")
        || property.get("minLength").and_then(Value::as_u64) != Some(minimum)
        || property.get("maxLength").and_then(Value::as_u64) != Some(maximum)
    {
        return Err(invalid(code));
    }
    Ok(())
}

fn validate_schema_root_version(
    text: &str,
    expected_id: &str,
    required: &[&str],
    properties: &[&str],
    expected_schema_version: u64,
) -> Result<Map<String, Value>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|_| invalid("workflow-pack.schema-json-invalid"))?;
    let root = object(&value, "workflow-pack.schema-root-invalid")?;
    exact_object_keys(
        root,
        &[
            "$id",
            "$schema",
            "additionalProperties",
            "properties",
            "required",
            "title",
            "type",
        ],
        "workflow-pack.schema-root-fields-invalid",
    )?;
    if root.get("$schema").and_then(Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
        || root.get("$id").and_then(Value::as_str) != Some(expected_id)
        || root.get("type").and_then(Value::as_str) != Some("object")
        || root.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid("workflow-pack.schema-contract-invalid"));
    }
    exact_string_set(
        root.get("required")
            .ok_or_else(|| invalid("workflow-pack.schema-required-missing"))?,
        required,
        "workflow-pack.schema-required-invalid",
    )?;
    let schema_properties = object(
        root.get("properties")
            .ok_or_else(|| invalid("workflow-pack.schema-properties-missing"))?,
        "workflow-pack.schema-properties-invalid",
    )?;
    exact_object_keys(
        schema_properties,
        properties,
        "workflow-pack.schema-property-set-invalid",
    )?;
    let version = object(
        schema_properties
            .get("schemaVersion")
            .ok_or_else(|| invalid("workflow-pack.schema-version-missing"))?,
        "workflow-pack.schema-version-invalid",
    )?;
    if version.get("type").and_then(Value::as_str) != Some("integer")
        || version.get("const").and_then(Value::as_u64) != Some(expected_schema_version)
    {
        return Err(invalid("workflow-pack.schema-version-invalid"));
    }
    Ok(schema_properties.clone())
}

fn validate_schema_root(
    text: &str,
    expected_id: &str,
    required: &[&str],
    properties: &[&str],
) -> Result<Map<String, Value>, String> {
    validate_schema_root_version(text, expected_id, required, properties, 1)
}

fn validate_input_schema(text: &str) -> Result<(), String> {
    let fields = ["brief", "mode", "schemaVersion"];
    let properties = validate_schema_root_version(
        text,
        "https://schemas.novelgainker.local/workflows/character-brief.v2.json",
        &fields,
        &fields,
        2,
    )?;
    validate_enum_string_property(
        properties
            .get("mode")
            .ok_or_else(|| invalid("workflow-pack.input-mode-missing"))?,
        &["create", "enhance"],
        "workflow-pack.input-mode-invalid",
    )?;
    validate_string_property(
        properties
            .get("brief")
            .ok_or_else(|| invalid("workflow-pack.input-brief-missing"))?,
        1,
        4_000,
        "workflow-pack.input-brief-invalid",
    )
}

fn validate_output_schema(text: &str) -> Result<(), String> {
    let fields = ["character", "rationale", "schemaVersion", "workflowId"];
    let properties = validate_schema_root_version(
        text,
        "https://schemas.novelgainker.local/workflows/character-proposal.v2.json",
        &fields,
        &fields,
        2,
    )?;
    validate_const_string_property(
        properties
            .get("workflowId")
            .ok_or_else(|| invalid("workflow-pack.output-workflow-id-missing"))?,
        CHARACTER_WORKFLOW_ID,
        "workflow-pack.output-workflow-id-invalid",
    )?;
    validate_string_property(
        properties
            .get("rationale")
            .ok_or_else(|| invalid("workflow-pack.output-rationale-missing"))?,
        1,
        1_000,
        "workflow-pack.output-rationale-invalid",
    )?;
    let character = object(
        properties
            .get("character")
            .ok_or_else(|| invalid("workflow-pack.output-character-missing"))?,
        "workflow-pack.output-character-invalid",
    )?;
    exact_object_keys(
        character,
        &["additionalProperties", "properties", "required", "type"],
        "workflow-pack.output-character-fields-invalid",
    )?;
    if character.get("type").and_then(Value::as_str) != Some("object")
        || character
            .get("additionalProperties")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid("workflow-pack.output-character-invalid"));
    }
    let character_fields = [
        "conflict",
        "displayName",
        "goal",
        "motivations",
        "plotFunctions",
        "relationships",
        "role",
        "secrets",
        "summary",
        "traits",
    ];
    exact_string_set(
        character
            .get("required")
            .ok_or_else(|| invalid("workflow-pack.output-character-required-missing"))?,
        &character_fields,
        "workflow-pack.output-character-required-invalid",
    )?;
    let values = object(
        character
            .get("properties")
            .ok_or_else(|| invalid("workflow-pack.output-character-properties-missing"))?,
        "workflow-pack.output-character-properties-invalid",
    )?;
    exact_object_keys(
        values,
        &character_fields,
        "workflow-pack.output-character-property-set-invalid",
    )?;
    for (field, maximum) in [
        ("displayName", 80),
        ("role", 120),
        ("summary", 500),
        ("goal", 1_000),
        ("conflict", 1_000),
    ] {
        validate_string_property(
            values
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.output-character-field-missing"))?,
            1,
            maximum,
            "workflow-pack.output-character-string-invalid",
        )?;
    }
    for (field, minimum, maximum, item_maximum) in [
        ("traits", 1, 12, 80),
        ("motivations", 1, 8, 300),
        ("secrets", 0, 8, 500),
        ("plotFunctions", 1, 8, 300),
    ] {
        validate_string_array_property(
            values
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.output-character-list-missing"))?,
            minimum,
            maximum,
            item_maximum,
            "workflow-pack.output-character-list-invalid",
        )?;
    }
    let relationships = object(
        values
            .get("relationships")
            .ok_or_else(|| invalid("workflow-pack.output-relationships-missing"))?,
        "workflow-pack.output-relationships-invalid",
    )?;
    exact_object_keys(
        relationships,
        &["items", "maxItems", "minItems", "type", "uniqueItems"],
        "workflow-pack.output-relationships-invalid",
    )?;
    if relationships.get("type").and_then(Value::as_str) != Some("array")
        || relationships.get("minItems").and_then(Value::as_u64) != Some(0)
        || relationships.get("maxItems").and_then(Value::as_u64) != Some(12)
        || relationships.get("uniqueItems").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid("workflow-pack.output-relationships-invalid"));
    }
    let relation = object(
        relationships
            .get("items")
            .ok_or_else(|| invalid("workflow-pack.output-relationship-item-missing"))?,
        "workflow-pack.output-relationship-item-invalid",
    )?;
    exact_object_keys(
        relation,
        &["additionalProperties", "properties", "required", "type"],
        "workflow-pack.output-relationship-item-invalid",
    )?;
    let relation_fields = ["description", "relationType", "targetId"];
    if relation.get("type").and_then(Value::as_str) != Some("object")
        || relation
            .get("additionalProperties")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid("workflow-pack.output-relationship-item-invalid"));
    }
    exact_string_set(
        relation
            .get("required")
            .ok_or_else(|| invalid("workflow-pack.output-relationship-required-missing"))?,
        &relation_fields,
        "workflow-pack.output-relationship-required-invalid",
    )?;
    let relation_values = object(
        relation
            .get("properties")
            .ok_or_else(|| invalid("workflow-pack.output-relationship-properties-missing"))?,
        "workflow-pack.output-relationship-properties-invalid",
    )?;
    exact_object_keys(
        relation_values,
        &relation_fields,
        "workflow-pack.output-relationship-properties-invalid",
    )?;
    validate_string_property(
        relation_values
            .get("targetId")
            .ok_or_else(|| invalid("workflow-pack.output-relationship-target-missing"))?,
        41,
        46,
        "workflow-pack.output-relationship-target-invalid",
    )?;
    validate_enum_string_property(
        relation_values
            .get("relationType")
            .ok_or_else(|| invalid("workflow-pack.output-relationship-type-missing"))?,
        &[
            "ally",
            "rival",
            "family",
            "mentor",
            "romantic",
            "subordinate",
            "associated_with",
        ],
        "workflow-pack.output-relationship-type-invalid",
    )?;
    validate_string_property(
        relation_values
            .get("description")
            .ok_or_else(|| invalid("workflow-pack.output-relationship-description-missing"))?,
        1,
        300,
        "workflow-pack.output-relationship-description-invalid",
    )
}

fn validate_template(text: &str) -> Result<(), String> {
    let markers = [
        "{{characterIdYaml}}",
        "{{displayNameYaml}}",
        "{{displayName}}",
        "{{role}}",
        "{{summary}}",
        "{{traitsMarkdown}}",
        "{{goal}}",
        "{{conflict}}",
        "{{motivationsMarkdown}}",
        "{{secretsMarkdown}}",
        "{{plotFunctionsMarkdown}}",
        "{{relationshipsMarkdown}}",
    ];
    let mut remaining = text.to_owned();
    for marker in markers {
        if text.match_indices(marker).count() != 1 {
            return Err(invalid("workflow-pack.template-marker-invalid"));
        }
        remaining = remaining.replace(marker, "");
    }
    for required in [
        "writingLanguage: zh-CN",
        "kind: character",
        "schemaVersion: 2",
        "## 角色定位",
        "## 核心特征",
        "## 目标",
        "## 冲突",
        "## 动机",
        "## 秘密",
        "## 剧情功能",
        "## 关系",
    ] {
        if !text.contains(required) {
            return Err(invalid("workflow-pack.template-contract-invalid"));
        }
    }
    if remaining.contains("{{") || remaining.contains("}}") {
        return Err(invalid("workflow-pack.template-contract-invalid"));
    }
    Ok(())
}

fn validate_workflow_instructions(text: &str) -> Result<(), String> {
    for required in [
        "# 创建或完善人物",
        "CharacterBriefV2",
        "CharacterProposalV2",
        "本地人物目标",
        "关系目标 ID",
        "不得指定文件路径",
        "Rust",
        "简体中文",
    ] {
        if !text.contains(required) {
            return Err(invalid("workflow-pack.workflow-instruction-invalid"));
        }
    }
    Ok(())
}

fn validate_nullable_string_property(
    property: &Value,
    maximum: u64,
    code: &str,
) -> Result<(), String> {
    let property = object(property, code)?;
    exact_object_keys(property, &["maxLength", "type"], code)?;
    exact_string_set(
        property.get("type").ok_or_else(|| invalid(code))?,
        &["null", "string"],
        code,
    )?;
    if property.get("maxLength").and_then(Value::as_u64) != Some(maximum) {
        return Err(invalid(code));
    }
    Ok(())
}

fn validate_project_input_schema(text: &str) -> Result<(), String> {
    let fields = [
        "authorDisplayName",
        "genre",
        "premise",
        "schemaVersion",
        "title",
    ];
    let properties = validate_schema_root(
        text,
        "https://schemas.novelgainker.local/workflows/project-create-input.v1.json",
        &fields,
        &fields,
    )?;
    validate_string_property(
        properties
            .get("title")
            .ok_or_else(|| invalid("workflow-pack.project-input-title-missing"))?,
        1,
        120,
        "workflow-pack.project-input-title-invalid",
    )?;
    for (field, maximum) in [("authorDisplayName", 120), ("genre", 80), ("premise", 300)] {
        validate_nullable_string_property(
            properties
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.project-input-field-missing"))?,
            maximum,
            "workflow-pack.project-input-field-invalid",
        )?;
    }
    Ok(())
}

fn validate_const_string_property(
    property: &Value,
    expected: &str,
    code: &str,
) -> Result<(), String> {
    let property = object(property, code)?;
    exact_object_keys(property, &["const", "type"], code)?;
    if property.get("type").and_then(Value::as_str) != Some("string")
        || property.get("const").and_then(Value::as_str) != Some(expected)
    {
        return Err(invalid(code));
    }
    Ok(())
}

fn validate_pattern_string_property(
    property: &Value,
    pattern: &str,
    code: &str,
) -> Result<(), String> {
    let property = object(property, code)?;
    exact_object_keys(property, &["pattern", "type"], code)?;
    if property.get("type").and_then(Value::as_str) != Some("string")
        || property.get("pattern").and_then(Value::as_str) != Some(pattern)
    {
        return Err(invalid(code));
    }
    Ok(())
}

fn validate_project_output_schema(text: &str) -> Result<(), String> {
    let fields = [
        "defaultProviderId",
        "directories",
        "existingUnrelatedEntryCount",
        "files",
        "previewHash",
        "projectId",
        "schemaVersion",
        "workflowPackVersion",
        "writingLanguage",
    ];
    let properties = validate_schema_root(
        text,
        "https://schemas.novelgainker.local/workflows/project-scaffold-preview.v1.json",
        &fields,
        &fields,
    )?;
    validate_pattern_string_property(
        properties
            .get("previewHash")
            .ok_or_else(|| invalid("workflow-pack.project-output-hash-missing"))?,
        "^[0-9a-f]{64}$",
        "workflow-pack.project-output-hash-invalid",
    )?;
    validate_string_property(
        properties
            .get("projectId")
            .ok_or_else(|| invalid("workflow-pack.project-output-id-missing"))?,
        36,
        36,
        "workflow-pack.project-output-id-invalid",
    )?;
    for (field, expected) in [
        ("writingLanguage", "zh-CN"),
        ("workflowPackVersion", WORKFLOW_PACK_VERSION),
        ("defaultProviderId", "openai"),
    ] {
        validate_const_string_property(
            properties
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.project-output-const-missing"))?,
            expected,
            "workflow-pack.project-output-const-invalid",
        )?;
    }
    let count = object(
        properties
            .get("existingUnrelatedEntryCount")
            .ok_or_else(|| invalid("workflow-pack.project-output-count-missing"))?,
        "workflow-pack.project-output-count-invalid",
    )?;
    exact_object_keys(
        count,
        &["minimum", "type"],
        "workflow-pack.project-output-count-invalid",
    )?;
    if count.get("type").and_then(Value::as_str) != Some("integer")
        || count.get("minimum").and_then(Value::as_u64) != Some(0)
    {
        return Err(invalid("workflow-pack.project-output-count-invalid"));
    }

    let directories = object(
        properties
            .get("directories")
            .ok_or_else(|| invalid("workflow-pack.project-output-directories-missing"))?,
        "workflow-pack.project-output-directories-invalid",
    )?;
    exact_object_keys(
        directories,
        &["items", "maxItems", "minItems", "type", "uniqueItems"],
        "workflow-pack.project-output-directories-invalid",
    )?;
    if directories.get("type").and_then(Value::as_str) != Some("array")
        || directories.get("minItems").and_then(Value::as_u64) != Some(11)
        || directories.get("maxItems").and_then(Value::as_u64) != Some(11)
        || directories.get("uniqueItems").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid("workflow-pack.project-output-directories-invalid"));
    }
    validate_string_property(
        directories
            .get("items")
            .ok_or_else(|| invalid("workflow-pack.project-output-directory-item-missing"))?,
        1,
        80,
        "workflow-pack.project-output-directory-item-invalid",
    )?;

    let files = object(
        properties
            .get("files")
            .ok_or_else(|| invalid("workflow-pack.project-output-files-missing"))?,
        "workflow-pack.project-output-files-invalid",
    )?;
    exact_object_keys(
        files,
        &["items", "maxItems", "minItems", "type"],
        "workflow-pack.project-output-files-invalid",
    )?;
    if files.get("type").and_then(Value::as_str) != Some("array")
        || files.get("minItems").and_then(Value::as_u64) != Some(4)
        || files.get("maxItems").and_then(Value::as_u64) != Some(4)
    {
        return Err(invalid("workflow-pack.project-output-files-invalid"));
    }
    let file = object(
        files
            .get("items")
            .ok_or_else(|| invalid("workflow-pack.project-output-file-missing"))?,
        "workflow-pack.project-output-file-invalid",
    )?;
    exact_object_keys(
        file,
        &["additionalProperties", "properties", "required", "type"],
        "workflow-pack.project-output-file-invalid",
    )?;
    let file_fields = ["byteLength", "content", "contentSha256", "relativePath"];
    if file.get("type").and_then(Value::as_str) != Some("object")
        || file.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid("workflow-pack.project-output-file-invalid"));
    }
    exact_string_set(
        file.get("required")
            .ok_or_else(|| invalid("workflow-pack.project-output-file-required-missing"))?,
        &file_fields,
        "workflow-pack.project-output-file-required-invalid",
    )?;
    let file_properties = object(
        file.get("properties")
            .ok_or_else(|| invalid("workflow-pack.project-output-file-properties-missing"))?,
        "workflow-pack.project-output-file-properties-invalid",
    )?;
    exact_object_keys(
        file_properties,
        &file_fields,
        "workflow-pack.project-output-file-properties-invalid",
    )?;
    validate_string_property(
        file_properties
            .get("relativePath")
            .ok_or_else(|| invalid("workflow-pack.project-output-path-missing"))?,
        1,
        180,
        "workflow-pack.project-output-path-invalid",
    )?;
    validate_pattern_string_property(
        file_properties
            .get("contentSha256")
            .ok_or_else(|| invalid("workflow-pack.project-output-content-hash-missing"))?,
        "^[0-9a-f]{64}$",
        "workflow-pack.project-output-content-hash-invalid",
    )?;
    let bytes = object(
        file_properties
            .get("byteLength")
            .ok_or_else(|| invalid("workflow-pack.project-output-bytes-missing"))?,
        "workflow-pack.project-output-bytes-invalid",
    )?;
    exact_object_keys(
        bytes,
        &["maximum", "minimum", "type"],
        "workflow-pack.project-output-bytes-invalid",
    )?;
    if bytes.get("type").and_then(Value::as_str) != Some("integer")
        || bytes.get("minimum").and_then(Value::as_u64) != Some(0)
        || bytes.get("maximum").and_then(Value::as_u64) != Some(65_536)
    {
        return Err(invalid("workflow-pack.project-output-bytes-invalid"));
    }
    let content = object(
        file_properties
            .get("content")
            .ok_or_else(|| invalid("workflow-pack.project-output-content-missing"))?,
        "workflow-pack.project-output-content-invalid",
    )?;
    exact_object_keys(
        content,
        &["maxLength", "type"],
        "workflow-pack.project-output-content-invalid",
    )?;
    if content.get("type").and_then(Value::as_str) != Some("string")
        || content.get("maxLength").and_then(Value::as_u64) != Some(65_536)
    {
        return Err(invalid("workflow-pack.project-output-content-invalid"));
    }
    Ok(())
}

fn validate_project_template(text: &str) -> Result<(), String> {
    for marker in [
        "{{titleYaml}}",
        "{{authorFrontmatter}}",
        "{{genreFrontmatter}}",
        "{{title}}",
        "{{premiseSection}}",
    ] {
        if text.match_indices(marker).count() != 1 {
            return Err(invalid("workflow-pack.project-template-marker-invalid"));
        }
    }
    if !text.contains("writingLanguage: zh-CN")
        || !text.contains("kind: project")
        || !text.contains("schemaVersion: 1")
        || !text.contains("## 项目说明")
    {
        return Err(invalid("workflow-pack.project-template-contract-invalid"));
    }
    Ok(())
}

fn validate_project_workflow_instructions(text: &str) -> Result<(), String> {
    for required in [
        "# 创建项目",
        "ProjectCreateInputV1",
        "ProjectScaffoldPreviewV1",
        "零写入",
        "Preview Hash",
        "不调用 AI",
        "简体中文",
    ] {
        if !text.contains(required) {
            return Err(invalid("workflow-pack.project-instruction-invalid"));
        }
    }
    Ok(())
}

fn validate_enum_string_property(
    property: &Value,
    expected: &[&str],
    code: &str,
) -> Result<(), String> {
    let property = object(property, code)?;
    exact_object_keys(property, &["enum", "type"], code)?;
    if property.get("type").and_then(Value::as_str) != Some("string") {
        return Err(invalid(code));
    }
    exact_string_set(
        property.get("enum").ok_or_else(|| invalid(code))?,
        expected,
        code,
    )
}

fn validate_string_array_property(
    property: &Value,
    minimum: u64,
    maximum: u64,
    item_maximum: u64,
    code: &str,
) -> Result<(), String> {
    let property = object(property, code)?;
    exact_object_keys(
        property,
        &["items", "maxItems", "minItems", "type", "uniqueItems"],
        code,
    )?;
    if property.get("type").and_then(Value::as_str) != Some("array")
        || property.get("minItems").and_then(Value::as_u64) != Some(minimum)
        || property.get("maxItems").and_then(Value::as_u64) != Some(maximum)
        || property.get("uniqueItems").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid(code));
    }
    validate_string_property(
        property.get("items").ok_or_else(|| invalid(code))?,
        1,
        item_maximum,
        code,
    )
}

fn validate_setting_input_schema(text: &str) -> Result<(), String> {
    let fields = [
        "constraints",
        "displayName",
        "mustInclude",
        "objective",
        "schemaVersion",
        "settingType",
    ];
    let properties = validate_schema_root(
        text,
        "https://schemas.novelgainker.local/workflows/setting-brief.v1.json",
        &fields,
        &fields,
    )?;
    validate_string_property(
        properties
            .get("displayName")
            .ok_or_else(|| invalid("workflow-pack.setting-input-name-missing"))?,
        1,
        80,
        "workflow-pack.setting-input-name-invalid",
    )?;
    validate_enum_string_property(
        properties
            .get("settingType")
            .ok_or_else(|| invalid("workflow-pack.setting-input-type-missing"))?,
        &["world", "location", "rule"],
        "workflow-pack.setting-input-type-invalid",
    )?;
    validate_string_property(
        properties
            .get("objective")
            .ok_or_else(|| invalid("workflow-pack.setting-input-objective-missing"))?,
        1,
        1000,
        "workflow-pack.setting-input-objective-invalid",
    )?;
    validate_string_array_property(
        properties
            .get("mustInclude")
            .ok_or_else(|| invalid("workflow-pack.setting-input-points-missing"))?,
        1,
        12,
        160,
        "workflow-pack.setting-input-points-invalid",
    )?;
    validate_nullable_string_property(
        properties
            .get("constraints")
            .ok_or_else(|| invalid("workflow-pack.setting-input-constraints-missing"))?,
        1000,
        "workflow-pack.setting-input-constraints-invalid",
    )
}

fn validate_setting_output_schema(text: &str) -> Result<(), String> {
    let fields = ["schemaVersion", "setting", "workflowId"];
    let properties = validate_schema_root(
        text,
        "https://schemas.novelgainker.local/workflows/setting-proposal.v1.json",
        &fields,
        &fields,
    )?;
    validate_const_string_property(
        properties
            .get("workflowId")
            .ok_or_else(|| invalid("workflow-pack.setting-output-workflow-missing"))?,
        SETTING_WORKFLOW_ID,
        "workflow-pack.setting-output-workflow-invalid",
    )?;
    let setting = object(
        properties
            .get("setting")
            .ok_or_else(|| invalid("workflow-pack.setting-output-data-missing"))?,
        "workflow-pack.setting-output-data-invalid",
    )?;
    exact_object_keys(
        setting,
        &["additionalProperties", "properties", "required", "type"],
        "workflow-pack.setting-output-data-invalid",
    )?;
    let fields = [
        "atmosphere",
        "constraints",
        "displayName",
        "plotHooks",
        "rules",
        "settingType",
        "summary",
    ];
    if setting.get("type").and_then(Value::as_str) != Some("object")
        || setting.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid("workflow-pack.setting-output-data-invalid"));
    }
    exact_string_set(
        setting
            .get("required")
            .ok_or_else(|| invalid("workflow-pack.setting-output-required-missing"))?,
        &fields,
        "workflow-pack.setting-output-required-invalid",
    )?;
    let values = object(
        setting
            .get("properties")
            .ok_or_else(|| invalid("workflow-pack.setting-output-properties-missing"))?,
        "workflow-pack.setting-output-properties-invalid",
    )?;
    exact_object_keys(
        values,
        &fields,
        "workflow-pack.setting-output-properties-invalid",
    )?;
    for (field, maximum) in [("displayName", 80), ("summary", 800), ("atmosphere", 500)] {
        validate_string_property(
            values
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.setting-output-text-missing"))?,
            1,
            maximum,
            "workflow-pack.setting-output-text-invalid",
        )?;
    }
    validate_enum_string_property(
        values
            .get("settingType")
            .ok_or_else(|| invalid("workflow-pack.setting-output-type-missing"))?,
        &["world", "location", "rule"],
        "workflow-pack.setting-output-type-invalid",
    )?;
    for (field, minimum, maximum, item_maximum) in [
        ("rules", 1, 12, 200),
        ("constraints", 0, 10, 200),
        ("plotHooks", 1, 10, 300),
    ] {
        validate_string_array_property(
            values
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.setting-output-list-missing"))?,
            minimum,
            maximum,
            item_maximum,
            "workflow-pack.setting-output-list-invalid",
        )?;
    }
    Ok(())
}

fn validate_setting_template(text: &str) -> Result<(), String> {
    for marker in [
        "{{settingIdYaml}}",
        "{{displayNameYaml}}",
        "{{displayName}}",
        "{{settingType}}",
        "{{settingTypeLabel}}",
        "{{summary}}",
        "{{atmosphere}}",
        "{{rulesMarkdown}}",
        "{{constraintsMarkdown}}",
        "{{plotHooksMarkdown}}",
    ] {
        if text.match_indices(marker).count() != 1 {
            return Err(invalid("workflow-pack.setting-template-marker-invalid"));
        }
    }
    if !text.contains("writingLanguage: zh-CN")
        || !text.contains("kind: setting")
        || !text.contains("schemaVersion: 1")
        || !text.contains("## 设定类型")
        || !text.contains("## 规则")
        || !text.contains("## 剧情钩子")
    {
        return Err(invalid("workflow-pack.setting-template-contract-invalid"));
    }
    Ok(())
}

fn validate_setting_workflow_instructions(text: &str) -> Result<(), String> {
    for required in [
        "# 创建设定",
        "SettingBriefV1",
        "SettingProposalV1",
        "Source Picker",
        "不得指定实体 ID",
        "简体中文",
    ] {
        if !text.contains(required) {
            return Err(invalid("workflow-pack.setting-instruction-invalid"));
        }
    }
    Ok(())
}

fn validate_material_input_schema(text: &str) -> Result<(), String> {
    let fields = [
        "constraints",
        "displayName",
        "intendedUse",
        "materialType",
        "schemaVersion",
        "seed",
    ];
    let properties = validate_schema_root(
        text,
        "https://schemas.novelgainker.local/workflows/material-brief.v1.json",
        &fields,
        &fields,
    )?;
    validate_string_property(
        properties
            .get("displayName")
            .ok_or_else(|| invalid("workflow-pack.material-input-name-missing"))?,
        1,
        80,
        "workflow-pack.material-input-name-invalid",
    )?;
    validate_enum_string_property(
        properties
            .get("materialType")
            .ok_or_else(|| invalid("workflow-pack.material-input-type-missing"))?,
        &["theme", "clue", "object", "scene_seed"],
        "workflow-pack.material-input-type-invalid",
    )?;
    validate_string_property(
        properties
            .get("seed")
            .ok_or_else(|| invalid("workflow-pack.material-input-seed-missing"))?,
        1,
        2_000,
        "workflow-pack.material-input-seed-invalid",
    )?;
    validate_string_property(
        properties
            .get("intendedUse")
            .ok_or_else(|| invalid("workflow-pack.material-input-use-missing"))?,
        1,
        1_000,
        "workflow-pack.material-input-use-invalid",
    )?;
    validate_nullable_string_property(
        properties
            .get("constraints")
            .ok_or_else(|| invalid("workflow-pack.material-input-constraints-missing"))?,
        1_000,
        "workflow-pack.material-input-constraints-invalid",
    )
}

fn validate_material_output_schema(text: &str) -> Result<(), String> {
    let fields = ["material", "schemaVersion", "workflowId"];
    let properties = validate_schema_root(
        text,
        "https://schemas.novelgainker.local/workflows/material-proposal.v1.json",
        &fields,
        &fields,
    )?;
    validate_const_string_property(
        properties
            .get("workflowId")
            .ok_or_else(|| invalid("workflow-pack.material-output-workflow-missing"))?,
        MATERIAL_WORKFLOW_ID,
        "workflow-pack.material-output-workflow-invalid",
    )?;
    let material = object(
        properties
            .get("material")
            .ok_or_else(|| invalid("workflow-pack.material-output-data-missing"))?,
        "workflow-pack.material-output-data-invalid",
    )?;
    exact_object_keys(
        material,
        &["additionalProperties", "properties", "required", "type"],
        "workflow-pack.material-output-data-invalid",
    )?;
    let fields = [
        "canonReason",
        "content",
        "displayName",
        "intendedUse",
        "materialType",
        "summary",
    ];
    if material.get("type").and_then(Value::as_str) != Some("object")
        || material
            .get("additionalProperties")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid("workflow-pack.material-output-data-invalid"));
    }
    exact_string_set(
        material
            .get("required")
            .ok_or_else(|| invalid("workflow-pack.material-output-required-missing"))?,
        &fields,
        "workflow-pack.material-output-required-invalid",
    )?;
    let values = object(
        material
            .get("properties")
            .ok_or_else(|| invalid("workflow-pack.material-output-properties-missing"))?,
        "workflow-pack.material-output-properties-invalid",
    )?;
    exact_object_keys(
        values,
        &fields,
        "workflow-pack.material-output-properties-invalid",
    )?;
    for (field, maximum) in [
        ("displayName", 80),
        ("summary", 800),
        ("content", 4_000),
        ("intendedUse", 1_000),
        ("canonReason", 1_000),
    ] {
        validate_string_property(
            values
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.material-output-text-missing"))?,
            1,
            maximum,
            "workflow-pack.material-output-text-invalid",
        )?;
    }
    validate_enum_string_property(
        values
            .get("materialType")
            .ok_or_else(|| invalid("workflow-pack.material-output-type-missing"))?,
        &["theme", "clue", "object", "scene_seed"],
        "workflow-pack.material-output-type-invalid",
    )
}

fn validate_material_template(text: &str) -> Result<(), String> {
    for marker in [
        "{{materialIdYaml}}",
        "{{displayNameYaml}}",
        "{{displayName}}",
        "{{materialType}}",
        "{{materialTypeLabel}}",
        "{{summary}}",
        "{{content}}",
        "{{intendedUse}}",
        "{{canonReason}}",
    ] {
        if text.match_indices(marker).count() != 1 {
            return Err(invalid("workflow-pack.material-template-marker-invalid"));
        }
    }
    if !text.contains("writingLanguage: zh-CN")
        || !text.contains("kind: material")
        || !text.contains("schemaVersion: 1")
        || !text.contains("## 素材类型")
        || !text.contains("## 内容")
        || !text.contains("## 成为 Canon 的理由")
    {
        return Err(invalid("workflow-pack.material-template-contract-invalid"));
    }
    Ok(())
}

fn validate_material_workflow_instructions(text: &str) -> Result<(), String> {
    for required in [
        "# 创建素材",
        "MaterialBriefV1",
        "MaterialProposalV1",
        "Source Picker",
        "不得指定实体 ID",
        "简体中文",
    ] {
        if !text.contains(required) {
            return Err(invalid("workflow-pack.material-instruction-invalid"));
        }
    }
    Ok(())
}

fn validate_plot_input_schema(text: &str) -> Result<(), String> {
    let fields = [
        "coreQuestion",
        "desiredEnding",
        "forbiddenDirections",
        "genre",
        "mustInclude",
        "schemaVersion",
        "title",
    ];
    let properties = validate_schema_root(
        text,
        "https://schemas.novelgainker.local/workflows/plot-brief.v1.json",
        &fields,
        &fields,
    )?;
    for (field, maximum) in [
        ("title", 120),
        ("genre", 80),
        ("coreQuestion", 1_000),
        ("desiredEnding", 2_000),
    ] {
        validate_string_property(
            properties
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.plot-input-text-missing"))?,
            1,
            maximum,
            "workflow-pack.plot-input-text-invalid",
        )?;
    }
    for field in ["mustInclude", "forbiddenDirections"] {
        validate_string_array_property(
            properties
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.plot-input-list-missing"))?,
            0,
            8,
            500,
            "workflow-pack.plot-input-list-invalid",
        )?;
    }
    Ok(())
}

fn validate_plot_output_schema(text: &str) -> Result<(), String> {
    let fields = ["plot", "schemaVersion", "workflowId"];
    let properties = validate_schema_root(
        text,
        "https://schemas.novelgainker.local/workflows/plot-proposal.v1.json",
        &fields,
        &fields,
    )?;
    validate_const_string_property(
        properties
            .get("workflowId")
            .ok_or_else(|| invalid("workflow-pack.plot-output-workflow-missing"))?,
        PLOT_WORKFLOW_ID,
        "workflow-pack.plot-output-workflow-invalid",
    )?;
    let plot = object(
        properties
            .get("plot")
            .ok_or_else(|| invalid("workflow-pack.plot-output-data-missing"))?,
        "workflow-pack.plot-output-data-invalid",
    )?;
    exact_object_keys(
        plot,
        &["additionalProperties", "properties", "required", "type"],
        "workflow-pack.plot-output-data-invalid",
    )?;
    let plot_fields = [
        "openQuestions",
        "payoffs",
        "pressure",
        "promise",
        "reveals",
        "title",
        "turns",
    ];
    if plot.get("type").and_then(Value::as_str) != Some("object")
        || plot.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid("workflow-pack.plot-output-data-invalid"));
    }
    exact_string_set(
        plot.get("required")
            .ok_or_else(|| invalid("workflow-pack.plot-output-required-missing"))?,
        &plot_fields,
        "workflow-pack.plot-output-required-invalid",
    )?;
    let values = object(
        plot.get("properties")
            .ok_or_else(|| invalid("workflow-pack.plot-output-properties-missing"))?,
        "workflow-pack.plot-output-properties-invalid",
    )?;
    exact_object_keys(
        values,
        &plot_fields,
        "workflow-pack.plot-output-properties-invalid",
    )?;
    for (field, maximum) in [("title", 120), ("promise", 2_000), ("pressure", 2_000)] {
        validate_string_property(
            values
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.plot-output-text-missing"))?,
            1,
            maximum,
            "workflow-pack.plot-output-text-invalid",
        )?;
    }
    for (field, minimum, maximum) in [
        ("turns", 3, 12),
        ("reveals", 1, 8),
        ("payoffs", 1, 8),
        ("openQuestions", 0, 8),
    ] {
        validate_string_array_property(
            values
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.plot-output-list-missing"))?,
            minimum,
            maximum,
            1_000,
            "workflow-pack.plot-output-list-invalid",
        )?;
    }
    Ok(())
}

fn validate_plot_template(text: &str) -> Result<(), String> {
    for marker in [
        "{{plotIdYaml}}",
        "{{titleYaml}}",
        "{{title}}",
        "{{promise}}",
        "{{pressure}}",
        "{{turns}}",
        "{{reveals}}",
        "{{payoffs}}",
        "{{openQuestions}}",
    ] {
        if text.match_indices(marker).count() != 1 {
            return Err(invalid("workflow-pack.plot-template-marker-invalid"));
        }
    }
    for required in [
        "writingLanguage: zh-CN",
        "kind: plot",
        "schemaVersion: 1",
        "## 故事承诺",
        "## 核心压力",
        "## 转折",
        "## 揭示",
        "## 回收",
        "## 未决问题",
    ] {
        if !text.contains(required) {
            return Err(invalid("workflow-pack.plot-template-contract-invalid"));
        }
    }
    Ok(())
}

fn validate_plot_workflow_instructions(text: &str) -> Result<(), String> {
    for required in [
        "# 创建主剧情",
        "PlotBriefV1",
        "PlotProposalV1",
        "Source Picker",
        "不得指定剧情 ID",
        "简体中文",
    ] {
        if !text.contains(required) {
            return Err(invalid("workflow-pack.plot-instruction-invalid"));
        }
    }
    Ok(())
}
fn validate_storytelling_input_schema(text: &str) -> Result<(), String> {
    let fields = [
        "action",
        "chapterNumber",
        "chapterTitle",
        "schemaVersion",
        "selectedSourcePaths",
    ];
    let properties = validate_schema_root_version(
        text,
        "https://schemas.novelgainker.local/workflows/story-context-build.v2.json",
        &fields,
        &fields,
        2,
    )?;
    validate_enum_string_property(
        properties
            .get("action")
            .ok_or_else(|| invalid("workflow-pack.storytelling-action-missing"))?,
        &["create", "revise"],
        "workflow-pack.storytelling-action-invalid",
    )?;
    validate_string_property(
        properties
            .get("chapterTitle")
            .ok_or_else(|| invalid("workflow-pack.storytelling-title-missing"))?,
        1,
        120,
        "workflow-pack.storytelling-title-invalid",
    )?;
    let chapter_number = object(
        properties
            .get("chapterNumber")
            .ok_or_else(|| invalid("workflow-pack.storytelling-number-missing"))?,
        "workflow-pack.storytelling-number-invalid",
    )?;
    exact_object_keys(
        chapter_number,
        &["maximum", "minimum", "type"],
        "workflow-pack.storytelling-number-invalid",
    )?;
    if chapter_number.get("type").and_then(Value::as_str) != Some("integer")
        || chapter_number.get("minimum").and_then(Value::as_u64) != Some(1)
        || chapter_number.get("maximum").and_then(Value::as_u64) != Some(999_999)
    {
        return Err(invalid("workflow-pack.storytelling-number-invalid"));
    }
    let selected = object(
        properties
            .get("selectedSourcePaths")
            .ok_or_else(|| invalid("workflow-pack.storytelling-sources-missing"))?,
        "workflow-pack.storytelling-sources-invalid",
    )?;
    exact_object_keys(
        selected,
        &["items", "maxItems", "type", "uniqueItems"],
        "workflow-pack.storytelling-sources-invalid",
    )?;
    if selected.get("type").and_then(Value::as_str) != Some("array")
        || selected.get("maxItems").and_then(Value::as_u64) != Some(256)
        || selected.get("uniqueItems").and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid("workflow-pack.storytelling-sources-invalid"));
    }
    validate_string_property(
        selected
            .get("items")
            .ok_or_else(|| invalid("workflow-pack.storytelling-source-item-missing"))?,
        1,
        512,
        "workflow-pack.storytelling-source-item-invalid",
    )
}

fn validate_storytelling_output_schema(text: &str) -> Result<(), String> {
    let fields = [
        "action",
        "aggregateHash",
        "budget",
        "createdAt",
        "excludedSources",
        "manifestVersion",
        "projectRootIdentity",
        "selectedSources",
        "sourceOrder",
        "targetChapter",
        "workflowId",
        "workflowVersion",
    ];
    let value: Value =
        serde_json::from_str(text).map_err(|_| invalid("workflow-pack.schema-json-invalid"))?;
    let root = object(&value, "workflow-pack.schema-root-invalid")?;
    exact_object_keys(
        root,
        &[
            "$id",
            "$schema",
            "additionalProperties",
            "properties",
            "required",
            "title",
            "type",
        ],
        "workflow-pack.schema-root-fields-invalid",
    )?;
    if root.get("$schema").and_then(Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
        || root.get("$id").and_then(Value::as_str)
            != Some("https://schemas.novelgainker.local/workflows/story-context-manifest.v2.json")
        || root.get("type").and_then(Value::as_str) != Some("object")
        || root.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid(
            "workflow-pack.storytelling-output-contract-invalid",
        ));
    }
    exact_string_set(
        root.get("required")
            .ok_or_else(|| invalid("workflow-pack.schema-required-missing"))?,
        &fields,
        "workflow-pack.schema-required-invalid",
    )?;
    let properties = object(
        root.get("properties")
            .ok_or_else(|| invalid("workflow-pack.schema-properties-missing"))?,
        "workflow-pack.schema-properties-invalid",
    )?;
    exact_object_keys(
        properties,
        &fields,
        "workflow-pack.schema-property-set-invalid",
    )?;
    let manifest_version = object(
        properties
            .get("manifestVersion")
            .ok_or_else(|| invalid("workflow-pack.storytelling-manifest-version-missing"))?,
        "workflow-pack.storytelling-manifest-version-invalid",
    )?;
    if manifest_version.get("type").and_then(Value::as_str) != Some("integer")
        || manifest_version.get("const").and_then(Value::as_u64) != Some(2)
    {
        return Err(invalid(
            "workflow-pack.storytelling-manifest-version-invalid",
        ));
    }
    validate_const_string_property(
        properties
            .get("workflowId")
            .ok_or_else(|| invalid("workflow-pack.storytelling-workflow-id-missing"))?,
        STORYTELLING_WORKFLOW_ID,
        "workflow-pack.storytelling-workflow-id-invalid",
    )?;
    validate_enum_string_property(
        properties
            .get("action")
            .ok_or_else(|| invalid("workflow-pack.storytelling-output-action-missing"))?,
        &["create", "revise"],
        "workflow-pack.storytelling-output-action-invalid",
    )?;
    validate_string_property(
        properties
            .get("aggregateHash")
            .ok_or_else(|| invalid("workflow-pack.storytelling-hash-missing"))?,
        64,
        64,
        "workflow-pack.storytelling-hash-invalid",
    )?;
    validate_string_property(
        properties
            .get("projectRootIdentity")
            .ok_or_else(|| invalid("workflow-pack.storytelling-project-missing"))?,
        1,
        128,
        "workflow-pack.storytelling-project-invalid",
    )?;
    let workflow_version = object(
        properties
            .get("workflowVersion")
            .ok_or_else(|| invalid("workflow-pack.storytelling-version-missing"))?,
        "workflow-pack.storytelling-version-invalid",
    )?;
    if workflow_version.get("type").and_then(Value::as_str) != Some("integer")
        || workflow_version.get("const").and_then(Value::as_u64) != Some(1)
    {
        return Err(invalid("workflow-pack.storytelling-version-invalid"));
    }
    Ok(())
}

fn validate_storytelling_request_preview_input_schema(text: &str) -> Result<(), String> {
    let fields = [
        "context",
        "readerMentalModel",
        "reviewedManifestHash",
        "schemaVersion",
    ];
    let properties = validate_schema_root_version(
        text,
        "https://schemas.novelgainker.local/workflows/story-context-request-preview.v1.json",
        &fields,
        &fields,
        1,
    )?;
    let reviewed_hash = object(
        properties
            .get("reviewedManifestHash")
            .ok_or_else(|| invalid("workflow-pack.storytelling-reviewed-hash-missing"))?,
        "workflow-pack.storytelling-reviewed-hash-invalid",
    )?;
    exact_object_keys(
        reviewed_hash,
        &["pattern", "type"],
        "workflow-pack.storytelling-reviewed-hash-invalid",
    )?;
    if reviewed_hash.get("type").and_then(Value::as_str) != Some("string")
        || reviewed_hash.get("pattern").and_then(Value::as_str) != Some("^[0-9a-f]{64}$")
    {
        return Err(invalid("workflow-pack.storytelling-reviewed-hash-invalid"));
    }

    let context = object(
        properties
            .get("context")
            .ok_or_else(|| invalid("workflow-pack.storytelling-preview-context-missing"))?,
        "workflow-pack.storytelling-preview-context-invalid",
    )?;
    exact_object_keys(
        context,
        &["additionalProperties", "properties", "required", "type"],
        "workflow-pack.storytelling-preview-context-invalid",
    )?;
    let context_fields = [
        "action",
        "chapterNumber",
        "chapterTitle",
        "schemaVersion",
        "selectedSourcePaths",
    ];
    if context.get("type").and_then(Value::as_str) != Some("object")
        || context.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid(
            "workflow-pack.storytelling-preview-context-invalid",
        ));
    }
    exact_string_set(
        context
            .get("required")
            .ok_or_else(|| invalid("workflow-pack.storytelling-preview-context-invalid"))?,
        &context_fields,
        "workflow-pack.storytelling-preview-context-invalid",
    )?;
    let context_properties = object(
        context
            .get("properties")
            .ok_or_else(|| invalid("workflow-pack.storytelling-preview-context-invalid"))?,
        "workflow-pack.storytelling-preview-context-invalid",
    )?;
    exact_object_keys(
        context_properties,
        &context_fields,
        "workflow-pack.storytelling-preview-context-invalid",
    )?;
    let context_version = object(
        context_properties
            .get("schemaVersion")
            .ok_or_else(|| invalid("workflow-pack.storytelling-preview-context-invalid"))?,
        "workflow-pack.storytelling-preview-context-invalid",
    )?;
    if context_version.get("type").and_then(Value::as_str) != Some("integer")
        || context_version.get("const").and_then(Value::as_u64) != Some(2)
    {
        return Err(invalid(
            "workflow-pack.storytelling-preview-context-invalid",
        ));
    }

    let reader_model = object(
        properties
            .get("readerMentalModel")
            .ok_or_else(|| invalid("workflow-pack.storytelling-reader-model-missing"))?,
        "workflow-pack.storytelling-reader-model-invalid",
    )?;
    exact_object_keys(
        reader_model,
        &["additionalProperties", "properties", "required", "type"],
        "workflow-pack.storytelling-reader-model-invalid",
    )?;
    let reader_fields = [
        "chapterPromises",
        "forbiddenReveals",
        "readerInfers",
        "readerKnows",
        "readerMisreads",
        "readerQuestions",
    ];
    if reader_model.get("type").and_then(Value::as_str) != Some("object")
        || reader_model
            .get("additionalProperties")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid("workflow-pack.storytelling-reader-model-invalid"));
    }
    exact_string_set(
        reader_model
            .get("required")
            .ok_or_else(|| invalid("workflow-pack.storytelling-reader-model-invalid"))?,
        &reader_fields,
        "workflow-pack.storytelling-reader-model-invalid",
    )?;
    let reader_properties = object(
        reader_model
            .get("properties")
            .ok_or_else(|| invalid("workflow-pack.storytelling-reader-model-invalid"))?,
        "workflow-pack.storytelling-reader-model-invalid",
    )?;
    exact_object_keys(
        reader_properties,
        &reader_fields,
        "workflow-pack.storytelling-reader-model-invalid",
    )?;
    for field in reader_fields {
        let property = object(
            reader_properties
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.storytelling-reader-model-invalid"))?,
            "workflow-pack.storytelling-reader-model-invalid",
        )?;
        exact_object_keys(
            property,
            &["maxLength", "type"],
            "workflow-pack.storytelling-reader-model-invalid",
        )?;
        if property.get("type").and_then(Value::as_str) != Some("string")
            || property.get("maxLength").and_then(Value::as_u64) != Some(4_096)
        {
            return Err(invalid("workflow-pack.storytelling-reader-model-invalid"));
        }
    }
    Ok(())
}

fn validate_storytelling_request_snapshot_output_schema(text: &str) -> Result<(), String> {
    let fields = [
        "budget",
        "createdAt",
        "manifest",
        "readerMentalModel",
        "readerModelHash",
        "requestHash",
        "snapshotVersion",
        "sourceReview",
    ];
    let value: Value =
        serde_json::from_str(text).map_err(|_| invalid("workflow-pack.schema-json-invalid"))?;
    let root = object(&value, "workflow-pack.schema-root-invalid")?;
    exact_object_keys(
        root,
        &[
            "$id",
            "$schema",
            "additionalProperties",
            "properties",
            "required",
            "title",
            "type",
        ],
        "workflow-pack.schema-root-fields-invalid",
    )?;
    if root.get("$schema").and_then(Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
        || root.get("$id").and_then(Value::as_str)
            != Some(
                "https://schemas.novelgainker.local/workflows/story-context-request-snapshot.v1.json",
            )
        || root.get("type").and_then(Value::as_str) != Some("object")
        || root.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid(
            "workflow-pack.storytelling-request-snapshot-invalid",
        ));
    }
    exact_string_set(
        root.get("required")
            .ok_or_else(|| invalid("workflow-pack.schema-required-missing"))?,
        &fields,
        "workflow-pack.schema-required-invalid",
    )?;
    let properties = object(
        root.get("properties")
            .ok_or_else(|| invalid("workflow-pack.schema-properties-missing"))?,
        "workflow-pack.schema-properties-invalid",
    )?;
    exact_object_keys(
        properties,
        &fields,
        "workflow-pack.schema-property-set-invalid",
    )?;
    let snapshot_version = object(
        properties
            .get("snapshotVersion")
            .ok_or_else(|| invalid("workflow-pack.storytelling-request-snapshot-invalid"))?,
        "workflow-pack.storytelling-request-snapshot-invalid",
    )?;
    if snapshot_version.get("type").and_then(Value::as_str) != Some("integer")
        || snapshot_version.get("const").and_then(Value::as_u64) != Some(1)
    {
        return Err(invalid(
            "workflow-pack.storytelling-request-snapshot-invalid",
        ));
    }
    for field in ["readerModelHash", "requestHash"] {
        let property = object(
            properties
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.storytelling-request-snapshot-invalid"))?,
            "workflow-pack.storytelling-request-snapshot-invalid",
        )?;
        exact_object_keys(
            property,
            &["pattern", "type"],
            "workflow-pack.storytelling-request-snapshot-invalid",
        )?;
        if property.get("type").and_then(Value::as_str) != Some("string")
            || property.get("pattern").and_then(Value::as_str) != Some("^[0-9a-f]{64}$")
        {
            return Err(invalid(
                "workflow-pack.storytelling-request-snapshot-invalid",
            ));
        }
    }
    Ok(())
}

fn validate_storytelling_chapter_proposal_output_schema(text: &str) -> Result<(), String> {
    let fields = [
        "continuityClaims",
        "draftMarkdown",
        "intentSummary",
        "ledgerCandidates",
        "readerModelDelta",
        "schemaVersion",
        "title",
    ];
    let value: Value =
        serde_json::from_str(text).map_err(|_| invalid("workflow-pack.schema-json-invalid"))?;
    let root = object(&value, "workflow-pack.schema-root-invalid")?;
    exact_object_keys(
        root,
        &[
            "$id",
            "$schema",
            "additionalProperties",
            "properties",
            "required",
            "title",
            "type",
        ],
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    if root.get("$id").and_then(Value::as_str)
        != Some("https://schemas.novelgainker.local/workflows/chapter-proposal-payload.v1.json")
        || root.get("title").and_then(Value::as_str) != Some("ChapterProposalPayloadV1")
        || root.get("type").and_then(Value::as_str) != Some("object")
        || root.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid("workflow-pack.storytelling-chapter-schema-invalid"));
    }
    exact_string_set(
        root.get("required")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        &fields,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    let properties = object(
        root.get("properties")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    exact_object_keys(
        properties,
        &fields,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    let version = object(
        properties
            .get("schemaVersion")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    if version.get("type").and_then(Value::as_str) != Some("integer")
        || version.get("const").and_then(Value::as_u64) != Some(1)
    {
        return Err(invalid("workflow-pack.storytelling-chapter-schema-invalid"));
    }
    validate_string_property(
        properties
            .get("title")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        1,
        120,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    validate_string_property(
        properties
            .get("draftMarkdown")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        1,
        65_536,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    validate_string_property(
        properties
            .get("intentSummary")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        1,
        4_096,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    let reader = object(
        properties
            .get("readerModelDelta")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    let reader_fields = [
        "chapterPromises",
        "forbiddenReveals",
        "readerInfers",
        "readerKnows",
        "readerMisreads",
        "readerQuestions",
    ];
    if reader.get("type").and_then(Value::as_str) != Some("object")
        || reader.get("additionalProperties").and_then(Value::as_bool) != Some(false)
    {
        return Err(invalid("workflow-pack.storytelling-chapter-schema-invalid"));
    }
    exact_string_set(
        reader
            .get("required")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        &reader_fields,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    let reader_properties = object(
        reader
            .get("properties")
            .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    exact_object_keys(
        reader_properties,
        &reader_fields,
        "workflow-pack.storytelling-chapter-schema-invalid",
    )?;
    for field in reader_fields {
        let property = object(
            reader_properties
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        exact_object_keys(
            property,
            &["maxLength", "type"],
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        if property.get("type").and_then(Value::as_str) != Some("string")
            || property.get("maxLength").and_then(Value::as_u64) != Some(4_096)
        {
            return Err(invalid("workflow-pack.storytelling-chapter-schema-invalid"));
        }
    }
    for (field, text_field) in [
        ("continuityClaims", "claim"),
        ("ledgerCandidates", "summary"),
    ] {
        let list = object(
            properties
                .get(field)
                .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        if list.get("type").and_then(Value::as_str) != Some("array")
            || list.get("maxItems").and_then(Value::as_u64) != Some(64)
        {
            return Err(invalid("workflow-pack.storytelling-chapter-schema-invalid"));
        }
        let item = object(
            list.get("items")
                .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        if item.get("type").and_then(Value::as_str) != Some("object")
            || item.get("additionalProperties").and_then(Value::as_bool) != Some(false)
        {
            return Err(invalid("workflow-pack.storytelling-chapter-schema-invalid"));
        }
        exact_string_set(
            item.get("required")
                .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
            &[text_field, "sourceIds"],
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        let item_properties = object(
            item.get("properties")
                .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        exact_object_keys(
            item_properties,
            &[text_field, "sourceIds"],
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        validate_string_property(
            item_properties
                .get(text_field)
                .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
            1,
            1_000,
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        let source_ids = object(
            item_properties
                .get("sourceIds")
                .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
        if source_ids.get("type").and_then(Value::as_str) != Some("array")
            || source_ids.get("maxItems").and_then(Value::as_u64) != Some(16)
        {
            return Err(invalid("workflow-pack.storytelling-chapter-schema-invalid"));
        }
        validate_string_property(
            source_ids
                .get("items")
                .ok_or_else(|| invalid("workflow-pack.storytelling-chapter-schema-invalid"))?,
            1,
            512,
            "workflow-pack.storytelling-chapter-schema-invalid",
        )?;
    }
    Ok(())
}

fn validate_storytelling_template(text: &str) -> Result<(), String> {
    for marker in ["{{chapterId}}", "{{chapterNumber}}", "{{draft}}"] {
        if text.match_indices(marker).count() != 1 {
            return Err(invalid(
                "workflow-pack.storytelling-template-marker-invalid",
            ));
        }
    }
    if text.match_indices("{{title}}").count() != 2 {
        return Err(invalid(
            "workflow-pack.storytelling-template-marker-invalid",
        ));
    }
    for required in [
        "kind: chapter",
        "schemaVersion: 1",
        "writingLanguage: zh-CN",
        "status: draft",
        "## Draft",
    ] {
        if !text.contains(required) {
            return Err(invalid(
                "workflow-pack.storytelling-template-contract-invalid",
            ));
        }
    }
    Ok(())
}

fn validate_storytelling_workflow_instructions(text: &str) -> Result<(), String> {
    for required in [
        "# 小说续写与章节创作",
        "StoryContextManifestV2",
        "StoryContextRequestSnapshotV1",
        "story.build_context",
        "story.preview_request",
        "story.generate_proposal",
        "story.check_continuity",
        "story.preview_chapter_patch",
        "story.apply_chapter_patch",
        "story.transition_chapter_status",
        "story.update_ledger",
        "Reader Mental Model",
        "ChapterProposalV1",
        "needsReview",
        "4096 tokens",
        "固定 Provider",
        "Continuity Check",
        "不调用第二个 AI",
        "不写入 App State",
        "materials/000.story-ledger.md",
        "独立确认",
        "Ledger Pending",
        "简体中文",
    ] {
        if !text.contains(required) {
            return Err(invalid("workflow-pack.storytelling-instruction-invalid"));
        }
    }
    Ok(())
}

fn validate_router_resources(resources: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let rules_text = resource_text(resources, "router/deterministic-intents.v1.json")?;
    let rules: Value = serde_json::from_str(rules_text)
        .map_err(|_| invalid("workflow-pack.router-rules-json-invalid"))?;
    let root = object(&rules, "workflow-pack.router-rules-root-invalid")?;
    exact_object_keys(
        root,
        &["blockedTerms", "commands", "schemaVersion"],
        "workflow-pack.router-rules-fields-invalid",
    )?;
    if root.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err(invalid("workflow-pack.router-rules-version-invalid"));
    }
    let commands = root
        .get("commands")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("workflow-pack.router-commands-invalid"))?;
    let expected = [
        ("创建项目", PROJECT_WORKFLOW_ID, "default"),
        ("创建人物", CHARACTER_WORKFLOW_ID, "character-create"),
        ("完善人物", CHARACTER_WORKFLOW_ID, "character-enhance"),
        ("创建设定", SETTING_WORKFLOW_ID, "default"),
        ("创建素材", MATERIAL_WORKFLOW_ID, "default"),
        ("创建主剧情", PLOT_WORKFLOW_ID, "default"),
    ];
    if commands.len() != expected.len() {
        return Err(invalid("workflow-pack.router-commands-invalid"));
    }
    for (value, (command, workflow_id, entry_mode)) in commands.iter().zip(expected) {
        let item = object(value, "workflow-pack.router-command-invalid")?;
        exact_object_keys(
            item,
            &["command", "entryMode", "workflowId"],
            "workflow-pack.router-command-invalid",
        )?;
        if item.get("command").and_then(Value::as_str) != Some(command)
            || item.get("workflowId").and_then(Value::as_str) != Some(workflow_id)
            || item.get("entryMode").and_then(Value::as_str) != Some(entry_mode)
        {
            return Err(invalid("workflow-pack.router-command-invalid"));
        }
    }
    let blocked = root
        .get("blockedTerms")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("workflow-pack.router-blocked-terms-invalid"))?;
    if blocked.is_empty()
        || blocked.len() > 32
        || blocked
            .iter()
            .any(|value| value.as_str().is_none_or(str::is_empty))
    {
        return Err(invalid("workflow-pack.router-blocked-terms-invalid"));
    }

    let fields = ["confidence", "schemaVersion", "workflowId"];
    let properties = validate_schema_root_version(
        resource_text(resources, "router/router-decision.schema.json")?,
        "https://schemas.novelgainker.local/router-decision.v1.json",
        &fields,
        &fields,
        1,
    )?;
    let workflow_property = object(
        properties
            .get("workflowId")
            .ok_or_else(|| invalid("workflow-pack.router-schema-workflow-invalid"))?,
        "workflow-pack.router-schema-workflow-invalid",
    )?;
    exact_object_keys(
        workflow_property,
        &["enum", "type"],
        "workflow-pack.router-schema-workflow-invalid",
    )?;
    if workflow_property.get("type").and_then(Value::as_str) != Some("string") {
        return Err(invalid("workflow-pack.router-schema-workflow-invalid"));
    }
    exact_string_set(
        workflow_property
            .get("enum")
            .ok_or_else(|| invalid("workflow-pack.router-schema-workflow-invalid"))?,
        &[
            CHARACTER_WORKFLOW_ID,
            PROJECT_WORKFLOW_ID,
            SETTING_WORKFLOW_ID,
            MATERIAL_WORKFLOW_ID,
            PLOT_WORKFLOW_ID,
        ],
        "workflow-pack.router-schema-workflow-invalid",
    )?;
    let confidence_property = object(
        properties
            .get("confidence")
            .ok_or_else(|| invalid("workflow-pack.router-schema-confidence-invalid"))?,
        "workflow-pack.router-schema-confidence-invalid",
    )?;
    exact_object_keys(
        confidence_property,
        &["enum", "type"],
        "workflow-pack.router-schema-confidence-invalid",
    )?;
    if confidence_property.get("type").and_then(Value::as_str) != Some("string") {
        return Err(invalid("workflow-pack.router-schema-confidence-invalid"));
    }
    exact_string_set(
        confidence_property
            .get("enum")
            .ok_or_else(|| invalid("workflow-pack.router-schema-confidence-invalid"))?,
        &["high", "medium", "low"],
        "workflow-pack.router-schema-confidence-invalid",
    )?;

    let instructions = resource_text(resources, "router/workflow.md")?;
    for required in [
        "RouterDecisionV1",
        "executionAvailable",
        "routable",
        "不执行工具",
        "简体中文",
    ] {
        if !instructions.contains(required) {
            return Err(invalid("workflow-pack.router-instruction-invalid"));
        }
    }
    Ok(())
}
fn update_hash(hasher: &mut Sha256, path: &str, bytes: &[u8]) {
    hasher.update((path.len() as u64).to_be_bytes());
    hasher.update(path.as_bytes());
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn pack_hash(resources: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in resources {
        update_hash(&mut hasher, path, bytes);
    }
    format!("{:x}", hasher.finalize())
}

fn workflow_hash(
    workflow: &WorkflowManifest,
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<String, String> {
    let manifest_bytes = serde_json::to_vec(workflow)
        .map_err(|_| invalid("workflow-pack.workflow-canonicalization-failed"))?;
    let mut hasher = Sha256::new();
    let manifest_label = format!("@manifest/{}", workflow.id);
    update_hash(&mut hasher, &manifest_label, &manifest_bytes);
    let mut paths = [
        workflow.workflow_resource.as_str(),
        workflow.input_schema.as_str(),
        workflow.output_schema.as_str(),
        workflow.template_resource.as_str(),
    ];
    paths.sort_unstable();
    for path in paths {
        let bytes = resources
            .get(path)
            .ok_or_else(|| invalid("workflow-pack.resource-missing"))?;
        update_hash(&mut hasher, path, bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn validate_pack(resources: &BTreeMap<String, Vec<u8>>) -> Result<ValidatedPack, String> {
    let actual_paths = resources
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_paths = EXPECTED_RESOURCE_PATHS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        return Err(invalid("workflow-pack.resource-set-invalid"));
    }

    for (path, bytes) in resources {
        validate_relative_path(path)?;
        let text = resource_text(resources, path)?;
        validate_language_and_origin(path, text)?;
        if bytes.is_empty() {
            return Err(invalid("workflow-pack.resource-empty"));
        }
    }

    validate_router_resources(resources)?;

    let manifest_text = resource_text(resources, "pack.json")?;
    let manifest: PackManifest = serde_json::from_str(manifest_text)
        .map_err(|_| invalid("workflow-pack.manifest-invalid"))?;
    if manifest.schema_version != PACK_SCHEMA_VERSION
        || manifest.workflow_pack_version != WORKFLOW_PACK_VERSION
    {
        return Err(invalid("workflow-pack.version-invalid"));
    }
    if manifest.workflows.len() != 6 {
        return Err(invalid("workflow-pack.workflow-count-invalid"));
    }

    let mut ids = BTreeSet::new();
    let mut validated_workflows = Vec::with_capacity(manifest.workflows.len());
    for workflow in &manifest.workflows {
        if !ids.insert(workflow.id.as_str()) {
            return Err(invalid("workflow-pack.workflow-id-duplicate"));
        }
        let storytelling = workflow.id == STORYTELLING_WORKFLOW_ID;
        if (storytelling && (!workflow.execution_available || workflow.routable))
            || (!storytelling && (!workflow.execution_available || !workflow.routable))
            || workflow.description.trim().is_empty()
            || workflow.description.chars().count() > 160
            || ((workflow.id == CHARACTER_WORKFLOW_ID || storytelling)
                && (workflow.input_schema_version != 2 || workflow.output_schema_version != 2))
            || (workflow.id != CHARACTER_WORKFLOW_ID
                && !storytelling
                && (workflow.input_schema_version != 1 || workflow.output_schema_version != 1))
        {
            return Err(invalid("workflow-pack.workflow-contract-invalid"));
        }
        match workflow.id.as_str() {
            CHARACTER_WORKFLOW_ID => {
                if workflow.version != CHARACTER_WORKFLOW_VERSION
                    || workflow.mode != CHARACTER_MODE
                    || workflow.display_name != "创建/完善人物"
                    || workflow.allowed_tools.as_slice() != [CHARACTER_TOOL]
                    || workflow.required_confirmations.as_slice() != CHARACTER_CONFIRMATIONS
                {
                    return Err(invalid("workflow-pack.character-contract-invalid"));
                }
                let expected_references = [
                    (
                        "character-create/workflow.md",
                        workflow.workflow_resource.as_str(),
                    ),
                    (
                        "character-create/input.schema.json",
                        workflow.input_schema.as_str(),
                    ),
                    (
                        "character-create/output.schema.json",
                        workflow.output_schema.as_str(),
                    ),
                    (
                        "character-create/character.template.md",
                        workflow.template_resource.as_str(),
                    ),
                ];
                for (expected, actual) in expected_references {
                    validate_relative_path(actual)?;
                    if actual != expected || !resources.contains_key(actual) {
                        return Err(invalid("workflow-pack.resource-reference-invalid"));
                    }
                }
                validate_workflow_instructions(resource_text(
                    resources,
                    &workflow.workflow_resource,
                )?)?;
                validate_input_schema(resource_text(resources, &workflow.input_schema)?)?;
                validate_output_schema(resource_text(resources, &workflow.output_schema)?)?;
                validate_template(resource_text(resources, &workflow.template_resource)?)?;
            }
            PROJECT_WORKFLOW_ID => {
                if workflow.version != PROJECT_WORKFLOW_VERSION
                    || workflow.mode != PROJECT_MODE
                    || workflow.display_name != "创建项目"
                    || workflow.allowed_tools.as_slice() != PROJECT_TOOLS
                    || workflow.required_confirmations.as_slice() != PROJECT_CONFIRMATIONS
                {
                    return Err(invalid("workflow-pack.project-contract-invalid"));
                }
                let expected_references = [
                    (
                        "project-create/workflow.md",
                        workflow.workflow_resource.as_str(),
                    ),
                    (
                        "project-create/input.schema.json",
                        workflow.input_schema.as_str(),
                    ),
                    (
                        "project-create/output.schema.json",
                        workflow.output_schema.as_str(),
                    ),
                    (
                        "project-create/project.template.md",
                        workflow.template_resource.as_str(),
                    ),
                ];
                for (expected, actual) in expected_references {
                    validate_relative_path(actual)?;
                    if actual != expected || !resources.contains_key(actual) {
                        return Err(invalid("workflow-pack.resource-reference-invalid"));
                    }
                }
                validate_project_workflow_instructions(resource_text(
                    resources,
                    &workflow.workflow_resource,
                )?)?;
                validate_project_input_schema(resource_text(resources, &workflow.input_schema)?)?;
                validate_project_output_schema(resource_text(resources, &workflow.output_schema)?)?;
                validate_project_template(resource_text(resources, &workflow.template_resource)?)?;
            }
            SETTING_WORKFLOW_ID => {
                if workflow.version != SETTING_WORKFLOW_VERSION
                    || workflow.mode != SETTING_MODE
                    || workflow.display_name != "创建设定"
                    || workflow.allowed_tools.as_slice() != [SETTING_TOOL]
                    || workflow.required_confirmations.as_slice() != SETTING_CONFIRMATIONS
                {
                    return Err(invalid("workflow-pack.setting-contract-invalid"));
                }
                let expected_references = [
                    (
                        "setting-create/workflow.md",
                        workflow.workflow_resource.as_str(),
                    ),
                    (
                        "setting-create/input.schema.json",
                        workflow.input_schema.as_str(),
                    ),
                    (
                        "setting-create/output.schema.json",
                        workflow.output_schema.as_str(),
                    ),
                    (
                        "setting-create/setting.template.md",
                        workflow.template_resource.as_str(),
                    ),
                ];
                for (expected, actual) in expected_references {
                    validate_relative_path(actual)?;
                    if actual != expected || !resources.contains_key(actual) {
                        return Err(invalid("workflow-pack.resource-reference-invalid"));
                    }
                }
                validate_setting_workflow_instructions(resource_text(
                    resources,
                    &workflow.workflow_resource,
                )?)?;
                validate_setting_input_schema(resource_text(resources, &workflow.input_schema)?)?;
                validate_setting_output_schema(resource_text(resources, &workflow.output_schema)?)?;
                validate_setting_template(resource_text(resources, &workflow.template_resource)?)?;
            }
            MATERIAL_WORKFLOW_ID => {
                if workflow.version != MATERIAL_WORKFLOW_VERSION
                    || workflow.mode != MATERIAL_MODE
                    || workflow.display_name != "创建素材"
                    || workflow.allowed_tools.as_slice() != [MATERIAL_TOOL]
                    || workflow.required_confirmations.as_slice() != MATERIAL_CONFIRMATIONS
                {
                    return Err(invalid("workflow-pack.material-contract-invalid"));
                }
                let expected_references = [
                    (
                        "material-create/workflow.md",
                        workflow.workflow_resource.as_str(),
                    ),
                    (
                        "material-create/input.schema.json",
                        workflow.input_schema.as_str(),
                    ),
                    (
                        "material-create/output.schema.json",
                        workflow.output_schema.as_str(),
                    ),
                    (
                        "material-create/material.template.md",
                        workflow.template_resource.as_str(),
                    ),
                ];
                for (expected, actual) in expected_references {
                    validate_relative_path(actual)?;
                    if actual != expected || !resources.contains_key(actual) {
                        return Err(invalid("workflow-pack.resource-reference-invalid"));
                    }
                }
                validate_material_workflow_instructions(resource_text(
                    resources,
                    &workflow.workflow_resource,
                )?)?;
                validate_material_input_schema(resource_text(resources, &workflow.input_schema)?)?;
                validate_material_output_schema(resource_text(
                    resources,
                    &workflow.output_schema,
                )?)?;
                validate_material_template(resource_text(resources, &workflow.template_resource)?)?;
            }
            PLOT_WORKFLOW_ID => {
                if workflow.version != PLOT_WORKFLOW_VERSION
                    || workflow.mode != PLOT_MODE
                    || workflow.display_name != "创建主剧情"
                    || workflow.allowed_tools.as_slice() != [PLOT_TOOL]
                    || workflow.required_confirmations.as_slice() != PLOT_CONFIRMATIONS
                {
                    return Err(invalid("workflow-pack.plot-contract-invalid"));
                }
                let expected_references = [
                    (
                        "plot-create/workflow.md",
                        workflow.workflow_resource.as_str(),
                    ),
                    (
                        "plot-create/input.schema.json",
                        workflow.input_schema.as_str(),
                    ),
                    (
                        "plot-create/output.schema.json",
                        workflow.output_schema.as_str(),
                    ),
                    (
                        "plot-create/plot.template.md",
                        workflow.template_resource.as_str(),
                    ),
                ];
                for (expected, actual) in expected_references {
                    validate_relative_path(actual)?;
                    if actual != expected || !resources.contains_key(actual) {
                        return Err(invalid("workflow-pack.resource-reference-invalid"));
                    }
                }
                validate_plot_workflow_instructions(resource_text(
                    resources,
                    &workflow.workflow_resource,
                )?)?;
                validate_plot_input_schema(resource_text(resources, &workflow.input_schema)?)?;
                validate_plot_output_schema(resource_text(resources, &workflow.output_schema)?)?;
                validate_plot_template(resource_text(resources, &workflow.template_resource)?)?;
            }
            STORYTELLING_WORKFLOW_ID => {
                if workflow.version != STORYTELLING_WORKFLOW_VERSION
                    || workflow.mode != STORYTELLING_MODE
                    || workflow.display_name != "小说续写与章节创作"
                    || workflow.allowed_tools.as_slice() != STORYTELLING_TOOLS
                    || workflow.required_confirmations.as_slice() != STORYTELLING_CONFIRMATIONS
                {
                    return Err(invalid("workflow-pack.storytelling-contract-invalid"));
                }
                let expected_references = [
                    (
                        "storytelling/workflow.md",
                        workflow.workflow_resource.as_str(),
                    ),
                    (
                        "storytelling/input.schema.json",
                        workflow.input_schema.as_str(),
                    ),
                    (
                        "storytelling/output.schema.json",
                        workflow.output_schema.as_str(),
                    ),
                    (
                        "storytelling/chapter.template.md",
                        workflow.template_resource.as_str(),
                    ),
                ];
                for (expected, actual) in expected_references {
                    validate_relative_path(actual)?;
                    if actual != expected || !resources.contains_key(actual) {
                        return Err(invalid("workflow-pack.resource-reference-invalid"));
                    }
                }
                validate_storytelling_workflow_instructions(resource_text(
                    resources,
                    &workflow.workflow_resource,
                )?)?;
                validate_storytelling_input_schema(resource_text(
                    resources,
                    &workflow.input_schema,
                )?)?;
                validate_storytelling_output_schema(resource_text(
                    resources,
                    &workflow.output_schema,
                )?)?;
                validate_storytelling_request_preview_input_schema(resource_text(
                    resources,
                    "storytelling/request-preview.input.schema.json",
                )?)?;
                validate_storytelling_request_snapshot_output_schema(resource_text(
                    resources,
                    "storytelling/request-snapshot.output.schema.json",
                )?)?;
                validate_storytelling_chapter_proposal_output_schema(resource_text(
                    resources,
                    "storytelling/chapter-proposal.output.schema.json",
                )?)?;
                validate_storytelling_template(resource_text(
                    resources,
                    &workflow.template_resource,
                )?)?;
            }
            _ => return Err(invalid("workflow-pack.workflow-id-invalid")),
        }
        validated_workflows.push(ValidatedWorkflow {
            id: workflow.id.clone(),
            content_hash: workflow_hash(workflow, resources)?,
        });
    }

    Ok(ValidatedPack {
        pack_hash: pack_hash(resources),
        workflows: validated_workflows,
    })
}
