use tauri::State;

use crate::db;
use crate::models::ActivityBlock;
use crate::AppState;

#[tauri::command]
pub fn get_activity_for_day(
    date: String,
    state: State<AppState>,
) -> Result<Vec<ActivityBlock>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let settings = db::get_settings(&conn).map_err(|e| e.to_string())?;
    let rules = db::get_filter_rules(&conn).map_err(|e| e.to_string())?;
    db::get_activity_for_date(&conn, &date, &settings, &rules).map_err(|e| e.to_string())
}
