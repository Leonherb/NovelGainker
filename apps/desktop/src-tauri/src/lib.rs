use tauri::Manager;

mod command_manifest;
#[cfg(not(test))]
#[path = "commands_current.rs"]
mod commands;
#[cfg(test)]
#[path = "commands_current.rs"]
pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    debug_assert_eq!(app_core::application_name(), "NovelGainker");
    debug_assert_eq!(novelgainker_domain::DOMAIN_CONTRACT_VERSION, 1);
    debug_assert_eq!(
        command_manifest::APP_COMMANDS,
        [
            "health_check",
            "app_state_load",
            "app_state_save_ui",
            "conversation_session_load",
            "conversation_turn_start",
            "conversation_turn_cancel",
            "canon_change_list",
            "canon_change_detail",
            "canon_change_decide",
            "canon_editor_state",
            "publication_list",
            "publication_detail",
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
            "project_close",
            "conversation_session_list",
            "conversation_session_create",
            "conversation_session_rename",
            "conversation_session_activate",
            "conversation_session_manage",
            "conversation_export_prepare",
            "conversation_export_commit",
            "conversation_turn_retry"
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
            commands::conversation_session_load,
            commands::conversation_turn_start,
            commands::conversation_turn_cancel,
            commands::canon_change_list,
            commands::canon_change_detail,
            commands::canon_change_decide,
            commands::canon_editor_state,
            commands::publication_list,
            commands::publication_detail,
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
            commands::project_close,
            commands::conversation_session_list,
            commands::conversation_session_create,
            commands::conversation_session_rename,
            commands::conversation_session_activate,
            commands::conversation_session_manage,
            commands::conversation_export_prepare,
            commands::conversation_export_commit,
            commands::conversation_turn_retry
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window
                    .state::<commands::ProjectRegistry>()
                    .cancel_active_provider_run();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run NovelGainker desktop application");
}
