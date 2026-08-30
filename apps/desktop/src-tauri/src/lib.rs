use tauri::Manager;

mod command_manifest;
#[cfg(not(test))]
mod commands;
#[cfg(test)]
pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    debug_assert_eq!(app_core::application_name(), "NovelGainker");
    debug_assert_eq!(novelgainker_workflows::registry().workflows.len(), 6);
    debug_assert_eq!(novelgainker_domain::DOMAIN_CONTRACT_VERSION, 1);
    debug_assert_eq!(
        command_manifest::APP_COMMANDS,
        [
            "health_check",
            "app_state_load",
            "app_state_save_ui",
            "agent_skill_registry_load",
            "agent_session_load",
            "agent_turn_start",
            "agent_turn_cancel",
            "agent_session_clear",
            "agent_memory_compact",
            "agent_history_branch_resolve",
            "agent_write_policy_set",
            "agent_context_mark_upsert",
            "agent_context_mark_remove",
            "agent_conflict_resolution_submit",
            "agent_write_confirmation_submit",
            "agent_write_plan_reject",
            "agent_write_undo_apply",
            "provider_status_load",
            "provider_key_save",
            "provider_key_delete",
            "provider_select",
            "provider_connection_test",
            "project_scan",
            "project_create_preview",
            "project_create",
            "project_open",
            "project_open_recent",
            "project_list_files",
            "project_read_file",
            "app_state_set_current_file",
            "project_watch_file",
            "project_save_file",
            "project_validate_draft",
            "project_search",
            "project_close"
        ]
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_state_root = app.path().app_data_dir()?;
            let registry = commands::ProjectRegistry::new(app_state_root)
                .map_err(|_| std::io::Error::other("project registry initialization failed"))?;
            app.manage(registry);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::app_state_load,
            commands::app_state_save_ui,
            commands::agent_commands::agent_skill_registry_load,
            commands::agent_commands::agent_session_load,
            commands::agent_commands::agent_turn_start,
            commands::agent_commands::agent_turn_cancel,
            commands::agent_commands::agent_session_clear,
            commands::agent_commands::agent_memory_compact,
            commands::agent_commands::agent_history_branch_resolve,
            commands::agent_commands::agent_write_policy_set,
            commands::agent_commands::agent_context_mark_upsert,
            commands::agent_commands::agent_context_mark_remove,
            commands::agent_commands::agent_conflict_resolution_submit,
            commands::agent_commands::agent_write_confirmation_submit,
            commands::agent_commands::agent_write_plan_reject,
            commands::agent_commands::agent_write_undo_apply,
            commands::provider_status_load,
            commands::provider_key_save,
            commands::provider_key_delete,
            commands::provider_select,
            commands::provider_connection_test,
            commands::project_scan,
            commands::project_create_preview,
            commands::project_create,
            commands::project_open,
            commands::project_open_recent,
            commands::project_list_files,
            commands::project_read_file,
            commands::app_state_set_current_file,
            commands::project_watch_file,
            commands::project_save_file,
            commands::project_validate_draft,
            commands::project_search,
            commands::project_close
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window
                    .state::<commands::ProjectRegistry>()
                    .cancel_active_provider_run();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run NovelGainker desktop application");
}
