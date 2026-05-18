use tauri::State;

use crate::db;
use crate::models::{CreateProjectMatchRule, ProjectMatchRule, SuggestedEntry};
use crate::AppState;

#[tauri::command]
pub fn get_project_match_rules(state: State<AppState>) -> Result<Vec<ProjectMatchRule>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_project_match_rules(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_project_match_rule(
    payload: CreateProjectMatchRule,
    state: State<AppState>,
) -> Result<ProjectMatchRule, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::insert_project_match_rule(&conn, payload.project_id, &payload.rule_type, &payload.value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_project_match_rule(id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_project_match_rule(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_suggested_entries_for_day(
    date: String,
    state: State<AppState>,
) -> Result<Vec<SuggestedEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let settings = db::get_settings(&conn).map_err(|e| e.to_string())?;
    let filter_rules = db::get_filter_rules(&conn).map_err(|e| e.to_string())?;
    let match_rules = db::get_project_match_rules(&conn).map_err(|e| e.to_string())?;
    let blocks = db::get_activity_for_date(&conn, &date, &settings, &filter_rules)
        .map_err(|e| e.to_string())?;
    Ok(db::compute_suggestions(blocks, &match_rules))
}
