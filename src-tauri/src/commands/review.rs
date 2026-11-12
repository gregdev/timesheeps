use tauri::State;

use crate::db;
use crate::models::PeriodReview;
use crate::reporting;
use crate::AppState;

/// The whole period in one call: tracked vs logged per day, unclaimed
/// suggestions, untracked gaps, notes and a list of reasons each day still
/// needs attention.
///
/// The period views used to issue two IPC calls per day and then work out
/// "needs attention" from a bare `hasActivity` flag, which could not see that
/// six tracked hours had only one logged.
#[tauri::command]
pub fn get_period_review(
    start_date: String,
    end_date: String,
    gap_min_secs: Option<i64>,
    state: State<AppState>,
) -> Result<PeriodReview, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    reporting::period_review(&conn, &start_date, &end_date, gap_min_secs)
        .map_err(|e| e.to_string())
}

/// Create entries for every unclaimed suggestion on one day.
///
/// Returns the number created. Overlap checking is delegated to the same rules
/// the background sweep uses, so pressing this after the sweep has already run
/// creates nothing rather than duplicating work.
#[tauri::command]
pub fn accept_suggestions_for_day(date: String, state: State<AppState>) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let review = reporting::period_review(&conn, &date, &date, None).map_err(|e| e.to_string())?;

    let Some(day) = review.days.first() else {
        return Ok(0);
    };

    let mut created = 0;
    for suggestion in &day.suggestions {
        match db::insert_time_entry(
            &conn,
            &date,
            suggestion.project_id,
            suggestion.start_minutes,
            suggestion.end_minutes,
            "",
        ) {
            Ok(_) => created += 1,
            Err(e) => eprintln!("[timesheeps] accepting suggestion on {date} failed: {e}"),
        }
    }

    Ok(created)
}

/// Remember which project a meeting series belongs to.
///
/// Keyed on the series, not the occurrence, so logging this week's standup teaches
/// the app about every future standup — which is the whole point of having a
/// calendar at all.
#[tauri::command]
pub fn set_meeting_project(
    i_cal_uid: String,
    project_id: i64,
    state: State<AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::set_calendar_event_project(&conn, &i_cal_uid, project_id).map_err(|e| e.to_string())
}

/// Stop offering a meeting series at all (lunch, focus time, a blocked hour).
#[tauri::command]
pub fn hide_meeting(i_cal_uid: String, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::hide_calendar_series(&conn, &i_cal_uid).map_err(|e| e.to_string())
}
