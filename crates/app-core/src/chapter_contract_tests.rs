use super::*;
use crate::conversation::{ConversationMessage, ConversationTurn};
use crate::credential_vault::ProviderId;

fn uid(n: u128) -> String {
    Uuid::from_u128(n).to_string()
}
fn markdown(status: &str) -> String {
    format!("---\nid: {}\ntitle: 第一章\nkind: chapter\nschemaVersion: 1\nwritingLanguage: zh-CN\nchapterNumber: 1\nstatus: {status}\n---\n\n# 第一章\n\n## Draft\n\n林舟走过北桥。\n", uid(4))
}
fn fixture() -> (ConversationSession, ChapterCandidate) {
    let scope = ChapterScope {
        project_id: uid(1),
        session_id: uid(2),
        turn_id: uid(3),
    };
    let message = ConversationMessage {
        message_id: uid(5),
        role: ConversationRole::User,
        content: "写并保存第一章".to_owned(),
        created_at_ms: 1,
    };
    let source = SourceReference {
        message_id: message.message_id.clone(),
        content_hash: sha256_hex(message.content.as_bytes()),
    };
    let session = ConversationSession {
        schema_version: 1,
        project_id: scope.project_id.clone(),
        session_id: scope.session_id.clone(),
        turns: vec![ConversationTurn {
            turn_id: scope.turn_id.clone(),
            provider_id: ProviderId::Openai,
            model: "synthetic".to_owned(),
            status: ConversationTurnStatus::Running,
            started_at_ms: 1,
            updated_at_ms: 1,
            messages: vec![message],
            error_code: None,
            retry_state: None,
        }],
    };
    let body = markdown("draft");
    let candidate = ChapterCandidate {
        schema_version: 1,
        candidate_id: uid(6),
        scope,
        source,
        intent: ChapterIntent::WriteAndSave,
        target: ChapterTarget::Create {
            identity: ChapterIdentity {
                chapter_id: uid(4),
                chapter_number: 1,
            },
        },
        content_hash: sha256_hex(body.as_bytes()),
        proposed_markdown: body,
        complete: true,
        discarded: false,
    };
    (session, candidate)
}
#[test]
fn nr501_structural_template_identity_and_byte_hash() {
    let body = markdown("draft");
    let doc = ChapterDocument::parse("chapters/one.md", body.as_bytes()).unwrap();
    assert_eq!(doc.status, ChapterStatus::Draft);
    let windows = format!("\u{feff}{}", body.replace('\n', "\r\n"));
    let windows_doc = ChapterDocument::parse("chapters/一.md", windows.as_bytes()).unwrap();
    assert_eq!(doc.identity, windows_doc.identity);
    assert_ne!(doc.content_hash, windows_doc.content_hash);
    for path in [
        "../chapters/x.md",
        "chapters/../x.md",
        "characters/x.md",
        "chapters/x.md:stream",
        "chapters/\\x.md",
    ] {
        assert!(
            ChapterDocument::parse(path, body.as_bytes()).is_err(),
            "{path}"
        );
    }
    for invalid in [
        body.replace("schemaVersion: 1", "schemaVersion: 2"),
        body.replace("kind: chapter", "kind: character"),
        body.replace("chapterNumber: 1", "chapterNumber: 0"),
        body.replace("chapterNumber: 1", "chapterNumber: null"),
        body.replace("status: draft", "status: saved"),
        body.replace("status: draft", "status: draft\nextra: value"),
        body.replace("title: 第一章", "title: 第一章\ntitle: 第二章"),
        body.replace(&uid(4), "new"),
        body.replace("## Draft", "## 正文"),
        body.replace("## Draft", "## Draft\n\nbody\n\n## Draft"),
        body.replace("林舟走过北桥。", ""),
    ] {
        assert!(
            ChapterDocument::parse("chapters/x.md", invalid.as_bytes()).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(
        ChapterDocument::parse("chapters/x.md", &vec![b'a'; MAX_CHAPTER_BYTES + 1]),
        Err(ChapterContractError::TooLarge)
    );
}
#[test]
fn nr501_inventory_rejects_duplicate_identity_or_number() {
    let a = ChapterIdentity {
        chapter_id: uid(1),
        chapter_number: 1,
    };
    assert!(validate_chapter_inventory(std::slice::from_ref(&a)).is_ok());
    assert!(validate_chapter_inventory(&[a.clone(), a.clone()]).is_err());
    assert!(validate_chapter_inventory(&[
        a.clone(),
        ChapterIdentity {
            chapter_id: uid(2),
            chapter_number: 1
        }
    ])
    .is_err());
    assert!(validate_chapter_inventory(&[
        a,
        ChapterIdentity {
            chapter_id: uid(1),
            chapter_number: 2
        }
    ])
    .is_err());
}
#[test]
fn nr501_discussion_trial_and_ambiguous_never_authorize_candidate() {
    let (session, candidate) = fixture();
    assert!(candidate
        .validate(&session, &candidate.scope, &[], None, None)
        .is_ok());
    for intent in [
        ChapterIntent::Discussion,
        ChapterIntent::TrialDraft,
        ChapterIntent::Ambiguous,
        ChapterIntent::AcceptCandidate,
    ] {
        let mut c = candidate.clone();
        c.intent = intent;
        assert_eq!(
            c.validate(&session, &c.scope, &[], None, None),
            Err(ChapterContractError::NotAuthorized)
        );
    }
}
#[test]
fn nr501_sources_require_current_persisted_user_and_matching_scope() {
    let (session, candidate) = fixture();
    for change in 0..6 {
        let mut session = session.clone();
        match change {
            0 => session.project_id = uid(40),
            1 => session.session_id = uid(41),
            2 => session.turns[0].messages[0].role = ConversationRole::Assistant,
            3 => session.turns[0].messages[0].content.push('!'),
            4 => session.turns[0].status = ConversationTurnStatus::Completed,
            _ => {
                let m = session.turns[0].messages[0].clone();
                session.turns[0].messages.push(m);
            }
        }
        assert_eq!(
            candidate.validate(&session, &candidate.scope, &[], None, None),
            Err(ChapterContractError::InvalidSource)
        );
    }
    let mut other = candidate.scope.clone();
    other.turn_id = uid(40);
    assert_eq!(
        candidate.validate(&session, &other, &[], None, None),
        Err(ChapterContractError::Stale)
    );
}
#[test]
fn nr501_acceptance_binds_exact_displayed_candidate_and_not_other_session() {
    let (session, candidate) = fixture();
    let mut accepted = candidate.clone();
    accepted.intent = ChapterIntent::AcceptCandidate;
    assert!(accepted
        .validate(&session, &accepted.scope, &[], None, Some(&candidate))
        .is_ok());
    for change in 0..4 {
        let mut old = candidate.clone();
        match change {
            0 => old.scope.session_id = uid(40),
            1 => old.complete = false,
            2 => old.discard(),
            _ => old.proposed_markdown.push('x'),
        }
        assert!(accepted
            .validate(&session, &accepted.scope, &[], None, Some(&old))
            .is_err());
    }
}
#[test]
fn nr501_replacement_needs_full_current_read_and_stable_identity() {
    let (session, mut c) = fixture();
    let identity = ChapterIdentity {
        chapter_id: uid(4),
        chapter_number: 1,
    };
    let source_hash = sha256_hex(b"original");
    c.target = ChapterTarget::Replace {
        identity: identity.clone(),
        expected_hash: source_hash.clone(),
        read_receipt_id: uid(9),
    };
    let read = ChapterReadReceipt {
        receipt_id: uid(9),
        scope: c.scope.clone(),
        identity: identity.clone(),
        content_hash: source_hash,
        complete: true,
    };
    assert!(c
        .validate(
            &session,
            &c.scope,
            std::slice::from_ref(&identity),
            Some(&read),
            None
        )
        .is_ok());
    assert!(c
        .validate(
            &session,
            &c.scope,
            std::slice::from_ref(&identity),
            None,
            None
        )
        .is_err());
    for change in 0..4 {
        let mut r = read.clone();
        match change {
            0 => r.complete = false,
            1 => r.scope.turn_id = uid(40),
            2 => r.content_hash = sha256_hex(b"changed"),
            _ => r.identity.chapter_number = 2,
        }
        assert!(c
            .validate(
                &session,
                &c.scope,
                std::slice::from_ref(&identity),
                Some(&r),
                None
            )
            .is_err());
    }
}

#[test]
fn nr501_accepting_replacement_in_next_turn_needs_fresh_read_not_old_receipt() {
    let (mut session, mut old) = fixture();
    let identity = ChapterIdentity {
        chapter_id: uid(4),
        chapter_number: 1,
    };
    let original_hash = sha256_hex(markdown("draft").as_bytes());
    old.target = ChapterTarget::Replace {
        identity: identity.clone(),
        expected_hash: original_hash.clone(),
        read_receipt_id: uid(8),
    };
    let mut accepted = old.clone();
    accepted.intent = ChapterIntent::AcceptCandidate;
    accepted.scope.turn_id = uid(20);
    accepted.source.message_id = uid(21);
    session.turns[0].turn_id = uid(20);
    session.turns[0].messages[0].message_id = uid(21);
    session.turns[0].messages[0].content = "接受上一轮展示的第一章版本".to_owned();
    accepted.source.content_hash = sha256_hex(session.turns[0].messages[0].content.as_bytes());
    accepted.target = ChapterTarget::Replace {
        identity: identity.clone(),
        expected_hash: original_hash.clone(),
        read_receipt_id: uid(22),
    };
    let read = ChapterReadReceipt {
        receipt_id: uid(22),
        scope: accepted.scope.clone(),
        identity: identity.clone(),
        content_hash: original_hash,
        complete: true,
    };
    assert!(accepted
        .validate(
            &session,
            &accepted.scope,
            std::slice::from_ref(&identity),
            Some(&read),
            Some(&old)
        )
        .is_ok());
    let mut stale = read.clone();
    stale.scope = old.scope.clone();
    stale.receipt_id = uid(8);
    assert!(accepted
        .validate(
            &session,
            &accepted.scope,
            std::slice::from_ref(&identity),
            Some(&stale),
            Some(&old)
        )
        .is_err());
    let mut changed = old;
    if let ChapterTarget::Replace { expected_hash, .. } = &mut changed.target {
        *expected_hash = sha256_hex(b"external edit");
    }
    assert!(accepted
        .validate(
            &session,
            &accepted.scope,
            &[identity],
            Some(&read),
            Some(&changed)
        )
        .is_err());
}
#[test]
fn nr501_truncation_discard_and_tampering_fail_without_mutation() {
    let (session, candidate) = fixture();
    for change in 0..4 {
        let mut c = candidate.clone();
        match change {
            0 => c.complete = false,
            1 => c.discard(),
            2 => c.proposed_markdown.push('x'),
            _ => c.schema_version = 2,
        }
        assert!(c.validate(&session, &c.scope, &[], None, None).is_err());
        assert_eq!(session, fixture().0);
    }
}
#[test]
fn nr501_strict_wire_rejects_unknown_missing_and_null_target_fields() {
    let (_, candidate) = fixture();
    let value = serde_json::to_value(&candidate).unwrap();
    let mut unknown = value.clone();
    unknown["secret"] = serde_json::json!("no");
    assert!(serde_json::from_value::<ChapterCandidate>(unknown).is_err());
    let mut missing = value.clone();
    missing.as_object_mut().unwrap().remove("complete");
    assert!(serde_json::from_value::<ChapterCandidate>(missing).is_err());
    let mut invalid = value.clone();
    invalid["source"]["messageId"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<ChapterCandidate>(invalid).is_err());
    for target in [
        serde_json::json!({"kind":"replace","identity":{"chapterId":uid(4),"chapterNumber":1},"expectedHash":null,"readReceiptId":uid(9)}),
        serde_json::json!({"kind":"replace","identity":{"chapterId":uid(4),"chapterNumber":1},"expectedHash":sha256_hex(b"a")}),
        serde_json::json!({"kind":"create","identity":{"chapterId":uid(4),"chapterNumber":1},"path":"../bad.md"}),
    ] {
        let mut invalid = value.clone();
        invalid["target"] = target;
        assert!(serde_json::from_value::<ChapterCandidate>(invalid).is_err());
    }
}
#[test]
fn nr501_finalization_requires_final_current_commit_and_current_user() {
    let (session, c) = fixture();
    let doc = ChapterDocument::parse("chapters/x.md", markdown("final").as_bytes()).unwrap();
    let receipt = ChapterCommitReceipt {
        schema_version: 1,
        transaction_id: uid(8),
        scope: c.scope.clone(),
        identity: doc.identity.clone(),
        revision: 1,
        before_hash: None,
        after_hash: doc.content_hash.clone(),
        source: ChapterCommitSource::UserMessage {
            reference: c.source.clone(),
        },
        status: ChapterCommitStatus::Committed,
    };
    let confirmed = ChapterFinalization::confirm(
        receipt.clone(),
        &doc,
        c.scope.clone(),
        c.source.clone(),
        &session,
    )
    .unwrap();
    assert!(confirmed.permits_publication(&receipt, &doc));
    let mut serialized = serde_json::to_value(&receipt).unwrap();
    assert!(serde_json::from_value::<ChapterCommitReceipt>(serialized.clone()).is_ok());
    serialized.as_object_mut().unwrap().remove("beforeHash");
    assert!(serde_json::from_value::<ChapterCommitReceipt>(serialized).is_err());
    for fault in 0..3 {
        let mut invalid = confirmed.clone();
        match fault {
            0 => invalid.confirmation_source.content_hash = "bad".to_owned(),
            1 => invalid.confirmation_source.message_id = uid(0),
            _ => invalid.confirmation_scope.session_id = "bad".to_owned(),
        }
        assert!(!invalid.permits_publication(&receipt, &doc));
    }
    let draft = ChapterDocument::parse("chapters/x.md", markdown("draft").as_bytes()).unwrap();
    assert!(ChapterFinalization::confirm(
        receipt.clone(),
        &draft,
        c.scope.clone(),
        c.source.clone(),
        &session
    )
    .is_err());
    for status in [
        ChapterCommitStatus::Undone,
        ChapterCommitStatus::RecoveryRequired,
    ] {
        let mut bad = receipt.clone();
        bad.status = status;
        assert!(!confirmed.permits_publication(&bad, &doc));
    }
    let mut newer = receipt.clone();
    newer.revision = 2;
    assert!(!confirmed.permits_publication(&newer, &doc));
    let mut invalidated = confirmed.clone();
    invalidated.observe(&c.scope.project_id, &draft);
    invalidated.observe(&c.scope.project_id, &doc);
    assert!(!invalidated.permits_publication(&receipt, &doc));
    let reopened: ChapterFinalization =
        serde_json::from_str(&serde_json::to_string(&invalidated).unwrap()).unwrap();
    assert!(!reopened.permits_publication(&receipt, &doc));
    let mut unchanged = confirmed;
    unchanged.observe(&c.scope.project_id, &doc);
    assert!(unchanged.permits_publication(&receipt, &doc));
}
#[test]
fn nr501_profile_is_separate_from_nr4_runtime_and_bounded() {
    assert_eq!(NR5_AGENT_ATTEMPTS, 8);
    assert_eq!(NR5_REQUESTED_OUTPUT_TOKENS, 8 * 4096 + 1024);
    assert_eq!(crate::canon_change::MAX_NR4_AGENT_ATTEMPTS, 4);
    assert_eq!(crate::canon_change::NR4_CUMULATIVE_OUTPUT_TOKENS, 17408);
    assert_eq!(NR5_TOOL_ARGUMENT_BYTES, 65536);
    assert_eq!(NR5_TOOL_RESULT_BYTES, 65536);
    assert_eq!(NR5_CONTINUATION_BYTES, 524288);
    assert_eq!(NR5_REQUEST_BYTES, 786432);
}
