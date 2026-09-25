use tauri::State;

use crate::appicon;
use crate::db;
use crate::AppState;

/// Icon for an application, as a PNG `data:` URI. `None` when the app has no
/// recorded executable path yet (activity rows written before paths were
/// captured) or the icon could not be extracted — the UI falls back to a
/// coloured letter avatar in that case.
#[tauri::command]
pub fn get_app_icon(app_name: String, state: State<AppState>) -> Result<Option<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let exe_path = db::get_exe_path_for_app(&conn, &app_name).map_err(|e| e.to_string())?;
    Ok(exe_path.and_then(|path| appicon::icon_data_uri(&app_name, &path)))
}
