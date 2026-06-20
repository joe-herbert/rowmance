// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

mod commands;
mod connections;
mod db;
mod error;
mod lib_sql;

use connections::pool_manager::ConnectionManager;
use connections::ssh_tunnel::SshTunnelManager;
use tauri::{Emitter, Manager};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, Some("cmd+,"))?;
            let app_submenu = Submenu::with_items(app, "Rowmance", true, &[&settings_item])?;
            let edit_submenu = Submenu::with_items(app, "Edit", true, &[
                &PredefinedMenuItem::undo(app, None)?,
                &PredefinedMenuItem::redo(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::cut(app, None)?,
                &PredefinedMenuItem::copy(app, None)?,
                &PredefinedMenuItem::paste(app, None)?,
                &PredefinedMenuItem::select_all(app, None)?,
            ])?;
            let menu = Menu::with_items(app, &[&app_submenu, &edit_submenu])?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| {
                if event.id() == "settings" {
                    let _ = app.emit("menu:open-settings", ());
                }
            });

            // Initialise the SQLite pool synchronously via a blocking call during setup.
            let sqlite = tauri::async_runtime::block_on(db::init_pool())
                .expect("Failed to initialise local SQLite database");

            // Migrate any existing plaintext passwords to the OS keychain.
            tauri::async_runtime::block_on(
                commands::keychain::migrate_passwords_to_keychain(&sqlite),
            );

            let connection_manager = ConnectionManager::new();
            let ssh_tunnel_manager = SshTunnelManager::new();

            app.manage(sqlite);
            app.manage(connection_manager);
            app.manage(ssh_tunnel_manager);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Connection profiles
            commands::connections::connections_list,
            commands::connections::connections_create,
            commands::connections::connections_update,
            commands::connections::connections_delete,
            commands::connections::connections_test,
            commands::connections::connections_test_unsaved,
            commands::connections::connections_connect,
            commands::connections::connections_disconnect,
            commands::connections::connections_list_active,
            commands::connections::connection_groups_list,
            commands::connections::connection_groups_create,
            commands::connections::connection_groups_delete,
            commands::connections::connection_groups_update,
            commands::connections::connection_groups_reorder,
            // Keychain
            commands::keychain::keychain_store,
            commands::keychain::keychain_retrieve,
            commands::keychain::keychain_delete,
            // SSH tunnels
            commands::ssh::ssh_create_tunnel,
            commands::ssh::ssh_destroy_tunnel,
            commands::ssh::ssh_tunnel_status,
            // Query execution
            commands::query::query_execute,
            commands::query::query_execute_selection,
            commands::query::query_update_rows,
            commands::query::query_delete_rows,
            commands::query::query_insert_row,
            commands::query::query_format,
            // Schema introspection & modification
            commands::schema::schema_list_databases,
            commands::schema::schema_list_tables,
            commands::schema::schema_list_columns,
            commands::schema::schema_list_indexes,
            commands::schema::schema_list_foreign_keys,
            commands::schema::schema_get_ddl,
            commands::schema::schema_execute_ddl,
            // Query history
            commands::history::history_list,
            commands::history::history_clear,
            commands::history::history_delete,
            // Saved queries
            commands::saved_queries::saved_queries_list_folders,
            commands::saved_queries::saved_queries_create_folder,
            commands::saved_queries::saved_queries_update_folder,
            commands::saved_queries::saved_queries_delete_folder,
            commands::saved_queries::saved_queries_list,
            commands::saved_queries::saved_queries_create,
            commands::saved_queries::saved_queries_update,
            commands::saved_queries::saved_queries_delete,
            // Settings
            commands::settings::settings_get_all,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::settings::settings_reset,
            // Export
            commands::export::export_result_to_clipboard,
            commands::export::export_result_to_file,
            // Import
            commands::import::import_csv_preview,
            commands::import::import_csv_preview_text,
            commands::import::import_csv_execute,
            commands::import::import_csv_execute_text,
            commands::import::import_sql_file,
            commands::import::import_sql_text,
            // ERD
            commands::erd::erd_get_graph,
            // Themes
            commands::themes::themes_list,
            commands::themes::themes_read,
            commands::themes::themes_write,
            commands::themes::themes_delete,
            commands::themes::themes_duplicate,
            commands::themes::themes_rename,
            commands::themes::themes_export,
            commands::themes::themes_import,
            // Updater
            commands::updater::updater_check,
            commands::updater::updater_install,
            // Window
            commands::window::window_set_traffic_light_position,
            // Explain
            commands::query::query_explain,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rowmance");
}
