use super::*;
use crate::capability_pack::embedded_pack;
use crate::conversation::{ConversationMessage, ConversationTurn, ConversationTurnStatus};
use crate::credential_vault::ProviderId;

fn id() -> String {
    Uuid::new_v4().to_string()
}

fn body(kind: CanonEntityKind, identity: &str, text: &str) -> String {
    format!("---\nid: {identity}\ntitle: 测试资料\nwritingLanguage: zh-CN\nkind: {}\nschemaVersion: {}\n---\n\n# 测试资料\n\n## 摘要\n\n{text}\n",
        kind.metadata_kind(), kind.metadata_version())
}

fn session() -> ConversationSession {
    ConversationSession {
        schema_version: 1,
        project_id: id(),
        session_id: id(),
        turns: vec![ConversationTurn {
            turn_id: id(),
            provider_id: ProviderId::Deepseek,
            model: "fixture".to_owned(),
            status: ConversationTurnStatus::Running,
            started_at_ms: 1,
            updated_at_ms: 1,
            error_code: None,
            retry_state: None,
            messages: vec![
                ConversationMessage {
                    message_id: id(),
                    role: ConversationRole::Assistant,
                    content: "建议角色喜欢观察。".to_owned(),
                    created_at_ms: 1,
                },
                ConversationMessage {
                    message_id: id(),
                    role: ConversationRole::User,
                    content: "采用这个设定并保存角色。".to_owned(),
                    created_at_ms: 2,
                },
            ],
        }],
    }
}

fn fixture(kind: CanonEntityKind) -> (ChangeEvidence, Value, ConversationSession) {
    let session = session();
    let mut evidence =
        ChangeEvidence::from_conversation(&session, &session.turns[0].turn_id).unwrap();
    let pack = embedded_pack().unwrap();
    let capability = pack.capability(kind.capability_id()).unwrap();
    let method = evidence
        .record_method(
            &pack,
            kind.capability_id(),
            pack.resource_sha256(&capability.instruction_resource)
                .unwrap(),
        )
        .unwrap();
    let user = &session.turns[0].messages[1];
    let request = json!({
        "schemaVersion":1,"sourceKind":"user_decision","risk":"low",
        "sources":[{"messageId":user.message_id,"contentHash":sha256_hex(user.content.as_bytes())}],
        "operations":[{
            "kind":"create","entityKind":kind,"capabilityId":kind.capability_id(),
            "methodReceiptId":method,"targetId":null,"readReceiptId":null,
            "expectedSourceHash":null,"proposedMarkdown":body(kind,"new","喜欢观察。")
        }]
    });
    (evidence, request, session)
}

fn make_update(evidence: &mut ChangeEvidence, request: &mut Value, dirty: Option<u64>) {
    let before = body(CanonEntityKind::Character, "character-one", "喜欢观察。");
    let hash = sha256_hex(before.as_bytes());
    let (target, receipt) = evidence
        .record_complete_read(
            CanonEntityKind::Character,
            "characters/one.md",
            &before,
            &hash,
            dirty,
        )
        .unwrap();
    let op = &mut request["operations"][0];
    op["kind"] = json!("update");
    op["targetId"] = json!(target);
    op["readReceiptId"] = json!(receipt);
    op["expectedSourceHash"] = json!(hash);
    op["proposedMarkdown"] = json!(body(
        CanonEntityKind::Character,
        "character-one",
        "喜欢观察和写作。"
    ));
}

fn error(evidence: &ChangeEvidence, request: &Value) -> ChangeError {
    match evidence.validate_request(request) {
        Err(error) => error,
        Ok(_) => panic!("invalid request accepted"),
    }
}

#[test]
fn five_types_create_and_upsert_are_validated_without_io() {
    propose_change_definition().validate().unwrap();
    for kind in [
        CanonEntityKind::Character,
        CanonEntityKind::World,
        CanonEntityKind::Material,
        CanonEntityKind::Plot,
        CanonEntityKind::Outline,
    ] {
        let (evidence, mut request, _) = fixture(kind);
        assert_eq!(
            evidence.validate_request(&request).unwrap().eligibility,
            ChangeEligibility::AutomaticEligible
        );
        request["operations"][0]["kind"] = json!("upsert");
        assert_eq!(
            evidence.validate_request(&request).unwrap().eligibility,
            ChangeEligibility::AutomaticEligible
        );
    }
}

#[test]
fn strict_schema_rejects_unknown_omitted_and_ill_typed_fields() {
    let (evidence, request, _) = fixture(CanonEntityKind::Character);
    for field in ["targetId", "readReceiptId", "expectedSourceHash"] {
        let mut invalid = request.clone();
        invalid["operations"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(error(&evidence, &invalid), ChangeError::InvalidRequest);
    }
    for (field, value) in [
        ("schemaVersion", json!(2)),
        ("sourceKind", json!("admin")),
        ("sources", json!("not-array")),
        ("projectId", json!(id())),
    ] {
        let mut invalid = request.clone();
        invalid[field] = value;
        assert_eq!(error(&evidence, &invalid), ChangeError::InvalidRequest);
    }
    for field in ["relativePath", "shell", "force"] {
        let mut invalid = request.clone();
        invalid["operations"][0][field] = json!("x");
        assert_eq!(error(&evidence, &invalid), ChangeError::InvalidRequest);
    }
}

#[test]
fn current_user_source_cannot_be_forged_or_replaced_by_assistant() {
    let (evidence, request, session) = fixture(CanonEntityKind::Character);
    let assistant = &session.turns[0].messages[0];
    let mut invalid = request.clone();
    invalid["sources"] = json!([{"messageId":assistant.message_id,
        "contentHash":sha256_hex(assistant.content.as_bytes())}]);
    assert_eq!(error(&evidence, &invalid), ChangeError::SourceInvalid);
    invalid = request.clone();
    invalid["sources"][0]["messageId"] = json!(id());
    assert_eq!(error(&evidence, &invalid), ChangeError::SourceInvalid);
    invalid = request.clone();
    invalid["sources"][0]["contentHash"] = json!("0".repeat(64));
    assert_eq!(error(&evidence, &invalid), ChangeError::SourceInvalid);
    invalid = request.clone();
    invalid["sources"]
        .as_array_mut()
        .unwrap()
        .push(request["sources"][0].clone());
    assert_eq!(error(&evidence, &invalid), ChangeError::SourceInvalid);
}

#[test]
fn accepted_suggestion_requires_user_acceptance_and_actual_suggestion() {
    let (evidence, mut request, session) = fixture(CanonEntityKind::Character);
    request["sourceKind"] = json!("accepted_suggestion");
    assert_eq!(error(&evidence, &request), ChangeError::SourceInvalid);
    let assistant = &session.turns[0].messages[0];
    request["sources"].as_array_mut().unwrap().push(json!({
        "messageId":assistant.message_id,"contentHash":sha256_hex(assistant.content.as_bytes())
    }));
    assert_eq!(
        evidence.validate_request(&request).unwrap().eligibility,
        ChangeEligibility::AutomaticEligible
    );
}

#[test]
fn source_kind_and_risk_do_not_silently_promote_suggestions_or_chapters() {
    let (evidence, mut request, _) = fixture(CanonEntityKind::Character);
    for risk in ["semantic_conflict", "ambiguous_target", "broad_replacement"] {
        request["risk"] = json!(risk);
        assert_eq!(
            evidence.validate_request(&request).unwrap().eligibility,
            ChangeEligibility::ConfirmationRequired
        );
    }
    request["sourceKind"] = json!("ai_suggestion");
    request["risk"] = json!("low");
    assert_eq!(
        evidence.validate_request(&request).unwrap().eligibility,
        ChangeEligibility::ConversationOnly
    );
    request["sourceKind"] = json!("chapter_commit");
    assert_eq!(
        error(&evidence, &request),
        ChangeError::ChapterSourceUnavailable
    );
}

#[test]
fn evidence_is_bound_to_project_session_turn_and_method() {
    let (evidence, mut request, session) = fixture(CanonEntityKind::Character);
    let turn = &session.turns[0].turn_id;
    assert!(evidence.matches_context(&session.project_id, &session.session_id, turn));
    assert!(!evidence.matches_context(&id(), &session.session_id, turn));
    assert!(!evidence.matches_context(&session.project_id, &id(), turn));
    assert!(!evidence.matches_context(&session.project_id, &session.session_id, &id()));
    let mut other = ChangeEvidence::from_conversation(&session, turn).unwrap();
    assert_eq!(error(&other, &request), ChangeError::MethodNotRead);
    assert_eq!(
        other.record_method(
            &embedded_pack().unwrap(),
            "create-character",
            &"0".repeat(64)
        ),
        Err(ChangeError::MethodNotRead)
    );
    request["operations"][0]["capabilityId"] = json!("publish-novel");
    assert_eq!(error(&evidence, &request), ChangeError::MethodNotRead);
}

#[test]
fn update_requires_complete_bound_read_and_preserved_identity() {
    let (mut evidence, mut request, _) = fixture(CanonEntityKind::Character);
    make_update(&mut evidence, &mut request, None);
    assert_eq!(
        evidence.validate_request(&request).unwrap().eligibility,
        ChangeEligibility::AutomaticEligible
    );
    for (field, value, expected) in [
        ("readReceiptId", json!(id()), ChangeError::IncompleteRead),
        ("targetId", json!(id()), ChangeError::TargetInvalid),
        (
            "expectedSourceHash",
            json!("0".repeat(64)),
            ChangeError::SourceChanged,
        ),
        (
            "proposedMarkdown",
            json!(body(CanonEntityKind::Character, "other", "x")),
            ChangeError::TargetInvalid,
        ),
    ] {
        let mut invalid = request.clone();
        invalid["operations"][0][field] = value;
        assert_eq!(error(&evidence, &invalid), expected);
    }
    assert_eq!(
        evidence.record_complete_read(
            CanonEntityKind::Character,
            "characters/one.md",
            "fragment",
            &"0".repeat(64),
            None
        ),
        Err(ChangeError::IncompleteRead)
    );
}

#[test]
fn dirty_or_structurally_destructive_updates_require_confirmation() {
    let (mut evidence, mut request, _) = fixture(CanonEntityKind::Character);
    make_update(&mut evidence, &mut request, Some(1));
    assert_eq!(
        evidence.validate_request(&request).unwrap().eligibility,
        ChangeEligibility::ConfirmationRequired
    );
    let (mut evidence, mut request, _) = fixture(CanonEntityKind::Character);
    make_update(&mut evidence, &mut request, None);
    let content = request["operations"][0]["proposedMarkdown"]
        .as_str()
        .unwrap()
        .replace("## 摘要\n", "");
    request["operations"][0]["proposedMarkdown"] = json!(content);
    assert_eq!(
        evidence.validate_request(&request).unwrap().eligibility,
        ChangeEligibility::ConfirmationRequired
    );
}

#[test]
fn duplicate_targets_and_over_budget_batches_are_rejected() {
    let (mut evidence, mut request, _) = fixture(CanonEntityKind::Character);
    make_update(&mut evidence, &mut request, None);
    let op = request["operations"][0].clone();
    request["operations"].as_array_mut().unwrap().push(op);
    assert_eq!(error(&evidence, &request), ChangeError::DuplicateTarget);
    let (evidence, mut request, _) = fixture(CanonEntityKind::Character);
    let op = request["operations"][0].clone();
    request["operations"] = json!((0..4)
        .map(|i| {
            let mut item = op.clone();
            item["proposedMarkdown"] = json!(body(CanonEntityKind::Character, "new", "x")
                .replace("测试资料", &format!("角色{i}")));
            item
        })
        .collect::<Vec<_>>());
    evidence.validate_request(&request).unwrap();
    request["operations"].as_array_mut().unwrap().push(op);
    assert_eq!(error(&evidence, &request), ChangeError::InvalidRequest);
}

#[test]
fn byte_limits_measure_json_escaping_and_never_truncate_content() {
    let (evidence, mut request, _) = fixture(CanonEntityKind::Character);
    request["operations"][0]["proposedMarkdown"] =
        json!(body(CanonEntityKind::Character, "new", &"界".repeat(3000)));
    assert_eq!(error(&evidence, &request), ChangeError::BudgetExceeded);
    request["operations"][0]["proposedMarkdown"] =
        json!(body(CanonEntityKind::Character, "new", &"\t".repeat(8000)));
    assert_eq!(error(&evidence, &request), ChangeError::BudgetExceeded);
    assert_eq!(NR4_CUMULATIVE_OUTPUT_TOKENS, 4 * 4096 + 1024);
    assert_eq!(
        MAX_NR4_AGENT_ATTEMPTS,
        MAX_NR4_READ_ROUNDS + MAX_NR4_MUTATION_ROUNDS + 1
    );
}

#[test]
fn path_allowlist_denies_other_scopes_traversal_and_windows_aliases() {
    for path in [
        "chapters/a.md",
        "project.md",
        ".novelgainker/tasks/a.md",
        "characters/../world/a.md",
        "characters\\a.md",
        "characters/a.md:ads",
        "C:/characters/a.md",
        "characters/a.txt",
        "characters/CON.md",
    ] {
        assert_eq!(
            CanonEntityKind::Character.validate_path(path),
            Err(ChangeError::TargetInvalid),
            "{path}"
        );
    }
    CanonEntityKind::Outline
        .validate_path("outlines/a.md")
        .unwrap();
}

#[test]
fn candidate_metadata_and_identity_are_not_guessed() {
    let original = body(CanonEntityKind::Character, "new", "资料");
    for invalid in [
        original.replace("schemaVersion: 2", "schemaVersion: 1"),
        original.replace("kind: character", "kind: outline"),
        original.replace("id: new", "id: ../../other"),
        original.replace("writingLanguage: zh-CN", "writingLanguage: en"),
        original.replace('\n', "\r\n"),
        format!("\u{feff}{original}"),
    ] {
        assert_eq!(
            validate_candidate(CanonEntityKind::Character, &invalid),
            Err(ChangeError::CandidateInvalid)
        );
    }
    assert_eq!(
        metadata_field("---\ntitle: \"角色\"\n---\n", "title"),
        Some("角色".to_owned())
    );
}

#[test]
fn local_decisions_are_cas_references_not_write_payloads() {
    let request = ChangeDecisionRequest {
        project_session_id: id(),
        change_id: id(),
        revision: 1,
        decision: ChangeDecision::Undo,
    };
    request.validate().unwrap();
    let mut value = serde_json::to_value(&request).unwrap();
    value["content"] = json!("overwrite");
    assert!(serde_json::from_value::<ChangeDecisionRequest>(value).is_err());
    let mut invalid = request;
    invalid.revision = 0;
    assert_eq!(invalid.validate(), Err(ChangeError::RevisionConflict));
    for status in [
        ChangeStatus::Prepared,
        ChangeStatus::PendingConfirmation,
        ChangeStatus::Applying,
        ChangeStatus::Rejected,
        ChangeStatus::Cancelled,
        ChangeStatus::RecoveryRequired,
        ChangeStatus::Undone,
    ] {
        assert!(!status.is_saved());
    }
    assert!(ChangeStatus::Committed.is_saved());
}

#[test]
fn turn_budget_reserves_final_response_before_any_mutation() {
    let mut budget = ChangeTurnBudget::default();
    budget.consume_compactor().unwrap();
    budget.consume_agent(4096).unwrap();
    budget.consume_read().unwrap();
    budget.consume_agent(4096).unwrap();
    budget.consume_read().unwrap();
    assert_eq!(budget.consume_read(), Err(ChangeError::BudgetExceeded));
    budget.consume_agent(4096).unwrap();
    assert_eq!(
        budget.consume_mutation(NR4_REQUEST_BYTES + 1, 0),
        Err(ChangeError::BudgetExceeded)
    );
    assert_eq!(
        budget.consume_mutation(0, NR4_CONTINUATION_BYTES + 1),
        Err(ChangeError::BudgetExceeded)
    );
    budget
        .consume_mutation(NR4_REQUEST_BYTES, NR4_CONTINUATION_BYTES)
        .unwrap();
    assert_eq!(
        budget.consume_mutation(0, 0),
        Err(ChangeError::BudgetExceeded)
    );
    assert_eq!(budget.consume_read(), Err(ChangeError::BudgetExceeded));
    budget.consume_agent(4096).unwrap();
    assert_eq!(budget.consume_agent(1), Err(ChangeError::BudgetExceeded));
    assert_eq!(budget.consume_compactor(), Err(ChangeError::BudgetExceeded));
}

#[test]
fn durable_record_rejects_corrupt_hashes_unknown_payloads_and_identity_changes() {
    let (_, _, session) = fixture(CanonEntityKind::Character);
    let before = body(CanonEntityKind::Character, "character-one", "之前");
    let after = body(CanonEntityKind::Character, "character-one", "之后");
    let mut record = ChangeRecord {
        undo_revision: None,
        chapter: None,
        chapter_dependencies: Vec::new(),
        editor_revisions: BTreeMap::new(),
        schema_version: 1,
        project_id: session.project_id,
        conversation_session_id: session.session_id,
        turn_id: session.turns[0].turn_id.clone(),
        call_id: "call_fixture".to_owned(),
        request_hash: sha256_hex(b"normalized"),
        source_kind: ChangeSourceKind::UserDecision,
        sources: vec![SourceReference {
            message_id: session.turns[0].messages[1].message_id.clone(),
            content_hash: sha256_hex(session.turns[0].messages[1].content.as_bytes()),
        }],
        receipt: ChangeReceipt {
            schema_version: 1,
            change_id: id(),
            revision: 1,
            status: ChangeStatus::Prepared,
            files: vec![ChangeFileReceipt {
                relative_path: "characters/one.md".to_owned(),
                before_hash: Some(sha256_hex(before.as_bytes())),
                after_hash: sha256_hex(after.as_bytes()),
            }],
        },
        snapshots: vec![ChangeSnapshot {
            entity_kind: CanonEntityKind::Character,
            relative_path: "characters/one.md".to_owned(),
            before: Some(before),
            after,
        }],
    };
    record.validate().unwrap();
    let mut value = serde_json::to_value(&record).unwrap();
    value["providerContinuation"] = json!({"secret":"not_allowed"});
    assert!(serde_json::from_value::<ChangeRecord>(value).is_err());
    record.receipt.files[0].after_hash = "0".repeat(64);
    assert_eq!(record.validate(), Err(ChangeError::RecoveryRequired));
}

#[test]
fn nr504_budget_accepts_non_linear_seven_tools_and_reserves_eighth_reply_once() {
    use crate::chapter_contract::{NR5_CONTINUATION_BYTES, NR5_REQUEST_BYTES};
    use crate::protocol_profile::ToolProtocolProfile;
    let mut budget = ChangeTurnBudget::for_protocol(ToolProtocolProfile::Nr5);
    budget.consume_compactor().unwrap();
    for step in 0..8 {
        budget.consume_agent(4096).unwrap();
        match step {
            0 | 2 | 4 | 5 => budget.consume_read().unwrap(),
            1 | 3 => budget
                .consume_mutation(NR5_REQUEST_BYTES, NR5_CONTINUATION_BYTES)
                .unwrap(),
            6 => budget
                .consume_publication(NR5_REQUEST_BYTES, NR5_CONTINUATION_BYTES)
                .unwrap(),
            _ => {
                assert!(!budget.tool_available(PROPOSE_CHANGE_TOOL_NAME));
                assert!(
                    !budget.tool_available(crate::canon_context::NOVELIST_READ_CONTEXT_TOOL_NAME)
                );
                assert!(!budget.tool_available("novelist.publish"));
            }
        }
    }
    assert_eq!(budget.requested_tokens, 33792);
    assert_eq!(budget.consume_compactor(), Err(ChangeError::BudgetExceeded));
    assert_eq!(budget.consume_agent(1), Err(ChangeError::BudgetExceeded));
    assert_eq!(
        budget.consume_mutation(0, 0),
        Err(ChangeError::BudgetExceeded)
    );
    assert_eq!(
        budget.consume_publication(0, 0),
        Err(ChangeError::BudgetExceeded)
    );
    assert_eq!(budget.consume_read(), Err(ChangeError::BudgetExceeded));
    for publication in [false, true] {
        for (body, native) in [(NR5_REQUEST_BYTES + 1, 0), (0, NR5_CONTINUATION_BYTES + 1)] {
            let mut fresh = ChangeTurnBudget::for_protocol(ToolProtocolProfile::Nr5);
            fresh.consume_agent(4096).unwrap();
            assert_eq!(
                if publication {
                    fresh.consume_publication(body, native)
                } else {
                    fresh.consume_mutation(body, native)
                },
                Err(ChangeError::BudgetExceeded)
            );
            assert_eq!(fresh.mutation_rounds + fresh.publication_rounds, 0);
        }
        let mut exhausted = ChangeTurnBudget::for_protocol(ToolProtocolProfile::Nr5);
        for _ in 0..8 {
            exhausted.consume_agent(4096).unwrap();
        }
        assert_eq!(
            if publication {
                exhausted.consume_publication(0, 0)
            } else {
                exhausted.consume_mutation(0, 0)
            },
            Err(ChangeError::BudgetExceeded)
        );
    }
}
