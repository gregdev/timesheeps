//! Background auto-accept sweep.
//!
//! Auto-accepting suggestions used to live in `stores/day.ts`, inside
//! `loadDay` — which meant it only ran for the date the user had open, and only
//! while the Timeline view was mounted. A day never opened kept its suggestions
//! forever, so "auto-accept" quietly meant "auto-accept once you look at it",
//! which is exactly backwards for a tool whose job is to spare you from looking.
//!
//! This loop owns the behaviour instead. Every [`TICK_SECS`] it re-reads the
//! settings (so toggling the setting takes effect without a restart), sweeps the
//! last [`SWEEP_DAYS`] days, and writes an entry for each matched span no entry
//! covers yet. It runs on startup *and* continuously, because the app is left
//! open for weeks at a time here — a startup-only pass would do nothing.
//!
//! Safety rules, all of which the old frontend version had:
//!
//! * a suggestion that is still growing (its end is within [`SETTLE_MINUTES`] of
//!   now) is skipped — the poller extends the open row every tick, so accepting
//!   it early freezes a sliver into an entry that then blocks the real span as
//!   an overlap forever;
//! * anything overlapping an existing entry is skipped, so manual logging always
//!   wins;
//! * ranges accepted in the same pass claim each other, so two suggestions that
//!   overlap cannot both become entries.
//!
//! On top of those, a **watermark** is kept in the settings table: the instant up
//! to which suggestions have already been considered. Only spans ending after it
//! are eligible. Without it, deleting an auto-accepted entry would do nothing —
//! the next tick would recompute the same suggestion and write it straight back,
//! every 60 seconds, forever. A growing session still qualifies, because its end
//! keeps moving forward past the watermark until it settles.

use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, Timelike, Utc};
use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::matcher;
use crate::reporting::local_minutes;

/// Long enough to be invisible, short enough that a settled stretch is logged
/// before the user can think about it.
const TICK_SECS: u64 = 60;

/// A pay period plus a week of slack, so a fortnightly timesheet is fully swept
/// without re-scanning months of activity on every tick. Also the backfill
/// window the first time the sweep runs.
const SWEEP_DAYS: i64 = 21;

/// Same tolerance the timeline uses before auto-accepting: comfortably larger
/// than both the 20s poll interval and the 30s UI refresh.
const SETTLE_MINUTES: i64 = 2;

/// Bookkeeping, not a user preference: kept out of [`crate::models::Settings`]
/// so the settings form never round-trips (and never overwrites) it.
const WATERMARK_KEY: &str = "auto_accept_watermark";

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(TICK_SECS)).await;
            match sweep(&app) {
                Ok(0) => {}
                Ok(n) => eprintln!("[timesheeps] auto-accepted {n} suggested entr{}",
                    if n == 1 { "y" } else { "ies" }),
                Err(e) => eprintln!("[timesheeps] auto-accept sweep failed: {e}"),
            }
        }
    });
}

/// Sweep the recent past once. Returns how many entries were created.
fn sweep(app: &AppHandle) -> Result<usize> {
    let state = app.state::<crate::AppState>();
    let conn = state
        .db
        .lock()
        .map_err(|e| anyhow!("database lock poisoned: {e}"))?;

    let created = sweep_conn(&conn)?;

    // Release the DB before emitting: the frontend reacts by re-reading the day,
    // which needs this same lock.
    drop(conn);

    if created > 0 {
        let _ = app.emit("activity-updated", ());
    }

    Ok(created)
}

/// The sweep itself, against an open connection. Split out so it can be tested
/// without an `AppHandle`.
fn sweep_conn(conn: &rusqlite::Connection) -> Result<usize> {
    // Re-read per tick: the toggle, the rules and the gaps may all have changed
    // since the last pass.
    let settings = db::get_settings(conn)?;
    if !settings.auto_accept_suggested {
        return Ok(0);
    }

    let filter_rules = db::get_filter_rules(conn)?;
    let rules = matcher::compile(&db::get_project_match_rules(conn)?);

    let today = Local::now().date_naive();
    let now = Local::now();
    let now_minutes = now.hour() as i64 * 60 + now.minute() as i64;
    let mut created = 0usize;

    // Every suggestion that ended at or before this has already been *decided*:
    // accepted, or deliberately left alone because an entry already covers it.
    // Missing means this is the first run, so backfill the whole window.
    let cutoff: DateTime<Utc> = db::get_setting_str(conn, WATERMARK_KEY, "")
        .ok()
        .and_then(|raw| DateTime::parse_from_rfc3339(&raw).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(SWEEP_DAYS));

    // Only ever moves forward, and only past spans that are finished with. A
    // span skipped because it is still being recorded must stay eligible, or a
    // long session would become permanently unloggable once it ended.
    let mut decided_through = cutoff;

    for offset in (0..SWEEP_DAYS).rev() {
        let date = (today - chrono::Duration::days(offset))
            .format("%Y-%m-%d")
            .to_string();

        let blocks = db::get_activity_for_date(conn, &date, &settings, &filter_rules)?;
        if blocks.is_empty() {
            continue;
        }

        let mut entries = db::get_time_entries_for_date(conn, &date)?;
        let mut claimed: Vec<(i64, i64)> = Vec::new();

        for suggestion in matcher::compute_suggestions(&blocks, &rules, &settings) {
            // Already decided on an earlier pass. This is what stops a deleted
            // entry from being written straight back, every tick, forever.
            if suggestion.ended_at <= cutoff {
                continue;
            }

            let start = local_minutes(suggestion.started_at);
            let end = local_minutes(suggestion.ended_at);

            // A span that crosses midnight cannot be expressed in one entry on
            // one date; leave it for the user rather than writing a negative one.
            // Deliberately NOT marked decided: moving the watermark to a
            // next-day instant would hide the following morning's work.
            if end <= start {
                continue;
            }

            // Still being recorded (today only — a past day's end cannot move).
            // Not decided either: it must be picked up once it settles.
            if offset == 0 && now_minutes - end < SETTLE_MINUTES {
                continue;
            }

            if entries
                .iter()
                .any(|e| e.start_minutes < end && e.end_minutes > start)
            {
                decided_through = decided_through.max(suggestion.ended_at);
                continue;
            }
            if claimed.iter().any(|&(cs, ce)| cs < end && ce > start) {
                decided_through = decided_through.max(suggestion.ended_at);
                continue;
            }

            match db::insert_time_entry(conn, &date, suggestion.project_id, start, end, "") {
                Ok(entry) => {
                    entries.push(entry);
                    claimed.push((start, end));
                    created += 1;
                    decided_through = decided_through.max(suggestion.ended_at);
                }
                // Left undecided on failure, so the next tick retries it.
                Err(e) => eprintln!("[timesheeps] auto-accept on {date} failed: {e}"),
            }
        }
    }

    if decided_through > cutoff {
        db::set_setting_str(conn, WATERMARK_KEY, &decided_through.to_rfc3339())?;
    }

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{insert_activity, insert_project, insert_project_match_rule};
    use crate::models::{MatchCondition, MatchField, MatchOperator};
    use chrono::TimeZone;

    /// A timestamp `days_ago` days back at `hour:minute` **local time**, which is
    /// the frame the sweep iterates in.
    fn local_ts(days_ago: i64, hour: u32, minute: u32) -> DateTime<Utc> {
        let date = Local::now().date_naive() - chrono::Duration::days(days_ago);
        let naive = date.and_hms_opt(hour, minute, 0).unwrap();
        Local
            .from_local_datetime(&naive)
            .single()
            .expect("unambiguous local time")
            .with_timezone(&Utc)
    }

    fn local_date_of(instant: DateTime<Utc>) -> String {
        instant
            .with_timezone(&Local)
            .format("%Y-%m-%d")
            .to_string()
    }

    fn db_with_rule() -> (rusqlite::Connection, i64) {
        let conn = db::migrated_in_memory();
        db::set_setting_str(&conn, "min_duration_secs", "0").unwrap();
        db::set_setting_str(&conn, "auto_accept_suggested", "1").unwrap();

        let project = insert_project(&conn, "Lotteries", "#6366f1", None, None).unwrap();
        insert_project_match_rule(
            &conn,
            project.id,
            "Jira",
            &[MatchCondition {
                field: MatchField::WindowTitle,
                operator: MatchOperator::Contains,
                value: "jira".to_string(),
                negate: false,
            }],
            None,
        )
        .unwrap();

        (conn, project.id)
    }

    #[test]
    fn sweep_logs_a_settled_suggestion_exactly_once() {
        let (conn, _) = db_with_rule();
        let start = local_ts(1, 9, 0);
        let end = local_ts(1, 10, 0);
        insert_activity(&conn, "brave", "Sprint - Jira", 1, "", &start, &end).unwrap();

        let date = local_date_of(start);

        assert_eq!(sweep_conn(&conn).unwrap(), 1);
        let entries = db::get_time_entries_for_date(&conn, &date).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].end_minutes - entries[0].start_minutes, 60);

        // A second tick must not duplicate it...
        assert_eq!(sweep_conn(&conn).unwrap(), 0);
        assert_eq!(db::get_time_entries_for_date(&conn, &date).unwrap().len(), 1);

        // ...and deleting it must not bring it back. Without the watermark the
        // same suggestion would be recomputed and re-inserted every 60s.
        db::delete_time_entry(&conn, entries[0].id).unwrap();
        assert_eq!(sweep_conn(&conn).unwrap(), 0);
        assert!(db::get_time_entries_for_date(&conn, &date).unwrap().is_empty());
    }

    #[test]
    fn sweep_is_inert_when_the_setting_is_off() {
        let (conn, _) = db_with_rule();
        db::set_setting_str(&conn, "auto_accept_suggested", "0").unwrap();

        let start = local_ts(1, 9, 0);
        let end = local_ts(1, 10, 0);
        insert_activity(&conn, "brave", "Sprint - Jira", 1, "", &start, &end).unwrap();

        assert_eq!(sweep_conn(&conn).unwrap(), 0);
        assert!(db::get_time_entries_for_date(&conn, &local_date_of(start))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn sweep_leaves_manually_logged_time_alone() {
        let (conn, project_id) = db_with_rule();
        let start = local_ts(1, 9, 0);
        let end = local_ts(1, 10, 0);
        insert_activity(&conn, "brave", "Sprint - Jira", 1, "", &start, &end).unwrap();

        let date = local_date_of(start);
        db::insert_time_entry(&conn, &date, project_id, 9 * 60, 10 * 60, "written by hand")
            .unwrap();

        assert_eq!(sweep_conn(&conn).unwrap(), 0);
        let entries = db::get_time_entries_for_date(&conn, &date).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].note, "written by hand");
    }

    #[test]
    fn sweep_skips_a_session_that_is_still_being_recorded() {
        let (conn, _) = db_with_rule();
        let now = Utc::now();
        insert_activity(
            &conn,
            "brave",
            "Sprint - Jira",
            1,
            "",
            &(now - chrono::Duration::minutes(30)),
            &now,
        )
        .unwrap();

        // The poller is still extending this row, so its end is not final. (A
        // block that crosses midnight for a late-evening run is skipped by the
        // negative-duration guard instead, and also creates nothing.)
        assert_eq!(sweep_conn(&conn).unwrap(), 0);
    }
}
