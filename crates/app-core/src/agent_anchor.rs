use crate::agent_v2::{
    AgentSessionSnapshotV2, ConversationMessageV2, ConversationRoleV2, ConversationStatusV2,
};
use crate::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MAX_ANCHOR_SOURCE_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnchorReferenceV1 {
    pub source_message_id: String,
    pub message_body_hash: String,
    pub start_utf8_byte: Option<u32>,
    pub end_utf8_byte: Option<u32>,
    pub selection_hash: Option<String>,
    pub candidate_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorIntentKindV1 {
    DecisionDeclaration,
    WriteDirective,
    CandidateAcceptance,
    TextSelection,
    AgentSuggestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorTargetKindV1 {
    Character,
    Setting,
    Material,
    Plot,
    Chapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorConfirmationProvenanceV1 {
    ExplicitAuthorMessage,
    UniqueCandidateShortConfirmation,
    RightPanelConfirmation,
    AiSuggestionOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorStateV1 {
    Suggested,
    Confirmed,
    Materialized,
    Committed,
    Discarded,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnchorSourceBindingV1 {
    pub source_message_id: String,
    pub message_body_hash: String,
    pub start_utf8_byte: Option<u32>,
    pub end_utf8_byte: Option<u32>,
    pub selection_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnchorIntentV1 {
    pub schema_version: u16,
    pub anchor_id: String,
    pub project_id: String,
    pub session_id: String,
    pub branch_id: String,
    pub turn_id: String,
    pub intent_kind: AnchorIntentKindV1,
    pub target_kind: AnchorTargetKindV1,
    pub display_name: Option<String>,
    pub source_bindings: Vec<AnchorSourceBindingV1>,
    pub candidate_id: Option<String>,
    pub confirmation_provenance: AnchorConfirmationProvenanceV1,
    pub state: AnchorStateV1,
    pub created_at_ms: u64,
    pub anchor_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AnchorResolutionV1 {
    NoAnchor,
    ClarificationRequired { visible_reason: String },
    Ready { anchor: Box<AnchorIntentV1> },
}

pub struct ResolveAnchorInputV1<'a> {
    pub snapshot: &'a AgentSessionSnapshotV2,
    pub turn_id: &'a str,
    pub current_user_message_id: &'a str,
    pub user_text: &'a str,
    pub reference: Option<&'a AnchorReferenceV1>,
    pub now_ms: u64,
}

pub fn resolve_anchor_intent(
    input: ResolveAnchorInputV1<'_>,
) -> Result<AnchorResolutionV1, AppError> {
    validate_id(input.turn_id, "trn_")?;
    validate_id(input.current_user_message_id, "msg_")?;
    if input.user_text.trim().is_empty() || input.user_text.len() > MAX_ANCHOR_SOURCE_BYTES {
        return Err(anchor_error("agent-anchor.message-invalid"));
    }
    let session_id = input
        .snapshot
        .session_id
        .as_deref()
        .ok_or_else(|| anchor_error("agent-anchor.session-invalid"))?;
    let branch_id = input
        .snapshot
        .active_branch_id
        .as_deref()
        .ok_or_else(|| anchor_error("agent-anchor.branch-invalid"))?;

    if is_exploratory(input.user_text) {
        return Ok(AnchorResolutionV1::NoAnchor);
    }

    let source = match input.reference {
        Some(reference) => validate_reference(input.snapshot, reference)?,
        None => AnchorSourceBindingV1 {
            source_message_id: input.current_user_message_id.to_owned(),
            message_body_hash: sha256(input.user_text.as_bytes()),
            start_utf8_byte: None,
            end_utf8_byte: None,
            selection_hash: None,
        },
    };
    let referenced_message = input.reference.and_then(|reference| {
        input
            .snapshot
            .messages
            .iter()
            .find(|message| message.message_id == reference.source_message_id)
    });

    let short_confirmation = is_short_confirmation(input.user_text);
    let directive = is_write_directive(input.user_text);
    let decision = is_decision_declaration(input.user_text);
    let explicit_selection = source.start_utf8_byte.is_some();
    if !short_confirmation && !directive && !decision && !explicit_selection {
        return Ok(AnchorResolutionV1::NoAnchor);
    }

    if references_unbound_visible_text(input.user_text) && input.reference.is_none() {
        return Ok(AnchorResolutionV1::ClarificationRequired {
            visible_reason: "请先在唯一一条可见消息中选择需要锚定的正文，再说明写入目标。"
                .to_owned(),
        });
    }

    if short_confirmation && referenced_message.is_none() {
        return Ok(AnchorResolutionV1::ClarificationRequired {
            visible_reason: "当前没有唯一、可验证的候选；请说明要确认哪一项。".to_owned(),
        });
    }

    let Some(target_kind) = detect_target(input.user_text)
        .or_else(|| referenced_message.and_then(|message| detect_target(&message.body)))
    else {
        return Ok(AnchorResolutionV1::ClarificationRequired {
            visible_reason: "请说明要固定到人物、世界设定、素材、剧情还是章节正文。".to_owned(),
        });
    };
    let intent_kind = if explicit_selection {
        AnchorIntentKindV1::TextSelection
    } else if short_confirmation {
        AnchorIntentKindV1::CandidateAcceptance
    } else if directive {
        AnchorIntentKindV1::WriteDirective
    } else {
        AnchorIntentKindV1::DecisionDeclaration
    };
    let provenance = if short_confirmation {
        AnchorConfirmationProvenanceV1::UniqueCandidateShortConfirmation
    } else {
        AnchorConfirmationProvenanceV1::ExplicitAuthorMessage
    };
    let mut anchor = AnchorIntentV1 {
        schema_version: 1,
        anchor_id: format!("anc_{}", Uuid::new_v4().simple()),
        project_id: input.snapshot.project_id.clone(),
        session_id: session_id.to_owned(),
        branch_id: branch_id.to_owned(),
        turn_id: input.turn_id.to_owned(),
        intent_kind,
        target_kind,
        display_name: detect_display_name(input.user_text)
            .or_else(|| referenced_message.and_then(|message| detect_display_name(&message.body))),
        source_bindings: vec![source],
        candidate_id: input.reference.and_then(|value| {
            value.candidate_id.clone().or_else(|| {
                short_confirmation
                    .then(|| format!("cand_{}", &sha256(value.message_body_hash.as_bytes())[..24]))
            })
        }),
        confirmation_provenance: provenance,
        state: AnchorStateV1::Confirmed,
        created_at_ms: input.now_ms,
        anchor_hash: String::new(),
    };
    anchor.anchor_hash = hash_anchor(&anchor)?;
    Ok(AnchorResolutionV1::Ready {
        anchor: Box::new(anchor),
    })
}

pub fn create_agent_suggestion(
    snapshot: &AgentSessionSnapshotV2,
    turn_id: &str,
    source_message: &ConversationMessageV2,
    target_kind: AnchorTargetKindV1,
    now_ms: u64,
) -> Result<AnchorIntentV1, AppError> {
    if source_message.role != ConversationRoleV2::Assistant
        || source_message.status != ConversationStatusV2::Complete
        || !snapshot
            .messages
            .iter()
            .any(|message| message.message_id == source_message.message_id)
    {
        return Err(anchor_error("agent-anchor.suggestion-source-invalid"));
    }
    let mut anchor = AnchorIntentV1 {
        schema_version: 1,
        anchor_id: format!("anc_{}", Uuid::new_v4().simple()),
        project_id: snapshot.project_id.clone(),
        session_id: snapshot
            .session_id
            .clone()
            .ok_or_else(|| anchor_error("agent-anchor.session-invalid"))?,
        branch_id: snapshot
            .active_branch_id
            .clone()
            .ok_or_else(|| anchor_error("agent-anchor.branch-invalid"))?,
        turn_id: turn_id.to_owned(),
        intent_kind: AnchorIntentKindV1::AgentSuggestion,
        target_kind,
        display_name: None,
        source_bindings: vec![AnchorSourceBindingV1 {
            source_message_id: source_message.message_id.clone(),
            message_body_hash: source_message.body_hash.clone(),
            start_utf8_byte: None,
            end_utf8_byte: None,
            selection_hash: None,
        }],
        candidate_id: None,
        confirmation_provenance: AnchorConfirmationProvenanceV1::AiSuggestionOnly,
        state: AnchorStateV1::Suggested,
        created_at_ms: now_ms,
        anchor_hash: String::new(),
    };
    anchor.anchor_hash = hash_anchor(&anchor)?;
    Ok(anchor)
}

fn validate_reference(
    snapshot: &AgentSessionSnapshotV2,
    reference: &AnchorReferenceV1,
) -> Result<AnchorSourceBindingV1, AppError> {
    validate_id(&reference.source_message_id, "msg_")?;
    validate_hash(&reference.message_body_hash)?;
    if reference
        .candidate_id
        .as_ref()
        .is_some_and(|candidate| validate_id(candidate, "cand_").is_err())
    {
        return Err(anchor_error("agent-anchor.candidate-invalid"));
    }
    let message = snapshot
        .messages
        .iter()
        .find(|message| message.message_id == reference.source_message_id)
        .ok_or_else(|| anchor_error("agent-anchor.source-missing"))?;
    if message.status != ConversationStatusV2::Complete
        || message.body_hash != reference.message_body_hash
        || sha256(message.body.as_bytes()) != message.body_hash
    {
        return Err(anchor_error("agent-anchor.source-changed"));
    }
    match (
        reference.start_utf8_byte,
        reference.end_utf8_byte,
        reference.selection_hash.as_ref(),
    ) {
        (None, None, None) => {}
        (Some(start), Some(end), Some(expected_hash)) => {
            validate_hash(expected_hash)?;
            let start = start as usize;
            let end = end as usize;
            if start >= end
                || end > message.body.len()
                || !message.body.is_char_boundary(start)
                || !message.body.is_char_boundary(end)
                || sha256(&message.body.as_bytes()[start..end]) != *expected_hash
            {
                return Err(anchor_error("agent-anchor.selection-invalid"));
            }
        }
        _ => return Err(anchor_error("agent-anchor.selection-invalid")),
    }
    Ok(AnchorSourceBindingV1 {
        source_message_id: reference.source_message_id.clone(),
        message_body_hash: reference.message_body_hash.clone(),
        start_utf8_byte: reference.start_utf8_byte,
        end_utf8_byte: reference.end_utf8_byte,
        selection_hash: reference.selection_hash.clone(),
    })
}

fn is_exploratory(text: &str) -> bool {
    ["也许", "要不", "可以考虑", "可能", "暂时想", "先聊聊"]
        .iter()
        .any(|marker| text.contains(marker))
}

fn is_write_directive(text: &str) -> bool {
    [
        "写进",
        "固定到",
        "锚定",
        "落到",
        "存入",
        "收录到",
        "作为第一章",
        "作为章节",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn is_decision_declaration(text: &str) -> bool {
    [
        "就叫",
        "确定为",
        "确定了",
        "定为",
        "就用",
        "决定是",
        "规则确定",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn is_short_confirmation(text: &str) -> bool {
    let text = text.trim().trim_end_matches(['。', '！', '!', '？', '?']);
    text.chars().count() <= 16
        && ["就按这个", "按这个", "确认", "同意", "选第二个", "选第一个"].contains(&text)
}

fn references_unbound_visible_text(text: &str) -> bool {
    ["上面这段", "上文", "刚才那段", "这段正文"]
        .iter()
        .any(|marker| text.contains(marker))
}

fn detect_target(text: &str) -> Option<AnchorTargetKindV1> {
    let candidates: &[(AnchorTargetKindV1, &[&str])] = &[
        (
            AnchorTargetKindV1::Chapter,
            &["章节", "正文", "第一章", "开头", "结尾", "场景"],
        ),
        (
            AnchorTargetKindV1::Character,
            &["人物", "角色", "主角", "配角"],
        ),
        (
            AnchorTargetKindV1::Setting,
            &["世界观", "设定", "规则", "地点", "时代"],
        ),
        (
            AnchorTargetKindV1::Material,
            &["素材", "线索", "意象", "物件", "道具"],
        ),
        (
            AnchorTargetKindV1::Plot,
            &["剧情", "主线", "转折", "情节", "支线"],
        ),
    ];
    candidates
        .iter()
        .find(|(_, markers)| markers.iter().any(|marker| text.contains(marker)))
        .map(|(kind, _)| *kind)
}

fn detect_display_name(text: &str) -> Option<String> {
    let (_, value) = text.split_once("就叫")?;
    let name = value
        .trim()
        .trim_end_matches(['。', '！', '!', '，', ',', '；', ';']);
    if name.is_empty() || name.chars().count() > 80 {
        None
    } else {
        Some(name.to_owned())
    }
}

fn hash_anchor(anchor: &AnchorIntentV1) -> Result<String, AppError> {
    let mut canonical = anchor.clone();
    canonical.anchor_hash.clear();
    serde_json::to_vec(&canonical)
        .map(|bytes| sha256(&bytes))
        .map_err(|_| anchor_error("agent-anchor.hash-invalid"))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_hash(value: &str) -> Result<(), AppError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(anchor_error("agent-anchor.hash-invalid"))
    }
}

fn validate_id(value: &str, prefix: &str) -> Result<(), AppError> {
    if value.len() >= prefix.len() + 8
        && value.len() <= 96
        && value.starts_with(prefix)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Ok(())
    } else {
        Err(anchor_error("agent-anchor.binding-invalid"))
    }
}

fn anchor_error(code: &str) -> AppError {
    AppError::agent_contract(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_v2::{AgentHistoryBranchV2, AgentHistoryHashStatusV2};

    fn snapshot(body: &str) -> AgentSessionSnapshotV2 {
        let hash = sha256(body.as_bytes());
        AgentSessionSnapshotV2 {
            schema_version: 2,
            project_id: "project-test".to_owned(),
            history_consent_required: false,
            branch_selection_required: false,
            active_branch_id: Some("brn_12345678".to_owned()),
            session_id: Some("ses_12345678".to_owned()),
            branches: vec![AgentHistoryBranchV2 {
                branch_id: "brn_12345678".to_owned(),
                session_id: "ses_12345678".to_owned(),
                last_turn_at_ms: Some(1),
                message_count: 1,
                hash_status: AgentHistoryHashStatusV2::Valid,
            }],
            messages: vec![ConversationMessageV2 {
                schema_version: 2,
                message_id: "msg_12345678".to_owned(),
                response_id: Some("rsp_12345678".to_owned()),
                turn_id: "trn_12345678".to_owned(),
                role: ConversationRoleV2::Assistant,
                body: body.to_owned(),
                body_hash: hash,
                source_ids: Vec::new(),
                status: ConversationStatusV2::Complete,
                created_at_ms: 1,
            }],
            memory: None,
            draft_quota: 0,
            write_policy: novelgainker_agent::contracts::AuthorizationPolicyV1::AutoApplySafe,
            write_policy_hash: Some("a".repeat(64)),
            context_marks: crate::agent_marks::ContextMarkStoreV1 {
                schema_version: 1,
                project_id: "project-test".to_owned(),
                marks: Vec::new(),
                store_hash: "a".repeat(64),
            },
        }
    }

    fn resolve(text: &str, reference: Option<&AnchorReferenceV1>) -> AnchorResolutionV1 {
        resolve_anchor_intent(ResolveAnchorInputV1 {
            snapshot: &snapshot("林舟推开门，潮声越过城墙。"),
            turn_id: "trn_87654321",
            current_user_message_id: "msg_87654321",
            user_text: text,
            reference,
            now_ms: 2,
        })
        .unwrap()
    }

    #[test]
    fn accepts_explicit_decision_without_forcing_a_second_confirmation() {
        let AnchorResolutionV1::Ready { anchor } = resolve("主角就叫林舟", None) else {
            panic!("explicit decision must anchor");
        };
        assert_eq!(anchor.intent_kind, AnchorIntentKindV1::DecisionDeclaration);
        assert_eq!(anchor.target_kind, AnchorTargetKindV1::Character);
        assert_eq!(anchor.display_name.as_deref(), Some("林舟"));
        assert_eq!(anchor.state, AnchorStateV1::Confirmed);
        assert_eq!(anchor.anchor_hash.len(), 64);
    }

    #[test]
    fn exploratory_language_remains_conversation_only() {
        assert_eq!(
            resolve("也许主角可以叫林舟", None),
            AnchorResolutionV1::NoAnchor
        );
    }

    #[test]
    fn ambiguous_visible_text_reference_requires_clarification() {
        assert!(matches!(
            resolve("把上面这段作为第一章开头", None),
            AnchorResolutionV1::ClarificationRequired { .. }
        ));
    }

    #[test]
    fn validates_utf8_byte_selection_and_hash() {
        let body = "林舟推开门，潮声越过城墙。";
        let start = body.find('潮').unwrap();
        let end = body.len() - '。'.len_utf8();
        let reference = AnchorReferenceV1 {
            source_message_id: "msg_12345678".to_owned(),
            message_body_hash: sha256(body.as_bytes()),
            start_utf8_byte: Some(start as u32),
            end_utf8_byte: Some(end as u32),
            selection_hash: Some(sha256(&body.as_bytes()[start..end])),
            candidate_id: None,
        };
        let AnchorResolutionV1::Ready { anchor } =
            resolve("把这段正文锚定为第一章开头", Some(&reference))
        else {
            panic!("valid selection must anchor");
        };
        assert_eq!(anchor.intent_kind, AnchorIntentKindV1::TextSelection);
        assert_eq!(anchor.target_kind, AnchorTargetKindV1::Chapter);
    }

    #[test]
    fn rejects_split_utf8_and_changed_message_hash() {
        let body = "林舟推开门，潮声越过城墙。";
        let broken = AnchorReferenceV1 {
            source_message_id: "msg_12345678".to_owned(),
            message_body_hash: sha256(body.as_bytes()),
            start_utf8_byte: Some(1),
            end_utf8_byte: Some(4),
            selection_hash: Some(sha256(&body.as_bytes()[1..4])),
            candidate_id: None,
        };
        let result = resolve_anchor_intent(ResolveAnchorInputV1 {
            snapshot: &snapshot(body),
            turn_id: "trn_87654321",
            current_user_message_id: "msg_87654321",
            user_text: "把这段正文锚定为第一章开头",
            reference: Some(&broken),
            now_ms: 2,
        });
        assert_eq!(result.unwrap_err().code, "agent-anchor.selection-invalid");
    }

    #[test]
    fn ai_suggestion_never_self_confirms() {
        let snapshot = snapshot("可以让林舟在海禁日第一次越墙。");
        let anchor = create_agent_suggestion(
            &snapshot,
            "trn_87654321",
            &snapshot.messages[0],
            AnchorTargetKindV1::Plot,
            2,
        )
        .unwrap();
        assert_eq!(anchor.state, AnchorStateV1::Suggested);
        assert_eq!(
            anchor.confirmation_provenance,
            AnchorConfirmationProvenanceV1::AiSuggestionOnly
        );
    }

    #[test]
    fn short_confirmation_can_bind_one_explicitly_selected_visible_message() {
        let body = "建议固定为角色设定：主角林舟是雾港的守灯人。";
        let reference = AnchorReferenceV1 {
            source_message_id: "msg_12345678".to_owned(),
            message_body_hash: sha256(body.as_bytes()),
            start_utf8_byte: None,
            end_utf8_byte: None,
            selection_hash: None,
            candidate_id: None,
        };
        let AnchorResolutionV1::Ready { anchor } = resolve_anchor_intent(ResolveAnchorInputV1 {
            snapshot: &snapshot(body),
            turn_id: "trn_87654321",
            current_user_message_id: "msg_87654321",
            user_text: "就按这个",
            reference: Some(&reference),
            now_ms: 2,
        })
        .unwrap() else {
            panic!("explicitly selected visible message must be a unique candidate");
        };
        assert_eq!(anchor.intent_kind, AnchorIntentKindV1::CandidateAcceptance);
        assert_eq!(anchor.target_kind, AnchorTargetKindV1::Character);
        assert_eq!(
            anchor.confirmation_provenance,
            AnchorConfirmationProvenanceV1::UniqueCandidateShortConfirmation
        );
        assert!(anchor
            .candidate_id
            .as_deref()
            .is_some_and(|id| id.starts_with("cand_")));
    }
}
