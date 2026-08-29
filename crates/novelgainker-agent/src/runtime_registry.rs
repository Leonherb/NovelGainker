use crate::action_contracts::RegisteredToolIdV1;
use crate::SKILL_IDS;

pub const AM1_READ_ONLY_TOOL_IDS: [RegisteredToolIdV1; 4] = [
    RegisteredToolIdV1::ProjectListMarkdown,
    RegisteredToolIdV1::ProjectReadMarkdown,
    RegisteredToolIdV1::ProjectSearchMarkdown,
    RegisteredToolIdV1::ProjectValidateMarkdown,
];

pub const AM2_TOOL_IDS: [RegisteredToolIdV1; 6] = [
    RegisteredToolIdV1::ProjectListMarkdown,
    RegisteredToolIdV1::ProjectReadMarkdown,
    RegisteredToolIdV1::ProjectSearchMarkdown,
    RegisteredToolIdV1::ProjectValidateMarkdown,
    RegisteredToolIdV1::ProjectCreateMarkdown,
    RegisteredToolIdV1::ProjectPatchMarkdown,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSkillDescriptorV1 {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub trigger_summary: &'static str,
    pub read_scopes: &'static [&'static str],
    pub write_scopes: &'static [&'static str],
    pub instruction: &'static str,
}

const COMMON_ENTITY_SCOPES: &[&str] = &[
    "characters/*.md",
    "materials/*.md",
    "plot/*.md",
    "project.md",
    "style/*.md",
    "world/*.md",
];

const RUNTIME_SKILLS: [RuntimeSkillDescriptorV1; 9] = [
    RuntimeSkillDescriptorV1 {
        id: "create-novel-project",
        display_name: "小说项目",
        description: "检查小说项目结构、项目说明与写作规范。",
        trigger_summary: "项目结构、初始化、项目说明或写作规范。",
        read_scopes: &["project.md", "style/*.md"],
        write_scopes: &["project.md"],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/create-novel-project.md"
        )),
    },
    RuntimeSkillDescriptorV1 {
        id: "create-setting",
        display_name: "设定",
        description: "分析世界、地点、时代、规则、限制与氛围设定。",
        trigger_summary: "世界观、地点、时代、规则、社会或氛围。",
        read_scopes: COMMON_ENTITY_SCOPES,
        write_scopes: &["world/*.md"],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/create-setting.md"
        )),
    },
    RuntimeSkillDescriptorV1 {
        id: "create-character",
        display_name: "人物",
        description: "分析人物、关系、动机、秘密、冲突和剧情功能。",
        trigger_summary: "人物、角色、关系、动机、秘密或人物弧。",
        read_scopes: COMMON_ENTITY_SCOPES,
        write_scopes: &["characters/*.md"],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/create-character.md"
        )),
    },
    RuntimeSkillDescriptorV1 {
        id: "create-material",
        display_name: "素材",
        description: "分析主题、物件、线索、研究笔记与场景素材。",
        trigger_summary: "素材、主题、物件、线索、研究或场景种子。",
        read_scopes: &[
            "characters/*.md",
            "materials/*.md",
            "plot/*.md",
            "project.md",
            "world/*.md",
        ],
        write_scopes: &["materials/*.md"],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/create-material.md"
        )),
    },
    RuntimeSkillDescriptorV1 {
        id: "create-plot",
        display_name: "剧情",
        description: "分析主剧情、支线、转折、揭示、回收与未决问题。",
        trigger_summary: "剧情、情节、大纲、转折、揭示、伏笔或回收。",
        read_scopes: &[
            "characters/*.md",
            "materials/*.md",
            "plot/*.md",
            "project.md",
            "world/*.md",
        ],
        write_scopes: &["plot/*.md"],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/create-plot.md"
        )),
    },
    RuntimeSkillDescriptorV1 {
        id: "novel-story-telling",
        display_name: "叙事",
        description: "分析读者认知、章间流动、连续性、伏笔与 Story Ledger。",
        trigger_summary: "章节、写章、续写、修订、叙事节奏或连续性。",
        read_scopes: &[
            "chapters/*.md",
            "characters/*.md",
            "materials/*.md",
            "outlines/*.md",
            "plot/*.md",
            "project.md",
            "style/*.md",
            "world/*.md",
        ],
        write_scopes: &["chapters/*.md", "materials/000.story-ledger.md"],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/novel-story-telling.md"
        )),
    },
    RuntimeSkillDescriptorV1 {
        id: "analytical-review",
        display_name: "评审",
        description: "对大纲、章节、全稿或既有修订结论进行只读评审。",
        trigger_summary: "评审、批评、校对、检查、审稿或回归验证。",
        read_scopes: &["**/*.md"],
        write_scopes: &[],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/analytical-review.md"
        )),
    },
    RuntimeSkillDescriptorV1 {
        id: "create-visual-asset",
        display_name: "视觉素材",
        description: "只读分析封面、插图与视觉风格所需的项目来源。",
        trigger_summary: "封面、插图、配图、视觉、风格图或图像 Prompt。",
        read_scopes: &["assets/**", "project.md", "style/*.md"],
        write_scopes: &[],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/create-visual-asset.md"
        )),
    },
    RuntimeSkillDescriptorV1 {
        id: "publish-novel",
        display_name: "出版",
        description: "只读执行 EPUB 出版前的内容与资产准备检查。",
        trigger_summary: "出版、导出、EPUB、构建、打包或发布预检。",
        read_scopes: &["**/*.md", "assets/**"],
        write_scopes: &[],
        instruction: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resources/skills/novelist/v1/instructions/publish-novel.md"
        )),
    },
];

pub fn runtime_skill_registry() -> &'static [RuntimeSkillDescriptorV1] {
    &RUNTIME_SKILLS
}

pub fn runtime_skill(id: &str) -> Option<&'static RuntimeSkillDescriptorV1> {
    RUNTIME_SKILLS.iter().find(|skill| skill.id == id)
}

pub fn is_am1_read_only_tool(tool_id: RegisteredToolIdV1) -> bool {
    AM1_READ_ONLY_TOOL_IDS.contains(&tool_id)
}

pub fn is_am2_tool(tool_id: RegisteredToolIdV1) -> bool {
    AM2_TOOL_IDS.contains(&tool_id)
}

pub fn route_skill_ids(text: &str) -> Vec<&'static str> {
    let mut selected = Vec::new();
    let routes: [(&str, &[&str]); 9] = [
        (
            "create-character",
            &["人物", "角色", "人设", "关系", "动机", "秘密", "人物弧"],
        ),
        (
            "create-setting",
            &["设定", "世界观", "地点", "时代", "规则", "社会", "氛围"],
        ),
        (
            "create-material",
            &["素材", "主题", "物件", "线索", "研究", "场景种子"],
        ),
        (
            "create-plot",
            &["剧情", "情节", "大纲", "转折", "揭示", "伏笔", "回收"],
        ),
        (
            "novel-story-telling",
            &["章节", "写章", "续写", "修订", "叙事", "连续性", "读者"],
        ),
        (
            "analytical-review",
            &["评审", "批评", "校对", "检查", "审稿", "回归"],
        ),
        (
            "create-visual-asset",
            &["封面", "插图", "配图", "视觉", "图像"],
        ),
        ("publish-novel", &["出版", "导出", "epub", "打包", "发布"]),
        (
            "create-novel-project",
            &["项目结构", "项目说明", "写作规范", "初始化"],
        ),
    ];
    let lower = text.to_lowercase();
    for (skill_id, keywords) in routes {
        if keywords.iter().any(|keyword| lower.contains(keyword)) {
            selected.push(skill_id);
        }
    }
    if selected.is_empty() {
        selected.push("novel-story-telling");
    }
    selected
}

pub fn path_allowed_for_skills(skill_ids: &[&str], relative_path: &str) -> bool {
    !skill_ids.is_empty()
        && skill_ids.iter().all(|id| SKILL_IDS.contains(id))
        && skill_ids.iter().any(|id| {
            runtime_skill(id).is_some_and(|skill| {
                skill
                    .read_scopes
                    .iter()
                    .any(|scope| scope_matches(scope, relative_path))
            })
        })
}

pub fn write_path_allowed_for_skills(skill_ids: &[&str], relative_path: &str) -> bool {
    !skill_ids.is_empty()
        && skill_ids.iter().all(|id| SKILL_IDS.contains(id))
        && skill_ids.iter().any(|id| {
            runtime_skill(id).is_some_and(|skill| {
                skill
                    .write_scopes
                    .iter()
                    .any(|scope| scope_matches(scope, relative_path))
            })
        })
}

fn scope_matches(scope: &str, relative_path: &str) -> bool {
    if scope == "**/*.md" {
        return relative_path.ends_with(".md");
    }
    if let Some(prefix) = scope.strip_suffix("/**") {
        return relative_path == prefix || relative_path.starts_with(&format!("{prefix}/"));
    }
    if let Some(prefix) = scope.strip_suffix("/*.md") {
        return relative_path == prefix
            || (relative_path.starts_with(&format!("{prefix}/"))
                && relative_path.ends_with(".md")
                && !relative_path[prefix.len() + 1..].contains('/'));
    }
    relative_path == scope
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn am1_registry_matches_the_locked_pack_and_only_exposes_read_tools() {
        assert_eq!(
            runtime_skill_registry()
                .iter()
                .map(|skill| skill.id)
                .collect::<Vec<_>>(),
            SKILL_IDS
        );
        assert!(AM1_READ_ONLY_TOOL_IDS.iter().all(|tool| !tool.is_write()));
        assert!(runtime_skill_registry()
            .iter()
            .all(|skill| !skill.instruction.is_empty()));
    }

    #[test]
    fn am2_exposes_the_two_frozen_write_tools_with_skill_bound_scopes() {
        assert_eq!(AM2_TOOL_IDS.len(), 6);
        assert_eq!(
            AM2_TOOL_IDS.iter().filter(|tool| tool.is_write()).count(),
            2
        );
        assert!(write_path_allowed_for_skills(
            &["create-character"],
            "characters/a.md"
        ));
        assert!(!write_path_allowed_for_skills(
            &["create-character"],
            "world/a.md"
        ));
        assert!(write_path_allowed_for_skills(
            &["novel-story-telling"],
            "materials/000.story-ledger.md"
        ));
        assert!(!write_path_allowed_for_skills(
            &["analytical-review"],
            "chapters/001.md"
        ));
    }

    #[test]
    fn routing_selects_multiple_skills_without_project_data_authority() {
        assert_eq!(
            route_skill_ids("检查人物动机与世界规则是否支持剧情转折"),
            [
                "create-character",
                "create-setting",
                "create-plot",
                "analytical-review"
            ]
        );
        assert_eq!(route_skill_ids("随便聊聊下一步"), ["novel-story-telling"]);
    }

    #[test]
    fn read_scopes_are_project_relative_and_skill_bound() {
        assert!(path_allowed_for_skills(
            &["create-character"],
            "characters/a.md"
        ));
        assert!(!path_allowed_for_skills(
            &["create-character"],
            "chapters/001.md"
        ));
        assert!(path_allowed_for_skills(
            &["analytical-review"],
            "chapters/001.md"
        ));
        assert!(!path_allowed_for_skills(
            &["analytical-review"],
            "assets/cover.png"
        ));
    }
}
