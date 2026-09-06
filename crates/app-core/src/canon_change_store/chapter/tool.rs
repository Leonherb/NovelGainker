//! Strict v2 extension of the existing propose_change tool. No filesystem path
//! or executable authority can be supplied by the Provider.
use super::*;
use crate::protocol_profile::ToolProtocolProfile;
use serde_json::json;
use sha2::{Digest, Sha256};

pub(crate) fn definition(profile: ToolProtocolProfile) -> crate::tool_runtime::ToolDefinition {
    let mut tool = propose_change_definition();
    if profile == ToolProtocolProfile::Nr4 {
        return tool;
    }
    tool.version = 2;
    tool.display_label = "保存章节或维护资料".to_owned();
    tool.description = "提交有来源的小说资料或单章变更。先读能力方法；更新须完整原文回执和旧 Hash。章节用 entityKind=chapter、capabilityId=novel-story-telling、chapterIntent=write_and_save；新建 id: new，三个目标字段 null；更新使用完整读取返回的 targetId/readReceiptId/expectedSourceHash，保持章节 ID 与章号。章节 Markdown 不超过 32768 字节，一次仅一章；明确要求定稿才设 finalize=true。五类资料沿用最多四项、8192 字节，chapterIntent/finalize/candidateId 均 null。讨论或试写不调用写工具；候选接受须 Host 已展示凭证。只有 committed 回执表示实际保存，出错不阻止继续聊天。".to_owned();
    let properties = &mut tool.input_schema["properties"];
    properties["schemaVersion"]["enum"] = json!([2]);
    properties["operations"]["items"]["properties"]["entityKind"]["enum"] = json!([
        "character",
        "world",
        "material",
        "plot",
        "outline",
        "chapter"
    ]);
    properties["chapterIntent"] = json!({"type":["string","null"],"enum":["discussion","trial_draft","write_and_save","accept_candidate","ambiguous",null]});
    properties["finalize"] = json!({"type":["boolean","null"]});
    properties["candidateId"] = json!({"type":["string","null"]});
    tool.input_schema["required"] = json!([
        "schemaVersion",
        "sourceKind",
        "sources",
        "risk",
        "operations",
        "chapterIntent",
        "finalize",
        "candidateId"
    ]);
    tool
}

pub(crate) enum Request {
    Canon(Value),
    Chapter(Box<PreparedChapter>),
}

pub(crate) struct PreparedChapter {
    candidate: ChapterCandidate,
    method: String,
    finalize: bool,
    confirmation: bool,
    displayed: Option<ChapterCandidate>,
}

impl PreparedChapter {
    pub(crate) fn submit(
        &self,
        coordinator: &CanonChangeCoordinator,
        evidence: &ChangeEvidence,
        call: &str,
        cancellation: &CancellationToken,
    ) -> Result<ChangeReceipt, ChangeError> {
        coordinator.submit_chapter(
            evidence,
            ChapterSubmission {
                candidate: &self.candidate,
                method_receipt_id: &self.method,
                finalize: self.finalize,
                confirmation_required: self.confirmation,
                displayed: self.displayed.as_ref(),
            },
            call,
            cancellation,
        )
    }
}

pub(crate) fn decode(
    evidence: &ChangeEvidence,
    value: &Value,
    coordinator: &CanonChangeCoordinator,
) -> Result<Request, ChangeError> {
    definition(ToolProtocolProfile::Nr5)
        .validate_arguments(value)
        .map_err(|_| ChangeError::InvalidRequest)?;
    let mut legacy = value.clone();
    let root = legacy.as_object_mut().ok_or(ChangeError::InvalidRequest)?;
    let intent_value = root
        .remove("chapterIntent")
        .ok_or(ChangeError::InvalidRequest)?;
    let finalize_value = root.remove("finalize").ok_or(ChangeError::InvalidRequest)?;
    let candidate_value = root
        .remove("candidateId")
        .ok_or(ChangeError::InvalidRequest)?;
    root.insert("schemaVersion".to_owned(), json!(1));
    let request: ProposeChangeRequest =
        serde_json::from_value(legacy.clone()).map_err(|_| ChangeError::InvalidRequest)?;
    if !request
        .operations
        .iter()
        .any(|op| op.entity_kind == CanonEntityKind::Chapter)
    {
        if !intent_value.is_null() || !finalize_value.is_null() || !candidate_value.is_null() {
            return Err(ChangeError::InvalidRequest);
        }
        evidence.validate_request_nr5(&legacy)?;
        return Ok(Request::Canon(legacy));
    }
    if request.operations.len() != 1
        || request.sources.len() != 1
        || request.source_kind != ChangeSourceKind::UserDecision
        || !evidence.current_user_source(&request.sources[0])
    {
        return Err(ChangeError::SourceInvalid);
    }
    let intent: ChapterIntent =
        serde_json::from_value(intent_value).map_err(|_| ChangeError::InvalidRequest)?;
    let displayed = match intent {
        ChapterIntent::WriteAndSave if candidate_value.is_null() => None,
        ChapterIntent::AcceptCandidate => Some(
            coordinator.displayed_chapter_candidate(
                candidate_value
                    .as_str()
                    .ok_or(ChangeError::CandidateInvalid)?,
                &evidence.chapter_scope().session_id,
            )?,
        ),
        _ => return Err(ChangeError::CandidateInvalid),
    };
    let finalize = finalize_value
        .as_bool()
        .ok_or(ChangeError::InvalidRequest)?;
    let op = &request.operations[0];
    if op.capability_id != "novel-story-telling"
        || !evidence.has_chapter_method(&op.method_receipt_id)
    {
        return Err(ChangeError::MethodNotRead);
    }
    let mut markdown = op.proposed_markdown.clone();
    // Stable draft-only placeholder; the transaction replaces this with a fresh
    // Host UUID. It must be deterministic so replaying the same call is idempotent.
    let creating = op.kind == MutationKind::Create;
    if creating {
        if op.target_id.is_some()
            || op.read_receipt_id.is_some()
            || op.expected_source_hash.is_some()
            || displayed.is_none() && metadata_field(&markdown, "id").as_deref() != Some("new")
        {
            return Err(ChangeError::TargetInvalid);
        }
        if displayed.is_none() {
            markdown = replace_field(&markdown, "id", "00000000-0000-4000-8000-000000000001")?;
        }
    } else if op.kind != MutationKind::Update {
        return Err(ChangeError::TargetInvalid);
    }
    let document = ChapterDocument::parse("chapters/candidate.md", markdown.as_bytes())
        .map_err(|_| ChangeError::CandidateInvalid)?;
    let target = if creating {
        ChapterTarget::Create {
            identity: document.identity,
        }
    } else {
        let receipt_id = op
            .read_receipt_id
            .as_deref()
            .ok_or(ChangeError::IncompleteRead)?;
        let read = evidence
            .chapter_read(receipt_id)
            .ok_or(ChangeError::IncompleteRead)?;
        if !evidence.chapter_target_matches(
            receipt_id,
            op.target_id.as_deref().ok_or(ChangeError::TargetInvalid)?,
        ) || read.identity != document.identity
            || op.expected_source_hash.as_deref() != Some(&read.content_hash)
        {
            return Err(ChangeError::SourceChanged);
        }
        ChapterTarget::Replace {
            identity: document.identity,
            expected_hash: read.content_hash,
            read_receipt_id: receipt_id.to_owned(),
        }
    };
    let digest =
        Sha256::digest(serde_json::to_vec(value).map_err(|_| ChangeError::InvalidRequest)?);
    if displayed
        .as_ref()
        .is_some_and(|old| old.proposed_markdown != markdown || !old.target.same_version(&target))
    {
        return Err(ChangeError::CandidateInvalid);
    }
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Request::Chapter(Box::new(PreparedChapter {
        candidate: ChapterCandidate {
            schema_version: 1,
            candidate_id: displayed
                .as_ref()
                .map(|old| old.candidate_id.clone())
                .unwrap_or_else(|| Uuid::from_bytes(bytes).to_string()),
            scope: evidence.chapter_scope(),
            source: request.sources[0].clone(),
            intent,
            target,
            content_hash: sha256_hex(markdown.as_bytes()),
            proposed_markdown: markdown,
            complete: true,
            discarded: false,
        },
        method: op.method_receipt_id.clone(),
        finalize,
        confirmation: request.risk != ChangeRisk::Low,
        displayed,
    })))
}
