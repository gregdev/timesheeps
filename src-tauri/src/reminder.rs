//! Daily timesheet reminder.
//!
//! A small scheduler that wakes up every [`TICK_SECS`] seconds, re-reads the
//! settings (so enabling, disabling or retiming takes effect without a restart)
//! and, once per local day at the configured time, emits a `daily-reminder`
//! event carrying the hours tracked and logged so far. The frontend turns that
//! event into a prompt; this module also brings the window forward, because it
//! is usually hidden in the tray by the time the reminder is due, and posts a
//! native toast when it had to do so.
//!
//! "Already fired today" is stored in the `settings` table under
//! [`LAST_FIRED_KEY`] rather than in memory, so restarting the app (or
//! relaunching it after the reminder time) does not produce a second popup.

use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{Local, NaiveTime};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, Size};

use crate::db;
use crate::models::GroupBy;
use crate::reporting;

/// Short enough that retiming the reminder to a moment that has already passed
/// takes effect almost immediately; long enough to stay negligible.
const TICK_SECS: u64 = 30;

/// Bookkeeping, not a user preference: kept out of [`crate::models::Settings`]
/// so the settings form never round-trips (and never overwrites) it.
const LAST_FIRED_KEY: &str = "daily_reminder_last_fired";

/// One row of the per-project breakdown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReminderProject {
    pub project_id: i64,
    pub project_name: String,
    pub color: String,
    /// Merged activity attributed to this project today.
    pub tracked_secs: i64,
    /// Time entries logged against this project today.
    pub logged_secs: i64,
}

/// Payload of the `daily-reminder` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReminderEvent {
    /// Local `YYYY-MM-DD`.
    pub date: String,
    /// Tracked seconds after ignore rules, merging and `min_duration_secs` —
    /// the same figure the timeline would show.
    pub tracked_secs: i64,
    /// Seconds of time entries logged today.
    pub logged_secs: i64,
    /// Projects with either tracked or logged time, largest first.
    pub projects: Vec<DailyReminderProject>,
}

/// Starts the scheduler. Mirrors `activity::start_polling` and
/// `timer::start_ticker`: owned by the app, not driven by the frontend.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(TICK_SECS)).await;
            if let Err(e) = tick(&app) {
                eprintln!("[timesheeps] daily reminder failed: {e}");
            }
        }
    });
}

fn tick(app: &AppHandle) -> Result<()> {
    let state = app.state::<crate::AppState>();
    let conn = state
        .db
        .lock()
        .map_err(|e| anyhow!("database lock poisoned: {e}"))?;

    let settings = db::get_settings(&conn)?;
    if !settings.daily_reminder_enabled {
        return Ok(());
    }

    let Some(target) = parse_hhmm(&settings.daily_reminder_time) else {
        // A malformed time must never produce a popup at an arbitrary moment.
        return Ok(());
    };

    let now = Local::now();
    if now.time() < target {
        return Ok(());
    }

    let today = now.format("%Y-%m-%d").to_string();
    if db::get_setting_str(&conn, LAST_FIRED_KEY, "")? == today {
        return Ok(());
    }

    // Build first: if this fails there is nothing to show, and leaving the day
    // unclaimed means the next tick retries instead of silently skipping it.
    let reminder = build_event(&conn, &today)?;

    // Claim the day *before* emitting, so a slow tick or a second pass can never
    // fire twice for the same date. A claim that fails aborts the emit rather
    // than risking a popup on every tick.
    db::set_setting_str(&conn, LAST_FIRED_KEY, &today)?;

    // Release the DB before touching the UI: the frontend reacts to the event by
    // re-reading the day, which needs this same lock.
    drop(conn);

    let _ = app.emit("daily-reminder", &reminder);
    let was_obscured = reveal_window(app);

    // Only toast when the window had to be brought forward. When the app is
    // already on screen the prompt is the whole message and a duplicate toast
    // would be noise; when it was hidden in the tray the user may never come
    // back to the window, so the toast is the record in the Action Center.
    if was_obscured {
        notify(app, &reminder);
    }
    Ok(())
}

/// Classifies today the way the app does — `reporting::classify_range` runs
/// ignore rules, merging and match rules — then flattens it into the payload the
/// prompt renders.
fn build_event(conn: &Connection, date: &str) -> Result<DailyReminderEvent> {
    let totals = reporting::classify_range(conn, date, date, GroupBy::Project)?;

    let colors: std::collections::HashMap<i64, String> = db::get_projects(conn)?
        .into_iter()
        .map(|p| (p.id, p.color))
        .collect();

    let projects = totals
        .buckets
        .into_iter()
        .filter(|b| b.tracked_secs > 0 || b.logged_secs > 0)
        .filter_map(|b| {
            let project_id = b.project_id?;
            Some(DailyReminderProject {
                project_id,
                project_name: b.project_name.unwrap_or_default(),
                color: colors.get(&project_id).cloned().unwrap_or_default(),
                tracked_secs: b.tracked_secs,
                logged_secs: b.logged_secs,
            })
        })
        .collect();

    Ok(DailyReminderEvent {
        date: date.to_string(),
        tracked_secs: totals.total_tracked_secs,
        logged_secs: totals.total_logged_secs,
        projects,
    })
}

/// Bring the app forward so the prompt is actually seen.
///
/// The window is normally hidden to the tray, and the tray popup leaves it at a
/// 280 px width. A hidden or mini-sized window is restored to the normal size
/// first; an already visible, full-size window keeps its geometry so the
/// reminder does not shove the user's layout around. Route changes are left to
/// the frontend, which navigates off `/timer-popup` when it handles the event —
/// doing it here would reload the webview and race the event listener.
///
/// Returns whether the window had to be shown or resized, i.e. whether the user
/// was not already looking at the app.
fn reveal_window(app: &AppHandle) -> bool {
    let Some(win) = app.get_webview_window("main") else {
        return false;
    };

    let visible = win.is_visible().unwrap_or(false);
    let mini = match (win.outer_size(), win.scale_factor()) {
        (Ok(size), Ok(scale)) if scale > 0.0 => (size.width as f64 / scale) < 500.0,
        _ => false,
    };

    if !visible || mini {
        let _ = win.set_size(Size::Logical(LogicalSize::new(1100.0, 780.0)));
        let _ = win.center();
    }
    let _ = win.show();
    let _ = win.set_focus();
    !visible || mini
}

/// Native system notification, used when the app was not in front of the user.
///
/// On Windows this needs the *installed* build to show the app's own name and
/// icon — under `pnpm tauri dev` the toast is attributed to PowerShell. A
/// failure here is logged, never propagated: the in-app prompt has already been
/// shown, and permission can legitimately be refused (macOS).
fn notify(app: &AppHandle, reminder: &DailyReminderEvent) {
    use tauri_plugin_notification::NotificationExt;

    let body = if reminder.tracked_secs == 0 && reminder.logged_secs == 0 {
        "Nothing recorded yet today.".to_string()
    } else {
        format!(
            "{} logged · {} tracked today",
            format_secs(reminder.logged_secs),
            format_secs(reminder.tracked_secs)
        )
    };

    if let Err(e) = app
        .notification()
        .builder()
        .title("Time to check your timesheet")
        .body(body)
        .show()
    {
        eprintln!("[timesheeps] daily reminder notification failed: {e}");
    }
}

/// `5400` → `"1h 30m"`, `0` → `"0m"`.
///
/// Deliberately the same shape as `formatSeconds` in
/// `composables/useFormat.ts`, so the toast and the prompt agree.
fn format_secs(secs: i64) -> String {
    let mins = secs.max(0) / 60;
    let hours = mins / 60;
    let rest = mins % 60;

    match (hours, rest) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// `"17:00"` → 17:00. Trailing `:ss` is tolerated, since native time inputs vary.
fn parse_hhmm(raw: &str) -> Option<NaiveTime> {
    let mut parts = raw.trim().split(':');
    let hour: u32 = parts.next()?.trim().parse().ok()?;
    let minute: u32 = parts.next()?.trim().parse().ok()?;
    NaiveTime::from_hms_opt(hour, minute, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{insert_project, insert_time_entry};

    #[test]
    fn parses_valid_times() {
        assert_eq!(parse_hhmm("17:00"), NaiveTime::from_hms_opt(17, 0, 0));
        assert_eq!(parse_hhmm(" 09:05 "), NaiveTime::from_hms_opt(9, 5, 0));
        assert_eq!(parse_hhmm("00:00"), NaiveTime::from_hms_opt(0, 0, 0));
        assert_eq!(parse_hhmm("23:59"), NaiveTime::from_hms_opt(23, 59, 0));
        // Native time inputs may include seconds.
        assert_eq!(parse_hhmm("17:30:00"), NaiveTime::from_hms_opt(17, 30, 0));
    }

    #[test]
    fn rejects_invalid_times() {
        assert!(parse_hhmm("").is_none());
        assert!(parse_hhmm("24:00").is_none());
        assert!(parse_hhmm("12:60").is_none());
        assert!(parse_hhmm("noon").is_none());
        assert!(parse_hhmm("12").is_none());
    }

    #[test]
    fn formats_seconds_like_the_frontend() {
        assert_eq!(format_secs(0), "0m");
        assert_eq!(format_secs(59), "0m");
        assert_eq!(format_secs(60), "1m");
        assert_eq!(format_secs(3600), "1h");
        assert_eq!(format_secs(5400), "1h 30m");
        // Clamped, like every other total that can go negative pre-repair.
        assert_eq!(format_secs(-120), "0m");
    }

    #[test]
    fn event_reports_logged_and_tracked_totals() {
        let conn = db::migrated_in_memory();
        let project = insert_project(&conn, "Lotteries", "#6366f1", None, None).unwrap();
        insert_time_entry(&conn, "2026-09-01", project.id, 9 * 60, 10 * 60, "").unwrap();

        let event = build_event(&conn, "2026-09-01").unwrap();

        assert_eq!(event.date, "2026-09-01");
        assert_eq!(event.logged_secs, 3600);
        assert_eq!(event.tracked_secs, 0);
        assert_eq!(event.projects.len(), 1);
        assert_eq!(event.projects[0].project_id, project.id);
        assert_eq!(event.projects[0].project_name, "Lotteries");
        assert_eq!(event.projects[0].color, "#6366f1");
        assert_eq!(event.projects[0].logged_secs, 3600);
        assert_eq!(event.projects[0].tracked_secs, 0);
    }
}
