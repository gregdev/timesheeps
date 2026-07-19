use tauri::State;

use crate::db;
use crate::models::{ActivityBlock, SearchResults, WindowSummaryItem};
use crate::nl_query;
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

#[tauri::command]
pub fn get_window_summary_for_day(
    date: String,
    state: State<AppState>,
) -> Result<Vec<WindowSummaryItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let settings = db::get_settings(&conn).map_err(|e| e.to_string())?;
    db::get_window_summary_for_date(&conn, &date, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search(query: String, state: State<AppState>) -> Result<SearchResults, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let settings = db::get_settings(&conn).map_err(|e| e.to_string())?;
    let rules = db::get_filter_rules(&conn).map_err(|e| e.to_string())?;

    // Try natural-language parsing first; fall back to the raw query if it
    // looks like keyword syntax or can't be parsed.
    let effective_query = nl_query::parse_nl(&query).unwrap_or(query);

    db::search(&conn, &effective_query, &settings, &rules).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_activity_block(
    started_at: String,
    ended_at: String,
    app_name: String,
    window_title: String,
    state: State<AppState>,
) -> Result<usize, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_activity_block(&conn, &started_at, &ended_at, &app_name, &window_title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_activity_by_app_title(
    app_name: String,
    window_title: String,
    state: State<AppState>,
) -> Result<usize, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_activity_by_app_title(&conn, &app_name, &window_title)
        .map_err(|e| e.to_string())
}
