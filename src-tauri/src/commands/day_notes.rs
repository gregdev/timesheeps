use tauri::State;

use crate::db;
use crate::AppState;

/// The note for one local date, or an empty string when there is none.
#[tauri::command]
pub fn get_day_note(date: String, state: State<AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_day_note(&conn, &date).map_err(|e| e.to_string())
}

/// Upsert the note for a date. Passing a blank note clears it.
#[tauri::command]
pub fn set_day_note(date: String, note: String, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::set_day_note(&conn, &date, &note).map_err(|e| e.to_string())
}
