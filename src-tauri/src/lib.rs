// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

mod commands;
mod connections;
mod db;
mod error;

use connections::pool_manager::ConnectionManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialise the SQLite pool synchronously via a blocking call during setup.
            let sqlite = tauri::async_runtime::block_on(db::init_pool())
                .expect("Failed to initialise local SQLite database");

            let connection_manager = ConnectionManager::new();

            app.manage(sqlite);
            app.manage(connection_manager);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Connection profiles
            commands::connections::connections_list,
            commands::connections::connections_create,
            commands::connections::connections_update,
            commands::connections::connections_delete,
            commands::connections::connections_test,
            commands::connections::connections_connect,
            commands::connections::connections_disconnect,
            commands::connections::connections_list_active,
            commands::connections::connection_groups_list,
            commands::connections::connection_groups_create,
            commands::connections::connection_groups_delete,
            commands::connections::connection_groups_update,
            commands::connections::connection_groups_reorder,
            // Query execution
            commands::query::query_execute,
            commands::query::query_execute_selection,
            commands::query::query_update_rows,
            // Schema introspection
            commands::schema::schema_list_databases,
            commands::schema::schema_list_tables,
            commands::schema::schema_list_columns,
            commands::schema::schema_get_ddl,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rowmance");
}
