use super::*;
use crate::commands::story_context_commands::StoryContextRequestPreviewRequestV1;
use crate::commands::story_continuity_commands::{
    load_story_target_state, rebuild_registered_story_continuity,
};
use crate::commands::workflow_proposal_commands::{
    map_workflow_apply_error, mark_session_recovery_required, workflow_context_handle,
};
use app_core::canon_apply::{
    apply_chapter_file_recoverable, encode_chapter_target_bytes, ChapterFileApplyResult,
};
use app_core::workspace::{LineEnding, Utf8Bom};
use novelgainker_workflows::chapter_patch::{
    bind_chapter_patch_encoded_hash, build_chapter_patch, BuildChapterPatchInputV1,
    ChapterPatchDocumentV1, ChapterPatchError, ChapterPatchPreviewV1, CHAPTER_PATCH_SCHEMA_VERSION,
};
use novelgainker_workflows::continuity::{
    confirm_story_continuity_warnings, StoryContinuityReportV1,
};
use novelgainker_workflows::story_context::{sha256_hex, StoryActionV2};

#[derive(Clone)]
pub(super) struct StoredStoryChapterPatchV1 {
    pub(super) run_id: String,
    request: StoryContextRequestPreviewRequestV1,
    warning_rule_ids: Vec<String>,
    unknown_rule_ids: Vec<String>,
    pub(super) document: ChapterPatchDocumentV1,
    pub(super) applied: Option<ChapterFileApplyResult>,
}

impl std::fmt::Debug for StoredStoryChapterPatchV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredStoryChapterPatchV1")
            .field("run_id", &self.run_id)
            .field("request", &"[REDACTED]")
            .field("warning_rule_ids", &self.warning_rule_ids)
            .field("unknown_rule_ids", &self.unknown_rule_ids)
            .field("document", &self.document)
            .field("applied", &self.applied)
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryChapterPatchPreviewRequestV1 {
    schema_version: u16,
    run_id: String,
    proposal_hash: String,
    continuity_report_hash: String,
    request: StoryContextRequestPreviewRequestV1,
    accepted_or_final_unlock_confirmed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryChapterPatchApplyRequestV1 {
    schema_version: u16,
    run_id: String,
    proposal_hash: String,
    continuity_report_hash: String,
    preview_hash: String,
    confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryChapterPatchStatusV1 {
    schema_version: u16,
    preview: Option<ChapterPatchPreviewV1>,
    applied: Option<ChapterFileApplyResult>,
}

pub fn story_chapter_patch_preview(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryChapterPatchPreviewRequestV1,
) -> Result<ChapterPatchPreviewV1, AppError> {
    preview_registered_story_chapter_patch(&state, &session_id, input)
}

pub fn story_chapter_patch_status_load(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<StoryChapterPatchStatusV1, AppError> {
    session_handle(&state, &session_id)?;
    let stored = state
        .story_chapter_patches
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(&session_id)
        .cloned();
    Ok(StoryChapterPatchStatusV1 {
        schema_version: CHAPTER_PATCH_SCHEMA_VERSION,
        preview: stored.as_ref().map(|value| value.document.preview.clone()),
        applied: stored.and_then(|value| value.applied),
    })
}

pub fn story_chapter_patch_apply(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryChapterPatchApplyRequestV1,
) -> Result<ChapterFileApplyResult, AppError> {
    apply_registered_story_chapter_patch(&state, &session_id, input)
}

fn preview_registered_story_chapter_patch(
    state: &ProjectRegistry,
    session_id: &str,
    input: StoryChapterPatchPreviewRequestV1,
) -> Result<ChapterPatchPreviewV1, AppError> {
    if input.schema_version != CHAPTER_PATCH_SCHEMA_VERSION {
        return Err(AppError::workflow_input_invalid());
    }
    let handle = session_handle(state, session_id)?;
    let (run, proposal) =
        load_completed_proposal(state, session_id, &input.run_id, &input.proposal_hash)?;
    if run.request_hash != proposal.request_hash {
        return Err(AppError::workflow_invalid_state());
    }
    let stored_report = state
        .story_continuity_reports
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(session_id)
        .filter(|report| {
            report.report_hash == input.continuity_report_hash
                && report.proposal_hash == proposal.proposal_hash
        })
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let report = revalidate_continuity(
        state,
        session_id,
        &input.run_id,
        &input.proposal_hash,
        input.request.clone(),
        &stored_report,
    )?;
    let target = load_target_document(state, session_id, &proposal.target_chapter.relative_path)?;
    let (current_content, current_hash, bom, line_ending) = target_parts(target.as_ref());
    let mut document = build_chapter_patch(BuildChapterPatchInputV1 {
        proposal: &proposal,
        continuity_report: &report,
        current_content,
        current_content_hash: current_hash,
        accepted_or_final_unlock_confirmed: input.accepted_or_final_unlock_confirmed,
        create_chapter_id: None,
    })
    .map_err(map_patch_error)?;
    let encoded = encode_chapter_target_bytes(&document.target_content, bom, line_ending)
        .map_err(map_workflow_apply_error)?;
    bind_chapter_patch_encoded_hash(&mut document, &sha256_hex(&encoded))
        .map_err(map_patch_error)?;
    let preview = document.preview.clone();
    state
        .story_chapter_patches
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .insert(
            session_id.to_owned(),
            StoredStoryChapterPatchV1 {
                run_id: input.run_id,
                request: input.request,
                warning_rule_ids: stored_report.acknowledged_warning_rule_ids,
                unknown_rule_ids: stored_report.acknowledged_unknown_rule_ids,
                document,
                applied: None,
            },
        );
    let _ = handle;
    Ok(preview)
}

fn apply_registered_story_chapter_patch(
    state: &ProjectRegistry,
    session_id: &str,
    input: StoryChapterPatchApplyRequestV1,
) -> Result<ChapterFileApplyResult, AppError> {
    if input.schema_version != CHAPTER_PATCH_SCHEMA_VERSION || !input.confirmed {
        return Err(AppError::workflow_confirmation_required());
    }
    let handle = workflow_context_handle(state, session_id).map_err(map_workflow_apply_error)?;
    let stored = state
        .story_chapter_patches
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(session_id)
        .filter(|stored| {
            stored.run_id == input.run_id
                && stored.document.preview.preview_hash == input.preview_hash
                && stored.document.preview.proposal_hash == input.proposal_hash
                && stored.document.preview.continuity_report_hash == input.continuity_report_hash
                && stored.applied.is_none()
        })
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let (_, proposal) =
        load_completed_proposal(state, session_id, &input.run_id, &input.proposal_hash)?;
    if state.app_state.current_file(handle.project_id()).as_deref()
        == Some(stored.document.preview.target_relative_path.as_str())
    {
        return Err(AppError::workflow_target_conflict());
    }
    let stored_report = state
        .story_continuity_reports
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(session_id)
        .filter(|report| report.report_hash == input.continuity_report_hash)
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let report = revalidate_continuity(
        state,
        session_id,
        &input.run_id,
        &input.proposal_hash,
        stored.request.clone(),
        &stored_report,
    )?;
    let target = load_target_document(state, session_id, &proposal.target_chapter.relative_path)?;
    let (current_content, current_hash, bom, line_ending) = target_parts(target.as_ref());
    let create_chapter_id = if proposal.action == StoryActionV2::Create {
        Some(
            stored
                .document
                .preview
                .chapter_id
                .strip_prefix("chapter_")
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(AppError::workflow_invalid_state)?,
        )
    } else {
        None
    };
    let mut revalidated = build_chapter_patch(BuildChapterPatchInputV1 {
        proposal: &proposal,
        continuity_report: &report,
        current_content,
        current_content_hash: current_hash,
        accepted_or_final_unlock_confirmed: stored
            .document
            .preview
            .accepted_or_final_unlock_confirmed,
        create_chapter_id,
    })
    .map_err(map_patch_error)?;
    let encoded = encode_chapter_target_bytes(&revalidated.target_content, bom, line_ending)
        .map_err(map_workflow_apply_error)?;
    bind_chapter_patch_encoded_hash(&mut revalidated, &sha256_hex(&encoded))
        .map_err(map_patch_error)?;
    if revalidated != stored.document {
        return Err(AppError::workflow_context_changed());
    }

    let applied = apply_chapter_file_recoverable(
        &handle,
        revalidated.preview.operation,
        &revalidated.preview.target_relative_path,
        revalidated.preview.expected_source_hash.as_deref(),
        &encoded,
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
    if applied.file.content_hash != revalidated.preview.proposed_content_hash
        || applied.file.validation.content_hash != revalidated.preview.proposed_content_hash
        || !applied.file.validation.eligible_for_ai_apply
        || !applied.file.validation.diagnostics.is_empty()
    {
        mark_session_recovery_required(state, session_id)?;
        return Err(AppError::workflow_recovery_required());
    }
    let mut patches = state
        .story_chapter_patches
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let current = patches
        .get_mut(session_id)
        .filter(|current| current.document.preview.preview_hash == input.preview_hash)
        .ok_or_else(AppError::workflow_invalid_state)?;
    current.applied = Some(applied.clone());
    Ok(applied)
}

pub(super) fn load_completed_proposal(
    state: &ProjectRegistry,
    session_id: &str,
    run_id: &str,
    proposal_hash: &str,
) -> Result<
    (
        story_chapter_commands::StoryChapterRunSnapshotV1,
        novelgainker_workflows::chapter::ChapterProposalV1,
    ),
    AppError,
> {
    let run = state
        .story_chapter_runs
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(session_id)
        .filter(|run| {
            run.run_id == run_id
                && run.state == story_chapter_commands::StoryChapterRunStateV1::Completed
        })
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let proposal = run
        .proposal
        .as_ref()
        .filter(|proposal| proposal.proposal_hash == proposal_hash)
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    Ok((run, proposal))
}

fn revalidate_continuity(
    state: &ProjectRegistry,
    session_id: &str,
    run_id: &str,
    proposal_hash: &str,
    request: StoryContextRequestPreviewRequestV1,
    stored: &StoryContinuityReportV1,
) -> Result<StoryContinuityReportV1, AppError> {
    let mut report = rebuild_registered_story_continuity(
        state,
        session_id,
        run_id,
        proposal_hash,
        request,
        now_unix_seconds().max(0) as u64,
    )?;
    if report.report_hash != stored.report_hash
        || !confirm_story_continuity_warnings(
            &mut report,
            &stored.acknowledged_warning_rule_ids,
            &stored.acknowledged_unknown_rule_ids,
        )
        || !report.review_ready
    {
        return Err(AppError::workflow_context_changed());
    }
    Ok(report)
}

fn load_target_document(
    state: &ProjectRegistry,
    session_id: &str,
    relative_path: &str,
) -> Result<Option<ProjectFileDocument>, AppError> {
    let target = load_story_target_state(state, session_id, relative_path);
    if !target.available {
        return Err(AppError::workflow_unavailable());
    }
    if !target.exists {
        return Ok(None);
    }
    let handle = session_handle(state, session_id)?;
    read_project_file(&handle, relative_path)
        .map(Some)
        .map_err(map_workflow_apply_error)
}

fn target_parts(
    target: Option<&ProjectFileDocument>,
) -> (Option<&str>, Option<&str>, Utf8Bom, LineEnding) {
    target.map_or((None, None, Utf8Bom::None, LineEnding::Lf), |document| {
        (
            Some(document.content.as_str()),
            Some(document.content_hash.as_str()),
            document.bom,
            document.line_ending,
        )
    })
}

fn map_patch_error(error: ChapterPatchError) -> AppError {
    match error {
        ChapterPatchError::LockedStatus => {
            AppError::workflow_confirmation_required().with_diagnostic(error.code())
        }
        ChapterPatchError::TargetAlreadyExists
        | ChapterPatchError::TargetMissing
        | ChapterPatchError::SourceHashMismatch => {
            AppError::workflow_target_conflict().with_diagnostic(error.code())
        }
        ChapterPatchError::BindingMismatch => {
            AppError::workflow_context_changed().with_diagnostic(error.code())
        }
        ChapterPatchError::ContinuityNotReady
        | ChapterPatchError::InvalidTarget
        | ChapterPatchError::InvalidChapter
        | ChapterPatchError::InvalidProposal => {
            AppError::workflow_validation_failed().with_diagnostic(error.code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ng_505_patch_status_contract_exposes_diff_and_receipt_but_no_hidden_target_body() {
        let status = StoryChapterPatchStatusV1 {
            schema_version: 1,
            preview: None,
            applied: None,
        };
        let serialized = serde_json::to_value(status).unwrap();
        assert_eq!(serialized["schemaVersion"], 1);
        assert!(serialized["preview"].is_null());
        assert!(serialized["applied"].is_null());
        assert!(!serialized.to_string().contains("targetContent"));
        assert!(!serialized.to_string().contains("request"));
    }
}
