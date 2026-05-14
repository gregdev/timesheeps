use tauri::State;

use crate::db;
use crate::models::{CreateFilterRule, FilterRule};
use crate::AppState;

#[tauri::command]
pub fn get_filter_rules(state: State<AppState>) -> Result<Vec<FilterRule>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_filter_rules(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_filter_rule(
    payload: CreateFilterRule,
    state: State<AppState>,
) -> Result<FilterRule, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::insert_filter_rule(&conn, &payload.rule_type, &payload.value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_filter_rule(id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_filter_rule(&conn, id).map_err(|e| e.to_string())
}
