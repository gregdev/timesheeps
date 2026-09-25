use tauri::State;

use crate::db;
use crate::matcher;
use crate::models::{
    ActivityBlock, CreateProjectMatchRule, KnownApp, ProjectMatchRule, RuleStat, SuggestedEntry,
    UpdateProjectMatchRule,
};
use crate::AppState;

/// Days swept when computing rule hit counts and overlap warnings.
const STATS_DEFAULT_DAYS: i64 = 30;
/// Days of history offered to the app value picker.
const KNOWN_APPS_DEFAULT_DAYS: i64 = 90;

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
    db::insert_project_match_rule(&conn, payload.project_id, &payload.name, &payload.conditions)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_project_match_rule(
    payload: UpdateProjectMatchRule,
    state: State<AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::update_project_match_rule(&conn, payload.id, &payload.name, &payload.conditions)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_project_match_rule(id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_project_match_rule(&conn, id).map_err(|e| e.to_string())
}

/// Persist a new global precedence order for match rules.
#[tauri::command]
pub fn reorder_project_match_rules(
    ordered_ids: Vec<i64>,
    state: State<AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::reorder_project_match_rules(&conn, &ordered_ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_suggested_entries_for_day(
    date: String,
    state: State<AppState>,
) -> Result<Vec<SuggestedEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let settings = db::get_settings(&conn).map_err(|e| e.to_string())?;
    let filter_rules = db::get_filter_rules(&conn).map_err(|e| e.to_string())?;
    let rules = db::get_project_match_rules(&conn).map_err(|e| e.to_string())?;
    let blocks = db::get_activity_for_date(&conn, &date, &settings, &filter_rules)
        .map_err(|e| e.to_string())?;
    Ok(matcher::compute_suggestions(
        &blocks,
        &matcher::compile(&rules),
        &settings,
    ))
}

/// Hit counts, matched minutes and overlap warnings per rule. This is what
/// makes a rule that silently matches nothing visible to the user.
#[tauri::command]
pub fn get_rule_stats(days: Option<i64>, state: State<AppState>) -> Result<Vec<RuleStat>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let days = days.unwrap_or(STATS_DEFAULT_DAYS).clamp(1, 365);
    let settings = db::get_settings(&conn).map_err(|e| e.to_string())?;
    let filter_rules = db::get_filter_rules(&conn).map_err(|e| e.to_string())?;
    let rules = db::get_project_match_rules(&conn).map_err(|e| e.to_string())?;

    let dates = db::get_active_dates(&conn, days).map_err(|e| e.to_string())?;
    let mut day_blocks: Vec<(String, Vec<ActivityBlock>)> = Vec::new();
    for date in dates {
        let blocks = db::get_activity_for_date(&conn, &date, &settings, &filter_rules)
            .map_err(|e| e.to_string())?;
        day_blocks.push((date, blocks));
    }

    Ok(matcher::compute_rule_stats(
        &day_blocks,
        &matcher::compile(&rules),
        &settings,
    ))
}

/// Applications seen in recorded activity, used to populate the
/// search-as-you-type value picker with values that actually exist.
#[tauri::command]
pub fn get_known_apps(days: Option<i64>, state: State<AppState>) -> Result<Vec<KnownApp>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let days = days.unwrap_or(KNOWN_APPS_DEFAULT_DAYS).clamp(1, 3650);
    db::get_known_apps(&conn, days, 5).map_err(|e| e.to_string())
}
