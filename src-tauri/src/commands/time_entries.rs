use tauri::State;

use crate::db;
use crate::models::{CreateTimeEntry, TimeEntry, UpdateTimeEntry};
use crate::AppState;

#[tauri::command]
pub fn get_time_entries_for_day(
    date: String,
    state: State<AppState>,
) -> Result<Vec<TimeEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_time_entries_for_date(&conn, &date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_time_entry(
    payload: CreateTimeEntry,
    state: State<AppState>,
) -> Result<TimeEntry, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::insert_time_entry(
        &conn,
        &payload.date,
        payload.project_id,
        payload.start_minutes,
        payload.end_minutes,
        &payload.note,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_time_entry(
    payload: UpdateTimeEntry,
    state: State<AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::update_time_entry(
        &conn,
        payload.id,
        payload.project_id,
        payload.start_minutes,
        payload.end_minutes,
        &payload.note,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_time_entry(id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_time_entry(&conn, id).map_err(|e| e.to_string())
}
