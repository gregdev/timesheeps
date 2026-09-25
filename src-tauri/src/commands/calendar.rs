use tauri::State;

use crate::calendar;
use crate::models::{CalendarEvent, M365Status};
use crate::AppState;

#[tauri::command]
pub async fn start_m365_login(
    client_id: String,
    state: State<'_, AppState>,
) -> Result<calendar::OAuthUrl, String> {
    // Store client_id first
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::db::set_setting_str(&conn, "m365_client_id", &client_id)
            .map_err(|e| e.to_string())?;
    }

    let (oauth_url, rx) = calendar::start_oauth_flow(&client_id).map_err(|e| e.to_string())?;

    // Spawn a task to wait for the callback and store tokens
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        match rx.await {
            Ok(Ok(tokens)) => {
                if let Ok(conn) = db.lock() {
                    let _ = calendar::store_tokens(&conn, &tokens);
                }
            }
            Ok(Err(e)) => {
                eprintln!("OAuth flow failed: {}", e);
            }
            Err(_) => {
                eprintln!("OAuth flow cancelled (oneshot dropped)");
            }
        }
    });

    Ok(oauth_url)
}

#[tauri::command]
pub fn get_m365_status(state: State<'_, AppState>) -> Result<M365Status, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    calendar::get_status(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_calendar_events(
    date: String,
    state: State<'_, AppState>,
) -> Result<Vec<CalendarEvent>, String> {
    // Compute end_date and extract token info while holding the lock,
    // then release before the async Graph API call.
    let (token_info, end_date) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;

        let has_tokens = crate::db::get_setting_str(&conn, "m365_refresh_token", "")
            .map(|s| !s.is_empty())
            .unwrap_or(false);

        if !has_tokens {
            return Ok(Vec::new());
        }

        let client_id = crate::db::get_setting_str(&conn, "m365_client_id", "")
            .map_err(|e| e.to_string())?;
        let access_token = crate::db::get_setting_str(&conn, "m365_access_token", "")
            .map_err(|e| e.to_string())?;
        let refresh_token = crate::db::get_setting_str(&conn, "m365_refresh_token", "")
            .map_err(|e| e.to_string())?;
        let expiry_str = crate::db::get_setting_str(&conn, "m365_token_expiry", "")
            .map_err(|e| e.to_string())?;

        // Compute next day
        let end_date = {
            let parts: Vec<&str> = date.split('-').collect();
            if parts.len() == 3 {
                if let (Ok(y), Ok(m), Ok(d)) = (
                    parts[0].parse::<i32>(),
                    parts[1].parse::<u32>(),
                    parts[2].parse::<u32>(),
                ) {
                    use chrono::NaiveDate;
                    if let Some(next) =
                        NaiveDate::from_ymd_opt(y, m, d).and_then(|nd| nd.succ_opt())
                    {
                        next.format("%Y-%m-%d").to_string()
                    } else {
                        date.clone()
                    }
                } else {
                    date.clone()
                }
            } else {
                date.clone()
            }
        };

        ((client_id, access_token, refresh_token, expiry_str), end_date)
    };

    // Now do the async work without holding the lock
    calendar::get_calendar_events_from_parts(
        &token_info.0,
        &token_info.1,
        &token_info.2,
        &token_info.3,
        &date,
        &end_date,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disconnect_m365(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    calendar::disconnect(&conn).map_err(|e| e.to_string())
}
