use tauri::State;

use crate::db;
use crate::models::{CreateProject, Project, UpdateProject};
use crate::AppState;

#[tauri::command]
pub fn get_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_projects(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_project(payload: CreateProject, state: State<AppState>) -> Result<Project, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::insert_project(&conn, &payload.name, &payload.color, payload.parent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_project(payload: UpdateProject, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::update_project(&conn, payload.id, &payload.name, &payload.color, payload.parent_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn archive_project(id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::archive_project(&conn, id).map_err(|e| e.to_string())
}
