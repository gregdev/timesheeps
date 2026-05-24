use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use tauri::Manager;

use crate::models::{
    ActivityBlock, DaySearchResult, FilterRule, FilterRuleType, Project, ProjectMatchRule,
    RawActivity, SearchResults, Settings, SuggestedEntry, TimeEntry,
};

#[allow(dead_code)]
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
            window_title TEXT NOT NULL,
            window_id    INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_activity_raw_started
            ON activity_raw (started_at);

        CREATE TABLE IF NOT EXISTS projects (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            color       TEXT NOT NULL DEFAULT '#6366f1',
            archived_at TEXT,
            parent_id   INTEGER REFERENCES projects(id) ON DELETE SET NULL
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

        CREATE TABLE IF NOT EXISTS project_match_rules (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            rule_type  TEXT NOT NULL CHECK(rule_type IN ('title_pattern', 'app_name')),
            value      TEXT NOT NULL
        );
    ")?;

    seed_default_settings(conn)?;
    // Add window_id to existing DBs that predate this column (ignored if already present)
    let _ = conn.execute("ALTER TABLE activity_raw ADD COLUMN window_id INTEGER NOT NULL DEFAULT 0", []);
    // Add parent_id to existing DBs (ignored if already present)
    let _ = conn.execute("ALTER TABLE projects ADD COLUMN parent_id INTEGER REFERENCES projects(id) ON DELETE SET NULL", []);
    // Remove expression index if it was ever created (non-deterministic, SQLite 3.38+ rejects it)
    let _ = conn.execute("DROP INDEX IF EXISTS idx_activity_raw_localdate", []);
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
        ("start_on_login", if defaults.start_on_login { "1" } else { "0" }.to_string()),
        ("snap_minutes", defaults.snap_minutes.to_string()),
        ("window_summary_min_secs", defaults.window_summary_min_secs.to_string()),
        ("title_split_apps", "Brave,Chrome,Firefox,msedge,Opera,Vivaldi,Arc,Zen,Chromium".to_string()),
        ("week_starts_on", "1".to_string()),
        ("pay_schedule_frequency", defaults.pay_schedule_frequency.clone()),
        ("pay_schedule_anchor", defaults.pay_schedule_anchor.clone()),
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
    window_id: u64,
    started_at: &DateTime<Utc>,
    ended_at: &DateTime<Utc>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO activity_raw (started_at, ended_at, app_name, window_title, window_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            started_at.to_rfc3339(),
            ended_at.to_rfc3339(),
            app_name,
            window_title,
            window_id as i64,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_activity_end(
    conn: &Connection,
    id: i64,
    ended_at: &DateTime<Utc>,
) -> Result<()> {
    conn.execute(
        "UPDATE activity_raw SET ended_at = ?1 WHERE id = ?2",
        params![ended_at.to_rfc3339(), id],
    )?;
    Ok(())
}

pub fn get_raw_activity_for_date(conn: &Connection, date: &str) -> Result<Vec<RawActivity>> {
    // Compute UTC range for the given local date so the started_at index is used.
    let (start_utc, end_utc) = {
        use chrono::{Days, Local, NaiveDate};
        let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap_or_else(|_| Local::now().date_naive());
        let s = naive
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .earliest()
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
            .to_rfc3339();
        let e = naive
            .checked_add_days(Days::new(1))
            .unwrap_or(naive)
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .earliest()
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
            .to_rfc3339();
        (s, e)
    };
    let mut stmt = conn.prepare(
        "SELECT id, started_at, ended_at, app_name, window_title, window_id
         FROM activity_raw
         WHERE started_at >= ?1 AND started_at < ?2
         ORDER BY started_at",
    )?;
    let rows = stmt.query_map(params![start_utc, end_utc], |row| {
        let started_str: String = row.get(1)?;
        let ended_str: String = row.get(2)?;
        let window_id_i64: i64 = row.get(5).unwrap_or(0);
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
            window_id: window_id_i64 as u64,
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

/// Aggregate ALL raw activity for the day by window (no min-duration filter).
/// Groups by window_id when available, falling back to window_title for legacy rows.
/// The representative title for each group is taken from the longest individual segment.
pub fn get_window_summary_for_date(
    conn: &Connection,
    date: &str,
    settings: &Settings,
) -> Result<Vec<crate::models::WindowSummaryItem>> {
    use std::collections::HashMap;

    let raw = get_raw_activity_for_date(conn, date)?;

    // key → (total_secs, best_title, best_segment_secs)
    let mut groups: HashMap<(String, String), (i64, String, i64)> = HashMap::new();

    for event in &raw {
        let duration = (event.ended_at - event.started_at).num_seconds();
        // Apps in title_split_apps are grouped by window title (e.g. browsers where
        // each tab is a distinct title but shares the same HWND). All other apps use
        // window_id so that file switches inside the same project window are merged.
        let split_by_title = settings
            .title_split_apps
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&event.app_name));
        let key = if split_by_title {
            (event.app_name.clone(), format!("ttl:{}", event.window_title))
        } else if event.window_id != 0 {
            (event.app_name.clone(), format!("wid:{}", event.window_id))
        } else {
            (event.app_name.clone(), format!("ttl:{}", event.window_title))
        };
        let entry = groups
            .entry(key)
            .or_insert((0, event.window_title.clone(), 0));
        entry.0 += duration;
        if duration > entry.2 {
            entry.1 = event.window_title.clone(); // title from longest segment
            entry.2 = duration;
        }
    }

    let mut result: Vec<crate::models::WindowSummaryItem> = groups
        .into_iter()
        .map(|((app_name, _), (total_secs, window_title, _))| {
            crate::models::WindowSummaryItem { app_name, window_title, total_secs }
        })
        .collect();
    result.sort_by(|a, b| b.total_secs.cmp(&a.total_secs));
    result.retain(|item| item.total_secs >= settings.window_summary_min_secs);
    Ok(result)
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
        window_id: first.window_id,
    };

    for event in events.iter().skip(1) {
        let gap = event.started_at - current.ended_at;
        let same_window = if event.window_id != 0 {
                event.window_id == current.window_id
            } else {
                event.app_name == current.app_name && event.window_title == current.window_title
            };
        if same_window && event.app_name == current.app_name && gap <= merge_gap {
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
                window_id: event.window_id,
            };
        }
    }
    merged.push(current);

    // 3. Recalculate durations and filter by min_duration
    let filtered: Vec<ActivityBlock> = merged
        .into_iter()
        .map(|mut b| {
            b.duration_secs = (b.ended_at - b.started_at).num_seconds();
            b
        })
        .filter(|b| b.duration_secs >= settings.min_duration_secs)
        .collect();

    // 4. Second merge pass: short events from other apps may have been blocking
    //    same-app merges (e.g. a 30s blip between two Claude sessions). Now that
    //    those short blocks are gone, re-merge any adjacent same-app blocks.
    if filtered.is_empty() {
        return vec![];
    }
    let mut result: Vec<ActivityBlock> = Vec::new();
    let mut current = filtered[0].clone();
    for block in filtered.iter().skip(1) {
        let gap = block.started_at - current.ended_at;
        let same_window = if block.window_id != 0 {
                block.window_id == current.window_id
            } else {
                block.app_name == current.app_name && block.window_title == current.window_title
            };
        if same_window && block.app_name == current.app_name && gap <= merge_gap {
            current.ended_at = block.ended_at;
            current.window_title = block.window_title.clone();
            current.duration_secs = (current.ended_at - current.started_at).num_seconds();
        } else {
            result.push(current.clone());
            current = block.clone();
        }
    }
    result.push(current);
    result
}

// ── Projects ──────────────────────────────────────────────────────────────────

pub fn get_projects(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, archived_at, parent_id FROM projects ORDER BY name",
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
            parent_id: row.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn insert_project(conn: &Connection, name: &str, color: &str, parent_id: Option<i64>) -> Result<Project> {
    conn.execute(
        "INSERT INTO projects (name, color, parent_id) VALUES (?1, ?2, ?3)",
        params![name, color, parent_id],
    )?;
    let id = conn.last_insert_rowid();
    Ok(Project {
        id,
        name: name.to_string(),
        color: color.to_string(),
        archived_at: None,
        parent_id,
    })
}

pub fn update_project(conn: &Connection, id: i64, name: &str, color: &str, parent_id: Option<i64>) -> Result<()> {
    conn.execute(
        "UPDATE projects SET name = ?1, color = ?2, parent_id = ?3 WHERE id = ?4",
        params![name, color, parent_id, id],
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
    let get_bool = |key: &str, default: bool| -> bool {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|s| s == "1")
        .unwrap_or(default)
    };
    let get_str = |key: &str, default: &str| -> String {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .unwrap_or_else(|| default.to_string())
    };
    let d = Settings::default();
    let title_split_apps: Vec<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params!["title_split_apps"],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|s| s.split(',').filter(|p| !p.is_empty()).map(str::to_string).collect())
        .unwrap_or_else(|| d.title_split_apps.clone());
    Ok(Settings {
        min_duration_secs: get("min_duration_secs", d.min_duration_secs),
        merge_gap_secs: get("merge_gap_secs", d.merge_gap_secs),
        idle_timeout_secs: get("idle_timeout_secs", d.idle_timeout_secs),
        timeline_start_hour: get("timeline_start_hour", d.timeline_start_hour),
        timeline_end_hour: get("timeline_end_hour", d.timeline_end_hour),
        start_on_login: get_bool("start_on_login", d.start_on_login),
        snap_minutes: get("snap_minutes", d.snap_minutes),
        window_summary_min_secs: get("window_summary_min_secs", d.window_summary_min_secs),
        title_split_apps,
        week_starts_on: get("week_starts_on", d.week_starts_on),
        pay_schedule_frequency: get_str("pay_schedule_frequency", &d.pay_schedule_frequency),
        pay_schedule_anchor: get_str("pay_schedule_anchor", &d.pay_schedule_anchor),
    })
}

pub fn save_settings(conn: &Connection, s: &Settings) -> Result<()> {
    let pairs = [
        ("min_duration_secs", s.min_duration_secs.to_string()),
        ("merge_gap_secs", s.merge_gap_secs.to_string()),
        ("idle_timeout_secs", s.idle_timeout_secs.to_string()),
        ("timeline_start_hour", s.timeline_start_hour.to_string()),
        ("timeline_end_hour", s.timeline_end_hour.to_string()),
        ("start_on_login", if s.start_on_login { "1" } else { "0" }.to_string()),
        ("snap_minutes", s.snap_minutes.to_string()),
        ("window_summary_min_secs", s.window_summary_min_secs.to_string()),
        ("title_split_apps", s.title_split_apps.join(",")),
        ("week_starts_on", s.week_starts_on.to_string()),
        ("pay_schedule_frequency", s.pay_schedule_frequency.clone()),
        ("pay_schedule_anchor", s.pay_schedule_anchor.clone()),
    ];
    for (key, val) in pairs {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, val],
        )?;
    }
    Ok(())
}

// ── Project match rules ───────────────────────────────────────────────────────

pub fn get_project_match_rules(conn: &Connection) -> Result<Vec<ProjectMatchRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, rule_type, value FROM project_match_rules ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        let type_str: String = row.get(2)?;
        Ok(ProjectMatchRule {
            id: row.get(0)?,
            project_id: row.get(1)?,
            rule_type: FilterRuleType::from_str(&type_str)
                .unwrap_or(FilterRuleType::TitlePattern),
            value: row.get(3)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn insert_project_match_rule(
    conn: &Connection,
    project_id: i64,
    rule_type: &FilterRuleType,
    value: &str,
) -> Result<ProjectMatchRule> {
    conn.execute(
        "INSERT INTO project_match_rules (project_id, rule_type, value) VALUES (?1, ?2, ?3)",
        params![project_id, rule_type.as_str(), value],
    )?;
    let id = conn.last_insert_rowid();
    Ok(ProjectMatchRule {
        id,
        project_id,
        rule_type: rule_type.clone(),
        value: value.to_string(),
    })
}

pub fn delete_project_match_rule(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM project_match_rules WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn compute_suggestions(
    blocks: Vec<ActivityBlock>,
    rules: &[ProjectMatchRule],
) -> Vec<SuggestedEntry> {
    if rules.is_empty() {
        return vec![];
    }

    let mut raw: Vec<SuggestedEntry> = Vec::new();

    for block in &blocks {
        // First matching rule wins
        for rule in rules {
            let matches = match rule.rule_type {
                FilterRuleType::TitlePattern => block
                    .window_title
                    .to_lowercase()
                    .contains(&rule.value.to_lowercase()),
                FilterRuleType::AppName => {
                    block.app_name.to_lowercase() == rule.value.to_lowercase()
                }
            };
            if matches {
                raw.push(SuggestedEntry {
                    project_id: rule.project_id,
                    started_at: block.started_at,
                    ended_at: block.ended_at,
                });
                break;
            }
        }
    }

    if raw.is_empty() {
        return raw;
    }

    // Merge consecutive suggestions for the same project
    let mut merged: Vec<SuggestedEntry> = vec![raw[0].clone()];
    for s in raw.iter().skip(1) {
        let last = merged.last_mut().unwrap();
        if s.project_id == last.project_id && s.started_at <= last.ended_at {
            if s.ended_at > last.ended_at {
                last.ended_at = s.ended_at;
            }
        } else {
            merged.push(s.clone());
        }
    }

    merged
}

// ── Search ────────────────────────────────────────────────────────────────────

pub fn search(
    conn: &Connection,
    query: &str,
    settings: &Settings,
    rules: &[FilterRule],
) -> Result<SearchResults> {
    if query.trim().is_empty() {
        return Ok(SearchResults { days: vec![], note_matches: vec![] });
    }

    let q = format!("%{}%", query.to_lowercase());

    // Distinct dates that have matching raw activity (most recent first, max 60)
    let mut date_stmt = conn.prepare(
        "SELECT DISTINCT date(started_at, 'localtime') as d
         FROM activity_raw
         WHERE LOWER(window_title) LIKE ?1 OR LOWER(app_name) LIKE ?1
         ORDER BY d DESC
         LIMIT 60",
    )?;
    let dates: Vec<String> = date_stmt
        .query_map(params![q], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let q_lower = query.to_lowercase();
    let mut days = Vec::new();
    for date in &dates {
        let all_blocks = get_activity_for_date(conn, date, settings, rules)?;
        let matched_blocks: Vec<ActivityBlock> = all_blocks
            .iter()
            .filter(|b| {
                b.window_title.to_lowercase().contains(&q_lower)
                    || b.app_name.to_lowercase().contains(&q_lower)
            })
            .cloned()
            .collect();
        if matched_blocks.is_empty() {
            continue;
        }
        let total_matched_secs = matched_blocks.iter().map(|b| b.duration_secs).sum();
        days.push(DaySearchResult {
            date: date.clone(),
            all_blocks,
            matched_blocks,
            total_matched_secs,
        });
    }

    // Search time entry notes
    let mut note_stmt = conn.prepare(
        "SELECT id, date, project_id, start_minutes, end_minutes, note
         FROM time_entries
         WHERE LOWER(note) LIKE ?1
         ORDER BY date DESC, start_minutes",
    )?;
    let note_matches: Vec<TimeEntry> = note_stmt
        .query_map(params![q], |row| {
            Ok(TimeEntry {
                id: row.get(0)?,
                date: row.get(1)?,
                project_id: row.get(2)?,
                start_minutes: row.get(3)?,
                end_minutes: row.get(4)?,
                note: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(SearchResults { days, note_matches })
}
