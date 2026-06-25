use tauri::State;
use tauri::Manager;

use crate::db;
use crate::models::TimerState;
use crate::timer::TimerManager;

#[tauri::command]
pub fn start_timer(
    app: tauri::AppHandle,
    timer: State<'_, TimerManager>,
    project_id: i64,
    note: String,
) -> Result<TimerState, String> {
    // Resolve project name + color from the DB
    let db = app.state::<crate::AppState>();
    let conn = db.db.lock().map_err(|e| e.to_string())?;
    let project = db::get_project(&conn, project_id).map_err(|e| e.to_string())?;

    let name = project.as_ref().map(|p| p.name.clone());
    let color = project.as_ref().map(|p| p.color.clone());

    let state = timer.start(project_id, name, color, note);

    Ok(state)
}

#[tauri::command]
pub fn pause_timer(timer: State<'_, TimerManager>) -> Result<TimerState, String> {
    timer.pause().ok_or_else(|| "Timer is not running".to_string())
}

#[tauri::command]
pub fn resume_timer(timer: State<'_, TimerManager>) -> Result<TimerState, String> {
    timer.resume().ok_or_else(|| "Timer is not paused".to_string())
}

#[tauri::command]
pub fn stop_timer(timer: State<'_, TimerManager>) -> Result<TimerState, String> {
    timer.stop().ok_or_else(|| "Timer is not running".to_string())
}

#[tauri::command]
pub fn get_timer_state(timer: State<'_, TimerManager>) -> Result<TimerState, String> {
    Ok(timer.get_state())
}
