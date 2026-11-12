use tauri::State;

use crate::calendar;
use crate::models::{DeviceCodePrompt, M365Status};
use crate::AppState;

/// The client id to use, in order of preference:
///
/// 1. `TIMESHEEPS_M365_CLIENT_ID` — for a tenant that insists on its own app.
/// 2. whatever was saved the last time the user supplied one.
///
/// A public client id is not a secret, so an official build would bake one in as
/// the fallback and the user would never see this field at all.
fn resolve_client_id(conn: &rusqlite::Connection, provided: Option<&str>) -> String {
    if let Some(id) = provided.map(str::trim).filter(|s| !s.is_empty()) {
        return id.to_string();
    }
    if let Ok(id) = std::env::var(calendar::CLIENT_ID_OVERRIDE_ENV) {
        if !id.trim().is_empty() {
            return id.trim().to_string();
        }
    }
    crate::db::get_setting_str(conn, "m365_client_id", "").unwrap_or_default()
}

/// Begin device-code sign-in.
///
/// Returns the short code and URL to show the user, then polls Microsoft in the
/// background. The frontend watches `get_m365_status`, which flips to connected
/// when the poll succeeds — no callback listener, no redirect URI, and nothing to
/// configure beyond "allow public client flows" on the app registration.
#[tauri::command]
pub async fn start_m365_login(
    client_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<DeviceCodePrompt, String> {
    let resolved = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let resolved = resolve_client_id(&conn, client_id.as_deref());
        if resolved.is_empty() {
            return Err(
                "No Microsoft application (client) ID configured. Paste one in Settings — the app \
                 registration needs only \"Allow public client flows\" turned on, with no redirect \
                 URI."
                    .to_string(),
            );
        }
        crate::db::set_setting_str(&conn, "m365_client_id", &resolved)
            .map_err(|e| e.to_string())?;
        resolved
    };

    let (device_code, prompt) = calendar::start_device_code(&resolved)
        .await
        .map_err(|e| e.to_string())?;

    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        match calendar::poll_device_code(&resolved, &device_code).await {
            Ok(tokens) => {
                if let Ok(conn) = db.lock() {
                    let _ = calendar::store_tokens(&conn, &tokens);
                }
            }
            Err(e) => eprintln!("[timesheeps] Microsoft sign-in failed: {e}"),
        }
    });

    Ok(prompt)
}

#[tauri::command]
pub fn get_m365_status(state: State<'_, AppState>) -> Result<M365Status, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    calendar::get_status(&conn).map_err(|e| e.to_string())
}

/// Calendar events for one day, served from the local cache.
///
/// This used to call Graph once per day the timeline happened to show, which made
/// every view a network round-trip and left the review views with no access to
/// meetings at all. The background sync keeps the cache current instead.
#[tauri::command]
pub fn get_calendar_events(
    date: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::CalendarEvent>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let stored = crate::db::get_calendar_events_for_date(&conn, &date).map_err(|e| e.to_string())?;

    Ok(stored
        .into_iter()
        // A declined invitation is not your time, so it does not belong on the
        // timeline. Cancelled occurrences never reach the cache at all.
        .filter(|e| !e.response.eq_ignore_ascii_case("declined"))
        .filter_map(|e| {
            Some(crate::models::CalendarEvent {
                i_cal_uid: e.i_cal_uid,
                subject: e.subject,
                start_at: chrono::DateTime::parse_from_rfc3339(&e.started_at)
                    .ok()?
                    .with_timezone(&chrono::Utc),
                end_at: chrono::DateTime::parse_from_rfc3339(&e.ended_at)
                    .ok()?
                    .with_timezone(&chrono::Utc),
                is_all_day: e.is_all_day,
                organizer: e.organizer,
                location: e.location,
                is_teams_meeting: e.is_teams,
                response: e.response,
            })
        })
        .collect())
}

/// Refresh the calendar cache now, instead of waiting for the next tick.
#[tauri::command]
pub async fn refresh_calendar(app: tauri::AppHandle) -> Result<usize, String> {
    calendar::sync_once(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disconnect_m365(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    calendar::disconnect(&conn).map_err(|e| e.to_string())
}
