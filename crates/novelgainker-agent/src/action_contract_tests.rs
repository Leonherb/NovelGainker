use crate::action_contract_validation::{action_contract_set_hash, validate_action_contract_set};
use crate::action_contracts::{
    bind_tool_requests, AgentActionErrorCodeV1, AgentActionIdV1, AgentActionPayloadV1,
    AgentActionRecordContextV1, AgentActionRecordV1, AppPolicyDispositionV1, ModelToolRequestV1,
    ProjectRelativePathV1, ProjectToolBindingV1, RedactedAgentErrorLogV1, RegisteredToolIdV1,
    ToolArgumentsV1, ToolCallIdV1, ToolResultDataV1, ToolResultStatusV1, ToolResultV1,
};
use crate::contracts::{
    AgentTurnV1, ContentHashV1, EpochMillisV1, ProjectIdV1, SessionIdV1, SkillIdV1, TurnBudgetV1,
    TurnIdV1, MAX_AGENT_STEPS_PER_TURN, MAX_MODIFIED_PROJECT_FILES_PER_TURN,
    MAX_PROVIDER_REQUEST_BYTES, MAX_TOOL_CALLS_PER_BATCH, MAX_TOOL_CALLS_PER_TURN,
    MAX_UNIQUE_PROJECT_FILES_PER_TURN,
};
use crate::provider_adapter::{
    normalize_provider_output, normalize_with_single_repair, ProviderKindV1, ProviderOutputModeV1,
};
use crate::turn_budget::TurnBudgetTrackerV1;
use crate::{
    action_contract_set_source, action_tool_schema_source, action_typescript_source,
    AGENT_ACTION_CONTRACT_SET_SHA256,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/contracts/agent-action/v1/fixtures")
            .join(name),
    )
    .unwrap()
}

fn valid_turn() -> AgentTurnV1 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent/v1/fixtures/valid-turn.json"
    )))
    .unwrap()
}

fn binding() -> ProjectToolBindingV1 {
    ProjectToolBindingV1 {
        project_id: ProjectIdV1::parse("prj_00000001").unwrap(),
        session_id: SessionIdV1::parse("ses_00000001").unwrap(),
        turn_id: TurnIdV1::parse("trn_00000001").unwrap(),
    }
}

fn read_request(index: usize, path: &str) -> ModelToolRequestV1 {
    ModelToolRequestV1 {
        call_id: ToolCallIdV1::parse(format!("call_read{index:04}")).unwrap(),
        tool_id: RegisteredToolIdV1::ProjectReadMarkdown,
        arguments: ToolArgumentsV1 {
            path: Some(ProjectRelativePathV1::parse(path).unwrap()),
            directory: None,
            query: None,
            content: None,
            patch: None,
            expected_source_hash: None,
        },
        purpose: format!("读取第 {index} 个项目文件"),
    }
}

fn tool_payload(requests: Vec<ModelToolRequestV1>) -> AgentActionPayloadV1 {
    AgentActionPayloadV1::RequestTools {
        schema_version: 1,
        skill_ids: vec![SkillIdV1::parse("create-setting").unwrap()],
        tool_requests: requests,
    }
}

fn collect(root: &Path, directory: &Path, resources: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        assert!(!file_type.is_symlink());
        let path = entry.path();
        if file_type.is_dir() {
            collect(root, &path, resources);
        } else {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|component| component.as_os_str().to_str().unwrap())
                .collect::<Vec<_>>()
                .join("/");
            resources.insert(relative, fs::read(path).unwrap());
        }
    }
}

#[test]
fn checked_in_action_contract_set_is_deterministic_and_runtime_disabled() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/contracts/agent-action/v1");
    let mut resources = BTreeMap::new();
    collect(&root, &root, &mut resources);
    let first = validate_action_contract_set(&resources).unwrap();
    let second = validate_action_contract_set(&resources).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.resource_count, 10);
    assert_eq!(first.contract_set_hash, AGENT_ACTION_CONTRACT_SET_SHA256);
    assert_eq!(
        first.contract_set_hash,
        action_contract_set_hash(&resources)
    );

    let manifest: Value = serde_json::from_str(action_contract_set_source()).unwrap();
    assert_eq!(manifest["enabledAtRuntime"], false);
    assert_eq!(
        manifest["contractSetHash"],
        AGENT_ACTION_CONTRACT_SET_SHA256
    );
    let schema: Value = serde_json::from_str(action_tool_schema_source()).unwrap();
    assert!(schema["$defs"]["AgentActionRecordV1"].is_object());
    assert!(action_typescript_source().contains("untrusted_tool_data"));
}

#[test]
fn openai_deepseek_and_strict_json_normalize_to_one_payload() {
    let openai = normalize_provider_output(
        ProviderKindV1::OpenAi,
        ProviderOutputModeV1::NativeToolCalling,
        &fixture("openai-native-tool-call.json"),
    )
    .unwrap();
    let deepseek = normalize_provider_output(
        ProviderKindV1::DeepSeek,
        ProviderOutputModeV1::NativeToolCalling,
        &fixture("deepseek-native-tool-call.json"),
    )
    .unwrap();
    let strict = normalize_provider_output(
        ProviderKindV1::OpenAi,
        ProviderOutputModeV1::StrictJsonFallback,
        &fixture("strict-json-action.json"),
    )
    .unwrap();
    assert_eq!(openai.payload, deepseek.payload);
    assert_eq!(openai.payload, strict.payload);

    let requests = bind_tool_requests(&openai.payload, &binding()).unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].serial_ordinal, 0);
    assert_eq!(requests[0].binding, binding());
}

#[test]
fn unknown_duplicate_authority_path_and_tool_inputs_are_rejected() {
    assert_eq!(
        normalize_provider_output(
            ProviderKindV1::OpenAi,
            ProviderOutputModeV1::StrictJsonFallback,
            &fixture("invalid-unknown-field.json"),
        ),
        Err(AgentActionErrorCodeV1::InvalidPayload)
    );
    assert_eq!(
        normalize_provider_output(
            ProviderKindV1::OpenAi,
            ProviderOutputModeV1::StrictJsonFallback,
            &fixture("invalid-duplicate-call-id.json"),
        ),
        Err(AgentActionErrorCodeV1::DuplicateCallId)
    );
    assert_eq!(
        normalize_provider_output(
            ProviderKindV1::DeepSeek,
            ProviderOutputModeV1::NativeToolCalling,
            &fixture("invalid-absolute-path.json"),
        ),
        Err(AgentActionErrorCodeV1::AbsolutePathForbidden)
    );
    assert_eq!(
        normalize_provider_output(
            ProviderKindV1::OpenAi,
            ProviderOutputModeV1::StrictJsonFallback,
            &fixture("invalid-provider-authority.json"),
        ),
        Err(AgentActionErrorCodeV1::ForbiddenAuthorityField)
    );
    assert_eq!(
        normalize_provider_output(
            ProviderKindV1::DeepSeek,
            ProviderOutputModeV1::NativeToolCalling,
            &fixture("invalid-unknown-tool.json"),
        ),
        Err(AgentActionErrorCodeV1::UnknownTool)
    );

    let unsupported = serde_json::to_vec(&json!({
        "action": "respond",
        "schemaVersion": 2,
        "skillIds": [],
        "visibleText": "unsupported"
    }))
    .unwrap();
    assert_eq!(
        normalize_provider_output(
            ProviderKindV1::OpenAi,
            ProviderOutputModeV1::StrictJsonFallback,
            &unsupported,
        ),
        Err(AgentActionErrorCodeV1::UnsupportedVersion)
    );
}

#[test]
fn repair_is_at_most_once_and_never_crosses_provider() {
    let repaired = normalize_with_single_repair(
        ProviderKindV1::OpenAi,
        ProviderOutputModeV1::StrictJsonFallback,
        b"not-json",
        Some((
            ProviderKindV1::OpenAi,
            ProviderOutputModeV1::StrictJsonFallback,
            &fixture("strict-json-action.json"),
        )),
    )
    .unwrap();
    assert!(repaired.repaired);
    assert_eq!(repaired.attempt_count, 2);

    assert_eq!(
        normalize_with_single_repair(
            ProviderKindV1::OpenAi,
            ProviderOutputModeV1::StrictJsonFallback,
            b"not-json",
            Some((
                ProviderKindV1::DeepSeek,
                ProviderOutputModeV1::StrictJsonFallback,
                &fixture("strict-json-action.json"),
            )),
        ),
        Err(AgentActionErrorCodeV1::CrossProviderFallbackForbidden)
    );
    assert_eq!(
        normalize_with_single_repair(
            ProviderKindV1::OpenAi,
            ProviderOutputModeV1::StrictJsonFallback,
            b"not-json",
            Some((
                ProviderKindV1::OpenAi,
                ProviderOutputModeV1::StrictJsonFallback,
                b"still-not-json",
            )),
        ),
        Err(AgentActionErrorCodeV1::RepairExhausted)
    );
}

#[test]
fn app_binding_serial_order_action_chain_and_tool_results_are_stable() {
    let payload = tool_payload(vec![
        read_request(9, "world/z-last.md"),
        read_request(1, "world/a-first.md"),
    ]);
    let requests = bind_tool_requests(&payload, &binding()).unwrap();
    assert_eq!(requests[0].call_id.as_str(), "call_read0009");
    assert_eq!(requests[1].call_id.as_str(), "call_read0001");
    assert_eq!(requests[0].serial_ordinal, 0);
    assert_eq!(requests[1].serial_ordinal, 1);

    let turn = valid_turn();
    let first_context = AgentActionRecordContextV1 {
        action_id: AgentActionIdV1::parse("act_00000001").unwrap(),
        project_id: turn.project_id.clone(),
        session_id: turn.session_id.clone(),
        turn_id: turn.turn_id.clone(),
        sequence: 1,
        provider: turn.frozen.provider.clone(),
        skill_pack: turn.frozen.skill_pack.clone(),
        effective_skill_ids: turn.selected_skill_ids.clone(),
        policy_disposition: AppPolicyDispositionV1::Allowed,
        created_at_ms: EpochMillisV1(1_787_904_000_010),
    };
    let first = AgentActionRecordV1::wrap(first_context, payload, None).unwrap();
    first.validate().unwrap();

    let second_context = AgentActionRecordContextV1 {
        action_id: AgentActionIdV1::parse("act_00000002").unwrap(),
        sequence: 2,
        created_at_ms: EpochMillisV1(1_787_904_000_020),
        ..first.context.clone()
    };
    let complete = AgentActionPayloadV1::Complete {
        schema_version: 1,
        skill_ids: first.context.effective_skill_ids.clone(),
        visible_summary: "读取完成，未执行写入。".to_owned(),
    };
    let second =
        AgentActionRecordV1::wrap(second_context, complete, Some(first.action_hash.clone()))
            .unwrap();
    second.validate().unwrap();

    let mut forged_skill = second.clone();
    forged_skill.context.effective_skill_ids = vec![SkillIdV1::parse("forged.skill").unwrap()];
    assert_eq!(
        forged_skill.validate(),
        Err(AgentActionErrorCodeV1::UnknownSkill)
    );

    let mut out_of_range_sequence = second.clone();
    out_of_range_sequence.context.sequence = MAX_AGENT_STEPS_PER_TURN + 1;
    out_of_range_sequence.action_hash = ContentHashV1::digest(
        &serde_json::to_vec(&(
            1_u16,
            &out_of_range_sequence.context,
            &out_of_range_sequence.payload_hash,
            &out_of_range_sequence.previous_action_hash,
            out_of_range_sequence.redaction_state,
        ))
        .unwrap(),
    );
    assert_eq!(
        out_of_range_sequence.validate(),
        Err(AgentActionErrorCodeV1::ActionChainInvalid)
    );

    let data = ToolResultDataV1 {
        item_count: 1,
        content_hash: ContentHashV1::digest(b"bounded structured result"),
        affected_paths: vec![ProjectRelativePathV1::parse("world/z-last.md").unwrap()],
    };
    let result = ToolResultV1::build(
        &requests[0],
        ToolResultStatusV1::Succeeded,
        None,
        Some(data),
        false,
        None,
    )
    .unwrap();
    result.validate_against(&requests[0]).unwrap();
    assert_eq!(
        result.validate_against(&requests[1]),
        Err(AgentActionErrorCodeV1::ToolResultMismatch)
    );
}

#[test]
fn every_turn_budget_has_a_deterministic_boundary() {
    let limits = TurnBudgetV1::default();
    let mut steps = TurnBudgetTrackerV1::new(limits);
    for _ in 0..MAX_AGENT_STEPS_PER_TURN {
        steps.record_agent_step().unwrap();
    }
    assert_eq!(
        steps.record_agent_step(),
        Err(AgentActionErrorCodeV1::AgentStepBudgetExceeded)
    );

    let mut provider = TurnBudgetTrackerV1::new(limits);
    provider
        .record_provider_request(MAX_PROVIDER_REQUEST_BYTES)
        .unwrap();
    assert_eq!(
        provider.record_provider_request(MAX_PROVIDER_REQUEST_BYTES + 1),
        Err(AgentActionErrorCodeV1::ProviderRequestBudgetExceeded)
    );

    let batch = bind_tool_requests(
        &tool_payload(
            (0..MAX_TOOL_CALLS_PER_BATCH as usize)
                .map(|index| read_request(index, &format!("world/file-{index}.md")))
                .collect(),
        ),
        &binding(),
    )
    .unwrap();
    let mut tools = TurnBudgetTrackerV1::new(limits);
    tools.record_tool_batch(&batch).unwrap();
    tools.record_tool_batch(&batch).unwrap();
    assert_eq!(tools.usage().tool_calls, MAX_TOOL_CALLS_PER_TURN);
    assert_eq!(
        tools.record_tool_batch(&batch),
        Err(AgentActionErrorCodeV1::ToolCallBudgetExceeded)
    );

    let mut files = TurnBudgetTrackerV1::new(limits);
    for index in 0..MAX_UNIQUE_PROJECT_FILES_PER_TURN {
        files
            .record_project_file(
                ProjectRelativePathV1::parse(format!("world/file-{index}.md")).unwrap(),
                false,
            )
            .unwrap();
    }
    assert_eq!(
        files.record_project_file(
            ProjectRelativePathV1::parse("world/overflow.md").unwrap(),
            false,
        ),
        Err(AgentActionErrorCodeV1::ProjectFileBudgetExceeded)
    );

    let mut modified = TurnBudgetTrackerV1::new(limits);
    for index in 0..MAX_MODIFIED_PROJECT_FILES_PER_TURN {
        modified
            .record_project_file(
                ProjectRelativePathV1::parse(format!("materials/file-{index}.md")).unwrap(),
                true,
            )
            .unwrap();
    }
    assert_eq!(
        modified.record_project_file(
            ProjectRelativePathV1::parse("materials/overflow.md").unwrap(),
            true,
        ),
        Err(AgentActionErrorCodeV1::ModifiedFileBudgetExceeded)
    );

    let mut writes = TurnBudgetTrackerV1::new(limits);
    writes.begin_write_transaction().unwrap();
    assert_eq!(
        writes.begin_write_transaction(),
        Err(AgentActionErrorCodeV1::WriteTransactionBudgetExceeded)
    );
}

#[test]
fn error_logs_are_hash_only_and_never_echo_provider_or_project_secrets() {
    let secret =
        br#"{"apiKey":"sk-secret","path":"C:/private/manuscript.md","body":"full chapter"}"#;
    let log =
        RedactedAgentErrorLogV1::from_source(AgentActionErrorCodeV1::InvalidPayload, secret, 1);
    let serialized = serde_json::to_string(&log).unwrap();
    for forbidden in ["sk-secret", "C:/private", "full chapter", "apiKey"] {
        assert!(!serialized.contains(forbidden));
    }
    assert!(serialized.contains("sourceHash"));
    assert!(serialized.contains("invalid-payload"));
}
