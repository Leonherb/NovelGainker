use super::*;
use novelgainker_workflows::plot::MAIN_PLOT_RELATIVE_PATH;

pub fn workflow_proposal_apply(
    state: tauri::State<'_, ProjectRegistry>,
    run_id: String,
    proposal_hash: String,
    confirmed: bool,
) -> Result<WorkflowRunStatusSnapshot, AppError> {
    apply_registered_workflow_proposal(&state, &run_id, &proposal_hash, confirmed)
}

fn apply_registered_workflow_proposal(
    state: &ProjectRegistry,
    run_id: &str,
    proposal_hash: &str,
    confirmed: bool,
) -> Result<WorkflowRunStatusSnapshot, AppError> {
    if !confirmed {
        return Err(AppError::workflow_confirmation_required());
    }
    let session_id = state
        .workflow_sessions
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .get(run_id)
        .cloned()
        .ok_or_else(AppError::workflow_invalid_state)?;
    let handle = workflow_context_handle(state, &session_id).map_err(map_workflow_apply_error)?;
    let (current, material) = {
        let manager_guard = state
            .workflow_manager
            .lock()
            .map_err(|_| AppError::workflow_unavailable())?;
        let manager = manager_guard
            .as_ref()
            .ok_or_else(AppError::workflow_unavailable)?;
        let current = manager
            .record(run_id)
            .ok_or_else(AppError::workflow_invalid_state)?;
        if current.project_id != handle.project_id() {
            return Err(AppError::workflow_invalid_state());
        }
        let material = manager
            .proposal_apply_material(run_id, proposal_hash)
            .map_err(workflow_manager_error)?;
        (current, material)
    };

    verify_context_sources(&handle, &material.source_bindings)?;
    let metadata = material.proposal.metadata();
    let expected_prefix = match current.workflow_id.as_str() {
        CHARACTER_CREATE_WORKFLOW_ID => "characters/",
        SETTING_CREATE_WORKFLOW_ID => "world/",
        MATERIAL_CREATE_WORKFLOW_ID => "materials/",
        PLOT_CREATE_WORKFLOW_ID => "plot/",
        _ => return Err(AppError::workflow_validation_failed()),
    };
    if current.workflow_id == PLOT_CREATE_WORKFLOW_ID
        && metadata.operation.relative_path != MAIN_PLOT_RELATIVE_PATH
    {
        return Err(AppError::workflow_validation_failed());
    }
    if current.workflow_id == MATERIAL_CREATE_WORKFLOW_ID
        && metadata
            .operation
            .relative_path
            .strip_prefix("materials/")
            .is_none_or(|name| name.starts_with("000."))
    {
        return Err(AppError::workflow_validation_failed());
    }
    if !metadata
        .operation
        .relative_path
        .starts_with(expected_prefix)
    {
        return Err(AppError::workflow_validation_failed());
    }

    if state.app_state.current_file(&current.project_id).as_deref()
        == Some(metadata.operation.relative_path.as_str())
    {
        return Err(AppError::workflow_target_conflict());
    }
    let missing_hash = "0".repeat(64);
    let expected_watch_hash = if metadata.operation.expected_source_hash == "absent" {
        missing_hash.as_str()
    } else {
        metadata.operation.expected_source_hash.as_str()
    };
    let target_status = check_project_file(
        &handle,
        &metadata.operation.relative_path,
        expected_watch_hash,
    )
    .map_err(map_workflow_apply_error)?;
    let target_matches = match metadata.operation.kind {
        novelgainker_workflows::proposal::CanonFileOperationKind::Create => {
            target_status.state == ProjectFileWatchState::Missing
                && metadata.operation.expected_source_hash == "absent"
        }
        novelgainker_workflows::proposal::CanonFileOperationKind::Replace => {
            target_status.state == ProjectFileWatchState::Unchanged
                && target_status.observed_hash.as_deref()
                    == Some(metadata.operation.expected_source_hash.as_str())
        }
    };
    if !target_matches {
        return Err(AppError::workflow_target_conflict());
    }

    let mut manager_guard = state
        .workflow_manager
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?;
    let manager = manager_guard
        .as_mut()
        .ok_or_else(AppError::workflow_unavailable)?;
    let mut candidate = manager.clone();
    let applying = candidate
        .confirm_proposal_apply(run_id, proposal_hash, true, now_unix_seconds())
        .map_err(workflow_manager_error)?;
    state
        .app_state
        .save_workflow_run(&applying)
        .map_err(|_| AppError::workflow_unavailable())?;

    let applied = apply_canon_file_recoverable(
        &handle,
        metadata.operation.kind,
        &metadata.operation.relative_path,
        &metadata.operation.expected_source_hash,
        material.proposal.proposed_bytes(),
        &state.recovery_store,
    );
    let applied = match applied {
        Ok(result) => result,
        Err(error) => {
            let recovery_required = error.code == "project.recovery_required";
            let current = candidate
                .record(run_id)
                .ok_or_else(AppError::workflow_invalid_state)?;
            let failed = candidate
                .transition(
                    run_id,
                    current.sequence.saturating_add(1),
                    if recovery_required {
                        WorkflowRunState::RecoveryRequired
                    } else {
                        WorkflowRunState::Failed
                    },
                    now_unix_seconds(),
                )
                .map_err(workflow_manager_error)?;
            if recovery_required {
                mark_session_recovery_required(state, &session_id)?;
            }
            state
                .app_state
                .save_workflow_run(&failed)
                .map_err(|_| AppError::workflow_unavailable())?;
            *manager = candidate;
            if failed.state.is_terminal() {
                state
                    .workflow_sessions
                    .lock()
                    .map_err(|_| AppError::workflow_unavailable())?
                    .remove(run_id);
            }
            return Err(map_workflow_apply_error(error));
        }
    };
    if applied.content_hash != metadata.operation.proposed_content_hash
        || applied.validation.content_hash != metadata.operation.proposed_content_hash
        || !applied.validation.eligible_for_ai_apply
        || !applied.validation.diagnostics.is_empty()
    {
        mark_session_recovery_required(state, &session_id)?;
        let current = candidate
            .record(run_id)
            .ok_or_else(AppError::workflow_invalid_state)?;
        let recovery = candidate
            .transition(
                run_id,
                current.sequence.saturating_add(1),
                WorkflowRunState::RecoveryRequired,
                now_unix_seconds(),
            )
            .map_err(workflow_manager_error)?;
        state
            .app_state
            .save_workflow_run(&recovery)
            .map_err(|_| AppError::workflow_unavailable())?;
        *manager = candidate;
        return Err(AppError::workflow_validation_failed());
    }

    let validating = candidate
        .transition(
            run_id,
            applying.sequence.saturating_add(1),
            WorkflowRunState::Validating,
            now_unix_seconds(),
        )
        .map_err(workflow_manager_error)?;
    state
        .app_state
        .save_workflow_run(&validating)
        .map_err(|_| AppError::workflow_unavailable())?;
    let completed = candidate
        .transition(
            run_id,
            validating.sequence.saturating_add(1),
            WorkflowRunState::Completed,
            now_unix_seconds(),
        )
        .map_err(workflow_manager_error)?;
    state
        .app_state
        .save_workflow_run(&completed)
        .map_err(|_| AppError::workflow_unavailable())?;
    *manager = candidate;
    state
        .workflow_sessions
        .lock()
        .map_err(|_| AppError::workflow_unavailable())?
        .remove(run_id);
    Ok(workflow_status_snapshot(Some(manager), handle.project_id()))
}

pub(super) fn mark_session_recovery_required(
    state: &ProjectRegistry,
    session_id: &str,
) -> Result<(), AppError> {
    let mut registry = lock_registry(state)?;
    let session = registry
        .sessions
        .get_mut(session_id)
        .ok_or_else(AppError::project_session_not_found)?;
    session.recovery = RecoveryReport {
        status: RecoveryStatus::Required,
        recovered_transactions: session.recovery.recovered_transactions,
        required_transactions: session.recovery.required_transactions.saturating_add(1),
    };
    Ok(())
}

pub(super) fn map_workflow_apply_error(error: AppError) -> AppError {
    match error.code.as_str() {
        "project.writer_required" => AppError::workflow_writer_required(),
        "project.recovery_required" => AppError::workflow_recovery_required(),
        "file.source_changed" | "file.not_found" => AppError::workflow_target_conflict(),
        "file.save_failed" => AppError::workflow_validation_failed(),
        _ => error,
    }
}

pub(super) fn workflow_context_handle(
    state: &ProjectRegistry,
    session_id: &str,
) -> Result<ProjectHandle, AppError> {
    let registry = lock_registry(state)?;
    let session = registry
        .sessions
        .get(session_id)
        .ok_or_else(AppError::project_session_not_found)?;
    if session.write_lease.is_none() {
        return Err(AppError::project_writer_required());
    }
    if session.recovery.status == RecoveryStatus::Required {
        return Err(AppError::project_recovery_required());
    }
    Ok(session.handle.clone())
}

fn verify_context_sources(
    handle: &ProjectHandle,
    bindings: &[novelgainker_workflows::context::ContextSourceBinding],
) -> Result<(), AppError> {
    for binding in bindings {
        let status = check_project_file(handle, &binding.relative_path, &binding.content_hash)?;
        if status.state != ProjectFileWatchState::Unchanged {
            return Err(AppError::workflow_context_changed());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::coordination::try_acquire_project_write_lease;
    use app_core::project::{create_project, inspect_project_root, ProjectScanMode};
    use novelgainker_workflows::character_v2::{
        parse_and_serialize_character_v2_create, parse_and_serialize_character_v2_replace,
        EntityKindV1, ResolvedEntityReferenceV1,
    };
    use novelgainker_workflows::context::{
        prepare_character_context, prepare_material_context, prepare_plot_context,
        prepare_setting_context, CharacterBriefModeV2, CharacterBriefV2, ContextSourceInput,
        MaterialBriefV1, PlotBriefV1, PrepareCharacterContextInput, PrepareMaterialContextInput,
        PreparePlotContextInput, PrepareSettingContextInput, SettingBriefV1,
    };
    use novelgainker_workflows::material::{
        parse_and_serialize_material, parse_and_serialize_material_replace, MaterialTypeV1,
    };
    use novelgainker_workflows::plot::{
        parse_and_serialize_plot, parse_and_serialize_plot_replace, PlotArtifactV1,
    };
    use novelgainker_workflows::proposal::{
        build_canon_proposal, BuildCanonProposalInput, CanonFileOperationKind,
    };
    use novelgainker_workflows::setting::{
        parse_and_serialize_setting, parse_and_serialize_setting_replace, SettingTypeV1,
    };
    use sha2::{Digest, Sha256};
    use std::fs;

    fn prepare_proposal_fixture(
        state: &ProjectRegistry,
        session_id: &str,
        handle: &ProjectHandle,
        kind: CanonFileOperationKind,
        relative_path: &str,
        original: Option<&str>,
        proposed: &str,
    ) -> (String, String) {
        let project = read_project_file(handle, "project.md").unwrap();
        let run_id = Uuid::new_v4().to_string();
        let prepared = prepare_character_context(PrepareCharacterContextInput {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            brief: CharacterBriefV2 {
                schema_version: 2,
                mode: CharacterBriefModeV2::Create,
                brief: "人物简报".to_owned(),
            },
            sources: vec![ContextSourceInput {
                relative_path: project.relative_path,
                content: project.content,
                content_hash: project.content_hash,
                source_byte_count: project.size_bytes,
            }],
        })
        .unwrap();
        let record = WorkflowRunRecord {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("openai".to_owned()),
            preset_version: Some(1),
            model: Some("gpt-5.6-terra".to_owned()),
            manifest_hash: Some(prepared.manifest.manifest_hash.clone()),
            proposal_hash: None,
            target_relative_path: None,
            target_source_hash: None,
            confirmed_at: None,
            updated_at: 1,
        };
        let mut manager_guard = state.workflow_manager.lock().unwrap();
        let manager = manager_guard.as_mut().unwrap();
        manager.create_prepared(record, prepared.clone()).unwrap();
        manager
            .advance_prepared_to_context_ready(&run_id, 2)
            .unwrap();
        manager
            .confirm_generation(&run_id, &prepared.manifest.manifest_hash, true, 3)
            .unwrap();
        let expected_source_hash =
            original.map(|body| format!("{:x}", Sha256::digest(body.as_bytes())));
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: Uuid::new_v4().to_string(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            run_id: run_id.clone(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            manifest_hash: prepared.manifest.manifest_hash,
            operation_kind: kind,
            relative_path: relative_path.to_owned(),
            expected_source_hash,
            original_content: original.map(str::to_owned),
            proposed_content: proposed.to_owned(),
            warning_codes: Vec::new(),
        })
        .unwrap();
        let proposal_hash = proposal.metadata().proposal_hash.clone();
        let ready = manager
            .complete_generation_with_proposal(&run_id, proposal, 4)
            .unwrap();
        state.app_state.save_workflow_run(&ready).unwrap();
        state
            .workflow_sessions
            .lock()
            .unwrap()
            .insert(run_id.clone(), session_id.to_owned());
        (run_id, proposal_hash)
    }

    fn prepare_plot_proposal_fixture(
        state: &ProjectRegistry,
        session_id: &str,
        handle: &ProjectHandle,
        original: Option<&PlotArtifactV1>,
        proposed: PlotArtifactV1,
    ) -> (String, String) {
        let project = read_project_file(handle, "project.md").unwrap();
        let run_id = Uuid::new_v4().to_string();
        let prepared = prepare_plot_context(PreparePlotContextInput {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: PLOT_CREATE_WORKFLOW_ID.to_owned(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            brief: PlotBriefV1 {
                schema_version: 1,
                title: "退潮之塔".to_owned(),
                genre: "悬疑奇幻".to_owned(),
                core_question: "林遥是否愿意用记忆换取真相？".to_owned(),
                desired_ending: "林遥公开真相并承担代价。".to_owned(),
                must_include: vec!["铜钥匙必须被回收".to_owned()],
                forbidden_directions: vec!["不由巧合解决冲突".to_owned()],
            },
            sources: vec![ContextSourceInput {
                relative_path: project.relative_path,
                content: project.content,
                content_hash: project.content_hash,
                source_byte_count: project.size_bytes,
            }],
        })
        .unwrap();
        let target = original.map(|_| read_project_file(handle, MAIN_PLOT_RELATIVE_PATH).unwrap());
        let expected_source_hash = target.as_ref().map(|target| target.content_hash.clone());
        let record = WorkflowRunRecord {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: PLOT_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("openai".to_owned()),
            preset_version: Some(1),
            model: Some("gpt-5.6-terra".to_owned()),
            manifest_hash: Some(prepared.manifest.manifest_hash.clone()),
            proposal_hash: None,
            target_relative_path: original.map(|_| MAIN_PLOT_RELATIVE_PATH.to_owned()),
            target_source_hash: expected_source_hash.clone(),
            confirmed_at: None,
            updated_at: 1,
        };
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: Uuid::new_v4().to_string(),
            workflow_id: PLOT_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            run_id: run_id.clone(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            manifest_hash: prepared.manifest.manifest_hash.clone(),
            operation_kind: if original.is_some() {
                CanonFileOperationKind::Replace
            } else {
                CanonFileOperationKind::Create
            },
            relative_path: MAIN_PLOT_RELATIVE_PATH.to_owned(),
            expected_source_hash,
            original_content: original.map(|artifact| artifact.markdown.clone()),
            proposed_content: proposed.markdown,
            warning_codes: Vec::new(),
        })
        .unwrap();
        let proposal_hash = proposal.metadata().proposal_hash.clone();
        {
            let mut manager_guard = state.workflow_manager.lock().unwrap();
            let manager = manager_guard.as_mut().unwrap();
            manager.create_prepared(record, prepared.clone()).unwrap();
            manager
                .advance_prepared_to_context_ready(&run_id, 2)
                .unwrap();
            manager
                .confirm_generation(&run_id, &prepared.manifest.manifest_hash, true, 3)
                .unwrap();
            let ready = manager
                .complete_generation_with_proposal(&run_id, proposal, 4)
                .unwrap();
            state.app_state.save_workflow_run(&ready).unwrap();
        }
        state
            .workflow_sessions
            .lock()
            .unwrap()
            .insert(run_id.clone(), session_id.to_owned());
        (run_id, proposal_hash)
    }
    fn fixture() -> (
        PathBuf,
        PathBuf,
        ProjectRegistry,
        ProjectSession,
        ProjectHandle,
    ) {
        let root = std::env::temp_dir().join(format!("novelgainker-ng304-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let inspection = inspect_project_root(&root, ProjectScanMode::Create).unwrap();
        let handle = create_project(&inspection).unwrap();
        crate::commands::install_legacy_story_ledger_fixture(&root);
        let app_state = root.with_extension("state");
        let registry = ProjectRegistry::new(app_state.clone()).unwrap();
        let lease = try_acquire_project_write_lease(&handle.coordination_key())
            .unwrap()
            .unwrap();
        let session = register_session(&registry, None, handle.clone(), Some(lease)).unwrap();
        (root, app_state, registry, session, handle)
    }

    fn markdown(title: &str) -> String {
        format!("---\ntitle: {title}\nwritingLanguage: zh-CN\n---\n\n# {title}\n")
    }

    #[test]
    fn apply_reads_only_retained_bytes_and_requires_exact_confirmation() {
        let (root, app_state, registry, session, handle) = fixture();
        let proposed = markdown("林遥");
        let (run_id, proposal_hash) = prepare_proposal_fixture(
            &registry,
            &session.session_id,
            &handle,
            CanonFileOperationKind::Create,
            "characters/林遥.md",
            None,
            &proposed,
        );
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, false),
            Err(AppError::workflow_confirmation_required())
        );
        assert!(!root.join("characters/林遥.md").exists());
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &"0".repeat(64), true),
            Err(AppError::workflow_proposal_expired())
        );
        assert!(!root.join("characters/林遥.md").exists());
        let completed =
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true).unwrap();
        assert_eq!(completed.run.unwrap().state, "completed");
        assert_eq!(
            fs::read_to_string(root.join("characters/林遥.md")).unwrap(),
            proposed
        );
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_invalid_state())
        );
        assert_eq!(
            fs::read_to_string(root.join("characters/林遥.md")).unwrap(),
            proposed
        );
        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn confirmed_character_v2_enhance_reuses_id_and_path_without_reverse_writes() {
        let (root, app_state, registry, session, handle) = fixture();
        let original_id = "character_10000000-0000-4000-8000-000000000001";
        let related_id = "character_20000000-0000-4000-8000-000000000002";
        let original_json = serde_json::json!({
            "schemaVersion": 2,
            "workflowId": CHARACTER_CREATE_WORKFLOW_ID,
            "character": {
                "displayName": "旧人物",
                "role": "调查员",
                "summary": "负责整理旧档案。",
                "traits": ["谨慎"],
                "goal": "查明旧案真相。",
                "conflict": "公开真相会伤害同伴。",
                "motivations": ["保护仍在调查的人"],
                "secrets": [],
                "plotFunctions": ["推动调查线索"],
                "relationships": []
            },
            "rationale": "作为待完善的人物基线。"
        })
        .to_string();
        let original =
            parse_and_serialize_character_v2_create(&original_json, original_id, &[]).unwrap();
        fs::write(root.join(&original.relative_path), &original.markdown).unwrap();

        let related_json = serde_json::json!({
            "schemaVersion": 2,
            "workflowId": CHARACTER_CREATE_WORKFLOW_ID,
            "character": {
                "displayName": "林遥",
                "role": "记者",
                "summary": "追查同一宗旧案。",
                "traits": ["敏锐"],
                "goal": "找回缺失证据。",
                "conflict": "证据牵连亲友。",
                "motivations": ["阻止档案再次消失"],
                "secrets": [],
                "plotFunctions": ["提供外部调查线"],
                "relationships": []
            },
            "rationale": "作为已验证关系目标。"
        })
        .to_string();
        let related =
            parse_and_serialize_character_v2_create(&related_json, related_id, &[]).unwrap();
        fs::write(root.join(&related.relative_path), &related.markdown).unwrap();
        let related_before = fs::read(root.join(&related.relative_path)).unwrap();

        let project = read_project_file(&handle, "project.md").unwrap();
        let target = read_project_file(&handle, &original.relative_path).unwrap();
        let relation_source = read_project_file(&handle, &related.relative_path).unwrap();
        let run_id = Uuid::new_v4().to_string();
        let prepared = prepare_character_context(PrepareCharacterContextInput {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            brief: CharacterBriefV2 {
                schema_version: 2,
                mode: CharacterBriefModeV2::Enhance,
                brief: "补全人物动机、剧情功能和与林遥的关系。".to_owned(),
            },
            sources: vec![
                ContextSourceInput {
                    relative_path: project.relative_path,
                    content: project.content,
                    content_hash: project.content_hash,
                    source_byte_count: project.size_bytes,
                },
                ContextSourceInput {
                    relative_path: target.relative_path.clone(),
                    content: target.content.clone(),
                    content_hash: target.content_hash.clone(),
                    source_byte_count: target.size_bytes,
                },
                ContextSourceInput {
                    relative_path: relation_source.relative_path.clone(),
                    content: relation_source.content,
                    content_hash: relation_source.content_hash,
                    source_byte_count: relation_source.size_bytes,
                },
            ],
        })
        .unwrap();
        let reference = ResolvedEntityReferenceV1 {
            entity_id: related_id.to_owned(),
            display_name: "林遥".to_owned(),
            relative_path: related.relative_path.clone(),
            entity_kind: EntityKindV1::Character,
        };
        let replacement_json = serde_json::json!({
            "schemaVersion": 2,
            "workflowId": CHARACTER_CREATE_WORKFLOW_ID,
            "character": {
                "displayName": "改名后人物",
                "role": "调查组档案员",
                "summary": "负责整理档案并交叉验证记者线索。",
                "traits": ["谨慎", "坚定"],
                "goal": "公开可验证的旧案真相。",
                "conflict": "公开时机可能使证人陷入危险。",
                "motivations": ["保护证人", "修正旧档案错误"],
                "secrets": [],
                "plotFunctions": ["连接档案线与记者线"],
                "relationships": [{
                    "targetId": related_id,
                    "relationType": "ally",
                    "description": "互相核对调查结论"
                }]
            },
            "rationale": "完善内容保持目标身份和磁盘路径不变。"
        })
        .to_string();
        let replacement = parse_and_serialize_character_v2_replace(
            &replacement_json,
            original_id,
            &original.relative_path,
            &[reference],
        )
        .unwrap();
        assert_eq!(replacement.character_id, original_id);
        assert_eq!(replacement.relative_path, original.relative_path);
        assert!(replacement.markdown.contains("[林遥](<林遥.md>) · 同盟"));

        let record = WorkflowRunRecord {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("openai".to_owned()),
            preset_version: Some(1),
            model: Some("gpt-5.6-terra".to_owned()),
            manifest_hash: Some(prepared.manifest.manifest_hash.clone()),
            proposal_hash: None,
            target_relative_path: Some(target.relative_path.clone()),
            target_source_hash: Some(target.content_hash.clone()),
            confirmed_at: None,
            updated_at: 1,
        };
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: Uuid::new_v4().to_string(),
            workflow_id: CHARACTER_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 2,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            run_id: run_id.clone(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            manifest_hash: prepared.manifest.manifest_hash.clone(),
            operation_kind: CanonFileOperationKind::Replace,
            relative_path: target.relative_path.clone(),
            expected_source_hash: Some(target.content_hash),
            original_content: Some(target.content),
            proposed_content: replacement.markdown.clone(),
            warning_codes: Vec::new(),
        })
        .unwrap();
        let proposal_hash = proposal.metadata().proposal_hash.clone();
        {
            let mut manager_guard = registry.workflow_manager.lock().unwrap();
            let manager = manager_guard.as_mut().unwrap();
            manager.create_prepared(record, prepared.clone()).unwrap();
            manager
                .advance_prepared_to_context_ready(&run_id, 2)
                .unwrap();
            manager
                .confirm_generation(&run_id, &prepared.manifest.manifest_hash, true, 3)
                .unwrap();
            let ready = manager
                .complete_generation_with_proposal(&run_id, proposal, 4)
                .unwrap();
            registry.app_state.save_workflow_run(&ready).unwrap();
        }
        registry
            .workflow_sessions
            .lock()
            .unwrap()
            .insert(run_id.clone(), session.session_id.clone());

        let completed =
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true).unwrap();
        assert_eq!(completed.run.unwrap().state, "completed");
        assert_eq!(
            fs::read_to_string(root.join(&replacement.relative_path)).unwrap(),
            replacement.markdown
        );
        assert!(!root.join("characters/改名后人物.md").exists());
        assert_eq!(
            fs::read(root.join(&related.relative_path)).unwrap(),
            related_before
        );

        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn confirmed_setting_apply_writes_only_the_hash_bound_rust_artifact() {
        let (root, app_state, registry, session, handle) = fixture();
        let project = read_project_file(&handle, "project.md").unwrap();
        let run_id = Uuid::new_v4().to_string();
        let prepared = prepare_setting_context(PrepareSettingContextInput {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: SETTING_CREATE_WORKFLOW_ID.to_owned(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            brief: SettingBriefV1 {
                schema_version: 1,
                display_name: "雾城".to_owned(),
                setting_type: SettingTypeV1::Location,
                objective: "建立压迫感".to_owned(),
                must_include: vec!["终年浓雾".to_owned()],
                constraints: None,
            },
            sources: vec![ContextSourceInput {
                relative_path: project.relative_path,
                content: project.content,
                content_hash: project.content_hash,
                source_byte_count: project.size_bytes,
            }],
        })
        .unwrap();
        let artifact = parse_and_serialize_setting(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": SETTING_CREATE_WORKFLOW_ID,
                "setting": {
                    "displayName": "雾城",
                    "settingType": "location",
                    "summary": "一座终年被浓雾笼罩的工业城市",
                    "atmosphere": "潮湿、幽暗，钟声控制居民行动",
                    "rules": ["第三次钟声后任何人不得离开室内"],
                    "constraints": ["机械装置不能自行运转"],
                    "plotHooks": ["钟楼记录与官方历法相差一天"]
                }
            })
            .to_string(),
            "setting_40000000-0000-4000-8000-000000000004",
        )
        .unwrap();
        let expected = artifact.markdown.clone();
        let record = WorkflowRunRecord {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: SETTING_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("openai".to_owned()),
            preset_version: Some(1),
            model: Some("gpt-5.6-terra".to_owned()),
            manifest_hash: Some(prepared.manifest.manifest_hash.clone()),
            proposal_hash: None,
            target_relative_path: None,
            target_source_hash: None,
            confirmed_at: None,
            updated_at: 1,
        };
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: Uuid::new_v4().to_string(),
            workflow_id: SETTING_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            run_id: run_id.clone(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            manifest_hash: prepared.manifest.manifest_hash.clone(),
            operation_kind: CanonFileOperationKind::Create,
            relative_path: artifact.relative_path,
            expected_source_hash: None,
            original_content: None,
            proposed_content: artifact.markdown,
            warning_codes: Vec::new(),
        })
        .unwrap();
        let proposal_hash = proposal.metadata().proposal_hash.clone();
        {
            let mut manager_guard = registry.workflow_manager.lock().unwrap();
            let manager = manager_guard.as_mut().unwrap();
            manager.create_prepared(record, prepared.clone()).unwrap();
            manager
                .advance_prepared_to_context_ready(&run_id, 2)
                .unwrap();
            manager
                .confirm_generation(&run_id, &prepared.manifest.manifest_hash, true, 3)
                .unwrap();
            let ready = manager
                .complete_generation_with_proposal(&run_id, proposal, 4)
                .unwrap();
            registry.app_state.save_workflow_run(&ready).unwrap();
        }
        registry
            .workflow_sessions
            .lock()
            .unwrap()
            .insert(run_id.clone(), session.session_id.clone());

        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, false),
            Err(AppError::workflow_confirmation_required())
        );
        assert!(!root.join("world/雾城.md").exists());
        let completed =
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true).unwrap();
        assert_eq!(completed.run.unwrap().state, "completed");
        assert_eq!(
            fs::read_to_string(root.join("world/雾城.md")).unwrap(),
            expected
        );

        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn confirmed_plot_create_writes_only_fixed_main_and_collision_is_zero_overwrite() {
        let (root, app_state, registry, session, handle) = fixture();
        let ledger_path = root.join("materials/000.story-ledger.md");
        let ledger_before = fs::read(&ledger_path).unwrap();
        let proposed = parse_and_serialize_plot(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": PLOT_CREATE_WORKFLOW_ID,
                "plot": {
                    "title": "退潮之塔",
                    "promise": "林遥追查潮汐塔的真相。",
                    "pressure": "每次退潮都缩短调查窗口。",
                    "turns": ["铜钥匙显字", "导师身份揭晓", "林遥主动选择公开真相"],
                    "reveals": ["潮汐塔以记忆维持封印"],
                    "payoffs": ["铜钥匙成为公开真相的见证"],
                    "openQuestions": ["导师是否留下了规避代价的方法？"]
                }
            })
            .to_string(),
            "plot_70000000-0000-4000-8000-000000000007",
        )
        .unwrap();
        let expected = proposed.markdown.clone();
        let (run_id, proposal_hash) =
            prepare_plot_proposal_fixture(&registry, &session.session_id, &handle, None, proposed);
        assert!(!root.join(MAIN_PLOT_RELATIVE_PATH).exists());
        fs::write(
            root.join(MAIN_PLOT_RELATIVE_PATH),
            "EXTERNAL_PLOT_COLLISION_CANARY",
        )
        .unwrap();
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_target_conflict())
        );
        assert_eq!(
            fs::read_to_string(root.join(MAIN_PLOT_RELATIVE_PATH)).unwrap(),
            "EXTERNAL_PLOT_COLLISION_CANARY"
        );
        fs::remove_file(root.join(MAIN_PLOT_RELATIVE_PATH)).unwrap();
        let completed =
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true).unwrap();
        assert_eq!(completed.run.unwrap().state, "completed");
        assert_eq!(
            fs::read_to_string(root.join(MAIN_PLOT_RELATIVE_PATH)).unwrap(),
            expected
        );
        assert_eq!(fs::read(&ledger_path).unwrap(), ledger_before);
        assert_eq!(fs::read_dir(root.join("plot")).unwrap().count(), 1);

        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn confirmed_plot_replace_preserves_id_and_path_and_rechecks_target_hash() {
        let (root, app_state, registry, session, handle) = fixture();
        let ledger_path = root.join("materials/000.story-ledger.md");
        let ledger_before = fs::read(&ledger_path).unwrap();
        let original = parse_and_serialize_plot(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": PLOT_CREATE_WORKFLOW_ID,
                "plot": {
                    "title": "旧主剧情",
                    "promise": "旧故事承诺",
                    "pressure": "旧核心压力",
                    "turns": ["旧转折一", "旧转折二", "旧转折三"],
                    "reveals": ["旧揭示"],
                    "payoffs": ["旧回收"],
                    "openQuestions": ["旧未决问题"]
                }
            })
            .to_string(),
            "plot_90000000-0000-4000-8000-000000000009",
        )
        .unwrap();
        fs::write(root.join(MAIN_PLOT_RELATIVE_PATH), &original.markdown).unwrap();
        let replacement = parse_and_serialize_plot_replace(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": PLOT_CREATE_WORKFLOW_ID,
                "plot": {
                    "title": "退潮之塔",
                    "promise": "林遥追查潮汐塔真相。",
                    "pressure": "最终退潮迫使林遥立即选择。",
                    "turns": ["铜钥匙显字", "导师身份揭晓", "林遥主动公开真相"],
                    "reveals": ["潮汐塔以记忆维持封印"],
                    "payoffs": ["铜钥匙成为新制度的见证"],
                    "openQuestions": ["导师留下的方法是否有效？"]
                }
            })
            .to_string(),
            &original.plot_id,
            MAIN_PLOT_RELATIVE_PATH,
        )
        .unwrap();
        let expected = replacement.markdown.clone();
        let (run_id, proposal_hash) = prepare_plot_proposal_fixture(
            &registry,
            &session.session_id,
            &handle,
            Some(&original),
            replacement,
        );
        fs::write(
            root.join(MAIN_PLOT_RELATIVE_PATH),
            "EXTERNAL_PLOT_CHANGE_CANARY",
        )
        .unwrap();
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_target_conflict())
        );
        assert_eq!(
            fs::read_to_string(root.join(MAIN_PLOT_RELATIVE_PATH)).unwrap(),
            "EXTERNAL_PLOT_CHANGE_CANARY"
        );
        fs::write(root.join(MAIN_PLOT_RELATIVE_PATH), &original.markdown).unwrap();
        let completed =
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true).unwrap();
        assert_eq!(completed.run.unwrap().state, "completed");
        let actual = fs::read_to_string(root.join(MAIN_PLOT_RELATIVE_PATH)).unwrap();
        assert_eq!(actual, expected);
        assert!(actual.contains(&original.plot_id));
        assert_eq!(fs::read(&ledger_path).unwrap(), ledger_before);

        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn confirmed_material_apply_writes_one_normal_file_and_never_changes_story_ledger() {
        let (root, app_state, registry, session, handle) = fixture();
        let ledger_path = root.join("materials/000.story-ledger.md");
        let ledger_before = fs::read(&ledger_path).unwrap();
        let project = read_project_file(&handle, "project.md").unwrap();
        let run_id = Uuid::new_v4().to_string();
        let prepared = prepare_material_context(PrepareMaterialContextInput {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: MATERIAL_CREATE_WORKFLOW_ID.to_owned(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            brief: MaterialBriefV1 {
                schema_version: 1,
                display_name: "退潮铜钥匙".to_owned(),
                material_type: MaterialTypeV1::Clue,
                seed: "退潮后才显出字迹的旧铜钥匙".to_owned(),
                intended_use: "作为第二幕调查线索".to_owned(),
                constraints: None,
            },
            sources: vec![ContextSourceInput {
                relative_path: project.relative_path,
                content: project.content,
                content_hash: project.content_hash,
                source_byte_count: project.size_bytes,
            }],
        })
        .unwrap();
        let artifact = parse_and_serialize_material(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": MATERIAL_CREATE_WORKFLOW_ID,
                "material": {
                    "displayName": "退潮铜钥匙",
                    "materialType": "clue",
                    "summary": "一枚只在退潮后显出字迹的旧铜钥匙。",
                    "content": "钥匙来自封闭多年的潮汐塔。",
                    "intendedUse": "作为第二幕调查线索。",
                    "canonReason": "遵守既有潮汐规则，并可在后续回收。"
                }
            })
            .to_string(),
            "material_70000000-0000-4000-8000-000000000007",
        )
        .unwrap();
        let expected_path = artifact.relative_path.clone();
        let expected = artifact.markdown.clone();
        let record = WorkflowRunRecord {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: MATERIAL_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("openai".to_owned()),
            preset_version: Some(1),
            model: Some("gpt-5.6-terra".to_owned()),
            manifest_hash: Some(prepared.manifest.manifest_hash.clone()),
            proposal_hash: None,
            target_relative_path: None,
            target_source_hash: None,
            confirmed_at: None,
            updated_at: 1,
        };
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: Uuid::new_v4().to_string(),
            workflow_id: MATERIAL_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            run_id: run_id.clone(),
            provider_id: "openai".to_owned(),
            preset_version: 1,
            model: "gpt-5.6-terra".to_owned(),
            manifest_hash: prepared.manifest.manifest_hash.clone(),
            operation_kind: CanonFileOperationKind::Create,
            relative_path: expected_path.clone(),
            expected_source_hash: None,
            original_content: None,
            proposed_content: artifact.markdown,
            warning_codes: Vec::new(),
        })
        .unwrap();
        let proposal_hash = proposal.metadata().proposal_hash.clone();
        {
            let mut manager_guard = registry.workflow_manager.lock().unwrap();
            let manager = manager_guard.as_mut().unwrap();
            manager.create_prepared(record, prepared.clone()).unwrap();
            manager
                .advance_prepared_to_context_ready(&run_id, 2)
                .unwrap();
            manager
                .confirm_generation(&run_id, &prepared.manifest.manifest_hash, true, 3)
                .unwrap();
            let ready = manager
                .complete_generation_with_proposal(&run_id, proposal, 4)
                .unwrap();
            registry.app_state.save_workflow_run(&ready).unwrap();
        }
        registry
            .workflow_sessions
            .lock()
            .unwrap()
            .insert(run_id.clone(), session.session_id.clone());

        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, false),
            Err(AppError::workflow_confirmation_required())
        );
        assert!(!root.join(&expected_path).exists());
        assert_eq!(fs::read(&ledger_path).unwrap(), ledger_before);

        let completed =
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true).unwrap();
        assert_eq!(completed.run.unwrap().state, "completed");
        assert_eq!(
            fs::read_to_string(root.join(&expected_path)).unwrap(),
            expected
        );
        assert_eq!(fs::read(&ledger_path).unwrap(), ledger_before);
        let material_entries = fs::read_dir(root.join("materials"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(material_entries.len(), 2);
        assert!(material_entries.iter().all(|name| !name.starts_with('.')));

        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn confirmed_material_replace_keeps_id_path_and_ledger_and_rechecks_target_hash() {
        let (root, app_state, registry, session, handle) = fixture();
        let ledger_path = root.join("materials/000.story-ledger.md");
        let ledger_before = fs::read(&ledger_path).unwrap();
        let original = parse_and_serialize_material(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": MATERIAL_CREATE_WORKFLOW_ID,
                "material": {
                    "displayName": "旧潮汐钥匙",
                    "materialType": "object",
                    "summary": "一枚锈蚀的旧钥匙。",
                    "content": "钥匙柄刻着已经废止的潮汐塔编号。",
                    "intendedUse": "作为第一幕留下的未解物件。",
                    "canonReason": "与既有潮汐塔设定一致。"
                }
            })
            .to_string(),
            "material_90000000-0000-4000-8000-000000000009",
        )
        .unwrap();
        fs::write(root.join(&original.relative_path), &original.markdown).unwrap();
        let project = read_project_file(&handle, "project.md").unwrap();
        let target = read_project_file(&handle, &original.relative_path).unwrap();
        let original_hash = target.content_hash.clone();
        let run_id = Uuid::new_v4().to_string();
        let prepared = prepare_material_context(PrepareMaterialContextInput {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: MATERIAL_CREATE_WORKFLOW_ID.to_owned(),
            provider_id: "deepseek".to_owned(),
            preset_version: 1,
            model: "deepseek-v4-pro".to_owned(),
            brief: MaterialBriefV1 {
                schema_version: 1,
                display_name: "焕新潮汐钥匙".to_owned(),
                material_type: MaterialTypeV1::Clue,
                seed: "保留原钥匙身份，补入退潮显字线索".to_owned(),
                intended_use: "作为第二幕调查线索".to_owned(),
                constraints: None,
            },
            sources: vec![ContextSourceInput {
                relative_path: project.relative_path,
                content: project.content,
                content_hash: project.content_hash,
                source_byte_count: project.size_bytes,
            }],
        })
        .unwrap();
        let replacement = parse_and_serialize_material_replace(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": MATERIAL_CREATE_WORKFLOW_ID,
                "material": {
                    "displayName": "焕新潮汐钥匙",
                    "materialType": "clue",
                    "summary": "一枚退潮后才显出字迹的旧钥匙。",
                    "content": "钥匙柄的潮汐塔编号会在盐水退去后显出第二层刻痕。",
                    "intendedUse": "作为第二幕调查线索。",
                    "canonReason": "保留原物件身份，并加强既有潮汐规则的回收。"
                }
            })
            .to_string(),
            &original.material_id,
            &original.relative_path,
        )
        .unwrap();
        let expected = replacement.markdown.clone();
        let record = WorkflowRunRecord {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: MATERIAL_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("deepseek".to_owned()),
            preset_version: Some(1),
            model: Some("deepseek-v4-pro".to_owned()),
            manifest_hash: Some(prepared.manifest.manifest_hash.clone()),
            proposal_hash: None,
            target_relative_path: Some(original.relative_path.clone()),
            target_source_hash: Some(original_hash.clone()),
            confirmed_at: None,
            updated_at: 1,
        };
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: Uuid::new_v4().to_string(),
            workflow_id: MATERIAL_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            run_id: run_id.clone(),
            provider_id: "deepseek".to_owned(),
            preset_version: 1,
            model: "deepseek-v4-pro".to_owned(),
            manifest_hash: prepared.manifest.manifest_hash.clone(),
            operation_kind: CanonFileOperationKind::Replace,
            relative_path: original.relative_path.clone(),
            expected_source_hash: Some(original_hash),
            original_content: Some(original.markdown.clone()),
            proposed_content: replacement.markdown,
            warning_codes: Vec::new(),
        })
        .unwrap();
        assert!(!proposal.view().line_diff.is_empty());
        let proposal_hash = proposal.metadata().proposal_hash.clone();
        {
            let mut manager_guard = registry.workflow_manager.lock().unwrap();
            let manager = manager_guard.as_mut().unwrap();
            manager.create_prepared(record, prepared.clone()).unwrap();
            manager
                .advance_prepared_to_context_ready(&run_id, 2)
                .unwrap();
            manager
                .confirm_generation(&run_id, &prepared.manifest.manifest_hash, true, 3)
                .unwrap();
            let ready = manager
                .complete_generation_with_proposal(&run_id, proposal, 4)
                .unwrap();
            registry.app_state.save_workflow_run(&ready).unwrap();
        }
        registry
            .workflow_sessions
            .lock()
            .unwrap()
            .insert(run_id.clone(), session.session_id.clone());

        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, false),
            Err(AppError::workflow_confirmation_required())
        );
        assert_eq!(
            fs::read_to_string(root.join(&original.relative_path)).unwrap(),
            original.markdown
        );
        fs::write(
            root.join(&original.relative_path),
            "EXTERNAL_MATERIAL_CHANGE_CANARY",
        )
        .unwrap();
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_target_conflict())
        );
        assert_eq!(
            fs::read_to_string(root.join(&original.relative_path)).unwrap(),
            "EXTERNAL_MATERIAL_CHANGE_CANARY"
        );
        assert_eq!(fs::read(&ledger_path).unwrap(), ledger_before);
        fs::write(root.join(&original.relative_path), &original.markdown).unwrap();
        let completed =
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true).unwrap();
        assert_eq!(completed.run.unwrap().state, "completed");
        assert_eq!(
            fs::read_to_string(root.join(&original.relative_path)).unwrap(),
            expected
        );
        assert!(expected.contains(&original.material_id));
        assert_eq!(replacement.relative_path, original.relative_path);
        assert_eq!(replacement.material_id, original.material_id);
        assert_eq!(fs::read(&ledger_path).unwrap(), ledger_before);

        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn confirmed_setting_replace_keeps_id_and_path_and_rechecks_source_hash() {
        let (root, app_state, registry, session, handle) = fixture();
        let original = parse_and_serialize_setting(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": SETTING_CREATE_WORKFLOW_ID,
                "setting": {
                    "displayName": "旧都",
                    "settingType": "location",
                    "summary": "一座由旧钟律管理的城市",
                    "atmosphere": "潮湿而安静",
                    "rules": ["第三次钟声后不得外出"],
                    "constraints": ["钟楼不能停摆"],
                    "plotHooks": ["档案里缺少一天记录"]
                }
            })
            .to_string(),
            "setting_60000000-0000-4000-8000-000000000006",
        )
        .unwrap();
        fs::write(root.join(&original.relative_path), &original.markdown).unwrap();
        let project = read_project_file(&handle, "project.md").unwrap();
        let target = read_project_file(&handle, &original.relative_path).unwrap();
        let original_hash = target.content_hash.clone();
        let run_id = Uuid::new_v4().to_string();
        let prepared = prepare_setting_context(PrepareSettingContextInput {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: SETTING_CREATE_WORKFLOW_ID.to_owned(),
            provider_id: "deepseek".to_owned(),
            preset_version: 1,
            model: "deepseek-v4-pro".to_owned(),
            brief: SettingBriefV1 {
                schema_version: 1,
                display_name: "焕新旧都".to_owned(),
                setting_type: SettingTypeV1::Location,
                objective: "修订钟律".to_owned(),
                must_include: vec!["保留历史连续性".to_owned()],
                constraints: None,
            },
            sources: vec![
                ContextSourceInput {
                    relative_path: project.relative_path,
                    content: project.content,
                    content_hash: project.content_hash,
                    source_byte_count: project.size_bytes,
                },
                ContextSourceInput {
                    relative_path: target.relative_path,
                    content: target.content,
                    content_hash: target.content_hash,
                    source_byte_count: target.size_bytes,
                },
            ],
        })
        .unwrap();
        let replacement = parse_and_serialize_setting_replace(
            &serde_json::json!({
                "schemaVersion": 1,
                "workflowId": SETTING_CREATE_WORKFLOW_ID,
                "setting": {
                    "displayName": "焕新旧都",
                    "settingType": "location",
                    "summary": "旧都在新钟律下恢复夜间交通",
                    "atmosphere": "潮湿，但街道重新出现灯火",
                    "rules": ["第四次钟声后主街仍可通行"],
                    "constraints": ["钟楼仍是唯一授时来源"],
                    "plotHooks": ["缺失记录指向被封存的第四口钟"]
                }
            })
            .to_string(),
            &original.setting_id,
            &original.relative_path,
        )
        .unwrap();
        let expected = replacement.markdown.clone();
        let record = WorkflowRunRecord {
            run_id: run_id.clone(),
            project_id: handle.project_id().to_owned(),
            workflow_id: SETTING_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            state: WorkflowRunState::Created,
            sequence: 0,
            provider_id: Some("deepseek".to_owned()),
            preset_version: Some(1),
            model: Some("deepseek-v4-pro".to_owned()),
            manifest_hash: Some(prepared.manifest.manifest_hash.clone()),
            proposal_hash: None,
            target_relative_path: Some(original.relative_path.clone()),
            target_source_hash: Some(original_hash.clone()),
            confirmed_at: None,
            updated_at: 1,
        };
        let proposal = build_canon_proposal(BuildCanonProposalInput {
            proposal_id: Uuid::new_v4().to_string(),
            workflow_id: SETTING_CREATE_WORKFLOW_ID.to_owned(),
            workflow_version: 1,
            pack_hash: embedded_workflow_registry().pack_hash.to_owned(),
            run_id: run_id.clone(),
            provider_id: "deepseek".to_owned(),
            preset_version: 1,
            model: "deepseek-v4-pro".to_owned(),
            manifest_hash: prepared.manifest.manifest_hash.clone(),
            operation_kind: CanonFileOperationKind::Replace,
            relative_path: original.relative_path.clone(),
            expected_source_hash: Some(original_hash),
            original_content: Some(original.markdown.clone()),
            proposed_content: replacement.markdown,
            warning_codes: Vec::new(),
        })
        .unwrap();
        assert!(!proposal.view().line_diff.is_empty());
        let proposal_hash = proposal.metadata().proposal_hash.clone();
        {
            let mut manager_guard = registry.workflow_manager.lock().unwrap();
            let manager = manager_guard.as_mut().unwrap();
            manager.create_prepared(record, prepared.clone()).unwrap();
            manager
                .advance_prepared_to_context_ready(&run_id, 2)
                .unwrap();
            manager
                .confirm_generation(&run_id, &prepared.manifest.manifest_hash, true, 3)
                .unwrap();
            let ready = manager
                .complete_generation_with_proposal(&run_id, proposal, 4)
                .unwrap();
            registry.app_state.save_workflow_run(&ready).unwrap();
        }
        registry
            .workflow_sessions
            .lock()
            .unwrap()
            .insert(run_id.clone(), session.session_id.clone());

        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, false),
            Err(AppError::workflow_confirmation_required())
        );
        assert_eq!(
            fs::read_to_string(root.join(&original.relative_path)).unwrap(),
            original.markdown
        );
        fs::write(root.join(&original.relative_path), "EXTERNAL_CHANGE_CANARY").unwrap();
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_context_changed())
        );
        assert_eq!(
            fs::read_to_string(root.join(&original.relative_path)).unwrap(),
            "EXTERNAL_CHANGE_CANARY"
        );
        fs::write(root.join(&original.relative_path), &original.markdown).unwrap();
        let completed =
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true).unwrap();
        assert_eq!(completed.run.unwrap().state, "completed");
        assert_eq!(
            fs::read_to_string(root.join(&original.relative_path)).unwrap(),
            expected
        );
        assert!(expected.contains(&original.setting_id));

        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_collision_never_overwrites_the_existing_target() {
        let (root, app_state, registry, session, handle) = fixture();
        let proposed = markdown("冲突人物");
        let (run_id, proposal_hash) = prepare_proposal_fixture(
            &registry,
            &session.session_id,
            &handle,
            CanonFileOperationKind::Create,
            "characters/冲突人物.md",
            None,
            &proposed,
        );
        fs::write(root.join("characters/冲突人物.md"), markdown("外部人物")).unwrap();
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_target_conflict())
        );
        assert!(fs::read_to_string(root.join("characters/冲突人物.md"))
            .unwrap()
            .contains("外部人物"));
        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn context_change_open_target_read_only_and_recovery_sessions_make_zero_writes() {
        let (root, app_state, registry, session, handle) = fixture();
        let relative_path = "characters/上下文变化.md";
        let (run_id, proposal_hash) = prepare_proposal_fixture(
            &registry,
            &session.session_id,
            &handle,
            CanonFileOperationKind::Create,
            relative_path,
            None,
            &markdown("上下文变化"),
        );
        fs::write(root.join("project.md"), markdown("被外部修改的项目")).unwrap();
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_context_changed())
        );
        assert!(!root.join(relative_path).exists());
        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();

        let (root, app_state, registry, session, handle) = fixture();
        let relative_path = "characters/当前编辑.md";
        let (run_id, proposal_hash) = prepare_proposal_fixture(
            &registry,
            &session.session_id,
            &handle,
            CanonFileOperationKind::Create,
            relative_path,
            None,
            &markdown("当前编辑"),
        );
        registry
            .app_state
            .record_current_file(handle.project_id(), relative_path)
            .unwrap();
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_target_conflict())
        );
        assert!(!root.join(relative_path).exists());
        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();

        let (root, app_state, registry, session, handle) = fixture();
        let relative_path = "characters/只读会话.md";
        let (run_id, proposal_hash) = prepare_proposal_fixture(
            &registry,
            &session.session_id,
            &handle,
            CanonFileOperationKind::Create,
            relative_path,
            None,
            &markdown("只读会话"),
        );
        registry
            .inner
            .lock()
            .unwrap()
            .sessions
            .get_mut(&session.session_id)
            .unwrap()
            .write_lease
            .take();
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_writer_required())
        );
        assert!(!root.join(relative_path).exists());
        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();

        let (root, app_state, registry, session, handle) = fixture();
        let relative_path = "characters/恢复阻断.md";
        let (run_id, proposal_hash) = prepare_proposal_fixture(
            &registry,
            &session.session_id,
            &handle,
            CanonFileOperationKind::Create,
            relative_path,
            None,
            &markdown("恢复阻断"),
        );
        registry
            .inner
            .lock()
            .unwrap()
            .sessions
            .get_mut(&session.session_id)
            .unwrap()
            .recovery = RecoveryReport {
            status: RecoveryStatus::Required,
            recovered_transactions: 0,
            required_transactions: 1,
        };
        assert_eq!(
            apply_registered_workflow_proposal(&registry, &run_id, &proposal_hash, true),
            Err(AppError::workflow_recovery_required())
        );
        assert!(!root.join(relative_path).exists());
        drop(registry);
        fs::remove_dir_all(app_state).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
