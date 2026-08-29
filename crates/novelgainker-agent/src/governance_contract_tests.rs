use crate::action_contracts::{
    ProjectRelativePathV1, ProjectToolBindingV1, RegisteredToolIdV1, ToolArgumentsV1, ToolCallIdV1,
    ToolRequestV1, AGENT_ACTION_SCHEMA_VERSION,
};
use crate::authorization::{
    authorize, authorize_turn, registered_tool_risk, AuthorizationEffectV1, ConfirmationBindingV1,
    ConfirmationIdV1, GovernanceErrorCodeV1, GovernedActionV1, HighRiskConfirmationV1, RiskLevelV1,
    ValidatedMutationShapeV1, WriteToolClassificationV1,
};
use crate::contracts::{
    AuthorizationPolicyV1, ContentHashV1, EpochMillisV1, ProjectIdV1, SessionIdV1, SummaryIdV1,
    TurnIdV1,
};
use crate::governance_contract_validation::{
    governance_contract_set_hash, validate_governance_contract_set,
};
use crate::persistence_contracts::{
    evaluate_undo, reject_sensitive_text, FullChatPersistenceV1, FullChatProtectionV1,
    PersistedSessionSummaryV1, SessionClearReceiptV1, SessionPersistenceSnapshotV1, UndoBlobIdV1,
    UndoCatalogV1, UndoEligibilityV1, UndoRecordV1, UndoTargetV1, MAX_FULL_CHAT_BYTES_PER_PROJECT,
    UNDO_RETENTION_MS,
};
use crate::transaction_contracts::{
    FrozenFileStateV1, MultiFileTransactionPlanV1, ObservedFileStateV1, PlannedFileMutationV1,
    PreflightStatusV1,
};
use crate::{
    governance_contract_set_source, governance_schema_source, governance_typescript_source,
    risk_matrix_source, AGENT_GOVERNANCE_CONTRACT_SET_SHA256,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn project_id() -> ProjectIdV1 {
    ProjectIdV1::parse("prj_00000001").unwrap()
}

fn session_id() -> SessionIdV1 {
    SessionIdV1::parse("ses_00000001").unwrap()
}

fn turn_id() -> TurnIdV1 {
    TurnIdV1::parse("trn_00000001").unwrap()
}

fn hash(label: &str) -> ContentHashV1 {
    ContentHashV1::digest(label.as_bytes())
}

fn write_request(tool_id: RegisteredToolIdV1, path: &str, serial_ordinal: u16) -> ToolRequestV1 {
    let is_create = tool_id == RegisteredToolIdV1::ProjectCreateMarkdown;
    ToolRequestV1 {
        schema_version: AGENT_ACTION_SCHEMA_VERSION,
        binding: ProjectToolBindingV1 {
            project_id: project_id(),
            session_id: session_id(),
            turn_id: turn_id(),
        },
        serial_ordinal,
        call_id: ToolCallIdV1::parse(format!("call_{:08}", serial_ordinal + 1)).unwrap(),
        tool_id,
        arguments: ToolArgumentsV1 {
            path: Some(ProjectRelativePathV1::parse(path).unwrap()),
            directory: None,
            query: None,
            content: is_create.then(|| "# New\n".to_owned()),
            patch: (!is_create).then(|| "@@ validated patch @@".to_owned()),
            expected_source_hash: (!is_create).then(|| hash("expected-source")),
        },
        purpose: "governance contract test".to_owned(),
    }
}

fn classify(
    tool_id: RegisteredToolIdV1,
    path: &str,
    serial_ordinal: u16,
    shape: ValidatedMutationShapeV1,
) -> WriteToolClassificationV1 {
    WriteToolClassificationV1::build(&write_request(tool_id, path, serial_ordinal), shape).unwrap()
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
fn governance_contract_set_is_deterministic_strict_and_runtime_disabled() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/contracts/agent-governance/v1");
    let mut resources = BTreeMap::new();
    collect(&root, &root, &mut resources);
    let first = validate_governance_contract_set(&resources).unwrap();
    let second = validate_governance_contract_set(&resources).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.resource_count, 10);
    assert_eq!(
        first.contract_set_hash,
        governance_contract_set_hash(&resources)
    );
    assert_eq!(
        first.contract_set_hash,
        AGENT_GOVERNANCE_CONTRACT_SET_SHA256
    );

    let manifest: Value = serde_json::from_str(governance_contract_set_source()).unwrap();
    assert_eq!(manifest["enabledAtRuntime"], false);
    let schema: Value = serde_json::from_str(governance_schema_source()).unwrap();
    assert!(schema["$defs"]["UndoRecordV1"].is_object());
    assert!(governance_typescript_source().contains("windows_user_context"));
    assert!(!governance_typescript_source().contains("plaintext"));
    let matrix: Value = serde_json::from_str(risk_matrix_source()).unwrap();
    assert_eq!(matrix["actions"].as_array().unwrap().len(), 21);
}

#[test]
fn every_registered_tool_and_action_has_one_stable_risk() {
    let tools = [
        RegisteredToolIdV1::ProjectListMarkdown,
        RegisteredToolIdV1::ProjectReadMarkdown,
        RegisteredToolIdV1::ProjectSearchMarkdown,
        RegisteredToolIdV1::ProjectValidateMarkdown,
        RegisteredToolIdV1::ProjectCreateMarkdown,
        RegisteredToolIdV1::ProjectPatchMarkdown,
    ];
    assert_eq!(
        tools
            .iter()
            .filter(|tool| registered_tool_risk(**tool) == Some(RiskLevelV1::ReadSafe))
            .count(),
        4
    );
    assert_eq!(
        tools
            .iter()
            .filter(|tool| registered_tool_risk(**tool).is_none())
            .count(),
        2
    );

    let unique = GovernedActionV1::ALL.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), GovernedActionV1::ALL.len());
    assert_eq!(
        unique
            .iter()
            .filter(|action| action.risk() == RiskLevelV1::ReadSafe)
            .count(),
        4
    );
    assert_eq!(
        unique
            .iter()
            .filter(|action| action.risk() == RiskLevelV1::WriteSafe)
            .count(),
        3
    );
    assert_eq!(
        unique
            .iter()
            .filter(|action| action.risk() == RiskLevelV1::AlwaysConfirm)
            .count(),
        8
    );
    assert_eq!(
        unique
            .iter()
            .filter(|action| action.risk() == RiskLevelV1::ForbiddenV1)
            .count(),
        6
    );
}

#[test]
fn write_tool_classification_binds_request_target_shape_and_protected_objects() {
    let ordinary = classify(
        RegisteredToolIdV1::ProjectCreateMarkdown,
        "characters/mara.md",
        0,
        ValidatedMutationShapeV1::CreateEntityMarkdown,
    );
    assert_eq!(ordinary.action, GovernedActionV1::CreateEntityMarkdown);
    assert_eq!(ordinary.risk, RiskLevelV1::WriteSafe);

    let draft_chapter = classify(
        RegisteredToolIdV1::ProjectCreateMarkdown,
        "chapters/001.md",
        1,
        ValidatedMutationShapeV1::CreateDraftChapter,
    );
    assert_eq!(draft_chapter.action, GovernedActionV1::CreateDraftChapter);
    assert_eq!(draft_chapter.risk, RiskLevelV1::WriteSafe);
    assert_eq!(
        WriteToolClassificationV1::build(
            &write_request(
                RegisteredToolIdV1::ProjectCreateMarkdown,
                "chapters/accepted.md",
                2,
            ),
            ValidatedMutationShapeV1::CreateEntityMarkdown,
        ),
        Err(GovernanceErrorCodeV1::InvalidField)
    );

    for (path, expected) in [
        (
            "materials/000.story-ledger.md",
            GovernedActionV1::UpdateStoryLedger,
        ),
        ("AGENTS.md", GovernedActionV1::ModifyProjectRules),
        ("project.md", GovernedActionV1::ModifyCriticalMetadata),
        ("published/book.md", GovernedActionV1::PublishOrExport),
        (
            ".novelist/security.md",
            GovernedActionV1::ModifyAuthorizationOrSkillPack,
        ),
    ] {
        let classification = classify(
            RegisteredToolIdV1::ProjectPatchMarkdown,
            path,
            1,
            ValidatedMutationShapeV1::PatchHunks,
        );
        assert_eq!(classification.action, expected, "path: {path}");
        assert_ne!(classification.risk, RiskLevelV1::WriteSafe, "path: {path}");
    }

    let whole_chapter = classify(
        RegisteredToolIdV1::ProjectPatchMarkdown,
        "chapters/001.md",
        2,
        ValidatedMutationShapeV1::ReplaceWholeChapter,
    );
    assert_eq!(whole_chapter.action, GovernedActionV1::ReplaceWholeChapter);
    assert_eq!(whole_chapter.risk, RiskLevelV1::AlwaysConfirm);

    let mut tampered = ordinary;
    tampered.action = GovernedActionV1::PatchEntityMarkdown;
    assert_eq!(
        tampered.validate(),
        Err(GovernanceErrorCodeV1::InvalidField)
    );
}

#[test]
fn three_policies_and_mixed_risk_have_zero_partial_commit() {
    let patch = GovernedActionV1::PatchEntityMarkdown;
    assert_eq!(
        authorize(AuthorizationPolicyV1::PlanOnly, patch).effect,
        AuthorizationEffectV1::Block
    );
    assert_eq!(
        authorize(AuthorizationPolicyV1::ReviewEveryWrite, patch).effect,
        AuthorizationEffectV1::AwaitConfirmation
    );
    assert_eq!(
        authorize(AuthorizationPolicyV1::AutoApplySafe, patch).effect,
        AuthorizationEffectV1::Execute
    );
    for policy in [
        AuthorizationPolicyV1::ReviewEveryWrite,
        AuthorizationPolicyV1::AutoApplySafe,
    ] {
        assert_eq!(
            authorize(policy, GovernedActionV1::UpdateStoryLedger).effect,
            AuthorizationEffectV1::AwaitConfirmation
        );
        assert_eq!(
            authorize(policy, GovernedActionV1::DeletePath).effect,
            AuthorizationEffectV1::Block
        );
    }

    let mixed = authorize_turn(
        AuthorizationPolicyV1::AutoApplySafe,
        &[
            GovernedActionV1::PatchEntityMarkdown,
            GovernedActionV1::ReplaceWholeChapter,
        ],
    )
    .unwrap();
    assert_eq!(
        mixed.transaction_effect,
        AuthorizationEffectV1::AwaitConfirmation
    );
    assert_eq!(mixed.preconfirmation_write_count, 0);
    assert!(mixed.atomic_write_transaction);
}

#[test]
fn confirmation_is_app_bound_single_use_and_invalidated_by_any_hash_change() {
    let binding = ConfirmationBindingV1 {
        project_id: project_id(),
        session_id: session_id(),
        turn_id: turn_id(),
        action_plan_hash: hash("action-plan"),
        source_set_hash: hash("source-set"),
        policy_snapshot_hash: hash("policy"),
    };
    let mut confirmation = HighRiskConfirmationV1::issue(
        ConfirmationIdV1::parse("cfm_00000001").unwrap(),
        binding.clone(),
        1_787_904_000_000,
    );
    confirmation.validate().unwrap();

    let mut stale = binding.clone();
    stale.source_set_hash = hash("changed-source-set");
    assert_eq!(
        confirmation.consume(&stale),
        Err(GovernanceErrorCodeV1::ConfirmationInvalid)
    );
    confirmation.consume(&binding).unwrap();
    assert_eq!(
        confirmation.consume(&binding),
        Err(GovernanceErrorCodeV1::ConfirmationAlreadyUsed)
    );
}

#[test]
fn persistence_is_bounded_metadata_only_and_full_chat_is_opt_in_protected() {
    let text = "项目目标与已确认情节的有界摘要。".to_owned();
    let summary = PersistedSessionSummaryV1 {
        project_id: project_id(),
        session_id: session_id(),
        summary_id: SummaryIdV1::parse("sum_00000001").unwrap(),
        version: 1,
        content_hash: ContentHashV1::digest(text.as_bytes()),
        utf8_bytes: text.len() as u32,
        text,
    };
    let snapshot = SessionPersistenceSnapshotV1 {
        schema_version: 1,
        project_id: project_id(),
        session_id: session_id(),
        authorization_policy: AuthorizationPolicyV1::AutoApplySafe,
        summary: Some(summary),
        message_index: vec![],
        terminal_turns: vec![],
        full_chat: FullChatPersistenceV1::default(),
        clear_generation: 0,
    };
    snapshot.validate().unwrap();

    let protected = FullChatPersistenceV1 {
        enabled: true,
        user_opt_in: true,
        protection: FullChatProtectionV1::WindowsUserContext,
        max_utf8_bytes: MAX_FULL_CHAT_BYTES_PER_PROJECT,
        retention_days: 30,
        privacy_notice_version: Some(1),
        clear_generation: 2,
    };
    protected.validate().unwrap();
    let unprotected = FullChatPersistenceV1 {
        protection: FullChatProtectionV1::Disabled,
        ..protected
    };
    assert_eq!(
        unprotected.validate(),
        Err(GovernanceErrorCodeV1::PlaintextDowngradeForbidden)
    );
}

#[test]
fn sensitive_canary_fixture_is_rejected_by_rust_contract() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../resources/contracts/agent-governance/v1/fixtures/invalid-sensitive-canary.json",
    );
    let fixture: Value = serde_json::from_slice(&fs::read(fixture_path).unwrap()).unwrap();
    assert_eq!(
        fixture["expectedError"],
        "agent-governance.sensitive-data-forbidden"
    );
    for canary in fixture["canaries"].as_array().unwrap() {
        assert_eq!(
            reject_sensitive_text(canary.as_str().unwrap()),
            Err(GovernanceErrorCodeV1::SensitiveDataForbidden)
        );
    }
}

#[test]
fn canon_changing_clear_receipts_are_rejected() {
    let receipt = SessionClearReceiptV1 {
        schema_version: 1,
        project_id: project_id(),
        session_id: session_id(),
        previous_clear_generation: 4,
        next_clear_generation: 5,
        summaries_removed: 1,
        message_indexes_removed: 10,
        terminal_turns_removed: 4,
        undo_records_removed: 2,
        canon_write_count: 0,
    };
    receipt.validate().unwrap();
    let mut unsafe_receipt = receipt;
    unsafe_receipt.canon_write_count = 1;
    assert_eq!(
        unsafe_receipt.validate(),
        Err(GovernanceErrorCodeV1::InvalidField)
    );
}

fn transaction_plan() -> MultiFileTransactionPlanV1 {
    MultiFileTransactionPlanV1::build(
        project_id(),
        session_id(),
        turn_id(),
        hash("policy"),
        vec![
            PlannedFileMutationV1 {
                frozen: FrozenFileStateV1 {
                    path: ProjectRelativePathV1::parse("world/city.md").unwrap(),
                    source_hash: Some(hash("city-before")),
                    target_must_be_absent: false,
                },
                classification: classify(
                    RegisteredToolIdV1::ProjectPatchMarkdown,
                    "world/city.md",
                    0,
                    ValidatedMutationShapeV1::PatchHunks,
                ),
                planned_result_hash: hash("city-after"),
                estimated_undo_bytes: 1024,
            },
            PlannedFileMutationV1 {
                frozen: FrozenFileStateV1 {
                    path: ProjectRelativePathV1::parse("plot/thread.md").unwrap(),
                    source_hash: Some(hash("plot-before")),
                    target_must_be_absent: false,
                },
                classification: classify(
                    RegisteredToolIdV1::ProjectPatchMarkdown,
                    "plot/thread.md",
                    1,
                    ValidatedMutationShapeV1::PatchHunks,
                ),
                planned_result_hash: hash("plot-after"),
                estimated_undo_bytes: 2048,
            },
        ],
    )
    .unwrap()
}

#[test]
fn multi_file_preflight_freezes_every_hash_and_rejects_with_zero_writes() {
    let plan = transaction_plan();
    plan.validate().unwrap();
    let observed = plan
        .mutations
        .iter()
        .map(|mutation| ObservedFileStateV1 {
            path: mutation.frozen.path.clone(),
            current_hash: mutation.frozen.source_hash.clone(),
        })
        .collect::<Vec<_>>();
    let ready = plan.preflight(&observed).unwrap();
    assert_eq!(ready.status, PreflightStatusV1::ReadyForSingleWriter);
    assert_eq!(ready.writes_performed, 0);

    let mut stale = observed;
    stale[0].current_hash = Some(hash("external-edit"));
    let rejected = plan.preflight(&stale).unwrap();
    assert_eq!(rejected.status, PreflightStatusV1::RejectedZeroWrite);
    assert!(!rejected.all_hashes_match);
    assert_eq!(rejected.writes_performed, 0);
}

#[test]
fn transaction_rejects_zero_undo_budget_and_classification_path_mismatch() {
    let classification = classify(
        RegisteredToolIdV1::ProjectPatchMarkdown,
        "world/city.md",
        0,
        ValidatedMutationShapeV1::PatchHunks,
    );
    let zero_undo = PlannedFileMutationV1 {
        frozen: FrozenFileStateV1 {
            path: classification.path.clone(),
            source_hash: Some(hash("before")),
            target_must_be_absent: false,
        },
        classification: classification.clone(),
        planned_result_hash: hash("after"),
        estimated_undo_bytes: 0,
    };
    assert_eq!(
        MultiFileTransactionPlanV1::build(
            project_id(),
            session_id(),
            turn_id(),
            hash("policy"),
            vec![zero_undo]
        ),
        Err(GovernanceErrorCodeV1::TransactionInvalid)
    );

    let mismatched = PlannedFileMutationV1 {
        frozen: FrozenFileStateV1 {
            path: ProjectRelativePathV1::parse("plot/thread.md").unwrap(),
            source_hash: Some(hash("before")),
            target_must_be_absent: false,
        },
        classification,
        planned_result_hash: hash("after"),
        estimated_undo_bytes: 1,
    };
    assert_eq!(
        MultiFileTransactionPlanV1::build(
            project_id(),
            session_id(),
            turn_id(),
            hash("policy"),
            vec![mismatched]
        ),
        Err(GovernanceErrorCodeV1::TransactionInvalid)
    );
}

#[test]
fn undo_is_bounded_protected_expiring_clearable_and_hash_checked() {
    let plan = transaction_plan();
    let record = UndoRecordV1::build(
        project_id(),
        session_id(),
        turn_id(),
        plan.transaction_hash.clone(),
        UndoBlobIdV1::parse("ubl_00000001").unwrap(),
        hash("encrypted-blob"),
        plan.total_estimated_undo_bytes,
        plan.mutations
            .iter()
            .map(|mutation| UndoTargetV1 {
                path: mutation.frozen.path.clone(),
                expected_current_hash: mutation.planned_result_hash.clone(),
            })
            .collect(),
        EpochMillisV1(1_000),
        EpochMillisV1(1_000 + UNDO_RETENTION_MS),
        3,
    )
    .unwrap();
    let catalog = UndoCatalogV1 {
        schema_version: 1,
        project_id: project_id(),
        total_encrypted_bytes: 0,
        records: vec![],
        clear_generation: 3,
    };
    catalog.validate().unwrap();
    assert!(catalog.admits_auto_apply(&plan, &record));

    let mut wrong_transaction = record.clone();
    wrong_transaction.transaction_hash = hash("wrong-transaction");
    assert!(!catalog.admits_auto_apply(&plan, &wrong_transaction));

    let occupied_catalog = UndoCatalogV1 {
        schema_version: 1,
        project_id: project_id(),
        total_encrypted_bytes: record.encrypted_bytes,
        records: vec![record.clone()],
        clear_generation: 3,
    };
    occupied_catalog.validate().unwrap();
    assert!(!occupied_catalog.admits_auto_apply(&plan, &record));

    assert_eq!(
        evaluate_undo(
            &record,
            &record.target_set_hash,
            true,
            EpochMillisV1(2_000),
            3,
        )
        .unwrap(),
        UndoEligibilityV1::Ready
    );
    assert_eq!(
        evaluate_undo(
            &record,
            &hash("changed-target"),
            true,
            EpochMillisV1(2_000),
            3
        )
        .unwrap(),
        UndoEligibilityV1::HashConflict
    );
    assert_eq!(
        evaluate_undo(
            &record,
            &record.target_set_hash,
            false,
            EpochMillisV1(2_000),
            3,
        )
        .unwrap(),
        UndoEligibilityV1::DecryptFailed
    );
    assert_eq!(
        evaluate_undo(
            &record,
            &record.target_set_hash,
            true,
            record.expires_at_ms,
            3,
        )
        .unwrap(),
        UndoEligibilityV1::Expired
    );
    assert_eq!(
        evaluate_undo(
            &record,
            &record.target_set_hash,
            true,
            EpochMillisV1(2_000),
            4,
        )
        .unwrap(),
        UndoEligibilityV1::Cleared
    );
}
