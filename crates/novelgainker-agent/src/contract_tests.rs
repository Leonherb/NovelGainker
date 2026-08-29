use crate::contract_validation::{contract_set_hash, validate_contract_set};
use crate::contracts::{
    AgentTurnV1, AuthorizationPolicyV1, ContentHashV1, ContractError, EpochMillisV1, ModelIdV1,
    ProjectAgentBasePolicyV1, ProjectAgentSessionV1, ProjectIdV1, ProjectReferenceKindV1,
    SessionStatusV1, SessionTerminationReasonV1, SkillIdV1, TurnBudgetV1, TurnFailureCodeV1,
    TurnIntentV1, TurnStateV1, WriterLeaseV1, AGENT_CONTRACT_SCHEMA_VERSION,
    MAX_AGENT_STEPS_PER_TURN,
};
use crate::session_machine::{
    begin_turn, is_transition_allowed, request_cancel, resolve_recovery, terminate_session,
    transition_turn,
};
use crate::{
    contract_set_source, contract_typescript_source, session_turn_schema_source,
    AGENT_CONTRACT_SET_SHA256, SKILL_IDS,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const STATES: [TurnStateV1; 11] = [
    TurnStateV1::Idle,
    TurnStateV1::Planning,
    TurnStateV1::Reading,
    TurnStateV1::Acting,
    TurnStateV1::AwaitingHighRiskConfirmation,
    TurnStateV1::Applying,
    TurnStateV1::Validating,
    TurnStateV1::Completed,
    TurnStateV1::Cancelled,
    TurnStateV1::Failed,
    TurnStateV1::RecoveryRequired,
];

fn at(offset_ms: u64) -> EpochMillisV1 {
    EpochMillisV1(1_787_904_000_000 + offset_ms)
}

fn valid_session() -> ProjectAgentSessionV1 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent/v1/fixtures/valid-session.json"
    )))
    .unwrap()
}

fn valid_turn() -> AgentTurnV1 {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent/v1/fixtures/valid-turn.json"
    )))
    .unwrap()
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

fn contract_resources() -> BTreeMap<String, Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/contracts/agent/v1");
    let mut resources = BTreeMap::new();
    collect(&root, &root, &mut resources);
    resources
}

#[test]
fn checked_in_contract_set_is_deterministic_and_runtime_disabled() {
    let resources = contract_resources();
    let first = validate_contract_set(&resources).unwrap();
    let second = validate_contract_set(&resources).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.resource_count, 5);
    assert_eq!(first.contract_set_hash, AGENT_CONTRACT_SET_SHA256);
    assert_eq!(first.contract_set_hash, contract_set_hash(&resources));

    let manifest: Value = serde_json::from_str(contract_set_source()).unwrap();
    assert_eq!(manifest["enabledAtRuntime"], false);
    assert_eq!(manifest["contractSetHash"], AGENT_CONTRACT_SET_SHA256);
    let schema: Value = serde_json::from_str(session_turn_schema_source()).unwrap();
    assert!(schema["$defs"]["ProjectAgentSessionV1"].is_object());
    assert!(contract_typescript_source().contains("allowsBypassChat: false"));

    let mut changed = resources;
    changed
        .get_mut("contract-types.ts")
        .unwrap()
        .extend_from_slice(b"\n");
    assert!(validate_contract_set(&changed).is_err());
}

#[test]
fn fixtures_are_strict_and_cross_project_fixture_is_rejected() {
    let session = valid_session();
    let turn = valid_turn();
    session.validate().unwrap();
    turn.validate().unwrap();

    let cross_project: AgentTurnV1 = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/contracts/agent/v1/fixtures/cross-project-turn.json"
    )))
    .unwrap();
    assert_eq!(
        cross_project.validate(),
        Err(ContractError::CrossProjectReference)
    );

    for kind in [
        ProjectReferenceKindV1::Summary,
        ProjectReferenceKindV1::ToolResult,
        ProjectReferenceKindV1::Transaction,
    ] {
        let mut wrong_reference = valid_turn();
        wrong_reference.references[0].kind = kind;
        wrong_reference.references[0].project_id = ProjectIdV1::parse("prj_00000002").unwrap();
        assert_eq!(
            wrong_reference.validate(),
            Err(ContractError::CrossProjectReference)
        );
    }

    let mut wrong_project_turn = valid_turn();
    wrong_project_turn.project_id = ProjectIdV1::parse("prj_00000002").unwrap();
    wrong_project_turn.references[0].project_id = wrong_project_turn.project_id.clone();
    let mut isolated_session = session.clone();
    assert_eq!(
        begin_turn(&mut isolated_session, &wrong_project_turn, at(2)),
        Err(ContractError::SessionMismatch)
    );

    let mut unknown: Value = serde_json::to_value(&turn).unwrap();
    unknown["providerRawResponse"] = json!("forbidden");
    assert!(serde_json::from_value::<AgentTurnV1>(unknown).is_err());

    let mut unsupported = turn;
    unsupported.schema_version = AGENT_CONTRACT_SCHEMA_VERSION + 1;
    assert_eq!(
        unsupported.validate(),
        Err(ContractError::InvalidSchemaVersion)
    );
    assert!(ProjectIdV1::parse("C:/manuscript").is_err());
    assert!(ContentHashV1::parse("not-a-hash").is_err());
}

#[test]
fn transition_matrix_properties_hold_for_every_state_pair() {
    use TurnStateV1::{
        Acting, Applying, AwaitingHighRiskConfirmation, Cancelled, Completed, Failed, Idle,
        Planning, Reading, RecoveryRequired, Validating,
    };
    let expected = [
        (Idle, Planning),
        (Idle, Cancelled),
        (Planning, Reading),
        (Planning, Acting),
        (Planning, Completed),
        (Planning, Cancelled),
        (Planning, Failed),
        (Reading, Acting),
        (Reading, Completed),
        (Reading, Cancelled),
        (Reading, Failed),
        (Acting, AwaitingHighRiskConfirmation),
        (Acting, Applying),
        (Acting, Completed),
        (Acting, Cancelled),
        (Acting, Failed),
        (AwaitingHighRiskConfirmation, Applying),
        (AwaitingHighRiskConfirmation, Cancelled),
        (AwaitingHighRiskConfirmation, Failed),
        (Applying, Validating),
        (Applying, RecoveryRequired),
        (Validating, Completed),
        (Validating, RecoveryRequired),
    ];
    for from in STATES {
        for to in STATES {
            assert_eq!(
                is_transition_allowed(from, to),
                expected.contains(&(from, to)),
                "transition property drifted for {from:?} -> {to:?}"
            );
        }
    }
    for terminal in [Completed, Cancelled, Failed, RecoveryRequired] {
        assert!(STATES
            .into_iter()
            .all(|next| !is_transition_allowed(terminal, next)));
    }
}

#[test]
fn lifecycle_holds_one_active_turn_and_one_writer() {
    let mut session = valid_session();
    let mut turn = valid_turn();
    begin_turn(&mut session, &turn, at(2)).unwrap();

    let mut parallel = valid_turn();
    parallel.turn_id = crate::contracts::TurnIdV1::parse("trn_00000002").unwrap();
    assert_eq!(
        begin_turn(&mut session, &parallel, at(3)),
        Err(ContractError::TurnAlreadyActive)
    );

    for (state, timestamp) in [
        (TurnStateV1::Planning, 3),
        (TurnStateV1::Reading, 4),
        (TurnStateV1::Acting, 5),
        (TurnStateV1::Applying, 6),
    ] {
        transition_turn(&mut session, &mut turn, state, at(timestamp)).unwrap();
    }
    assert!(matches!(
        session.writer_lease,
        WriterLeaseV1::Held { ref turn_id } if turn_id == &turn.turn_id
    ));
    transition_turn(&mut session, &mut turn, TurnStateV1::Validating, at(7)).unwrap();
    transition_turn(&mut session, &mut turn, TurnStateV1::Completed, at(8)).unwrap();
    assert!(session.active_turn_id.is_none());
    assert!(matches!(session.writer_lease, WriterLeaseV1::Released));
    assert_eq!(
        transition_turn(&mut session, &mut turn, TurnStateV1::Planning, at(9)),
        Err(ContractError::TurnNotActive)
    );
}

#[test]
fn invalid_jumps_backtracks_and_terminal_reuse_are_rejected_without_mutation() {
    let mut session = valid_session();
    let mut turn = valid_turn();
    begin_turn(&mut session, &turn, at(2)).unwrap();
    let original = turn.clone();
    assert_eq!(
        transition_turn(&mut session, &mut turn, TurnStateV1::Applying, at(3)),
        Err(ContractError::InvalidTransition)
    );
    assert_eq!(turn, original);

    transition_turn(&mut session, &mut turn, TurnStateV1::Planning, at(4)).unwrap();
    assert_eq!(
        transition_turn(&mut session, &mut turn, TurnStateV1::Idle, at(5)),
        Err(ContractError::InvalidTransition)
    );
    transition_turn(&mut session, &mut turn, TurnStateV1::Completed, at(6)).unwrap();
    session.active_turn_id = Some(turn.turn_id.clone());
    assert_eq!(
        transition_turn(&mut session, &mut turn, TurnStateV1::Planning, at(7)),
        Err(ContractError::TerminalStateReuse)
    );
}

#[test]
fn frozen_snapshot_drift_fails_before_write_and_requires_recovery_after_write() {
    let mut provider_session = valid_session();
    let mut provider_turn = valid_turn();
    begin_turn(&mut provider_session, &provider_turn, at(2)).unwrap();
    transition_turn(
        &mut provider_session,
        &mut provider_turn,
        TurnStateV1::Planning,
        at(3),
    )
    .unwrap();
    provider_session.provider.model_id = ModelIdV1::parse("deepseek-reconfigured").unwrap();
    assert_eq!(
        transition_turn(
            &mut provider_session,
            &mut provider_turn,
            TurnStateV1::Reading,
            at(4)
        ),
        Err(ContractError::SnapshotDrift)
    );
    assert_eq!(
        provider_turn.failure.as_ref().map(|failure| failure.code),
        Some(TurnFailureCodeV1::ProviderSnapshotDrift)
    );

    let mut prewrite_session = valid_session();
    let mut prewrite_turn = valid_turn();
    begin_turn(&mut prewrite_session, &prewrite_turn, at(2)).unwrap();
    transition_turn(
        &mut prewrite_session,
        &mut prewrite_turn,
        TurnStateV1::Planning,
        at(3),
    )
    .unwrap();
    prewrite_session.authorization_policy = AuthorizationPolicyV1::PlanOnly;
    assert_eq!(
        transition_turn(
            &mut prewrite_session,
            &mut prewrite_turn,
            TurnStateV1::Reading,
            at(4)
        ),
        Err(ContractError::SnapshotDrift)
    );
    assert_eq!(prewrite_turn.state, TurnStateV1::Failed);
    assert_eq!(
        prewrite_turn.failure.as_ref().map(|failure| failure.code),
        Some(TurnFailureCodeV1::AuthorizationPolicySnapshotDrift)
    );
    assert!(prewrite_session.active_turn_id.is_none());

    let mut postwrite_session = valid_session();
    let mut postwrite_turn = valid_turn();
    begin_turn(&mut postwrite_session, &postwrite_turn, at(2)).unwrap();
    for (state, timestamp) in [
        (TurnStateV1::Planning, 3),
        (TurnStateV1::Acting, 4),
        (TurnStateV1::Applying, 5),
    ] {
        transition_turn(
            &mut postwrite_session,
            &mut postwrite_turn,
            state,
            at(timestamp),
        )
        .unwrap();
    }
    postwrite_session.skill_pack.hash = ContentHashV1::digest(b"drifted-pack");
    assert_eq!(
        transition_turn(
            &mut postwrite_session,
            &mut postwrite_turn,
            TurnStateV1::Validating,
            at(6)
        ),
        Err(ContractError::SnapshotDrift)
    );
    assert_eq!(postwrite_turn.state, TurnStateV1::RecoveryRequired);
    assert_eq!(
        postwrite_turn.failure.as_ref().map(|failure| failure.code),
        Some(TurnFailureCodeV1::SkillPackSnapshotDrift)
    );
    assert_eq!(postwrite_session.status, SessionStatusV1::RecoveryRequired);
    assert_eq!(
        begin_turn(&mut postwrite_session, &valid_turn(), at(7)),
        Err(ContractError::SessionNotActive)
    );
    resolve_recovery(&mut postwrite_session, &postwrite_turn, at(8)).unwrap();
    assert_eq!(postwrite_session.status, SessionStatusV1::Active);
}

#[test]
fn cancellation_is_deterministic_before_and_after_writer_acquisition() {
    let mut session = valid_session();
    let mut turn = valid_turn();
    begin_turn(&mut session, &turn, at(2)).unwrap();
    transition_turn(&mut session, &mut turn, TurnStateV1::Planning, at(3)).unwrap();
    request_cancel(&mut session, &mut turn, at(4)).unwrap();
    assert_eq!(turn.state, TurnStateV1::Cancelled);
    assert!(turn.cancellation_requested);
    assert!(session.active_turn_id.is_none());

    let mut write_session = valid_session();
    let mut write_turn = valid_turn();
    begin_turn(&mut write_session, &write_turn, at(2)).unwrap();
    for (state, timestamp) in [
        (TurnStateV1::Planning, 3),
        (TurnStateV1::Acting, 4),
        (TurnStateV1::Applying, 5),
    ] {
        transition_turn(&mut write_session, &mut write_turn, state, at(timestamp)).unwrap();
    }
    request_cancel(&mut write_session, &mut write_turn, at(6)).unwrap();
    assert_eq!(write_turn.state, TurnStateV1::RecoveryRequired);
    assert!(matches!(
        write_session.writer_lease,
        WriterLeaseV1::RecoveryRequired { .. }
    ));
}

#[test]
fn base_policy_is_not_a_skill_and_zero_skill_has_no_project_work_bypass() {
    let policy = ProjectAgentBasePolicyV1::locked();
    policy.validate().unwrap();
    assert!(!SKILL_IDS.contains(&policy.policy_id.as_str()));
    policy
        .validate_skill_selection(TurnIntentV1::Discussion, &[])
        .unwrap();
    assert_eq!(
        policy.validate_skill_selection(TurnIntentV1::ProjectWork, &[]),
        Err(ContractError::ZeroSkillNotAllowed)
    );

    let selected = [
        SkillIdV1::parse("create-setting").unwrap(),
        SkillIdV1::parse("create-plot").unwrap(),
    ];
    policy
        .validate_skill_selection(TurnIntentV1::ProjectWork, &selected)
        .unwrap();
    let duplicate = [selected[0].clone(), selected[0].clone()];
    assert_eq!(
        policy.validate_skill_selection(TurnIntentV1::ProjectWork, &duplicate),
        Err(ContractError::DuplicateSkill)
    );
    let unknown = [SkillIdV1::parse("ordinary-chat").unwrap()];
    assert_eq!(
        policy.validate_skill_selection(TurnIntentV1::ProjectWork, &unknown),
        Err(ContractError::UnknownSkill)
    );
}

#[test]
fn budgets_are_bounded_and_session_termination_never_interrupts_a_turn() {
    TurnBudgetV1::default().validate().unwrap();
    let excessive = TurnBudgetV1 {
        max_agent_steps: MAX_AGENT_STEPS_PER_TURN + 1,
        ..TurnBudgetV1::default()
    };
    assert_eq!(excessive.validate(), Err(ContractError::InvalidBudget));

    let mut session = valid_session();
    let turn = valid_turn();
    begin_turn(&mut session, &turn, at(2)).unwrap();
    assert_eq!(
        terminate_session(&mut session, SessionTerminationReasonV1::UserEnded, at(3)),
        Err(ContractError::TurnAlreadyActive)
    );

    let mut idle_session = valid_session();
    terminate_session(
        &mut idle_session,
        SessionTerminationReasonV1::ProjectClosed,
        at(3),
    )
    .unwrap();
    assert_eq!(idle_session.status, SessionStatusV1::Terminated);
}
