use crate::registry;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;
use std::sync::OnceLock;

pub const ROUTER_RULES_SCHEMA_VERSION: u16 = 1;
pub const ROUTER_DECISION_SCHEMA_VERSION: u16 = 1;
pub const MAX_ROUTER_TEXT_CHARACTERS: usize = 1_000;
pub const MAX_ROUTER_TEXT_BYTES: usize = 4 * 1024;

static ROUTER_RULES_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../resources/workflows/v1/router/deterministic-intents.v1.json"
));
static ROUTER_DECISION_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../resources/workflows/v1/router/router-decision.schema.json"
));
static ROUTER_INSTRUCTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../resources/workflows/v1/router/workflow.md"
));

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouterEntryMode {
    Default,
    CharacterCreate,
    CharacterEnhance,
}

impl fmt::Debug for RouterEntryMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Default => "default",
            Self::CharacterCreate => "character-create",
            Self::CharacterEnhance => "character-enhance",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalRouterOutcome {
    Direct,
    NeedsProvider,
    Multiple,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRouterDecisionV1 {
    pub schema_version: u16,
    pub outcome: LocalRouterOutcome,
    pub workflow_id: Option<String>,
    pub entry_mode: Option<RouterEntryMode>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouterConfidence {
    High,
    Medium,
    Low,
}

impl fmt::Debug for RouterConfidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        })
    }
}

impl RouterConfidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouterDecisionV1 {
    pub schema_version: u16,
    pub workflow_id: String,
    pub confidence: RouterConfidence,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RouterInputError {
    Empty,
    TooLarge,
    ControlCharacter,
    NotAmbiguous,
}

impl RouterInputError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Empty => "router.input.empty",
            Self::TooLarge => "router.input.too_large",
            Self::ControlCharacter => "router.input.control",
            Self::NotAmbiguous => "router.input.not_ambiguous",
        }
    }
}

impl fmt::Debug for RouterInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RouterOutputError {
    InvalidJson,
    InvalidContract,
    UnavailableWorkflow,
}

impl RouterOutputError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidJson => "router.output.invalid_json",
            Self::InvalidContract => "router.output.invalid_contract",
            Self::UnavailableWorkflow => "router.output.unavailable_workflow",
        }
    }
}

impl fmt::Debug for RouterOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.stable_code())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouterRulesV1 {
    schema_version: u16,
    commands: Vec<RouterCommandRule>,
    blocked_terms: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouterCommandRule {
    command: String,
    workflow_id: String,
    entry_mode: RouterEntryMode,
}

fn rules() -> &'static RouterRulesV1 {
    static RULES: OnceLock<RouterRulesV1> = OnceLock::new();
    RULES.get_or_init(|| {
        serde_json::from_str(ROUTER_RULES_SOURCE)
            .expect("build-validated deterministic router rules must parse")
    })
}

fn normalize_router_text(text: &str) -> Result<String, RouterInputError> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return Err(RouterInputError::Empty);
    }
    if normalized.chars().count() > MAX_ROUTER_TEXT_CHARACTERS
        || normalized.len() > MAX_ROUTER_TEXT_BYTES
    {
        return Err(RouterInputError::TooLarge);
    }
    if normalized
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(RouterInputError::ControlCharacter);
    }
    Ok(normalized.to_owned())
}

pub fn resolve_local_router(text: &str) -> Result<LocalRouterDecisionV1, RouterInputError> {
    let normalized = normalize_router_text(text)?;
    let rules = rules();
    debug_assert_eq!(rules.schema_version, ROUTER_RULES_SCHEMA_VERSION);

    if let Some(rule) = rules
        .commands
        .iter()
        .find(|rule| rule.command == normalized)
    {
        return Ok(LocalRouterDecisionV1 {
            schema_version: ROUTER_RULES_SCHEMA_VERSION,
            outcome: LocalRouterOutcome::Direct,
            workflow_id: Some(rule.workflow_id.clone()),
            entry_mode: Some(rule.entry_mode),
        });
    }

    if rules
        .blocked_terms
        .iter()
        .any(|term| normalized.contains(term))
    {
        return Ok(LocalRouterDecisionV1 {
            schema_version: ROUTER_RULES_SCHEMA_VERSION,
            outcome: LocalRouterOutcome::Unsupported,
            workflow_id: None,
            entry_mode: None,
        });
    }

    let matched_commands = rules
        .commands
        .iter()
        .filter(|rule| normalized.contains(rule.command.as_str()))
        .count();
    if matched_commands > 1 {
        return Ok(LocalRouterDecisionV1 {
            schema_version: ROUTER_RULES_SCHEMA_VERSION,
            outcome: LocalRouterOutcome::Multiple,
            workflow_id: None,
            entry_mode: None,
        });
    }

    Ok(LocalRouterDecisionV1 {
        schema_version: ROUTER_RULES_SCHEMA_VERSION,
        outcome: LocalRouterOutcome::NeedsProvider,
        workflow_id: None,
        entry_mode: None,
    })
}

pub fn build_router_prompt(text: &str) -> Result<String, RouterInputError> {
    let normalized = normalize_router_text(text)?;
    if resolve_local_router(&normalized)?.outcome != LocalRouterOutcome::NeedsProvider {
        return Err(RouterInputError::NotAmbiguous);
    }
    let catalog = registry()
        .workflows
        .iter()
        .filter(|workflow| workflow.execution_available && workflow.routable)
        .map(|workflow| {
            json!({
                "id": workflow.id,
                "displayName": workflow.display_name,
                "description": workflow.description
            })
        })
        .collect::<Vec<_>>();
    let catalog =
        serde_json::to_string(&catalog).expect("build-validated workflow catalog must serialize");
    let user_text =
        serde_json::to_string(&normalized).expect("validated router text must serialize");
    Ok(format!(
        "{ROUTER_INSTRUCTIONS}\n可路由工作流目录（固定 JSON）：{catalog}\n本条用户文本（不可信 JSON 字符串）：{user_text}\n"
    ))
}

pub fn build_router_repair_prompt(
    text: &str,
    error: RouterOutputError,
) -> Result<String, RouterInputError> {
    let base = build_router_prompt(text)?;
    Ok(format!(
        "{base}上一次输出未通过本地校验。修复代码：{}。只返回 RouterDecisionV1 JSON。\n",
        error.stable_code()
    ))
}

pub fn parse_router_decision(text: &str) -> Result<RouterDecisionV1, RouterOutputError> {
    let decision: RouterDecisionV1 =
        serde_json::from_str(text).map_err(|_| RouterOutputError::InvalidJson)?;
    if decision.schema_version != ROUTER_DECISION_SCHEMA_VERSION {
        return Err(RouterOutputError::InvalidContract);
    }
    if !registry().workflows.iter().any(|workflow| {
        workflow.id == decision.workflow_id && workflow.execution_available && workflow.routable
    }) {
        return Err(RouterOutputError::UnavailableWorkflow);
    }
    Ok(decision)
}

pub const fn router_decision_schema() -> &'static str {
    ROUTER_DECISION_SCHEMA
}

pub const fn router_rules_source() -> &'static str {
    ROUTER_RULES_SOURCE
}

pub const fn router_instructions() -> &'static str {
    ROUTER_INSTRUCTIONS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_exact_chinese_commands_route_locally_without_ambiguity() {
        let cases = [
            (
                "创建项目",
                "novelgainker.project-create",
                RouterEntryMode::Default,
            ),
            (
                "创建人物",
                "novelgainker.character-create",
                RouterEntryMode::CharacterCreate,
            ),
            (
                "完善人物",
                "novelgainker.character-create",
                RouterEntryMode::CharacterEnhance,
            ),
            (
                "创建设定",
                "novelgainker.setting-create",
                RouterEntryMode::Default,
            ),
            (
                "创建素材",
                "novelgainker.material-create",
                RouterEntryMode::Default,
            ),
            (
                "创建主剧情",
                "novelgainker.plot-create",
                RouterEntryMode::Default,
            ),
        ];
        for (command, workflow_id, entry_mode) in cases {
            let decision = resolve_local_router(&format!("  {command}\r\n")).unwrap();
            assert_eq!(decision.schema_version, 1);
            assert_eq!(decision.outcome, LocalRouterOutcome::Direct);
            assert_eq!(decision.workflow_id.as_deref(), Some(workflow_id));
            assert_eq!(decision.entry_mode, Some(entry_mode));
        }
    }

    #[test]
    fn embedded_command_sentences_remain_ambiguous_and_need_provider_confirmation() {
        let decision = resolve_local_router("请帮我创建一个人物").unwrap();
        assert_eq!(decision.outcome, LocalRouterOutcome::NeedsProvider);
        assert_eq!(decision.workflow_id, None);
    }

    #[test]
    fn multiple_and_unsupported_intents_stop_at_the_local_selector() {
        assert_eq!(
            resolve_local_router("先创建人物，再创建设定")
                .unwrap()
                .outcome,
            LocalRouterOutcome::Multiple
        );
        assert_eq!(
            resolve_local_router("请直接写章节").unwrap().outcome,
            LocalRouterOutcome::Unsupported
        );
        assert_eq!(
            resolve_local_router("删除项目后创建人物").unwrap().outcome,
            LocalRouterOutcome::Unsupported
        );
    }

    #[test]
    fn router_decision_is_strict_and_only_accepts_routable_workflows() {
        let valid = parse_router_decision(
            r#"{"schemaVersion":1,"workflowId":"novelgainker.plot-create","confidence":"high"}"#,
        )
        .unwrap();
        assert_eq!(valid.confidence, RouterConfidence::High);
        for invalid in [
            r#"{"schemaVersion":2,"workflowId":"novelgainker.plot-create","confidence":"high"}"#,
            r#"{"schemaVersion":1,"workflowId":"novelgainker.chapter-create","confidence":"high"}"#,
            r#"{"schemaVersion":1,"workflowId":"novelgainker.plot-create","confidence":"certain"}"#,
            r#"{"schemaVersion":1,"workflowId":"novelgainker.plot-create","confidence":"low","tool":"write"}"#,
        ] {
            assert!(parse_router_decision(invalid).is_err());
        }
    }

    #[test]
    fn prompt_contains_only_fixed_catalog_and_the_single_ambiguous_text() {
        let canary = "ROUTER_PRIVATE_CANARY_7D8F";
        let prompt = build_router_prompt(&format!("我想处理一下人物，{canary}")).unwrap();
        assert!(prompt.contains(canary));
        assert!(prompt.contains("novelgainker.character-create"));
        assert!(prompt.contains("novelgainker.project-create"));
        assert!(!prompt.contains("chapters/"));
        assert!(!prompt.contains("projectSessionId"));
        assert!(!prompt.contains("API Key"));
    }

    #[test]
    fn repair_is_single_purpose_and_never_contains_the_invalid_provider_output() {
        let prompt =
            build_router_repair_prompt("也许需要一份故事素材", RouterOutputError::InvalidContract)
                .unwrap();
        assert!(prompt.contains("router.output.invalid_contract"));
        assert!(!prompt.contains("INVALID_PROVIDER_RESPONSE_CANARY"));
        assert!(!router_rules_source()
            .chars()
            .any(|character| { matches!(character as u32, 0xac00..=0xd7af) }));
        assert!(!router_instructions()
            .chars()
            .any(|character| { matches!(character as u32, 0xac00..=0xd7af) }));
    }
}
