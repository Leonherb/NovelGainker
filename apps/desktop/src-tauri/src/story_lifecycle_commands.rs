use super::*;
use crate::commands::story_chapter_patch_commands::load_completed_proposal;
use crate::commands::workflow_proposal_commands::{
    map_workflow_apply_error, mark_session_recovery_required, workflow_context_handle,
};
use app_core::canon_apply::{
    apply_chapter_file_recoverable, encode_chapter_target_bytes, ChapterFileApplyResult,
};
use app_core::workspace::Utf8Bom;
use novelgainker_workflows::chapter::ChapterProposalV1;
use novelgainker_workflows::chapter_lifecycle::{
    bind_chapter_status_encoded_hash, bind_ledger_update_encoded_hash,
    build_chapter_status_transition, build_ledger_update, load_chapter_lifecycle_status,
    load_ledger_pending_status, BuildChapterStatusInputV1, BuildLedgerUpdateInputV1,
    ChapterLifecycleError, ChapterLifecycleStatusV1, ChapterStatusDocumentV1,
    ChapterStatusPreviewV1, LedgerPendingStatusV1, LedgerUpdateDocumentV1, LedgerUpdatePreviewV1,
    CHAPTER_LIFECYCLE_SCHEMA_VERSION, STORY_LEDGER_RELATIVE_PATH,
};
use novelgainker_workflows::chapter_patch::ChapterPatchOperationV1;
use novelgainker_workflows::story_context::sha256_hex;

#[derive(Clone)]
pub(super) struct StoredStoryChapterLifecycleV1 {
    target_relative_path: String,
    status_document: Option<ChapterStatusDocumentV1>,
    status_proposal: Option<ChapterProposalV1>,
    status_applied: Option<ChapterFileApplyResult>,
    ledger_document: Option<LedgerUpdateDocumentV1>,
    ledger_applied: Option<ChapterFileApplyResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryChapterStatePreviewRequestV1 {
    schema_version: u16,
    target_relative_path: String,
    next_status: ChapterLifecycleStatusV1,
    run_id: Option<String>,
    proposal_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryChapterStateApplyRequestV1 {
    schema_version: u16,
    preview_hash: String,
    confirmed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryLedgerUpdatePreviewRequestV1 {
    schema_version: u16,
    chapter_relative_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryLedgerUpdateApplyRequestV1 {
    schema_version: u16,
    preview_hash: String,
    confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryChapterLifecycleStatusV1 {
    schema_version: u16,
    target_relative_path: String,
    chapter_status: ChapterLifecycleStatusV1,
    ledger_pending: Vec<LedgerPendingStatusV1>,
    status_preview: Option<ChapterStatusPreviewV1>,
    status_applied: Option<ChapterFileApplyResult>,
    ledger_preview: Option<LedgerUpdatePreviewV1>,
    ledger_applied: Option<ChapterFileApplyResult>,
}

pub fn story_chapter_state_preview(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryChapterStatePreviewRequestV1,
) -> Result<ChapterStatusPreviewV1, AppError> {
    preview_registered_chapter_state(&state, &session_id, input)
}

pub fn story_chapter_lifecycle_status_load(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    target_relative_path: String,
) -> Result<StoryChapterLifecycleStatusV1, AppError> {
    let handle = session_handle(&state, &session_id)?;
    let chapter =
        read_project_file(&handle, &target_relative_path).map_err(map_workflow_apply_error)?;
    let ledger =
        read_project_file(&handle, STORY_LEDGER_RELATIVE_PATH).map_err(map_workflow_apply_error)?;
    let chapter_status = load_chapter_lifecycle_status(&target_relative_path, &chapter.content)
        .map_err(map_lifecycle_error)?;
    let ledger_pending =
        load_ledger_pending_status(&target_relative_path, &chapter.content, &ledger.content)
            .map_err(map_lifecycle_error)?;
    let stored = state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(&session_id)
        .filter(|item| item.target_relative_path == target_relative_path)
        .cloned();
    Ok(StoryChapterLifecycleStatusV1 {
        schema_version: CHAPTER_LIFECYCLE_SCHEMA_VERSION,
        target_relative_path,
        chapter_status,
        ledger_pending,
        status_preview: stored
            .as_ref()
            .and_then(|item| item.status_document.as_ref())
            .map(|item| item.preview.clone()),
        status_applied: stored.as_ref().and_then(|item| item.status_applied.clone()),
        ledger_preview: stored
            .as_ref()
            .and_then(|item| item.ledger_document.as_ref())
            .map(|item| item.preview.clone()),
        ledger_applied: stored.and_then(|item| item.ledger_applied),
    })
}

pub fn story_chapter_state_apply(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryChapterStateApplyRequestV1,
) -> Result<ChapterFileApplyResult, AppError> {
    apply_registered_chapter_state(&state, &session_id, input)
}

pub fn story_ledger_update_preview(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryLedgerUpdatePreviewRequestV1,
) -> Result<LedgerUpdatePreviewV1, AppError> {
    preview_registered_ledger_update(&state, &session_id, input)
}

pub fn story_ledger_update_apply(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryLedgerUpdateApplyRequestV1,
) -> Result<ChapterFileApplyResult, AppError> {
    apply_registered_ledger_update(&state, &session_id, input)
}

fn preview_registered_chapter_state(
    state: &ProjectRegistry,
    session_id: &str,
    input: StoryChapterStatePreviewRequestV1,
) -> Result<ChapterStatusPreviewV1, AppError> {
    if input.schema_version != CHAPTER_LIFECYCLE_SCHEMA_VERSION {
        return Err(AppError::workflow_input_invalid());
    }
    let handle = session_handle(state, session_id)?;
    let chapter = read_project_file(&handle, &input.target_relative_path)
        .map_err(map_workflow_apply_error)?;
    let proposal = if input.next_status == ChapterLifecycleStatusV1::Accepted {
        let run_id = input
            .run_id
            .as_deref()
            .ok_or_else(AppError::workflow_invalid_state)?;
        let proposal_hash = input
            .proposal_hash
            .as_deref()
            .ok_or_else(AppError::workflow_invalid_state)?;
        let patch = state
            .story_chapter_patches
            .lock()
            .map_err(|_| AppError::workflow_unavailable())?
            .get(session_id)
            .filter(|patch| {
                patch.run_id == run_id
                    && patch.document.preview.proposal_hash == proposal_hash
                    && patch.document.preview.target_relative_path == input.target_relative_path
                    && patch.applied.is_some()
            })
            .cloned()
            .ok_or_else(AppError::workflow_invalid_state)?;
        let (_, proposal) = load_completed_proposal(state, session_id, run_id, proposal_hash)?;
        let _ = patch;
        Some(proposal)
    } else {
        None
    };
    let mut document = build_chapter_status_transition(BuildChapterStatusInputV1 {
        target_relative_path: &input.target_relative_path,
        current_content: &chapter.content,
        current_content_hash: &chapter.content_hash,
        current_content_has_utf8_bom: chapter.bom == Utf8Bom::Utf8,
        next_status: input.next_status,
        proposal: proposal.as_ref(),
        pending_id: None,
    })
    .map_err(map_lifecycle_error)?;
    let encoded =
        encode_chapter_target_bytes(&document.target_content, chapter.bom, chapter.line_ending)
            .map_err(map_workflow_apply_error)?;
    bind_chapter_status_encoded_hash(&mut document, &sha256_hex(&encoded))
        .map_err(map_lifecycle_error)?;
    let preview = document.preview.clone();
    state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .insert(
            session_id.to_owned(),
            StoredStoryChapterLifecycleV1 {
                target_relative_path: input.target_relative_path,
                status_document: Some(document),
                status_proposal: proposal,
                status_applied: None,
                ledger_document: None,
                ledger_applied: None,
            },
        );
    Ok(preview)
}

fn apply_registered_chapter_state(
    state: &ProjectRegistry,
    session_id: &str,
    input: StoryChapterStateApplyRequestV1,
) -> Result<ChapterFileApplyResult, AppError> {
    if input.schema_version != CHAPTER_LIFECYCLE_SCHEMA_VERSION || !input.confirmed {
        return Err(AppError::workflow_confirmation_required());
    }
    let handle = workflow_context_handle(state, session_id).map_err(map_workflow_apply_error)?;
    let stored = state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(session_id)
        .filter(|item| {
            item.status_document
                .as_ref()
                .is_some_and(|document| document.preview.preview_hash == input.preview_hash)
                && item.status_applied.is_none()
        })
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    if state.app_state.current_file(handle.project_id()).as_deref()
        == Some(stored.target_relative_path.as_str())
    {
        return Err(AppError::workflow_target_conflict());
    }
    let original = stored
        .status_document
        .as_ref()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let chapter = read_project_file(&handle, &stored.target_relative_path)
        .map_err(map_workflow_apply_error)?;
    let pending_id = original
        .preview
        .created_pending_id
        .as_deref()
        .and_then(|item| item.strip_prefix("ledger_pending_"))
        .and_then(|item| Uuid::parse_str(item).ok());
    let mut revalidated = build_chapter_status_transition(BuildChapterStatusInputV1 {
        target_relative_path: &stored.target_relative_path,
        current_content: &chapter.content,
        current_content_hash: &chapter.content_hash,
        current_content_has_utf8_bom: chapter.bom == Utf8Bom::Utf8,
        next_status: original.preview.next_status,
        proposal: stored.status_proposal.as_ref(),
        pending_id,
    })
    .map_err(map_lifecycle_error)?;
    let encoded = encode_chapter_target_bytes(
        &revalidated.target_content,
        chapter.bom,
        chapter.line_ending,
    )
    .map_err(map_workflow_apply_error)?;
    bind_chapter_status_encoded_hash(&mut revalidated, &sha256_hex(&encoded))
        .map_err(map_lifecycle_error)?;
    if &revalidated != original {
        return Err(AppError::workflow_context_changed());
    }
    let applied = apply_recoverable_replace(
        state,
        session_id,
        &handle,
        &revalidated.preview.target_relative_path,
        &revalidated.preview.expected_source_hash,
        &revalidated.preview.proposed_content_hash,
        &encoded,
    )?;
    let mut lifecycles = state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let current = lifecycles
        .get_mut(session_id)
        .filter(|item| {
            item.status_document
                .as_ref()
                .is_some_and(|document| document.preview.preview_hash == input.preview_hash)
        })
        .ok_or_else(AppError::workflow_invalid_state)?;
    current.status_applied = Some(applied.clone());
    Ok(applied)
}

fn preview_registered_ledger_update(
    state: &ProjectRegistry,
    session_id: &str,
    input: StoryLedgerUpdatePreviewRequestV1,
) -> Result<LedgerUpdatePreviewV1, AppError> {
    if input.schema_version != CHAPTER_LIFECYCLE_SCHEMA_VERSION {
        return Err(AppError::workflow_input_invalid());
    }
    let handle = session_handle(state, session_id)?;
    let chapter = read_project_file(&handle, &input.chapter_relative_path)
        .map_err(map_workflow_apply_error)?;
    let ledger =
        read_project_file(&handle, STORY_LEDGER_RELATIVE_PATH).map_err(map_workflow_apply_error)?;
    let mut document = build_ledger_update(BuildLedgerUpdateInputV1 {
        chapter_relative_path: &input.chapter_relative_path,
        chapter_content: &chapter.content,
        chapter_content_hash: &chapter.content_hash,
        chapter_content_has_utf8_bom: chapter.bom == Utf8Bom::Utf8,
        ledger_content: &ledger.content,
        ledger_content_hash: &ledger.content_hash,
        ledger_content_has_utf8_bom: ledger.bom == Utf8Bom::Utf8,
    })
    .map_err(map_lifecycle_error)?;
    let encoded =
        encode_chapter_target_bytes(&document.target_content, ledger.bom, ledger.line_ending)
            .map_err(map_workflow_apply_error)?;
    bind_ledger_update_encoded_hash(&mut document, &sha256_hex(&encoded))
        .map_err(map_lifecycle_error)?;
    let preview = document.preview.clone();
    let mut lifecycles = state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let entry =
        lifecycles
            .entry(session_id.to_owned())
            .or_insert_with(|| StoredStoryChapterLifecycleV1 {
                target_relative_path: input.chapter_relative_path.clone(),
                status_document: None,
                status_proposal: None,
                status_applied: None,
                ledger_document: None,
                ledger_applied: None,
            });
    entry.target_relative_path = input.chapter_relative_path;
    entry.ledger_document = Some(document);
    entry.ledger_applied = None;
    Ok(preview)
}

fn apply_registered_ledger_update(
    state: &ProjectRegistry,
    session_id: &str,
    input: StoryLedgerUpdateApplyRequestV1,
) -> Result<ChapterFileApplyResult, AppError> {
    if input.schema_version != CHAPTER_LIFECYCLE_SCHEMA_VERSION || !input.confirmed {
        return Err(AppError::workflow_confirmation_required());
    }
    let handle = workflow_context_handle(state, session_id).map_err(map_workflow_apply_error)?;
    let stored = state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(session_id)
        .filter(|item| {
            item.ledger_document
                .as_ref()
                .is_some_and(|document| document.preview.preview_hash == input.preview_hash)
                && item.ledger_applied.is_none()
        })
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let current_file = state.app_state.current_file(handle.project_id());
    if matches!(
        current_file.as_deref(),
        Some(path) if path == STORY_LEDGER_RELATIVE_PATH || path == stored.target_relative_path
    ) {
        return Err(AppError::workflow_target_conflict());
    }
    let original = stored
        .ledger_document
        .as_ref()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let chapter = read_project_file(&handle, &stored.target_relative_path)
        .map_err(map_workflow_apply_error)?;
    let ledger =
        read_project_file(&handle, STORY_LEDGER_RELATIVE_PATH).map_err(map_workflow_apply_error)?;
    let mut revalidated = build_ledger_update(BuildLedgerUpdateInputV1 {
        chapter_relative_path: &stored.target_relative_path,
        chapter_content: &chapter.content,
        chapter_content_hash: &chapter.content_hash,
        chapter_content_has_utf8_bom: chapter.bom == Utf8Bom::Utf8,
        ledger_content: &ledger.content,
        ledger_content_hash: &ledger.content_hash,
        ledger_content_has_utf8_bom: ledger.bom == Utf8Bom::Utf8,
    })
    .map_err(map_lifecycle_error)?;
    let encoded =
        encode_chapter_target_bytes(&revalidated.target_content, ledger.bom, ledger.line_ending)
            .map_err(map_workflow_apply_error)?;
    bind_ledger_update_encoded_hash(&mut revalidated, &sha256_hex(&encoded))
        .map_err(map_lifecycle_error)?;
    if &revalidated != original {
        return Err(AppError::workflow_context_changed());
    }
    let applied = apply_recoverable_replace(
        state,
        session_id,
        &handle,
        STORY_LEDGER_RELATIVE_PATH,
        &revalidated.preview.expected_source_hash,
        &revalidated.preview.proposed_content_hash,
        &encoded,
    )?;
    let mut lifecycles = state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let current = lifecycles
        .get_mut(session_id)
        .filter(|item| {
            item.ledger_document
                .as_ref()
                .is_some_and(|document| document.preview.preview_hash == input.preview_hash)
        })
        .ok_or_else(AppError::workflow_invalid_state)?;
    current.ledger_applied = Some(applied.clone());
    Ok(applied)
}

fn apply_recoverable_replace(
    state: &ProjectRegistry,
    session_id: &str,
    handle: &ProjectHandle,
    relative_path: &str,
    expected_hash: &str,
    proposed_hash: &str,
    encoded: &[u8],
) -> Result<ChapterFileApplyResult, AppError> {
    let applied = apply_chapter_file_recoverable(
        handle,
        ChapterPatchOperationV1::Replace,
        relative_path,
        Some(expected_hash),
        encoded,
        &state.recovery_store,
    );
    let applied = match applied {
        Ok(value) => value,
        Err(error) => {
            if error.code == "project.recovery_required" {
                mark_session_recovery_required(state, session_id)?;
            }
            return Err(map_workflow_apply_error(error));
        }
    };
    if applied.file.content_hash != proposed_hash
        || applied.file.validation.content_hash != proposed_hash
        || !applied.file.validation.eligible_for_ai_apply
        || !applied.file.validation.diagnostics.is_empty()
    {
        mark_session_recovery_required(state, session_id)?;
        return Err(AppError::workflow_recovery_required());
    }
    Ok(applied)
}

fn map_lifecycle_error(error: ChapterLifecycleError) -> AppError {
    match error {
        ChapterLifecycleError::SourceHashMismatch => {
            AppError::workflow_target_conflict().with_diagnostic(error.code())
        }
        ChapterLifecycleError::ProposalBindingMismatch => {
            AppError::workflow_context_changed().with_diagnostic(error.code())
        }
        ChapterLifecycleError::TransitionForbidden
        | ChapterLifecycleError::ProposalRequired
        | ChapterLifecycleError::NoPending => {
            AppError::workflow_invalid_state().with_diagnostic(error.code())
        }
        ChapterLifecycleError::InvalidTarget
        | ChapterLifecycleError::InvalidChapter
        | ChapterLifecycleError::InvalidLedger
        | ChapterLifecycleError::PendingInvalid
        | ChapterLifecycleError::InvalidHash => {
            AppError::workflow_validation_failed().with_diagnostic(error.code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ng_506_status_contract_exposes_pending_and_receipts_without_hidden_bodies() {
        let status = StoryChapterLifecycleStatusV1 {
            schema_version: 1,
            target_relative_path: "chapters/003.md".to_owned(),
            chapter_status: ChapterLifecycleStatusV1::Draft,
            ledger_pending: Vec::new(),
            status_preview: None,
            status_applied: None,
            ledger_preview: None,
            ledger_applied: None,
        };
        let serialized = serde_json::to_value(status).unwrap();
        assert_eq!(serialized["schemaVersion"], 1);
        assert_eq!(serialized["chapterStatus"], "draft");
        assert_eq!(serialized["ledgerPending"], serde_json::json!([]));
        assert!(!serialized.to_string().contains("targetContent"));
        assert!(!serialized.to_string().contains("proposal"));
    }

    #[test]
    fn ng_506_apply_inputs_require_separate_explicit_confirmations() {
        let status: StoryChapterStateApplyRequestV1 = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "previewHash": "a".repeat(64),
            "confirmed": true
        }))
        .unwrap();
        let ledger: StoryLedgerUpdateApplyRequestV1 = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "previewHash": "b".repeat(64),
            "confirmed": true
        }))
        .unwrap();
        assert!(status.confirmed);
        assert!(ledger.confirmed);
        assert_ne!(status.preview_hash, ledger.preview_hash);
        assert!(
            serde_json::from_value::<StoryLedgerUpdateApplyRequestV1>(serde_json::json!({
                "schemaVersion": 1,
                "previewHash": "b".repeat(64),
                "confirmed": true,
                "chapterPreviewHash": "a".repeat(64)
            }))
            .is_err()
        );
    }
}
