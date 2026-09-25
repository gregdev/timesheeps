use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use tauri::Manager;

use crate::models::{
    ActivityBlock, DaySearchResult, FilterRule, FilterRuleType, KnownApp, MatchCondition,
    MatchField, MatchOperator, Project, ProjectMatchRule, RawActivity, SearchResults, Settings,
    TimeEntry,
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

/// Current schema version, stored in SQLite's `user_version` pragma.
/// Bump this and add a `migrate_vN` step whenever the schema changes shape.
///
/// v2 repairs databases migrated by the original v1, which renamed
/// `project_match_rules` out from under a foreign key that already pointed at
/// it (see `repair_legacy_foreign_key`).
const SCHEMA_VERSION: i64 = 2;

/// Match rules: a named group of conditions, ordered by `position`.
const CREATE_MATCH_RULES: &str = "
    CREATE TABLE IF NOT EXISTS project_match_rules (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name       TEXT NOT NULL DEFAULT '',
        position   INTEGER NOT NULL DEFAULT 0
    );";

/// Conditions are AND-ed together within their parent rule.
const CREATE_MATCH_CONDITIONS: &str = "
    CREATE TABLE IF NOT EXISTS project_match_rule_conditions (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        match_rule_id INTEGER NOT NULL REFERENCES project_match_rules(id) ON DELETE CASCADE,
        position      INTEGER NOT NULL DEFAULT 0,
        field         TEXT NOT NULL CHECK(field IN ('app_name', 'window_title')),
        operator      TEXT NOT NULL CHECK(operator IN ('contains', 'equals', 'starts_with', 'ends_with')),
        value         TEXT NOT NULL,
        negate        INTEGER NOT NULL DEFAULT 0
    );";

fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch("
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

        CREATE INDEX IF NOT EXISTS idx_activity_raw_app
            ON activity_raw (app_name);
    ")?;

    seed_default_settings(conn)?;
    // Add window_id to existing DBs that predate this column (ignored if already present)
    let _ = conn.execute("ALTER TABLE activity_raw ADD COLUMN window_id INTEGER NOT NULL DEFAULT 0", []);
    // Add exe_path to existing DBs that predate this column (ignored if already present)
    let _ = conn.execute("ALTER TABLE activity_raw ADD COLUMN exe_path TEXT NOT NULL DEFAULT ''", []);
    // Add parent_id to existing DBs (ignored if already present)
    let _ = conn.execute("ALTER TABLE projects ADD COLUMN parent_id INTEGER REFERENCES projects(id) ON DELETE SET NULL", []);
    // Remove expression index if it was ever created (non-deterministic, SQLite 3.38+ rejects it)
    let _ = conn.execute("DROP INDEX IF EXISTS idx_activity_raw_localdate", []);

    // Databases predating the rule-group model used a flat
    // `project_match_rules (rule_type, value)` shape. It is replaced outright —
    // existing flat rules are not carried over.
    if table_has_column(conn, "project_match_rules", "rule_type")? {
        drop_legacy_rule_tables(conn)?;
    }

    conn.execute_batch(CREATE_MATCH_RULES)?;
    conn.execute_batch(CREATE_MATCH_CONDITIONS)?;
    repair_legacy_foreign_key(conn)?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_pmr_project
            ON project_match_rules (project_id, position);
         CREATE INDEX IF NOT EXISTS idx_pmrc_rule
            ON project_match_rule_conditions (match_rule_id, position);",
    )?;

    set_schema_version(conn, SCHEMA_VERSION)?;
    Ok(())
}

fn current_schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

fn set_schema_version(conn: &Connection, version: i64) -> Result<()> {
    // PRAGMA cannot be parameterised; `version` is an internal constant.
    conn.execute_batch(&format!("PRAGMA user_version = {}", version))?;
    Ok(())
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The stored `CREATE TABLE` statement for a table, if it exists.
fn table_sql(conn: &Connection, table: &str) -> Result<Option<String>> {
    let mut stmt =
        conn.prepare("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1")?;
    let mut rows = stmt.query(params![table])?;
    match rows.next()? {
        Some(row) => Ok(row.get(0)?),
        None => Ok(None),
    }
}

/// Drop the pre-rule-group tables so they can be recreated with the current
/// shape. The child table goes first while foreign keys are disabled.
fn drop_legacy_rule_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys=OFF;
         BEGIN;
         DROP TABLE IF EXISTS project_match_rule_conditions;
         DROP TABLE IF EXISTS project_match_rules;
         DROP TABLE IF EXISTS project_match_rules_legacy;
         DROP TABLE IF EXISTS schema_version;
         COMMIT;
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(())
}

/// Repair databases written by the *original* v1 step, which renamed
/// `project_match_rules` to `project_match_rules_legacy` while a foreign key
/// already pointed at it. SQLite rewrites referencing keys when a table is
/// renamed, so `project_match_rule_conditions` was left pointing at a table
/// that was then dropped. Reads still worked, but every insert or delete on the
/// conditions table failed with "no such table: project_match_rules_legacy",
/// which made editing an existing rule impossible.
///
/// The child table is rebuilt with a correct key, preserving its rows.
fn repair_legacy_foreign_key(conn: &Connection) -> Result<()> {
    let Some(sql) = table_sql(conn, "project_match_rule_conditions")? else {
        return Ok(());
    };
    if !sql.contains("_legacy") {
        return Ok(());
    }

    conn.execute_batch(&format!(
        "PRAGMA foreign_keys=OFF;
         BEGIN;
         ALTER TABLE project_match_rule_conditions RENAME TO project_match_rule_conditions_stale;
         {CREATE_MATCH_CONDITIONS}
         INSERT INTO project_match_rule_conditions
             (id, match_rule_id, position, field, operator, value, negate)
             SELECT id, match_rule_id, position, field, operator, value, negate
             FROM project_match_rule_conditions_stale;
         DROP TABLE project_match_rule_conditions_stale;
         COMMIT;
         PRAGMA foreign_keys=ON;"
    ))?;
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
        ("title_group_apps", "Code".to_string()),
        ("week_starts_on", "1".to_string()),
        ("pay_schedule_frequency", defaults.pay_schedule_frequency.clone()),
        ("pay_schedule_anchor", defaults.pay_schedule_anchor.clone()),
        ("auto_accept_suggested", if defaults.auto_accept_suggested { "1" } else { "0" }.to_string()),
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
    exe_path: &str,
    started_at: &DateTime<Utc>,
    ended_at: &DateTime<Utc>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO activity_raw (started_at, ended_at, app_name, window_title, window_id, exe_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            started_at.to_rfc3339(),
            ended_at.to_rfc3339(),
            app_name,
            window_title,
            window_id as i64,
            exe_path,
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

/// RFC3339 UTC timestamp for the start of the local day `offset_days` before today.
fn local_day_start_utc(offset_days: i64) -> String {
    use chrono::Local;
    let date = (Local::now() + chrono::Duration::days(offset_days)).date_naive();
    date.and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
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
        "SELECT id, started_at, ended_at, app_name, window_title, window_id, exe_path
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
            exe_path: row.get(6).unwrap_or_default(),
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

/// Try to extract a project/workspace name from an IDE window title.
///
/// For VS Code titles like:
///   "file.ts - myproject [WSL: Ubuntu] - Visual Studio Code"
/// or:
///   "file.ts — myproject — Visual Studio Code"
///
/// The algorithm:
/// 1. Strip the trailing app-name segment (last ` — ` or ` - ` segment that
///    contains the app_name, case-insensitive — handles "Code" matching
///    "Visual Studio Code")
/// 2. Strip any trailing [...] bracket segment
/// 3. Return the last ` — ` or ` - ` segment as the project name
///
/// Returns None if no meaningful project name can be extracted.
pub(crate) fn extract_title_group_key(app_name: &str, window_title: &str) -> Option<String> {
    let title = window_title.trim();
    if title.is_empty() {
        return None;
    }

    // Split into segments by both dash styles
    let all_segments: Vec<&str> = title
        .split(" — ")
        .flat_map(|s| s.split(" - "))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if all_segments.is_empty() {
        return None;
    }

    // 1. Strip the trailing app-name segment if the last segment contains
    //    the app_name (case-insensitive). This handles "Code" ↔ "Visual Studio Code".
    let app_lower = app_name.to_lowercase();
    let last_is_app = all_segments
        .last()
        .map(|s| s.to_lowercase().contains(&app_lower))
        .unwrap_or(false);

    let start_idx = if last_is_app && all_segments.len() > 1 {
        all_segments.len() - 1
    } else {
        all_segments.len()
    };

    // Rebuild the title without the app segment(s), re-joining with " - "
    let without_app = all_segments[..start_idx].join(" - ");

    // 2. Strip trailing [...] bracket (e.g. "[WSL: LightbulbUbuntu]")
    let mut stripped = without_app;
    if let Some(bracket_start) = stripped.rfind('[') {
        if stripped.ends_with(']') {
            let before = stripped[..bracket_start].trim().to_string();
            if !before.is_empty() {
                stripped = before;
            }
        }
    }

    // 3. Re-split what remains and take the last segment as the project name
    let final_segments: Vec<&str> = stripped
        .split(" — ")
        .flat_map(|s| s.split(" - "))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if final_segments.len() <= 1 {
        // Only one segment — that's probably just the filename, not a project.
        return None;
    }

    let candidate = final_segments.last().unwrap().to_string();
    // If the candidate looks like a filename (has an extension), skip it
    if candidate.contains('.') && candidate.len() < 60 {
        return None;
    }

    Some(candidate)
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

    // key → (total_secs, best_title, best_segment_secs, window_id)
    let mut groups: HashMap<(String, String), (i64, String, i64, u64)> = HashMap::new();

    for event in &raw {
        let duration = (event.ended_at - event.started_at).num_seconds();
        // Apps in title_split_apps are grouped by window title (e.g. browsers where
        // each tab is a distinct title but shares the same HWND).
        let split_by_title = settings
            .title_split_apps
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&event.app_name));
        // Apps in title_group_apps are grouped by a project name extracted from the
        // title so that closing/reopening the app (new HWND) still merges entries.
        let group_key = if !split_by_title {
            settings
                .title_group_apps
                .iter()
                .any(|a| a.eq_ignore_ascii_case(&event.app_name))
                .then(|| extract_title_group_key(&event.app_name, &event.window_title))
                .flatten()
        } else {
            None
        };

        let key = if split_by_title {
            (event.app_name.clone(), format!("ttl:{}", event.window_title))
        } else if let Some(ref gk) = group_key {
            (event.app_name.clone(), format!("grp:{}", gk))
        } else if event.window_id != 0 {
            (event.app_name.clone(), format!("wid:{}", event.window_id))
        } else {
            (event.app_name.clone(), format!("ttl:{}", event.window_title))
        };

        // Only window-grouped rows have a meaningful handle: title-split groups
        // span every window of that app, and grouped apps span every window too.
        let group_window_id = if !split_by_title && group_key.is_none() && event.window_id != 0 {
            event.window_id
        } else {
            0
        };

        // For title_group_apps, use the extracted project name as the display title
        let display_title = group_key.as_ref().unwrap_or(&event.window_title);

        let entry = groups
            .entry(key)
            .or_insert((0, display_title.clone(), 0, group_window_id));
        entry.0 += duration;
        if duration > entry.2 {
            entry.1 = display_title.clone();
            entry.2 = duration;
        }
    }

    let mut result: Vec<crate::models::WindowSummaryItem> = groups
        .into_iter()
        .map(|((app_name, _), (total_secs, window_title, _, window_id))| {
            crate::models::WindowSummaryItem {
                app_name,
                window_title,
                total_secs,
                window_id,
            }
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

/// Max distinct window titles retained per merged block. Bounds the payload for
/// very long sessions where a single window cycles through many documents.
const MAX_BLOCK_TITLES: usize = 64;

/// Append `title` to a block's title list, ignoring duplicates and past the cap.
fn push_title(titles: &mut Vec<String>, title: &str) {
    if titles.len() >= MAX_BLOCK_TITLES || titles.iter().any(|t| t == title) {
        return;
    }
    titles.push(title.to_string());
}

fn new_block(event: &RawActivity) -> ActivityBlock {
    ActivityBlock {
        app_name: event.app_name.clone(),
        window_title: event.window_title.clone(),
        window_titles: vec![event.window_title.clone()],
        started_at: event.started_at,
        ended_at: event.ended_at,
        duration_secs: (event.ended_at - event.started_at).num_seconds(),
        window_id: event.window_id,
    }
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
    let mut current = new_block(&events[0]);

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
            push_title(&mut current.window_titles, &event.window_title);
        } else {
            merged.push(current);
            current = new_block(event);
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
            for title in &block.window_titles {
                push_title(&mut current.window_titles, title);
            }
            current.duration_secs = (current.ended_at - current.started_at).num_seconds();
        } else {
            result.push(current);
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

pub fn get_project(conn: &Connection, id: i64) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, archived_at, parent_id FROM projects WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
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
    Ok(rows.next().transpose()?)
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

pub fn unarchive_project(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE projects SET archived_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

/// Deletes a project. Time entries and match rules cascade away with it.
pub fn delete_project(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
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

// ── Generic key-value setting helpers ─────────────────────────────────────────

/// Get a setting value as a String (or default).
pub fn get_setting_str(conn: &Connection, key: &str, default: &str) -> Result<String> {
    let val = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or_else(|_| default.to_string());
    Ok(val)
}

/// Set a setting value as a String.
pub fn set_setting_str(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
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
    let title_group_apps: Vec<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params!["title_group_apps"],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|s| s.split(',').filter(|p| !p.is_empty()).map(str::to_string).collect())
        .unwrap_or_else(|| d.title_group_apps.clone());
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
        title_group_apps,
        week_starts_on: get("week_starts_on", d.week_starts_on),
        pay_schedule_frequency: get_str("pay_schedule_frequency", &d.pay_schedule_frequency),
        pay_schedule_anchor: get_str("pay_schedule_anchor", &d.pay_schedule_anchor),
        timeline_col_split_pct: get("timeline_col_split_pct", d.timeline_col_split_pct),
        layout_window_summary_width: get("layout_window_summary_width", d.layout_window_summary_width),
        layout_project_summary_width: get("layout_project_summary_width", d.layout_project_summary_width),
        auto_accept_suggested: get_bool("auto_accept_suggested", d.auto_accept_suggested),
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
        ("title_group_apps", s.title_group_apps.join(",")),
        ("week_starts_on", s.week_starts_on.to_string()),
        ("pay_schedule_frequency", s.pay_schedule_frequency.clone()),
        ("pay_schedule_anchor", s.pay_schedule_anchor.clone()),
        ("timeline_col_split_pct", s.timeline_col_split_pct.to_string()),
        ("layout_window_summary_width", s.layout_window_summary_width.to_string()),
        ("layout_project_summary_width", s.layout_project_summary_width.to_string()),
        ("auto_accept_suggested", if s.auto_accept_suggested { "1" } else { "0" }.to_string()),
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
    let mut rule_stmt = conn.prepare(
        "SELECT id, project_id, name, position FROM project_match_rules
         ORDER BY position, id",
    )?;
    let rows = rule_stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;

    let mut rules: Vec<ProjectMatchRule> = Vec::new();
    for row in rows {
        let (id, project_id, name, position) = row?;
        rules.push(ProjectMatchRule {
            id,
            project_id,
            name,
            position,
            conditions: get_rule_conditions(conn, id)?,
        });
    }
    Ok(rules)
}

fn get_rule_conditions(conn: &Connection, rule_id: i64) -> Result<Vec<MatchCondition>> {
    let mut stmt = conn.prepare(
        "SELECT field, operator, value, negate FROM project_match_rule_conditions
         WHERE match_rule_id = ?1 ORDER BY position, id",
    )?;
    let rows = stmt.query_map(params![rule_id], |row| {
        let field_str: String = row.get(0)?;
        let op_str: String = row.get(1)?;
        let negate: i64 = row.get(3)?;
        Ok(MatchCondition {
            field: MatchField::from_str(&field_str).unwrap_or(MatchField::WindowTitle),
            operator: MatchOperator::from_str(&op_str).unwrap_or(MatchOperator::Contains),
            value: row.get(2)?,
            negate: negate != 0,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn write_rule_conditions(
    conn: &Connection,
    rule_id: i64,
    conditions: &[MatchCondition],
) -> Result<()> {
    conn.execute(
        "DELETE FROM project_match_rule_conditions WHERE match_rule_id = ?1",
        params![rule_id],
    )?;
    for (i, c) in conditions.iter().enumerate() {
        conn.execute(
            "INSERT INTO project_match_rule_conditions
                (match_rule_id, position, field, operator, value, negate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rule_id,
                i as i64,
                c.field.as_str(),
                c.operator.as_str(),
                c.value,
                if c.negate { 1 } else { 0 },
            ],
        )?;
    }
    Ok(())
}

/// New rules are appended to the end of the global precedence order.
fn next_rule_position(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM project_match_rules",
        [],
        |row| row.get(0),
    )?)
}

pub fn insert_project_match_rule(
    conn: &Connection,
    project_id: i64,
    name: &str,
    conditions: &[MatchCondition],
) -> Result<ProjectMatchRule> {
    let position = next_rule_position(conn)?;
    conn.execute(
        "INSERT INTO project_match_rules (project_id, name, position) VALUES (?1, ?2, ?3)",
        params![project_id, name, position],
    )?;
    let id = conn.last_insert_rowid();
    write_rule_conditions(conn, id, conditions)?;
    Ok(ProjectMatchRule {
        id,
        project_id,
        name: name.to_string(),
        position,
        conditions: conditions.to_vec(),
    })
}

pub fn update_project_match_rule(
    conn: &Connection,
    id: i64,
    name: &str,
    conditions: &[MatchCondition],
) -> Result<()> {
    conn.execute(
        "UPDATE project_match_rules SET name = ?1 WHERE id = ?2",
        params![name, id],
    )?;
    write_rule_conditions(conn, id, conditions)?;
    Ok(())
}

pub fn delete_project_match_rule(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM project_match_rules WHERE id = ?1", params![id])?;
    Ok(())
}

/// Rewrite the global precedence order from an explicit list of rule ids.
pub fn reorder_project_match_rules(conn: &Connection, ordered_ids: &[i64]) -> Result<()> {
    for (i, id) in ordered_ids.iter().enumerate() {
        conn.execute(
            "UPDATE project_match_rules SET position = ?1 WHERE id = ?2",
            params![i as i64, id],
        )?;
    }
    Ok(())
}

/// Most recent non-empty executable path recorded for an app, used to look up
/// its icon. Returns `None` for apps only seen before `exe_path` existed.
pub fn get_exe_path_for_app(conn: &Connection, app_name: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT exe_path FROM activity_raw
         WHERE app_name = ?1 AND exe_path <> ''
         ORDER BY started_at DESC LIMIT 1",
    )?;
    let mut rows = stmt.query(params![app_name])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get(0)?)),
        None => Ok(None),
    }
}

/// Distinct local dates with recorded activity within the last `days` days,
/// newest first. Used to bound the rule-stats sweep.
pub fn get_active_dates(conn: &Connection, days: i64) -> Result<Vec<String>> {
    let since = local_day_start_utc(-(days.max(1) - 1));
    let mut stmt = conn.prepare(
        "SELECT DISTINCT date(started_at, 'localtime') AS d
         FROM activity_raw
         WHERE started_at >= ?1
         ORDER BY d DESC",
    )?;
    let rows = stmt.query_map(params![since], |row| row.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Applications seen in recorded activity, for the search-as-you-type value
/// picker. `sample_titles` are the most time-consuming real window titles, so
/// users pick values that actually exist instead of guessing them.
pub fn get_known_apps(conn: &Connection, days: i64, max_titles: usize) -> Result<Vec<KnownApp>> {
    let since = local_day_start_utc(-(days.max(1) - 1));

    let mut app_stmt = conn.prepare(
        "SELECT app_name,
                CAST(SUM(strftime('%s', ended_at) - strftime('%s', started_at)) AS INTEGER) AS secs,
                MAX(date(started_at, 'localtime')) AS last_seen,
                MAX(exe_path) AS exe
         FROM activity_raw
         WHERE started_at >= ?1
         GROUP BY app_name
         ORDER BY secs DESC",
    )?;
    let apps = app_stmt.query_map(params![since], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1).unwrap_or(0),
            row.get::<_, String>(2).unwrap_or_default(),
            row.get::<_, String>(3).unwrap_or_default(),
        ))
    })?;

    let mut title_stmt = conn.prepare(
        "SELECT window_title
         FROM activity_raw
         WHERE started_at >= ?1 AND app_name = ?2 AND window_title <> ''
         GROUP BY window_title
         ORDER BY SUM(strftime('%s', ended_at) - strftime('%s', started_at)) DESC
         LIMIT ?3",
    )?;

    let mut out: Vec<KnownApp> = Vec::new();
    for app in apps {
        let (app_name, total_secs, last_seen, exe_path) = app?;
        let sample_titles: Vec<String> = title_stmt
            .query_map(params![since, &app_name, max_titles as i64], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        out.push(KnownApp {
            app_name,
            total_secs,
            last_seen,
            exe_path,
            sample_titles,
        });
    }
    Ok(out)
}

// ── Search ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum TermMatch {
    Any(String),
    App(String),
    Title(String),
}

#[derive(Debug, Default)]
struct ParsedQuery {
    include_terms: Vec<TermMatch>,
    exclude_terms: Vec<TermMatch>,
    date_after: Option<String>,
    date_before: Option<String>,
}

fn parse_search_query(query: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    for token in query.split_whitespace() {
        let lower = token.to_lowercase();
        if let Some(rest) = lower.strip_prefix("-app:") {
            if !rest.is_empty() {
                parsed.exclude_terms.push(TermMatch::App(rest.to_string()));
            }
        } else if let Some(rest) = lower.strip_prefix("-title:") {
            if !rest.is_empty() {
                parsed.exclude_terms.push(TermMatch::Title(rest.to_string()));
            }
        } else if let Some(rest) = lower.strip_prefix('-') {
            if !rest.is_empty() {
                parsed.exclude_terms.push(TermMatch::Any(rest.to_string()));
            }
        } else if let Some(rest) = lower.strip_prefix("app:") {
            if !rest.is_empty() {
                parsed.include_terms.push(TermMatch::App(rest.to_string()));
            }
        } else if let Some(rest) = lower.strip_prefix("title:") {
            if !rest.is_empty() {
                parsed.include_terms.push(TermMatch::Title(rest.to_string()));
            }
        } else if let Some(rest) = lower.strip_prefix("date:") {
            if !rest.is_empty() {
                parsed.date_after = Some(rest.to_string());
                parsed.date_before = Some(rest.to_string());
            }
        } else if let Some(rest) = lower.strip_prefix("after:") {
            if !rest.is_empty() {
                parsed.date_after = Some(rest.to_string());
            }
        } else if let Some(rest) = lower.strip_prefix("before:") {
            if !rest.is_empty() {
                parsed.date_before = Some(rest.to_string());
            }
        } else if !lower.is_empty() {
            parsed.include_terms.push(TermMatch::Any(lower));
        }
    }
    parsed
}

/// Build the WHERE clause and parameter list for activity_raw date queries.
fn build_activity_where(parsed: &ParsedQuery) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut sql_params: Vec<String> = Vec::new();

    for term in &parsed.include_terms {
        match term {
            TermMatch::Any(t) => {
                conditions.push(
                    "(LOWER(window_title) LIKE ? OR LOWER(app_name) LIKE ?)".to_string(),
                );
                let pat = format!("%{}%", t);
                sql_params.push(pat.clone());
                sql_params.push(pat);
            }
            TermMatch::App(t) => {
                conditions.push("LOWER(app_name) LIKE ?".to_string());
                sql_params.push(format!("%{}%", t));
            }
            TermMatch::Title(t) => {
                conditions.push("LOWER(window_title) LIKE ?".to_string());
                sql_params.push(format!("%{}%", t));
            }
        }
    }
    for term in &parsed.exclude_terms {
        match term {
            TermMatch::Any(t) => {
                conditions.push(
                    "NOT (LOWER(window_title) LIKE ? OR LOWER(app_name) LIKE ?)".to_string(),
                );
                let pat = format!("%{}%", t);
                sql_params.push(pat.clone());
                sql_params.push(pat);
            }
            TermMatch::App(t) => {
                conditions.push("NOT LOWER(app_name) LIKE ?".to_string());
                sql_params.push(format!("%{}%", t));
            }
            TermMatch::Title(t) => {
                conditions.push("NOT LOWER(window_title) LIKE ?".to_string());
                sql_params.push(format!("%{}%", t));
            }
        }
    }
    if let Some(after) = &parsed.date_after {
        conditions.push("date(started_at, 'localtime') >= ?".to_string());
        sql_params.push(after.clone());
    }
    if let Some(before) = &parsed.date_before {
        conditions.push("date(started_at, 'localtime') <= ?".to_string());
        sql_params.push(before.clone());
    }

    let where_clause = if conditions.is_empty() {
        "1=0".to_string()
    } else {
        conditions.join(" AND ")
    };
    (where_clause, sql_params)
}

/// Returns true if the block satisfies all include/exclude term constraints.
///
/// Title terms are tested against *every* title seen inside the block, not just
/// the representative one. Blocks are merged by window, so a long browsing or
/// editor session collapses into a single block whose last title would
/// otherwise hide everything that came before it — and the candidate dates were
/// chosen from the raw rows, so filtering on one title silently drops days.
fn block_matches(block: &ActivityBlock, parsed: &ParsedQuery) -> bool {
    let app = block.app_name.to_lowercase();
    let mut titles: Vec<String> = block.window_titles.iter().map(|t| t.to_lowercase()).collect();

    if !titles.contains(&block.window_title.to_lowercase()) {
        titles.push(block.window_title.to_lowercase());
    }

    let any_title = |t: &str| titles.iter().any(|title| title.contains(t));

    for term in &parsed.include_terms {
        let hit = match term {
            TermMatch::Any(t) => app.contains(t.as_str()) || any_title(t),
            TermMatch::App(t) => app.contains(t.as_str()),
            TermMatch::Title(t) => any_title(t),
        };
        if !hit {
            return false;
        }
    }
    for term in &parsed.exclude_terms {
        let hit = match term {
            TermMatch::Any(t) => app.contains(t.as_str()) || any_title(t),
            TermMatch::App(t) => app.contains(t.as_str()),
            TermMatch::Title(t) => any_title(t),
        };
        if hit {
            return false;
        }
    }
    true
}

pub fn search(
    conn: &Connection,
    query: &str,
    settings: &Settings,
    rules: &[FilterRule],
) -> Result<SearchResults> {
    if query.trim().is_empty() {
        return Ok(SearchResults { days: vec![], note_matches: vec![] });
    }

    let parsed = parse_search_query(query);

    if parsed.include_terms.is_empty()
        && parsed.exclude_terms.is_empty()
        && parsed.date_after.is_none()
        && parsed.date_before.is_none()
    {
        return Ok(SearchResults { days: vec![], note_matches: vec![] });
    }

    let (where_clause, date_params) = build_activity_where(&parsed);
    let date_sql = format!(
        "SELECT DISTINCT date(started_at, 'localtime') as d
         FROM activity_raw
         WHERE {}
         ORDER BY d DESC
         LIMIT 60",
        where_clause
    );
    let mut date_stmt = conn.prepare(&date_sql)?;
    let dates: Vec<String> = date_stmt
        .query_map(rusqlite::params_from_iter(date_params.iter()), |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut days = Vec::new();
    for date in &dates {
        let all_blocks = get_activity_for_date(conn, date, settings, rules)?;
        let matched_blocks: Vec<ActivityBlock> =
            all_blocks.iter().filter(|b| block_matches(b, &parsed)).cloned().collect();
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

    // Note search: apply only Any-type terms + date range.
    // Skip entirely when all qualifiers are field-scoped (app:/title:) with no Any terms.
    let any_includes: Vec<&str> = parsed
        .include_terms
        .iter()
        .filter_map(|t| if let TermMatch::Any(s) = t { Some(s.as_str()) } else { None })
        .collect();
    let any_excludes: Vec<&str> = parsed
        .exclude_terms
        .iter()
        .filter_map(|t| if let TermMatch::Any(s) = t { Some(s.as_str()) } else { None })
        .collect();

    let search_notes =
        !any_includes.is_empty() || parsed.date_after.is_some() || parsed.date_before.is_some();

    let note_matches = if !search_notes {
        vec![]
    } else {
        let mut note_conditions: Vec<String> = Vec::new();
        let mut note_params: Vec<String> = Vec::new();
        for t in &any_includes {
            note_conditions.push("LOWER(note) LIKE ?".to_string());
            note_params.push(format!("%{}%", t));
        }
        for t in &any_excludes {
            note_conditions.push("NOT LOWER(note) LIKE ?".to_string());
            note_params.push(format!("%{}%", t));
        }
        if let Some(after) = &parsed.date_after {
            note_conditions.push("date >= ?".to_string());
            note_params.push(after.clone());
        }
        if let Some(before) = &parsed.date_before {
            note_conditions.push("date <= ?".to_string());
            note_params.push(before.clone());
        }
        let note_sql = format!(
            "SELECT id, date, project_id, start_minutes, end_minutes, note
             FROM time_entries
             WHERE {}
             ORDER BY date DESC, start_minutes",
            note_conditions.join(" AND ")
        );
        let mut note_stmt = conn.prepare(&note_sql)?;
        let collected: Vec<TimeEntry> = note_stmt
            .query_map(rusqlite::params_from_iter(note_params.iter()), |row| {
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
        collected
    };

    Ok(SearchResults { days, note_matches })
}

// ── Delete activity ─────────────────────────────────────────────────────────

/// Delete all raw activity rows that match the given time range, app name, and
/// window title. Used when the user right-clicks an ActivityBlock in search
/// results and chooses Delete.
pub fn delete_activity_block(
    conn: &Connection,
    started_at: &str,
    ended_at: &str,
    app_name: &str,
    window_title: &str,
) -> Result<usize> {
    let count = conn.execute(
        "DELETE FROM activity_raw
         WHERE started_at = ?1 AND ended_at = ?2 AND app_name = ?3 AND window_title = ?4",
        params![started_at, ended_at, app_name, window_title],
    )?;
    Ok(count)
}

/// Delete all raw activity rows for a given app_name and window_title across
/// all dates. Used when the user right-clicks a window-totals sidebar item in
/// search results and chooses Delete.
pub fn delete_activity_by_app_title(
    conn: &Connection,
    app_name: &str,
    window_title: &str,
) -> Result<usize> {
    let count = conn.execute(
        "DELETE FROM activity_raw WHERE app_name = ?1 AND window_title = ?2",
        params![app_name, window_title],
    )?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MatchField, MatchOperator};

    /// A merged block whose representative title is the *last* one seen.
    fn merged_block(app_name: &str, titles: &[&str]) -> ActivityBlock {
        let start = DateTime::parse_from_rfc3339("2026-09-23T09:00:00+00:00")
            .unwrap()
            .with_timezone(&Utc);
        let end = DateTime::parse_from_rfc3339("2026-09-23T10:00:00+00:00")
            .unwrap()
            .with_timezone(&Utc);
        ActivityBlock {
            app_name: app_name.to_string(),
            window_title: titles.last().unwrap().to_string(),
            window_titles: titles.iter().map(|t| t.to_string()).collect(),
            started_at: start,
            ended_at: end,
            duration_secs: 3600,
            window_id: 1,
        }
    }

    #[test]
    fn search_matches_any_title_inside_a_merged_block() {
        let block = merged_block("brave", &["[ELMS-5815] Waitlist - Jira", "New tab"]);
        // "jira" only appears in the earlier title, not the representative one.
        assert!(block_matches(&block, &parse_search_query("jira")));
        assert!(block_matches(&block, &parse_search_query("title:waitlist")));
        assert!(!block_matches(&block, &parse_search_query("console")));
    }

    #[test]
    fn search_exclusion_applies_to_any_title_inside_a_merged_block() {
        let block = merged_block("brave", &["[ELMS-5815] Waitlist - Jira", "New tab"]);
        assert!(!block_matches(&block, &parse_search_query("-jira")));
        assert!(!block_matches(&block, &parse_search_query("-title:waitlist")));
        // The app-name match still holds, but the excluded title wins.
        assert!(!block_matches(&block, &parse_search_query("app:brave -jira")));
        assert!(block_matches(&block, &parse_search_query("app:brave title:new")));
    }

    /// Build a database with the *legacy* (pre-v1) schema, mirroring what
    /// shipped before the rule-group rework.
    fn legacy_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE projects (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                color       TEXT NOT NULL DEFAULT '#6366f1',
                archived_at TEXT,
                parent_id   INTEGER REFERENCES projects(id) ON DELETE SET NULL
            );
            CREATE TABLE project_match_rules (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                rule_type  TEXT NOT NULL CHECK(rule_type IN ('title_pattern', 'app_name')),
                value      TEXT NOT NULL
            );
            INSERT INTO projects (id, name, color) VALUES (7, 'Proto', '#6366f1'), (8, 'Buyflow', '#22c55e');
            INSERT INTO project_match_rules (project_id, rule_type, value) VALUES
                (8, 'title_pattern', 'sdg-buyflow'),
                (7, 'title_pattern', 'sdg-connect'),
                (7, 'app_name', 'Code');
            ",
        )
        .unwrap();
        conn
    }

    #[test]
    fn legacy_flat_rules_are_replaced_not_migrated() {
        let conn = legacy_db();
        run_migrations(&conn).unwrap();

        // No backwards compatibility: the flat table is dropped and recreated.
        assert!(!table_has_column(&conn, "project_match_rules", "rule_type").unwrap());
        assert!(get_project_match_rules(&conn).unwrap().is_empty());
        assert!(!table_exists(&conn, "project_match_rules_legacy"));
        assert!(!table_exists(&conn, "schema_version"));
        assert_eq!(current_schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    /// The state left behind by the original v1 step: the conditions table's
    /// foreign key points at a table that was renamed away and then dropped.
    /// Reads worked, every write to the child table failed.
    fn dangling_fk_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=OFF;
             CREATE TABLE projects (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 name        TEXT NOT NULL,
                 color       TEXT NOT NULL DEFAULT '#6366f1',
                 archived_at TEXT,
                 parent_id   INTEGER REFERENCES projects(id) ON DELETE SET NULL
             );
             CREATE TABLE project_match_rules (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 name       TEXT NOT NULL DEFAULT '',
                 position   INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE project_match_rule_conditions (
                 id            INTEGER PRIMARY KEY AUTOINCREMENT,
                 match_rule_id INTEGER NOT NULL REFERENCES project_match_rules_legacy(id) ON DELETE CASCADE,
                 position      INTEGER NOT NULL DEFAULT 0,
                 field         TEXT NOT NULL CHECK(field IN ('app_name', 'window_title')),
                 operator      TEXT NOT NULL CHECK(operator IN ('contains', 'equals', 'starts_with', 'ends_with')),
                 value         TEXT NOT NULL,
                 negate        INTEGER NOT NULL DEFAULT 0
             );
             INSERT INTO projects (id, name, color) VALUES (7, 'Proto', '#6366f1');
             INSERT INTO project_match_rules (id, project_id, name, position)
                 VALUES (4, 7, 'sdg-connect', 4);
             INSERT INTO project_match_rule_conditions
                 (id, match_rule_id, position, field, operator, value, negate)
                 VALUES (1, 4, 0, 'window_title', 'contains', 'sdg-connect', 0);
             PRAGMA user_version = 1;",
        )
        .unwrap();
        conn
    }

    #[test]
    fn a_dangling_foreign_key_is_repaired_and_writes_work_again() {
        let conn = dangling_fk_db();
        conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();

        // Confirm the broken state first — this is the exact failure the user hit.
        let failure = conn
            .execute("DELETE FROM project_match_rule_conditions WHERE match_rule_id = 4", [])
            .unwrap_err();
        assert!(
            failure.to_string().contains("project_match_rules_legacy"),
            "expected the dangling key to be the cause, got: {failure}"
        );

        run_migrations(&conn).unwrap();

        let fk_errors: Vec<String> = conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(fk_errors.is_empty(), "foreign_key_check reported {fk_errors:?}");

        // Existing conditions survive the rebuild.
        let rules = get_project_match_rules(&conn).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "sdg-connect");
        assert_eq!(rules[0].conditions.len(), 1);
        assert_eq!(rules[0].conditions[0].value, "sdg-connect");

        // The write path that used to fail now works end to end.
        update_project_match_rule(
            &conn,
            4,
            "renamed",
            &[MatchCondition {
                field: MatchField::WindowTitle,
                operator: MatchOperator::Contains,
                value: "soa-buyflow".to_string(),
                negate: false,
            }],
        )
        .unwrap();

        let reloaded = get_project_match_rules(&conn).unwrap();
        assert_eq!(reloaded[0].name, "renamed");
        assert_eq!(reloaded[0].conditions.len(), 1);
        assert_eq!(reloaded[0].conditions[0].value, "soa-buyflow");
        assert_eq!(current_schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn the_repair_is_idempotent() {
        let conn = dangling_fk_db();
        run_migrations(&conn).unwrap();
        let first = get_project_match_rules(&conn).unwrap();

        run_migrations(&conn).unwrap();
        let second = get_project_match_rules(&conn).unwrap();

        assert_eq!(first.len(), second.len());
        assert_eq!(first[0].conditions.len(), second[0].conditions.len());
        assert_eq!(first[0].conditions[0].value, second[0].conditions[0].value);
        assert!(!table_exists(&conn, "project_match_rule_conditions_stale"));
    }

    #[test]
    fn migrations_are_idempotent_and_versioned() {
        let conn = legacy_db();
        run_migrations(&conn).unwrap();
        assert_eq!(current_schema_version(&conn).unwrap(), SCHEMA_VERSION);

        // The legacy table and the dead version table are both gone.
        assert!(!table_exists(&conn, "project_match_rules_legacy"));
        assert!(!table_exists(&conn, "schema_version"));

        // A rule written after the migration must survive a second startup.
        let project = insert_project(&conn, "Proto", "#6366f1", None).unwrap();
        let created = insert_project_match_rule(
            &conn,
            project.id,
            "Proto work",
            &[MatchCondition {
                field: MatchField::WindowTitle,
                operator: MatchOperator::Contains,
                value: "sdg-connect".to_string(),
                negate: false,
            }],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        let after = get_project_match_rules(&conn).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, created.id);
        assert_eq!(after[0].name, "Proto work");
        assert_eq!(after[0].conditions.len(), 1);
        assert_eq!(after[0].conditions[0].value, "sdg-connect");
        assert_eq!(current_schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn fresh_database_gets_the_current_shape_directly() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(current_schema_version(&conn).unwrap(), SCHEMA_VERSION);
        assert!(!table_has_column(&conn, "project_match_rules", "rule_type").unwrap());
        assert!(table_has_column(&conn, "activity_raw", "exe_path").unwrap());
        assert!(get_project_match_rules(&conn).unwrap().is_empty());
    }

    #[test]
    fn rule_conditions_round_trip_in_order() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let project = insert_project(&conn, "Buyflow", "#22c55e", None).unwrap();

        let conditions = vec![
            MatchCondition {
                field: MatchField::WindowTitle,
                operator: MatchOperator::Contains,
                value: "buyflow".to_string(),
                negate: false,
            },
            MatchCondition {
                field: MatchField::WindowTitle,
                operator: MatchOperator::Contains,
                value: "lottery".to_string(),
                negate: true,
            },
        ];
        let created =
            insert_project_match_rule(&conn, project.id, "Buyflow work", &conditions).unwrap();
        assert_eq!(created.conditions.len(), 2);

        let loaded = get_project_match_rules(&conn).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Buyflow work");
        assert_eq!(loaded[0].conditions[0].value, "buyflow");
        assert!(!loaded[0].conditions[0].negate);
        assert_eq!(loaded[0].conditions[1].value, "lottery");
        assert!(loaded[0].conditions[1].negate);

        // Updating replaces the condition set wholesale.
        update_project_match_rule(
            &conn,
            created.id,
            "renamed",
            &[MatchCondition {
                field: MatchField::AppName,
                operator: MatchOperator::Equals,
                value: "brave".to_string(),
                negate: false,
            }],
        )
        .unwrap();
        let reloaded = get_project_match_rules(&conn).unwrap();
        assert_eq!(reloaded[0].name, "renamed");
        assert_eq!(reloaded[0].conditions.len(), 1);
        assert_eq!(reloaded[0].conditions[0].field, MatchField::AppName);
    }

    #[test]
    fn deleting_a_project_cascades_to_conditions() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let project = insert_project(&conn, "Temp", "#6366f1", None).unwrap();
        let rule = insert_project_match_rule(
            &conn,
            project.id,
            "temp",
            &[MatchCondition {
                field: MatchField::AppName,
                operator: MatchOperator::Equals,
                value: "olk".to_string(),
                negate: false,
            }],
        )
        .unwrap();

        delete_project(&conn, project.id).unwrap();
        assert!(get_project_match_rules(&conn).unwrap().is_empty());

        let orphans: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_match_rule_conditions WHERE match_rule_id = ?1",
                params![rule.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
    }

    #[test]
    fn reorder_rewrites_global_precedence() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let a = insert_project(&conn, "A", "#6366f1", None).unwrap();
        let b = insert_project(&conn, "B", "#22c55e", None).unwrap();
        let cond = || {
            vec![MatchCondition {
                field: MatchField::AppName,
                operator: MatchOperator::Equals,
                value: "brave".to_string(),
                negate: false,
            }]
        };
        let r1 = insert_project_match_rule(&conn, a.id, "a", &cond()).unwrap();
        let r2 = insert_project_match_rule(&conn, b.id, "b", &cond()).unwrap();

        assert_eq!(get_project_match_rules(&conn).unwrap()[0].id, r1.id);

        reorder_project_match_rules(&conn, &[r2.id, r1.id]).unwrap();
        let after = get_project_match_rules(&conn).unwrap();
        assert_eq!(after[0].id, r2.id);
        assert_eq!(after[1].id, r1.id);
    }

    fn table_exists(conn: &Connection, table: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    }
}
