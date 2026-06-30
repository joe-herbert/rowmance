use tauri::{AppHandle, Runtime};

#[tauri::command]
pub fn menu_set_import_csv_enabled<R: Runtime>(app: AppHandle<R>, enabled: bool) {
    if let Some(menu) = app.menu() {
        if let Some(item) = menu.get("import-csv") {
            if let Some(mi) = item.as_menuitem() {
                let _ = mi.set_enabled(enabled);
            }
        }
    }
}
