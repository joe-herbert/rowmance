// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod connections;
mod db;
mod error;
mod lib_sql;
mod sessions;
mod transactions;

use connections::pool_manager::ConnectionManager;
use connections::ssh_tunnel::SshTunnelManager;
use sessions::SessionManager;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use transactions::TransactionManager;

const RELEASES_URL: &str = "https://github.com/joe-herbert/rowmance/releases";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // ── Rowmance app menu ──────────────────────────────────────────
            let settings_item =
                MenuItem::with_id(app, "settings", "Settings", true, Some("cmd+,"))?;
            let check_updates_item = MenuItem::with_id(
                app,
                "check-updates",
                "Check for Updates…",
                true,
                None::<&str>,
            )?;
            let app_submenu = Submenu::with_items(
                app,
                "Rowmance",
                true,
                &[
                    &settings_item,
                    &PredefinedMenuItem::separator(app)?,
                    &check_updates_item,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::show_all(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, None)?,
                ],
            )?;

            // ── File menu ──────────────────────────────────────────────────
            let open_file_item =
                MenuItem::with_id(app, "open-file", "Open File…", true, Some("cmd+o"))?;
            let new_query_item =
                MenuItem::with_id(app, "new-query", "New Query Editor", true, None::<&str>)?;
            let new_window_file_item =
                MenuItem::with_id(app, "new-window", "New Window", true, None::<&str>)?;
            let import_csv_item =
                MenuItem::with_id(app, "import-csv", "Import CSV…", true, None::<&str>)?;
            let import_sql_item =
                MenuItem::with_id(app, "import-sql", "Import SQL…", true, None::<&str>)?;
            let file_submenu = Submenu::with_items(
                app,
                "File",
                true,
                &[
                    &open_file_item,
                    &PredefinedMenuItem::separator(app)?,
                    &new_query_item,
                    &new_window_file_item,
                    &PredefinedMenuItem::separator(app)?,
                    &import_csv_item,
                    &import_sql_item,
                ],
            )?;

            // ── Edit menu ──────────────────────────────────────────────────
            let edit_submenu = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?;

            // ── View menu ──────────────────────────────────────────────────
            let toggle_left_item = MenuItem::with_id(
                app,
                "toggle-left-sidebar",
                "Toggle Left Sidebar",
                true,
                None::<&str>,
            )?;
            let toggle_right_item = MenuItem::with_id(
                app,
                "toggle-right-sidebar",
                "Toggle Right Sidebar",
                true,
                None::<&str>,
            )?;
            let toggle_system_item = MenuItem::with_id(
                app,
                "toggle-system-items",
                "Toggle System Items",
                true,
                None::<&str>,
            )?;
            let command_palette_item = MenuItem::with_id(
                app,
                "command-palette",
                "Command Palette",
                true,
                None::<&str>,
            )?;
            let split_right_item =
                MenuItem::with_id(app, "split-right", "Split Right", true, None::<&str>)?;
            let split_down_item =
                MenuItem::with_id(app, "split-down", "Split Down", true, None::<&str>)?;
            let split_close_item =
                MenuItem::with_id(app, "split-close", "Close Split", true, None::<&str>)?;
            let view_submenu = Submenu::with_items(
                app,
                "View",
                true,
                &[
                    &toggle_left_item,
                    &toggle_right_item,
                    &PredefinedMenuItem::separator(app)?,
                    &toggle_system_item,
                    &PredefinedMenuItem::separator(app)?,
                    &command_palette_item,
                    &PredefinedMenuItem::separator(app)?,
                    &split_right_item,
                    &split_down_item,
                    &split_close_item,
                ],
            )?;

            // ── Window menu ────────────────────────────────────────────────
            let new_window_win_item =
                MenuItem::with_id(app, "new-window-win", "New Window", true, None::<&str>)?;
            let window_submenu = Submenu::with_items(
                app,
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(app, None)?,
                    &PredefinedMenuItem::maximize(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &new_window_win_item,
                ],
            )?;

            // ── Help menu ──────────────────────────────────────────────────
            let help_updates_item = MenuItem::with_id(
                app,
                "help-check-updates",
                "Check for Updates…",
                true,
                None::<&str>,
            )?;
            let help_whats_new_item =
                MenuItem::with_id(app, "help-whats-new", "What's New…", true, None::<&str>)?;
            let help_view_releases_item =
                MenuItem::with_id(app, "help-view-releases", "View Releases", true, None::<&str>)?;
            let help_submenu = Submenu::with_items(
                app,
                "Help",
                true,
                &[
                    &help_whats_new_item,
                    &help_updates_item,
                    &help_view_releases_item,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::about(app, None, None)?,
                ],
            )?;

            // ── Developer menu (debug builds only) ─────────────────────────
            #[cfg(debug_assertions)]
            let dev_speed_analysis_item = MenuItem::with_id(
                app,
                "dev-speed-analysis",
                "Speed Analysis",
                true,
                None::<&str>,
            )?;
            #[cfg(debug_assertions)]
            let dev_submenu =
                Submenu::with_items(app, "Developer", true, &[&dev_speed_analysis_item])?;

            #[cfg(not(debug_assertions))]
            let menu = Menu::with_items(
                app,
                &[
                    &app_submenu,
                    &file_submenu,
                    &edit_submenu,
                    &view_submenu,
                    &window_submenu,
                    &help_submenu,
                ],
            )?;
            #[cfg(debug_assertions)]
            let menu = Menu::with_items(
                app,
                &[
                    &app_submenu,
                    &file_submenu,
                    &edit_submenu,
                    &view_submenu,
                    &window_submenu,
                    &help_submenu,
                    &dev_submenu,
                ],
            )?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| {
                let id = event.id().as_ref();
                match id {
                    "settings" => {
                        let _ = app.emit("menu:open-settings", ());
                    }
                    "check-updates" | "help-check-updates" => {
                        let _ = app.emit("menu:check-updates", ());
                    }
                    "help-whats-new" => {
                        let _ = app.emit("menu:whats-new", ());
                    }
                    "help-view-releases" => {
                        let _ = app.opener().open_url(RELEASES_URL, None::<&str>);
                    }
                    "new-query" => {
                        let _ = app.emit("menu:new-query", ());
                    }
                    "new-window" | "new-window-win" => {
                        let _ = app.emit("menu:new-window", ());
                    }
                    "open-file" => {
                        let _ = app.emit("menu:open-file", ());
                    }
                    "import-csv" => {
                        let _ = app.emit("menu:import-csv", ());
                    }
                    "import-sql" => {
                        let _ = app.emit("menu:import-sql", ());
                    }
                    "toggle-left-sidebar" => {
                        let _ = app.emit("menu:toggle-left-sidebar", ());
                    }
                    "toggle-right-sidebar" => {
                        let _ = app.emit("menu:toggle-right-sidebar", ());
                    }
                    "toggle-system-items" => {
                        let _ = app.emit("menu:toggle-system-items", ());
                    }
                    "command-palette" => {
                        let _ = app.emit("menu:command-palette", ());
                    }
                    "split-right" => {
                        let _ = app.emit("menu:split-right", ());
                    }
                    "split-down" => {
                        let _ = app.emit("menu:split-down", ());
                    }
                    "split-close" => {
                        let _ = app.emit("menu:split-close", ());
                    }
                    #[cfg(debug_assertions)]
                    "dev-speed-analysis" => {
                        let _ = app.emit("menu:speed-analysis", ());
                    }
                    _ => {}
                }
            });

            // Initialise the SQLite pool synchronously via a blocking call during setup.
            let sqlite = tauri::async_runtime::block_on(db::init_pool())
                .expect("Failed to initialise local SQLite database");

            // Migrate any existing plaintext passwords to the OS keychain.
            tauri::async_runtime::block_on(commands::keychain::migrate_passwords_to_keychain(
                &sqlite,
            ));

            let connection_manager = ConnectionManager::new();
            let ssh_tunnel_manager = SshTunnelManager::new();

            // Pre-register all profile names so error messages use the human-readable
            // name even for connections that haven't been opened yet this session.
            let _ = tauri::async_runtime::block_on(async {
                let rows: Vec<(String, String)> =
                    sqlx::query_as("SELECT id, name FROM connection_profiles")
                        .fetch_all(&sqlite)
                        .await?;
                for (id, name) in rows {
                    connection_manager.register_name(&id, &name);
                }
                Ok::<_, sqlx::Error>(())
            });

            let transaction_manager = TransactionManager::new();
            let session_manager = SessionManager::new();

            app.manage(sqlite);
            app.manage(connection_manager);
            app.manage(ssh_tunnel_manager);
            app.manage(transaction_manager);
            app.manage(session_manager);

            // Swizzle _NSTitlebarContainerView.setFrame: so macOS applies our custom
            // titlebar height/position synchronously on every resize (no JS round-trip,
            // no flicker).
            #[cfg(target_os = "macos")]
            commands::window::install_titlebar_swizzle();

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
            commands::connections::connections_connect_unsaved,
            commands::connections::connections_disconnect,
            commands::connections::connections_list_active,
            commands::connections::connection_groups_list,
            commands::connections::connection_groups_create,
            commands::connections::connection_groups_delete,
            commands::connections::connection_groups_update,
            commands::connections::connection_groups_reorder,
            commands::connections::connections_export,
            commands::connections::connections_import,
            commands::connections::connections_get_db_url,
            commands::connections::connections_copy_db_url_to_clipboard,
            commands::connections::connections_duplicate,
            commands::connections::connections_list_dialects,
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
            commands::query::query_execute_multi,
            commands::query::query_update_rows,
            commands::query::query_delete_rows,
            commands::query::query_insert_row,
            commands::query::query_save_table_changes,
            // Schema introspection & modification
            commands::schema::schema_list_databases,
            commands::schema::schema_list_schemas,
            commands::schema::schema_list_tables,
            commands::schema::schema_list_columns,
            commands::schema::schema_list_all_columns,
            commands::schema::schema_list_indexes,
            commands::schema::schema_list_foreign_keys,
            commands::schema::schema_get_ddl,
            commands::schema::schema_execute_ddl,
            // Query history
            commands::history::history_list,
            commands::history::history_clear,
            commands::history::history_delete,
            // Dashboards
            commands::dashboards::dashboards_list,
            commands::dashboards::dashboards_create,
            commands::dashboards::dashboards_update,
            commands::dashboards::dashboards_delete,
            // Saved queries (SQLite, kept for migration purposes)
            commands::saved_queries::saved_queries_list_folders,
            commands::saved_queries::saved_queries_create_folder,
            commands::saved_queries::saved_queries_update_folder,
            commands::saved_queries::saved_queries_delete_folder,
            commands::saved_queries::saved_queries_list,
            commands::saved_queries::saved_queries_create,
            commands::saved_queries::saved_queries_update,
            commands::saved_queries::saved_queries_delete,
            // File-based saved queries
            commands::file_saved_queries::file_saved_queries_get_dir,
            commands::file_saved_queries::file_saved_queries_list,
            commands::file_saved_queries::file_saved_queries_create,
            commands::file_saved_queries::file_saved_queries_update,
            commands::file_saved_queries::file_saved_queries_delete,
            commands::file_saved_queries::file_saved_queries_update_positions,
            commands::file_saved_queries::file_saved_queries_update_order,
            commands::file_saved_queries::file_saved_queries_create_folder,
            commands::file_saved_queries::file_saved_queries_delete_folder,
            commands::file_saved_queries::file_saved_queries_rename_folder,
            commands::file_saved_queries::file_saved_queries_move_folder,
            commands::file_saved_queries::file_saved_queries_assign_connection,
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
            // Menu
            commands::menu::menu_set_import_csv_enabled,
            // Window
            commands::window::window_set_traffic_light_position,
            // Transactions
            transactions::transaction_begin,
            transactions::transaction_commit,
            transactions::transaction_rollback,
            // Editor sessions
            sessions::session_acquire,
            sessions::session_release,
            // Explain
            commands::query::query_explain,
            // Virtual relations
            commands::virtual_relations::virtual_relations_list,
            commands::virtual_relations::virtual_relations_create,
            commands::virtual_relations::virtual_relations_update,
            commands::virtual_relations::virtual_relations_delete,
            // Polymorphic virtual relations
            commands::virtual_relations::polymorphic_vr_list,
            commands::virtual_relations::polymorphic_vr_create,
            commands::virtual_relations::polymorphic_vr_update,
            commands::virtual_relations::polymorphic_vr_delete,
            // Server administration
            commands::server_admin::server_admin_get_capabilities,
            commands::server_admin::server_admin_list_processes,
            commands::server_admin::server_admin_kill_session,
            commands::server_admin::server_admin_cancel_session,
            commands::server_admin::server_admin_get_status,
            commands::server_admin::server_admin_list_variables,
            commands::server_admin::server_admin_set_variable,
            commands::server_admin::server_admin_list_locks,
            commands::server_admin::server_admin_list_scheduled_jobs,
            commands::server_admin::server_admin_get_innodb_status,
            commands::server_admin::server_admin_get_vacuum_status,
            // User management
            commands::users::users_list,
            commands::users::users_get_grants,
            commands::users::users_create,
            commands::users::users_drop,
            commands::users::users_rename,
            commands::users::users_set_password,
            commands::users::users_execute_grant,
            // Speed analysis (dev-only)
            #[cfg(debug_assertions)]
            commands::speed_analysis::speed_analysis_list,
            #[cfg(debug_assertions)]
            commands::speed_analysis::speed_analysis_clear,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Rowmance")
        .run(|app, event| {
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let tauri::RunEvent::Opened { urls } = event {
                let paths: Vec<String> = urls
                    .iter()
                    .filter_map(|u| u.to_file_path().ok())
                    .filter_map(|p| p.to_str().map(String::from))
                    .collect();
                if !paths.is_empty() {
                    let _ = app.emit("file:opened", paths);
                }
            }
        });
}
