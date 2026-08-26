//! Build-validated, runtime-read-only NovelGainker workflow definitions.

pub mod character;
pub mod character_v2;
pub mod context;
pub mod manager;
pub mod material;
pub mod plot;
pub mod proposal;
pub mod router;
pub mod setting;
pub mod story_context;

pub const WORKFLOW_REGISTRY_SCHEMA_VERSION: u16 = 1;
pub const WORKFLOW_PACK_SCHEMA_VERSION: u16 = 1;
pub const WORKFLOW_PACK_VERSION: &str = "1.0.0";
pub const CHARACTER_CREATE_WORKFLOW_ID: &str = "novelgainker.character-create";
pub const PROJECT_CREATE_WORKFLOW_ID: &str = "novelgainker.project-create";
pub const SETTING_CREATE_WORKFLOW_ID: &str = "novelgainker.setting-create";
pub const MATERIAL_CREATE_WORKFLOW_ID: &str = "novelgainker.material-create";
pub const PLOT_CREATE_WORKFLOW_ID: &str = "novelgainker.plot-create";
pub const STORYTELLING_WORKFLOW_ID: &str = "novelgainker.storytelling";

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WorkflowDefinition {
    pub id: &'static str,
    pub version: u16,
    pub display_name: &'static str,
    pub description: &'static str,
    pub mode: &'static str,
    pub execution_available: bool,
    pub routable: bool,
    pub content_hash: &'static str,
    pub allowed_tools: &'static [&'static str],
    pub required_confirmations: &'static [&'static str],
    pub input_schema_version: u16,
    pub output_schema_version: u16,
    resources: &'static WorkflowResources,
}

impl WorkflowDefinition {
    pub fn resources(self) -> &'static WorkflowResources {
        self.resources
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WorkflowRegistry {
    pub schema_version: u16,
    pub pack_schema_version: u16,
    pub workflow_pack_version: &'static str,
    pub pack_hash: &'static str,
    pub workflows: &'static [WorkflowDefinition],
    manifest_source: &'static str,
}

impl WorkflowRegistry {
    pub fn manifest_source(self) -> &'static str {
        self.manifest_source
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WorkflowResources {
    pub workflow_instructions: &'static str,
    pub input_schema: &'static str,
    pub output_schema: &'static str,
    pub template_resource: &'static str,
}

static CHARACTER_RESOURCES: WorkflowResources = WorkflowResources {
    workflow_instructions: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/character-create/workflow.md"
    )),
    input_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/character-create/input.schema.json"
    )),
    output_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/character-create/output.schema.json"
    )),
    template_resource: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/character-create/character.template.md"
    )),
};
static PROJECT_RESOURCES: WorkflowResources = WorkflowResources {
    workflow_instructions: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/project-create/workflow.md"
    )),
    input_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/project-create/input.schema.json"
    )),
    output_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/project-create/output.schema.json"
    )),
    template_resource: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/project-create/project.template.md"
    )),
};
static SETTING_RESOURCES: WorkflowResources = WorkflowResources {
    workflow_instructions: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/setting-create/workflow.md"
    )),
    input_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/setting-create/input.schema.json"
    )),
    output_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/setting-create/output.schema.json"
    )),
    template_resource: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/setting-create/setting.template.md"
    )),
};
static MATERIAL_RESOURCES: WorkflowResources = WorkflowResources {
    workflow_instructions: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/material-create/workflow.md"
    )),
    input_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/material-create/input.schema.json"
    )),
    output_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/material-create/output.schema.json"
    )),
    template_resource: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/material-create/material.template.md"
    )),
};
static PLOT_RESOURCES: WorkflowResources = WorkflowResources {
    workflow_instructions: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/plot-create/workflow.md"
    )),
    input_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/plot-create/input.schema.json"
    )),
    output_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/plot-create/output.schema.json"
    )),
    template_resource: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/plot-create/plot.template.md"
    )),
};
static STORYTELLING_RESOURCES: WorkflowResources = WorkflowResources {
    workflow_instructions: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/storytelling/workflow.md"
    )),
    input_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/storytelling/input.schema.json"
    )),
    output_schema: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/storytelling/output.schema.json"
    )),
    template_resource: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/workflows/v1/storytelling/chapter.template.md"
    )),
};
static PACK_MANIFEST_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../resources/workflows/v1/pack.json"
));

const CHARACTER_TOOLS: &[&str] = &["source.create_character"];
const CHARACTER_CONFIRMATIONS: &[&str] = &["send_context", "create_or_replace_character_file"];
const PROJECT_TOOLS: &[&str] = &["project.scan", "project.create"];
const PROJECT_CONFIRMATIONS: &[&str] = &["create_project_scaffold"];
const SETTING_TOOLS: &[&str] = &["source.create_setting"];
const SETTING_CONFIRMATIONS: &[&str] = &["send_context", "create_setting_file"];
const MATERIAL_TOOLS: &[&str] = &["source.create_material"];
const MATERIAL_CONFIRMATIONS: &[&str] = &["send_context", "promote_material_to_canon"];
const PLOT_TOOLS: &[&str] = &["source.create_plot"];
const PLOT_CONFIRMATIONS: &[&str] = &["send_context", "create_or_replace_main_plot"];
const STORYTELLING_TOOLS: &[&str] = &["story.build_context"];
const STORYTELLING_CONFIRMATIONS: &[&str] = &["select_context"];
const WORKFLOWS: &[WorkflowDefinition] = &[
    WorkflowDefinition {
        id: CHARACTER_CREATE_WORKFLOW_ID,
        version: 2,
        display_name: "创建/完善人物",
        description: "审查显式选择的 Canon 来源，创建或完善单个人物 Markdown，并保留稳定实体引用。",
        mode: "interactive-write",
        execution_available: true,
        routable: true,
        content_hash: env!("NOVELGAINKER_CHARACTER_WORKFLOW_SHA256"),
        allowed_tools: CHARACTER_TOOLS,
        required_confirmations: CHARACTER_CONFIRMATIONS,
        input_schema_version: 2,
        output_schema_version: 2,
        resources: &CHARACTER_RESOURCES,
    },
    WorkflowDefinition {
        id: PROJECT_CREATE_WORKFLOW_ID,
        version: 1,
        display_name: "创建项目",
        description: "离线填写项目元数据，安全审查完整脚手架后创建本地 Markdown 小说项目。",
        mode: "deterministic-write",
        execution_available: true,
        routable: true,
        content_hash: env!("NOVELGAINKER_PROJECT_WORKFLOW_SHA256"),
        allowed_tools: PROJECT_TOOLS,
        required_confirmations: PROJECT_CONFIRMATIONS,
        input_schema_version: 1,
        output_schema_version: 1,
        resources: &PROJECT_RESOURCES,
    },
    WorkflowDefinition {
        id: SETTING_CREATE_WORKFLOW_ID,
        version: 1,
        display_name: "创建设定",
        description:
            "审查显式选择的世界资料，生成结构化设定提案，经确认后写入一个 world Markdown。",
        mode: "interactive-write",
        execution_available: true,
        routable: true,
        content_hash: env!("NOVELGAINKER_SETTING_WORKFLOW_SHA256"),
        allowed_tools: SETTING_TOOLS,
        required_confirmations: SETTING_CONFIRMATIONS,
        input_schema_version: 1,
        output_schema_version: 1,
        resources: &SETTING_RESOURCES,
    },
    WorkflowDefinition {
        id: MATERIAL_CREATE_WORKFLOW_ID,
        version: 1,
        display_name: "创建素材",
        description:
            "审查显式选择的 Canon 来源，生成结构化素材提案，经确认后写入一个普通 materials Markdown。",
        mode: "interactive-write",
        execution_available: true,
        routable: true,
        content_hash: env!("NOVELGAINKER_MATERIAL_WORKFLOW_SHA256"),
        allowed_tools: MATERIAL_TOOLS,
        required_confirmations: MATERIAL_CONFIRMATIONS,
        input_schema_version: 1,
        output_schema_version: 1,
        resources: &MATERIAL_RESOURCES,
    },
    WorkflowDefinition {
        id: PLOT_CREATE_WORKFLOW_ID,
        version: 1,
        display_name: "创建主剧情",
        description:
            "审查显式选择的人物、设定和普通素材，生成结构化主剧情提案，经确认后写入固定 plot/main.md。",
        mode: "interactive-write",
        execution_available: true,
        routable: true,
        content_hash: env!("NOVELGAINKER_PLOT_WORKFLOW_SHA256"),
        allowed_tools: PLOT_TOOLS,
        required_confirmations: PLOT_CONFIRMATIONS,
        input_schema_version: 1,
        output_schema_version: 1,
        resources: &PLOT_RESOURCES,
    },
    WorkflowDefinition {
        id: STORYTELLING_WORKFLOW_ID,
        version: 1,
        display_name: "小说续写与章节创作",
        description: "审查确定性 Context Manifest；本阶段只构建上下文，不调用 Provider 或写入章节。",
        mode: "interactive-write",
        execution_available: false,
        routable: false,
        content_hash: env!("NOVELGAINKER_STORYTELLING_WORKFLOW_SHA256"),
        allowed_tools: STORYTELLING_TOOLS,
        required_confirmations: STORYTELLING_CONFIRMATIONS,
        input_schema_version: 2,
        output_schema_version: 2,
        resources: &STORYTELLING_RESOURCES,
    },
];

static REGISTRY: WorkflowRegistry = WorkflowRegistry {
    schema_version: WORKFLOW_REGISTRY_SCHEMA_VERSION,
    pack_schema_version: WORKFLOW_PACK_SCHEMA_VERSION,
    workflow_pack_version: WORKFLOW_PACK_VERSION,
    pack_hash: env!("NOVELGAINKER_WORKFLOW_PACK_SHA256"),
    workflows: WORKFLOWS,
    manifest_source: PACK_MANIFEST_SOURCE,
};

pub fn registry() -> &'static WorkflowRegistry {
    &REGISTRY
}

pub fn workflow_resources(workflow_id: &str) -> Option<&'static WorkflowResources> {
    REGISTRY
        .workflows
        .iter()
        .find(|workflow| workflow.id == workflow_id)
        .map(|workflow| workflow.resources())
}

#[cfg(test)]
#[path = "pack_validation.rs"]
mod pack_validation;

#[cfg(test)]
mod tests {
    use super::{
        registry, workflow_resources, CHARACTER_CREATE_WORKFLOW_ID, PROJECT_CREATE_WORKFLOW_ID,
    };
    use crate::pack_validation::{validate_pack, EXPECTED_RESOURCE_PATHS};
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    fn embedded_resources() -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            (
                "pack.json".to_owned(),
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../resources/workflows/v1/pack.json"
                ))
                .to_vec(),
            ),
            (
                "character-create/workflow.md".to_owned(),
                super::CHARACTER_RESOURCES
                    .workflow_instructions
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "character-create/input.schema.json".to_owned(),
                super::CHARACTER_RESOURCES.input_schema.as_bytes().to_vec(),
            ),
            (
                "character-create/output.schema.json".to_owned(),
                super::CHARACTER_RESOURCES.output_schema.as_bytes().to_vec(),
            ),
            (
                "character-create/character.template.md".to_owned(),
                super::CHARACTER_RESOURCES
                    .template_resource
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "project-create/workflow.md".to_owned(),
                super::PROJECT_RESOURCES
                    .workflow_instructions
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "project-create/input.schema.json".to_owned(),
                super::PROJECT_RESOURCES.input_schema.as_bytes().to_vec(),
            ),
            (
                "project-create/output.schema.json".to_owned(),
                super::PROJECT_RESOURCES.output_schema.as_bytes().to_vec(),
            ),
            (
                "project-create/project.template.md".to_owned(),
                super::PROJECT_RESOURCES
                    .template_resource
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "router/deterministic-intents.v1.json".to_owned(),
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../resources/workflows/v1/router/deterministic-intents.v1.json"
                ))
                .to_vec(),
            ),
            (
                "router/router-decision.schema.json".to_owned(),
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../resources/workflows/v1/router/router-decision.schema.json"
                ))
                .to_vec(),
            ),
            (
                "router/workflow.md".to_owned(),
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../resources/workflows/v1/router/workflow.md"
                ))
                .to_vec(),
            ),
            (
                "setting-create/workflow.md".to_owned(),
                super::SETTING_RESOURCES
                    .workflow_instructions
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "setting-create/input.schema.json".to_owned(),
                super::SETTING_RESOURCES.input_schema.as_bytes().to_vec(),
            ),
            (
                "setting-create/output.schema.json".to_owned(),
                super::SETTING_RESOURCES.output_schema.as_bytes().to_vec(),
            ),
            (
                "setting-create/setting.template.md".to_owned(),
                super::SETTING_RESOURCES
                    .template_resource
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "material-create/workflow.md".to_owned(),
                super::MATERIAL_RESOURCES
                    .workflow_instructions
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "material-create/input.schema.json".to_owned(),
                super::MATERIAL_RESOURCES.input_schema.as_bytes().to_vec(),
            ),
            (
                "material-create/output.schema.json".to_owned(),
                super::MATERIAL_RESOURCES.output_schema.as_bytes().to_vec(),
            ),
            (
                "material-create/material.template.md".to_owned(),
                super::MATERIAL_RESOURCES
                    .template_resource
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "plot-create/workflow.md".to_owned(),
                super::PLOT_RESOURCES
                    .workflow_instructions
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "plot-create/input.schema.json".to_owned(),
                super::PLOT_RESOURCES.input_schema.as_bytes().to_vec(),
            ),
            (
                "plot-create/output.schema.json".to_owned(),
                super::PLOT_RESOURCES.output_schema.as_bytes().to_vec(),
            ),
            (
                "plot-create/plot.template.md".to_owned(),
                super::PLOT_RESOURCES.template_resource.as_bytes().to_vec(),
            ),
            (
                "storytelling/workflow.md".to_owned(),
                super::STORYTELLING_RESOURCES
                    .workflow_instructions
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "storytelling/input.schema.json".to_owned(),
                super::STORYTELLING_RESOURCES
                    .input_schema
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "storytelling/output.schema.json".to_owned(),
                super::STORYTELLING_RESOURCES
                    .output_schema
                    .as_bytes()
                    .to_vec(),
            ),
            (
                "storytelling/chapter.template.md".to_owned(),
                super::STORYTELLING_RESOURCES
                    .template_resource
                    .as_bytes()
                    .to_vec(),
            ),
        ])
    }

    fn mutate_manifest(mutator: impl FnOnce(&mut Value)) -> BTreeMap<String, Vec<u8>> {
        let mut resources = embedded_resources();
        let mut manifest: Value = serde_json::from_slice(&resources["pack.json"]).unwrap();
        mutator(&mut manifest);
        resources.insert(
            "pack.json".to_owned(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        resources
    }

    #[test]
    fn embedded_pack_matches_the_build_validated_registry() {
        let resources = embedded_resources();
        let validated = validate_pack(&resources).expect("embedded pack must validate");
        let registry = registry();

        assert_eq!(resources.len(), EXPECTED_RESOURCE_PATHS.len());
        assert_eq!(registry.schema_version, 1);
        assert_eq!(registry.pack_schema_version, 1);
        assert_eq!(registry.workflow_pack_version, "1.0.0");
        assert_eq!(registry.pack_hash, validated.pack_hash);
        assert_eq!(registry.workflows.len(), 6);
        let workflow = registry.workflows[0];
        assert_eq!(workflow.id, CHARACTER_CREATE_WORKFLOW_ID);
        assert_eq!(workflow.version, 2);
        assert_eq!(workflow.mode, "interactive-write");
        assert!(workflow.execution_available);
        assert!(workflow.routable);
        assert_eq!(workflow.allowed_tools, ["source.create_character"]);
        assert_eq!(
            workflow.required_confirmations,
            ["send_context", "create_or_replace_character_file"]
        );
        assert_eq!(workflow.content_hash, validated.workflows[0].content_hash);
        assert!(workflow_resources(CHARACTER_CREATE_WORKFLOW_ID).is_some());
        assert!(workflow_resources("novelgainker.unknown").is_none());
        let project = registry.workflows[1];
        assert_eq!(project.id, PROJECT_CREATE_WORKFLOW_ID);
        assert_eq!(project.version, 1);
        assert_eq!(project.mode, "deterministic-write");
        assert!(project.execution_available);
        assert_eq!(project.allowed_tools, ["project.scan", "project.create"]);
        let setting = registry.workflows[2];
        assert_eq!(setting.id, super::SETTING_CREATE_WORKFLOW_ID);
        assert_eq!(setting.version, 1);
        assert_eq!(setting.mode, "interactive-write");
        assert!(setting.execution_available);
        assert_eq!(setting.allowed_tools, ["source.create_setting"]);
        assert_eq!(
            setting.required_confirmations,
            ["send_context", "create_setting_file"]
        );
        assert!(workflow_resources(super::SETTING_CREATE_WORKFLOW_ID).is_some());
        let material = registry.workflows[3];
        assert_eq!(material.id, super::MATERIAL_CREATE_WORKFLOW_ID);
        assert_eq!(material.version, 1);
        assert_eq!(material.mode, "interactive-write");
        assert!(material.execution_available);
        assert_eq!(material.allowed_tools, ["source.create_material"]);
        assert_eq!(
            material.required_confirmations,
            ["send_context", "promote_material_to_canon"]
        );
        assert_eq!(material.content_hash, validated.workflows[3].content_hash);
        assert!(workflow_resources(super::MATERIAL_CREATE_WORKFLOW_ID).is_some());
        let plot = registry.workflows[4];
        assert_eq!(plot.id, super::PLOT_CREATE_WORKFLOW_ID);
        assert_eq!(plot.version, 1);
        assert_eq!(plot.mode, "interactive-write");
        assert!(plot.execution_available);
        assert_eq!(plot.allowed_tools, ["source.create_plot"]);
        assert_eq!(
            plot.required_confirmations,
            ["send_context", "create_or_replace_main_plot"]
        );
        assert_eq!(plot.content_hash, validated.workflows[4].content_hash);
        assert!(workflow_resources(super::PLOT_CREATE_WORKFLOW_ID).is_some());
        let storytelling = registry.workflows[5];
        assert_eq!(storytelling.id, super::STORYTELLING_WORKFLOW_ID);
        assert_eq!(storytelling.version, 1);
        assert_eq!(storytelling.mode, "interactive-write");
        assert!(!storytelling.execution_available);
        assert!(!storytelling.routable);
        assert_eq!(storytelling.allowed_tools, ["story.build_context"]);
        assert_eq!(storytelling.required_confirmations, ["select_context"]);
        assert_eq!(
            storytelling.content_hash,
            validated.workflows[5].content_hash
        );
        assert!(workflow_resources(super::STORYTELLING_WORKFLOW_ID).is_some());
        assert_eq!(project.required_confirmations, ["create_project_scaffold"]);
        assert_eq!(project.content_hash, validated.workflows[1].content_hash);
        assert!(workflow_resources(PROJECT_CREATE_WORKFLOW_ID).is_some());
    }

    #[test]
    fn hashes_are_deterministic_and_change_with_resource_bytes() {
        let original = embedded_resources();
        let first = validate_pack(&original).unwrap();
        let second = validate_pack(&original).unwrap();
        assert_eq!(first, second);

        let mut changed = original;
        changed
            .get_mut("character-create/workflow.md")
            .unwrap()
            .extend_from_slice(b"\n");
        let changed = validate_pack(&changed).unwrap();
        assert_ne!(first.pack_hash, changed.pack_hash);
        assert_ne!(
            first.workflows[0].content_hash,
            changed.workflows[0].content_hash
        );
    }

    #[test]
    fn rejects_duplicate_ids_unknown_fields_illegal_tools_and_missing_confirmations() {
        let duplicate = mutate_manifest(|manifest| {
            let workflows = manifest["workflows"].as_array_mut().unwrap();
            workflows.push(workflows[0].clone());
        });
        assert!(validate_pack(&duplicate).is_err());

        let unknown = mutate_manifest(|manifest| {
            manifest["workflows"][0]["unapprovedField"] = json!(true);
        });
        assert!(validate_pack(&unknown).is_err());

        let illegal_tool = mutate_manifest(|manifest| {
            manifest["workflows"][0]["allowedTools"] = json!(["filesystem.write"]);
        });
        assert!(validate_pack(&illegal_tool).is_err());

        let missing_confirmation = mutate_manifest(|manifest| {
            manifest["workflows"][0]["requiredConfirmations"] = json!(["send_context"]);
        });
        assert!(validate_pack(&missing_confirmation).is_err());
    }

    #[test]
    fn rejects_path_traversal_extra_files_and_invalid_encoding() {
        let traversal = mutate_manifest(|manifest| {
            manifest["workflows"][0]["inputSchema"] = json!("../input.schema.json");
        });
        assert!(validate_pack(&traversal).is_err());

        let mut extra = embedded_resources();
        extra.insert("SKILL.md".to_owned(), b"external runtime".to_vec());
        assert!(validate_pack(&extra).is_err());

        let mut invalid_utf8 = embedded_resources();
        invalid_utf8.insert("character-create/workflow.md".to_owned(), vec![0xff]);
        assert!(validate_pack(&invalid_utf8).is_err());

        let mut crlf = embedded_resources();
        crlf.get_mut("character-create/workflow.md")
            .unwrap()
            .extend_from_slice(b"\r\n");
        assert!(validate_pack(&crlf).is_err());
    }
}
