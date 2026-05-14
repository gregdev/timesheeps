use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use tauri::Manager;

use crate::models::{
    ActivityBlock, FilterRule, FilterRuleType, Project, RawActivity, Settings, TimeEntry,
};

pub struct Db(pub Connection);

pub fn open(app: &tauri::AppHandle) -> Result<Connection> {
    let app_dir: PathBuf = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;
    let db_path = app_dir.join("timesheeps.db");
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);

        CREATE TABLE IF NOT EXISTS activity_raw (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at   TEXT NOT NULL,
            ended_at     TEXT NOT NULL,
            app_name     TEXT NOT NULL,
            window_title TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_activity_raw_started
            ON activity_raw (started_at);

        CREATE TABLE IF NOT EXISTS projects (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            color       TEXT NOT NULL DEFAULT '#6366f1',
            archived_at TEXT
        );

        CREATE TABLE IF NOT EXISTS time_entries (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            date          TEXT NOT NULL,
            project_id    INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            start_minutes INTEGER NOT NULL,
            end_minutes   INTEGER NOT NULL,
            note          TEXT NOT NULL DEFAULT ''
        );

        CREATE INDEX IF NOT EXISTS idx_time_entries_date
            ON time_entries (date);

        CREATE TABLE IF NOT EXISTS filter_rules (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_type TEXT NOT NULL CHECK(rule_type IN ('title_pattern', 'app_name')),
            value     TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ")?;

    seed_default_settings(conn)?;
    Ok(())
}

fn seed_default_settings(conn: &Connection) -> Result<()> {
    let defaults = Settings::default();
    let pairs = [
        ("min_duration_secs", defaults.min_duration_secs.to_string()),
        ("merge_gap_secs", defaults.merge_gap_secs.to_string()),
        ("idle_timeout_secs", defaults.idle_timeout_secs.to_string()),
        ("timeline_start_hour", defaults.timeline_start_hour.to_string()),
        ("timeline_end_hour", defaults.timeline_end_hour.to_string()),
    ];
    for (key, val) in pairs {
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, val],
        )?;
    }
    Ok(())
}

// ── Activity ──────────────────────────────────────────────────────────────────

pub fn insert_activity(
    conn: &Connection,
    app_name: &str,
    window_title: &str,
    started_at: &DateTime<Utc>,
    ended_at: &DateTime<Utc>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO activity_raw (started_at, ended_at, app_name, window_title)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            started_at.to_rfc3339(),
            ended_at.to_rfc3339(),
            app_name,
            window_title,
        ],
    )?;
    Ok(())
}

pub fn get_raw_activity_for_date(conn: &Connection, date: &str) -> Result<Vec<RawActivity>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, ended_at, app_name, window_title
         FROM activity_raw
         WHERE date(started_at) = ?1
         ORDER BY started_at",
    )?;
    let rows = stmt.query_map(params![date], |row| {
        let started_str: String = row.get(1)?;
        let ended_str: String = row.get(2)?;
        Ok(RawActivity {
            id: row.get(0)?,
            started_at: DateTime::parse_from_rfc3339(&started_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            ended_at: DateTime::parse_from_rfc3339(&ended_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            app_name: row.get(3)?,
            window_title: row.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_activity_for_date(
    conn: &Connection,
    date: &str,
    settings: &Settings,
    rules: &[FilterRule],
) -> Result<Vec<ActivityBlock>> {
    let raw = get_raw_activity_for_date(conn, date)?;
    Ok(merge_and_filter(raw, settings, rules))
}

fn should_ignore(event: &RawActivity, rules: &[FilterRule]) -> bool {
    for rule in rules {
        match rule.rule_type {
            FilterRuleType::AppName => {
                if event.app_name.to_lowercase() == rule.value.to_lowercase() {
                    return true;
                }
            }
            FilterRuleType::TitlePattern => {
                if event
                    .window_title
                    .to_lowercase()
                    .contains(&rule.value.to_lowercase())
                {
                    return true;
                }
            }
        }
    }
    false
}

fn merge_and_filter(
    raw: Vec<RawActivity>,
    settings: &Settings,
    rules: &[FilterRule],
) -> Vec<ActivityBlock> {
    // 1. Apply ignore rules
    let mut events: Vec<RawActivity> = raw
        .into_iter()
        .filter(|e| !should_ignore(e, rules))
        .collect();

    if events.is_empty() {
        return vec![];
    }

    events.sort_by_key(|e| e.started_at);

    // 2. Merge consecutive same-app events where gap <= merge_gap_secs
    let merge_gap = chrono::Duration::seconds(settings.merge_gap_secs);
    let mut merged: Vec<ActivityBlock> = Vec::new();
    let first = &events[0];
    let mut current = ActivityBlock {
        app_name: first.app_name.clone(),
        window_title: first.window_title.clone(),
        started_at: first.started_at,
        ended_at: first.ended_at,
        duration_secs: (first.ended_at - first.started_at).num_seconds(),
    };

    for event in events.iter().skip(1) {
        let gap = event.started_at - current.ended_at;
        if event.app_name == current.app_name && gap <= merge_gap {
            current.ended_at = event.ended_at;
            current.window_title = event.window_title.clone();
        } else {
            merged.push(current.clone());
            current = ActivityBlock {
                app_name: event.app_name.clone(),
                window_title: event.window_title.clone(),
                started_at: event.started_at,
                ended_at: event.ended_at,
                duration_secs: (event.ended_at - event.started_at).num_seconds(),
            };
        }
    }
    merged.push(current);

    // 3. Recalculate durations and filter by min_duration
    merged
        .into_iter()
        .map(|mut b| {
            b.duration_secs = (b.ended_at - b.started_at).num_seconds();
            b
        })
        .filter(|b| b.duration_secs >= settings.min_duration_secs)
        .collect()
}

// ── Projects ──────────────────────────────────────────────────────────────────

pub fn get_projects(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, archived_at FROM projects ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| {
        let archived_str: Option<String> = row.get(3)?;
        let archived_at = archived_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            archived_at,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn insert_project(conn: &Connection, name: &str, color: &str) -> Result<Project> {
    conn.execute(
        "INSERT INTO projects (name, color) VALUES (?1, ?2)",
        params![name, color],
    )?;
    let id = conn.last_insert_rowid();
    Ok(Project {
        id,
        name: name.to_string(),
        color: color.to_string(),
        archived_at: None,
    })
}

pub fn update_project(conn: &Connection, id: i64, name: &str, color: &str) -> Result<()> {
    conn.execute(
        "UPDATE projects SET name = ?1, color = ?2 WHERE id = ?3",
        params![name, color, id],
    )?;
    Ok(())
}

pub fn archive_project(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE projects SET archived_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

// ── Time entries ──────────────────────────────────────────────────────────────

pub fn get_time_entries_for_date(conn: &Connection, date: &str) -> Result<Vec<TimeEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, date, project_id, start_minutes, end_minutes, note
         FROM time_entries WHERE date = ?1 ORDER BY start_minutes",
    )?;
    let rows = stmt.query_map(params![date], |row| {
        Ok(TimeEntry {
            id: row.get(0)?,
            date: row.get(1)?,
            project_id: row.get(2)?,
            start_minutes: row.get(3)?,
            end_minutes: row.get(4)?,
            note: row.get(5)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn insert_time_entry(
    conn: &Connection,
    date: &str,
    project_id: i64,
    start_minutes: i64,
    end_minutes: i64,
    note: &str,
) -> Result<TimeEntry> {
    conn.execute(
        "INSERT INTO time_entries (date, project_id, start_minutes, end_minutes, note)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![date, project_id, start_minutes, end_minutes, note],
    )?;
    let id = conn.last_insert_rowid();
    Ok(TimeEntry {
        id,
        date: date.to_string(),
        project_id,
        start_minutes,
        end_minutes,
        note: note.to_string(),
    })
}

pub fn update_time_entry(
    conn: &Connection,
    id: i64,
    project_id: i64,
    start_minutes: i64,
    end_minutes: i64,
    note: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE time_entries
         SET project_id = ?1, start_minutes = ?2, end_minutes = ?3, note = ?4
         WHERE id = ?5",
        params![project_id, start_minutes, end_minutes, note, id],
    )?;
    Ok(())
}

pub fn delete_time_entry(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM time_entries WHERE id = ?1", params![id])?;
    Ok(())
}

// ── Filter rules ──────────────────────────────────────────────────────────────

pub fn get_filter_rules(conn: &Connection) -> Result<Vec<FilterRule>> {
    let mut stmt = conn.prepare("SELECT id, rule_type, value FROM filter_rules ORDER BY id")?;
    let rows = stmt.query_map([], |row| {
        let type_str: String = row.get(1)?;
        Ok(FilterRule {
            id: row.get(0)?,
            rule_type: FilterRuleType::from_str(&type_str)
                .unwrap_or(FilterRuleType::TitlePattern),
            value: row.get(2)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn insert_filter_rule(
    conn: &Connection,
    rule_type: &FilterRuleType,
    value: &str,
) -> Result<FilterRule> {
    conn.execute(
        "INSERT INTO filter_rules (rule_type, value) VALUES (?1, ?2)",
        params![rule_type.as_str(), value],
    )?;
    let id = conn.last_insert_rowid();
    Ok(FilterRule {
        id,
        rule_type: rule_type.clone(),
        value: value.to_string(),
    })
}

pub fn delete_filter_rule(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM filter_rules WHERE id = ?1", params![id])?;
    Ok(())
}

// ── Settings ──────────────────────────────────────────────────────────────────

pub fn get_settings(conn: &Connection) -> Result<Settings> {
    let get = |key: &str, default: i64| -> i64 {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
    };
    let d = Settings::default();
    Ok(Settings {
        min_duration_secs: get("min_duration_secs", d.min_duration_secs),
        merge_gap_secs: get("merge_gap_secs", d.merge_gap_secs),
        idle_timeout_secs: get("idle_timeout_secs", d.idle_timeout_secs),
        timeline_start_hour: get("timeline_start_hour", d.timeline_start_hour),
        timeline_end_hour: get("timeline_end_hour", d.timeline_end_hour),
    })
}

pub fn save_settings(conn: &Connection, s: &Settings) -> Result<()> {
    let pairs = [
        ("min_duration_secs", s.min_duration_secs.to_string()),
        ("merge_gap_secs", s.merge_gap_secs.to_string()),
        ("idle_timeout_secs", s.idle_timeout_secs.to_string()),
        ("timeline_start_hour", s.timeline_start_hour.to_string()),
        ("timeline_end_hour", s.timeline_end_hour.to_string()),
    ];
    for (key, val) in pairs {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, val],
        )?;
    }
    Ok(())
}
