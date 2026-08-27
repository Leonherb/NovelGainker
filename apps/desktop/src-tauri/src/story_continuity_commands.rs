use super::*;
use crate::commands::story_context_commands::{
    build_registered_story_generation_material, StoryContextRequestPreviewRequestV1,
};
use novelgainker_workflows::continuity::{
    check_story_continuity, confirm_story_continuity_warnings, ContinuityCheckInputV1,
    ContinuitySourceV1, ContinuityTargetStateV1, StoryContinuityReportV1, CONTINUITY_CHECK_VERSION,
};
use novelgainker_workflows::story_context::StoryTargetChapterV2;

#[derive(Debug, Clone)]
pub(super) struct StoryTargetPreviewV1 {
    request_hash: String,
    target: ContinuityTargetStateV1,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryContinuityCheckRequestV1 {
    schema_version: u16,
    run_id: String,
    proposal_hash: String,
    request: StoryContextRequestPreviewRequestV1,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoryContinuityWarningConfirmRequestV1 {
    schema_version: u16,
    run_id: String,
    report_hash: String,
    warning_rule_ids: Vec<String>,
    unknown_rule_ids: Vec<String>,
    confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryContinuityStatusV1 {
    schema_version: u16,
    report: Option<StoryContinuityReportV1>,
}

#[tauri::command]
pub fn story_continuity_check(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryContinuityCheckRequestV1,
) -> Result<StoryContinuityReportV1, AppError> {
    if input.schema_version != CONTINUITY_CHECK_VERSION {
        return Err(AppError::workflow_input_invalid());
    }
    session_handle(&state, &session_id)?;
    state
        .story_chapter_patches
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    state
        .story_lifecycle
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(&session_id);
    let report = rebuild_registered_story_continuity(
        &state,
        &session_id,
        &input.run_id,
        &input.proposal_hash,
        input.request,
        now_unix_seconds().max(0) as u64,
    )?;
    state
        .story_continuity_reports
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .insert(session_id, report.clone());
    Ok(report)
}

pub(super) fn rebuild_registered_story_continuity(
    state: &ProjectRegistry,
    session_id: &str,
    run_id: &str,
    proposal_hash: &str,
    request: StoryContextRequestPreviewRequestV1,
    checked_at: u64,
) -> Result<StoryContinuityReportV1, AppError> {
    let run = state
        .story_chapter_runs
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(session_id)
        .filter(|run| run.run_id == run_id)
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let proposal = run
        .proposal
        .as_ref()
        .filter(|proposal| proposal.proposal_hash == proposal_hash)
        .ok_or_else(AppError::workflow_invalid_state)?;

    let expected_action = request.context.action;
    let expected_target = StoryTargetChapterV2 {
        chapter_number: request.context.chapter_number,
        chapter_title: request.context.chapter_title.trim().to_owned(),
        relative_path: format!("chapters/{:03}.md", request.context.chapter_number),
    };
    let reader_model = request.reader_mental_model.clone();
    let current_material =
        build_registered_story_generation_material(state, session_id, request).ok();
    let sources = current_material
        .as_ref()
        .map(|material| {
            material
                .sources
                .iter()
                .filter_map(|source| {
                    let manifest_source = material
                        .snapshot
                        .manifest
                        .selected_sources
                        .iter()
                        .find(|item| item.source_id == source.source_id)?;
                    Some(ContinuitySourceV1 {
                        source_id: source.source_id.clone(),
                        kind: manifest_source.kind,
                        content: source.content.clone(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let current_target =
        load_story_target_state(state, session_id, &proposal.target_chapter.relative_path);
    Ok(check_story_continuity(ContinuityCheckInputV1 {
        proposal,
        expected_action,
        expected_target: &expected_target,
        reader_model: &reader_model,
        current_snapshot: current_material.as_ref().map(|material| &material.snapshot),
        sources: &sources,
        generation_target: &run.generation_target,
        current_target: &current_target,
        checked_at,
    }))
}

#[tauri::command]
pub fn story_continuity_status_load(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
) -> Result<StoryContinuityStatusV1, AppError> {
    session_handle(&state, &session_id)?;
    let report = state
        .story_continuity_reports
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(&session_id)
        .cloned();
    Ok(StoryContinuityStatusV1 {
        schema_version: CONTINUITY_CHECK_VERSION,
        report,
    })
}

#[tauri::command]
pub fn story_continuity_warning_confirm(
    state: tauri::State<'_, ProjectRegistry>,
    session_id: String,
    input: StoryContinuityWarningConfirmRequestV1,
) -> Result<StoryContinuityReportV1, AppError> {
    if input.schema_version != CONTINUITY_CHECK_VERSION || !input.confirmed {
        return Err(AppError::workflow_confirmation_required());
    }
    session_handle(&state, &session_id)?;
    let run_matches = state
        .story_chapter_runs
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(&session_id)
        .is_some_and(|run| run.run_id == input.run_id);
    if !run_matches {
        return Err(AppError::workflow_invalid_state());
    }
    let mut reports = state
        .story_continuity_reports
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let report = reports
        .get_mut(&session_id)
        .filter(|report| report.report_hash == input.report_hash)
        .ok_or_else(AppError::workflow_invalid_state)?;
    if !confirm_story_continuity_warnings(report, &input.warning_rule_ids, &input.unknown_rule_ids)
    {
        return Err(AppError::workflow_input_invalid());
    }
    Ok(report.clone())
}

pub(super) fn load_story_target_state(
    state: &ProjectRegistry,
    session_id: &str,
    relative_path: &str,
) -> ContinuityTargetStateV1 {
    let Ok(handle) = session_handle(state, session_id) else {
        return unavailable_target();
    };
    let Ok(index) = list_project_files(&handle) else {
        return unavailable_target();
    };
    if !contains_path(&index.nodes, relative_path) {
        return ContinuityTargetStateV1::missing();
    }
    match read_project_file(&handle, relative_path) {
        Ok(document) => ContinuityTargetStateV1 {
            available: true,
            exists: true,
            content_hash: Some(document.content_hash),
            content: Some(document.content),
        },
        Err(_) => unavailable_target(),
    }
}

pub(super) fn record_story_target_preview(
    state: &ProjectRegistry,
    session_id: &str,
    request_hash: String,
    relative_path: &str,
) -> Result<(), AppError> {
    let target = load_story_target_state(state, session_id, relative_path);
    state
        .story_target_previews
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .insert(
            session_id.to_owned(),
            StoryTargetPreviewV1 {
                request_hash,
                target,
            },
        );
    Ok(())
}

pub(super) fn load_story_preview_target(
    state: &ProjectRegistry,
    session_id: &str,
    request_hash: &str,
) -> ContinuityTargetStateV1 {
    state
        .story_target_previews
        .lock()
        .ok()
        .and_then(|previews| previews.get(session_id).cloned())
        .filter(|preview| preview.request_hash == request_hash)
        .map(|preview| preview.target)
        .unwrap_or_else(unavailable_target)
}

fn contains_path(nodes: &[ProjectFileNode], relative_path: &str) -> bool {
    nodes.iter().any(|node| {
        node.relative_path == relative_path || contains_path(&node.children, relative_path)
    })
}

fn unavailable_target() -> ContinuityTargetStateV1 {
    ContinuityTargetStateV1 {
        available: false,
        exists: false,
        content_hash: None,
        content: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ng_504_status_contract_never_exposes_project_content_or_write_capability() {
        let status = StoryContinuityStatusV1 {
            schema_version: 1,
            report: None,
        };
        let serialized = serde_json::to_value(status).unwrap();
        assert_eq!(serialized["schemaVersion"], 1);
        assert!(serialized["report"].is_null());
        assert!(!serialized.to_string().contains("content"));
    }
}
