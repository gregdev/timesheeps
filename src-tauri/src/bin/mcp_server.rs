//! Timesheeps MCP server — standalone binary speaking the Model Context Protocol
//! over stdio. Claude Desktop launches this directly; no Node.js required.
//!
//! DB path: %APPDATA%\app.timesheeps.Timesheeps\timesheeps.db
//! Override: TIMESHEEPS_DB environment variable.
//!
//! This binary is a thin protocol shim. All the interesting work lives in
//! `timesheeps_lib`, so the numbers it reports are produced by exactly the same
//! pipeline the app uses (ignore rules → merge by window → drop sub-
//! `min_duration_secs` blocks → rule matching). An earlier version reimplemented
//! the aggregation in SQL, which counted noise the app never shows and disagreed
//! with the timeline.
//!
//! Writes are allowed only while the `mcp_allow_writes` setting is on. It is
//! re-read on every call so flipping it in the app takes effect immediately, and
//! there is deliberately no tool that can change it.

use std::io::{BufRead, BufReader, Write};

use rusqlite::Connection;
use serde_json::{json, Value};

use timesheeps_lib::db;
use timesheeps_lib::matcher;
use timesheeps_lib::models::{
    GroupBy, MatchCondition, MatchField, MatchOperator, Project, ProjectMatchRule,
};
use timesheeps_lib::reporting;

// ── Database ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn platform_db_path() -> std::path::PathBuf {
    let base = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let mut p = std::env::var("USERPROFILE")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            p.push("AppData");
            p.push("Roaming");
            p
        });
    base.join("app.timesheeps.Timesheeps").join("timesheeps.db")
}

#[cfg(target_os = "macos")]
fn platform_db_path() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(|home| {
            std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("app.timesheeps.Timesheeps")
                .join("timesheeps.db")
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("timesheeps.db"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn platform_db_path() -> std::path::PathBuf {
    std::path::PathBuf::from("timesheeps.db")
}

fn db_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("TIMESHEEPS_DB") {
        return std::path::PathBuf::from(p);
    }
    platform_db_path()
}

/// Open the database, refusing to operate on a schema this build predates.
///
/// This process never migrates. Two processes racing the migration and
/// legacy-repair path is worse than an explicit "open the app once" message, and
/// an old MCP binary must not be able to run DDL against a newer database.
fn open_db() -> Result<Connection, String> {
    let path = db_path();
    if !path.exists() {
        return Err(format!(
            "Timesheeps database not found at {}. Launch Timesheeps once so it can create it.",
            path.display()
        ));
    }

    let conn = db::open_at(&path).map_err(|e| e.to_string())?;
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);
    let expected = db::schema_version();
    if current < expected {
        return Err(format!(
            "The Timesheeps database is at schema version {current} but this MCP server expects \
             {expected}. Launch Timesheeps once to migrate it, then try again."
        ));
    }
    Ok(conn)
}

/// Whether write tools may run. Defaults to on, matching `Settings::default`.
fn writes_allowed(conn: &Connection) -> bool {
    db::get_setting_str(conn, "mcp_allow_writes", "1")
        .map(|v| v != "0")
        .unwrap_or(true)
}

// ── Formatting helpers ────────────────────────────────────────────────────────

fn today_local() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn fmt_dur(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        return format!("{secs}s");
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

fn mins_to_hhmm(m: i64) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

fn is_valid_date(date: &str) -> bool {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok()
}

// ── Argument helpers ──────────────────────────────────────────────────────────

fn arg_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn arg_i64(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn req_str(args: &Value, key: &str) -> Result<String, String> {
    arg_str(args, key).ok_or_else(|| format!("`{key}` is required"))
}

fn req_i64(args: &Value, key: &str) -> Result<i64, String> {
    arg_i64(args, key).ok_or_else(|| format!("`{key}` is required and must be an integer"))
}

/// The date range for a range tool. Both bounds are required: silently
/// defaulting one end would make the answer depend on the caller's guess.
fn req_range(args: &Value) -> Result<(String, String), String> {
    Ok((req_str(args, "start_date")?, req_str(args, "end_date")?))
}

// ── Validation ────────────────────────────────────────────────────────────────

const MATCH_FIELDS: [&str; 2] = ["app_name", "window_title"];
const MATCH_OPERATORS: [&str; 4] = ["contains", "equals", "starts_with", "ends_with"];

fn validate_color(color: &str) -> Result<(), String> {
    let hex = color.strip_prefix('#').unwrap_or("");
    let ok = matches!(hex.len(), 3 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    if ok {
        Ok(())
    } else {
        Err(format!(
            "`color` must be a hex colour such as \"#6366f1\", got {color:?}"
        ))
    }
}

/// Reject a pattern that does not compile, carrying the regex engine's message.
fn validate_pattern(pattern: Option<&str>) -> Result<(), String> {
    match pattern.map(str::trim).filter(|p| !p.is_empty()) {
        None => Ok(()),
        Some(p) => matcher::SubGroupPattern::compile(p)
            .map(|_| ())
            .map_err(|e| format!("`sub_group_pattern` is not a valid regex: {e}")),
    }
}

fn parse_condition(raw: &Value) -> Result<MatchCondition, String> {
    let field_str = raw
        .get("field")
        .and_then(|v| v.as_str())
        .ok_or("each condition needs a `field`")?;
    let field = MatchField::from_str(field_str).ok_or_else(|| {
        format!("condition `field` must be one of {MATCH_FIELDS:?}, got {field_str:?}")
    })?;

    let op_str = raw
        .get("operator")
        .and_then(|v| v.as_str())
        .ok_or("each condition needs an `operator`")?;
    let operator = MatchOperator::from_str(op_str).ok_or_else(|| {
        format!("condition `operator` must be one of {MATCH_OPERATORS:?}, got {op_str:?}")
    })?;

    let value = raw
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if value.is_empty() {
        return Err(
            "condition `value` must not be blank — a blank value is dropped at match time, so the \
             rule would silently never match"
                .to_string(),
        );
    }

    Ok(MatchCondition {
        field,
        operator,
        value,
        negate: raw.get("negate").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

fn parse_conditions(raw: &Value) -> Result<Vec<MatchCondition>, String> {
    let list = raw
        .as_array()
        .ok_or("`conditions` must be an array of {field, operator, value, negate}")?;
    if list.is_empty() {
        return Err("a rule needs at least one condition".to_string());
    }
    list.iter().map(parse_condition).collect()
}

/// Resolve a project by id or by (case-insensitive) name.
fn resolve_project_id(conn: &Connection, raw: &Value) -> Result<i64, String> {
    if let Some(id) = raw.get("project_id").and_then(|v| v.as_i64()) {
        return Ok(id);
    }
    if let Some(name) = raw.get("project_name").and_then(|v| v.as_str()) {
        let name = name.trim();
        let projects = db::get_projects(conn).map_err(|e| e.to_string())?;
        let hits: Vec<_> = projects
            .iter()
            .filter(|p| p.name.eq_ignore_ascii_case(name))
            .collect();
        return match hits.len() {
            1 => Ok(hits[0].id),
            0 => Err(format!(
                "no project is named {name:?}. Call `get_projects` to see them."
            )),
            n => Err(format!(
                "{n} projects are named {name:?}; pass `project_id` instead"
            )),
        };
    }
    Err("either `project_id` or `project_name` is required".to_string())
}

fn project_by_id(conn: &Connection, id: i64) -> Result<Project, String> {
    db::get_project(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no project with id {id}"))
}

fn parse_hhmm(raw: &str) -> Option<i64> {
    let (h, m) = raw.trim().split_once(':')?;
    let h: i64 = h.trim().parse().ok()?;
    let m: i64 = m.trim().parse().ok()?;
    if !(0..=23).contains(&h) || !(0..=59).contains(&m) {
        return None;
    }
    Some(h * 60 + m)
}

/// Minutes-from-midnight for one end of a proposed entry. Accepts either
/// `*_minutes` (0–1440) or `HH:MM`.
fn parse_bound(raw: &Value, minutes_key: &str, clock_key: &str) -> Result<i64, String> {
    if let Some(v) = raw.get(minutes_key).and_then(|v| v.as_i64()) {
        if !(0..=1440).contains(&v) {
            return Err(format!("`{minutes_key}` must be between 0 and 1440, got {v}"));
        }
        return Ok(v);
    }
    if let Some(s) = raw.get(clock_key).and_then(|v| v.as_str()) {
        return parse_hhmm(s).ok_or_else(|| {
            format!("`{clock_key}` must be a 24-hour time such as \"09:30\", got {s:?}")
        });
    }
    Err(format!("`{minutes_key}` or `{clock_key}` is required"))
}

/// An existing entry on `date` that would overlap `[start, end)`.
fn find_overlap(
    conn: &Connection,
    date: &str,
    start: i64,
    end: i64,
    ignore_id: Option<i64>,
) -> Result<Option<String>, String> {
    for entry in db::get_time_entries_for_date(conn, date).map_err(|e| e.to_string())? {
        if Some(entry.id) == ignore_id {
            continue;
        }
        if start < entry.end_minutes && entry.start_minutes < end {
            return Ok(Some(format!(
                "entry #{} ({}–{})",
                entry.id,
                mins_to_hhmm(entry.start_minutes),
                mins_to_hhmm(entry.end_minutes)
            )));
        }
    }
    Ok(None)
}

// ── Read tools ────────────────────────────────────────────────────────────────

/// Merged activity for one day, rolled up per app.
///
/// Built on the app's pipeline rather than raw rows, so sub-minute flapping and
/// legacy negative durations cannot leak into the totals.
fn activity_by_app(conn: &Connection, date: &str) -> Result<Value, String> {
    let settings = db::get_settings(conn).map_err(|e| e.to_string())?;
    let rules = db::get_filter_rules(conn).map_err(|e| e.to_string())?;
    let blocks =
        db::get_activity_for_date(conn, date, &settings, &rules).map_err(|e| e.to_string())?;

    let mut by_app: std::collections::HashMap<String, (i64, Vec<Value>)> =
        std::collections::HashMap::new();
    for block in blocks {
        let entry = by_app
            .entry(block.app_name.clone())
            .or_insert((0, Vec::new()));
        entry.0 += block.duration_secs;
        entry.1.push(json!({
            "title": block.window_title,
            "total_secs": block.duration_secs,
            "duration": fmt_dur(block.duration_secs),
            "started_at": block.started_at.to_rfc3339(),
            "ended_at": block.ended_at.to_rfc3339(),
        }));
    }

    let mut apps: Vec<Value> = by_app
        .into_iter()
        .map(|(app_name, (total_secs, windows))| {
            json!({
                "app_name": app_name,
                "total_secs": total_secs,
                "duration": fmt_dur(total_secs),
                "windows": windows,
            })
        })
        .collect();
    apps.sort_by(|a, b| {
        b["total_secs"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["total_secs"].as_i64().unwrap_or(0))
    });

    Ok(json!({ "date": date, "activity_by_app": apps }))
}

fn time_entries_for(conn: &Connection, date: &str) -> Result<Value, String> {
    let entries = db::get_time_entries_for_date(conn, date).map_err(|e| e.to_string())?;
    let projects = db::get_projects(conn).map_err(|e| e.to_string())?;
    let name_of: std::collections::HashMap<i64, (String, String)> = projects
        .into_iter()
        .map(|p| (p.id, (p.name, p.color)))
        .collect();

    let out: Vec<Value> = entries
        .into_iter()
        .map(|e| {
            let (project, color) = name_of
                .get(&e.project_id)
                .cloned()
                .unwrap_or_else(|| (format!("project #{}", e.project_id), String::new()));
            let mins = (e.end_minutes - e.start_minutes).max(0);
            json!({
                "id": e.id,
                "date": e.date,
                "project_id": e.project_id,
                "project": project,
                "color": color,
                "start": mins_to_hhmm(e.start_minutes),
                "end": mins_to_hhmm(e.end_minutes),
                "duration_mins": mins,
                "duration_secs": mins * 60,
                "duration": fmt_dur(mins * 60),
                "note": e.note,
            })
        })
        .collect();

    Ok(json!(out))
}

fn get_day_summary(conn: &Connection, date: &str) -> Result<Value, String> {
    if !is_valid_date(date) {
        return Err(format!("`date` must be YYYY-MM-DD, got {date:?}"));
    }
    let activity = activity_by_app(conn, date)?;
    let totals =
        reporting::classify_range(conn, date, date, GroupBy::Project).map_err(|e| e.to_string())?;
    let day = totals.per_day.get(date).cloned().unwrap_or_default();

    Ok(json!({
        "date": date,
        "total_tracked_secs": day.tracked_secs,
        "total_tracked": fmt_dur(day.tracked_secs),
        "total_logged_secs": day.logged_secs,
        "total_logged": fmt_dur(day.logged_secs),
        "unclassified_secs": day.unclassified_secs,
        "unclassified": fmt_dur(day.unclassified_secs),
        "by_project": totals.buckets,
        "activity_by_app": activity["activity_by_app"],
        "time_entries": time_entries_for(conn, date)?,
    }))
}

fn get_projects(conn: &Connection) -> Result<Value, String> {
    let projects = db::get_projects(conn).map_err(|e| e.to_string())?;
    let active: Vec<Value> = projects
        .iter()
        .filter(|p| p.archived_at.is_none())
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "color": p.color,
                "parent_id": p.parent_id,
                "sub_group_pattern": p.sub_group_pattern,
            })
        })
        .collect();
    let archived: Vec<Value> = projects
        .iter()
        .filter(|p| p.archived_at.is_some())
        .map(|p| json!({ "id": p.id, "name": p.name }))
        .collect();

    Ok(json!({ "projects": active, "archived_projects": archived }))
}

fn get_classified_totals(conn: &Connection, args: &Value) -> Result<Value, String> {
    let (start, end) = req_range(args)?;
    let group_by = match arg_str(args, "group_by") {
        None => GroupBy::Project,
        Some(raw) => GroupBy::from_str(&raw).ok_or_else(|| {
            format!("`group_by` must be one of [\"project\", \"ticket\", \"app\"], got {raw:?}")
        })?,
    };

    let totals =
        reporting::classify_range(conn, &start, &end, group_by).map_err(|e| e.to_string())?;

    let mut value = serde_json::to_value(&totals).map_err(|e| e.to_string())?;
    if let Some(obj) = value.as_object_mut() {
        // Display-only companions; the `*_secs` fields stay authoritative.
        obj.insert(
            "totalTrackedDisplay".to_string(),
            json!(fmt_dur(totals.total_tracked_secs)),
        );
        obj.insert(
            "totalLoggedDisplay".to_string(),
            json!(fmt_dur(totals.total_logged_secs)),
        );
        obj.insert(
            "unclassifiedDisplay".to_string(),
            json!(fmt_dur(totals.unclassified_secs)),
        );
    }
    Ok(value)
}

fn get_day_summary_range(conn: &Connection, args: &Value) -> Result<Value, String> {
    let (start, end) = req_range(args)?;
    let include_titles = arg_str(args, "include")
        .map(|i| i.eq_ignore_ascii_case("full"))
        .unwrap_or(false);

    // One classification pass for the whole range; the per-day rollup and each
    // bucket's `per_day` map are then sliced by date.
    let totals =
        reporting::classify_range(conn, &start, &end, GroupBy::Project).map_err(|e| e.to_string())?;

    let mut days: Vec<Value> = Vec::new();
    for (date, day) in &totals.per_day {
        let by_project: Vec<Value> = totals
            .buckets
            .iter()
            .filter_map(|b| {
                let secs = *b.per_day.get(date)?;
                (secs > 0).then(|| {
                    json!({
                        "project_id": b.project_id,
                        "project": b.project_name,
                        "tracked_secs": secs,
                        "tracked": fmt_dur(secs),
                    })
                })
            })
            .collect();

        let mut entry = json!({
            "date": date,
            "tracked_secs": day.tracked_secs,
            "tracked": fmt_dur(day.tracked_secs),
            "logged_secs": day.logged_secs,
            "logged": fmt_dur(day.logged_secs),
            "unclassified_secs": day.unclassified_secs,
            "unclassified": fmt_dur(day.unclassified_secs),
            "by_project": by_project,
        });

        if include_titles {
            entry["time_entries"] = time_entries_for(conn, date)?;
            entry["unclassified_titles"] =
                serde_json::to_value(reporting::unclassified_titles(conn, date, date, 0).map_err(
                    |e| e.to_string(),
                )?)
                .map_err(|e| e.to_string())?;
        }

        days.push(entry);
    }

    Ok(json!({
        "start_date": totals.start_date,
        "end_date": totals.end_date,
        "total_tracked_secs": totals.total_tracked_secs,
        "total_tracked": fmt_dur(totals.total_tracked_secs),
        "total_logged_secs": totals.total_logged_secs,
        "total_logged": fmt_dur(totals.total_logged_secs),
        "days": days,
    }))
}

fn get_window_summary_range(conn: &Connection, args: &Value) -> Result<Value, String> {
    let (start, end) = req_range(args)?;
    let items = reporting::window_summary_range(conn, &start, &end, arg_i64(args, "min_secs"))
        .map_err(|e| e.to_string())?;

    let windows: Vec<Value> = items
        .into_iter()
        .map(|i| {
            json!({
                "app_name": i.app_name,
                "window_title": i.window_title,
                "total_secs": i.total_secs,
                "duration": fmt_dur(i.total_secs),
            })
        })
        .collect();
    let total_secs: i64 = windows.iter().filter_map(|v| v["total_secs"].as_i64()).sum();

    Ok(json!({
        "start_date": start,
        "end_date": end,
        "total_secs": total_secs,
        "total": fmt_dur(total_secs),
        "windows": windows,
    }))
}

fn get_unclassified_titles(conn: &Connection, args: &Value) -> Result<Value, String> {
    let (start, end) = req_range(args)?;
    let min_secs = arg_i64(args, "min_secs").unwrap_or(60);
    let groups =
        reporting::unclassified_titles(conn, &start, &end, min_secs).map_err(|e| e.to_string())?;
    let total: i64 = groups.iter().map(|g| g.total_secs).sum();

    Ok(json!({
        "start_date": start,
        "end_date": end,
        "min_secs": min_secs,
        "total_secs": total,
        "total": fmt_dur(total),
        "hint": "These titles matched no rule. Ask the user which project each belongs to, then \
                 call validate_match_rules followed by create_match_rules. Prefer `contains` on a \
                 distinctive fragment over exact titles, which change constantly.",
        "titles": serde_json::to_value(&groups).map_err(|e| e.to_string())?,
    }))
}

fn get_match_rules(conn: &Connection) -> Result<Value, String> {
    let rules = db::get_project_match_rules(conn).map_err(|e| e.to_string())?;
    let projects = db::get_projects(conn).map_err(|e| e.to_string())?;
    let compiled = matcher::compile(&rules);
    let (_, warnings) = matcher::resolve_sub_groups(&rules, &projects);

    let name_of: std::collections::HashMap<i64, String> =
        projects.iter().map(|p| (p.id, p.name.clone())).collect();
    let project_pattern: std::collections::HashMap<i64, Option<String>> = projects
        .iter()
        .map(|p| (p.id, p.sub_group_pattern.clone()))
        .collect();

    let out: Vec<Value> = rules
        .iter()
        .map(|r| {
            let effective = r
                .sub_group_pattern
                .clone()
                .or_else(|| project_pattern.get(&r.project_id).cloned().flatten());
            json!({
                "id": r.id,
                "position": r.position,
                "name": r.name,
                "project_id": r.project_id,
                "project": name_of.get(&r.project_id),
                "sub_group_pattern": r.sub_group_pattern,
                "effective_sub_group_pattern": effective,
                "conditions": r.conditions.iter().map(|c| json!({
                    "field": c.field.as_str(),
                    "operator": c.operator.as_str(),
                    "value": c.value,
                    "negate": c.negate,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    Ok(json!({
        "rules": out,
        "precedence": "Rules are evaluated in `position` order across ALL projects; the first \
                       match wins. Reordering changes classification where rules overlap.",
        "compiled_rule_count": compiled.len(),
        "warnings": warnings,
    }))
}

fn get_known_apps(conn: &Connection, args: &Value) -> Result<Value, String> {
    let days = arg_i64(args, "days").unwrap_or(90).clamp(1, 3650);
    let apps = db::get_known_apps(conn, days, 10).map_err(|e| e.to_string())?;
    Ok(json!({ "days": days, "apps": serde_json::to_value(&apps).map_err(|e| e.to_string())? }))
}

fn get_settings(conn: &Connection) -> Result<Value, String> {
    let settings = db::get_settings(conn).map_err(|e| e.to_string())?;
    let mut value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    value["mcpWritesAllowed"] = json!(writes_allowed(conn));
    Ok(value)
}

// ── Write tools ───────────────────────────────────────────────────────────────

fn create_projects(conn: &Connection, args: &Value) -> Result<Value, String> {
    let list = args
        .get("projects")
        .and_then(|v| v.as_array())
        .ok_or("`projects` must be an array of {name, color?, parent_name?, sub_group_pattern?}")?;
    let existing = db::get_projects(conn).map_err(|e| e.to_string())?;

    let mut created = Vec::new();
    for (i, raw) in list.iter().enumerate() {
        let name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("projects[{i}].name is required"))?;

        if existing.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
            return Err(format!(
                "a project named {name:?} already exists — use update_project, or pick another name"
            ));
        }

        let color = raw
            .get("color")
            .and_then(|v| v.as_str())
            .unwrap_or("#6366f1")
            .to_string();
        validate_color(&color)?;

        let pattern = raw.get("sub_group_pattern").and_then(|v| v.as_str());
        validate_pattern(pattern)?;

        let parent_id = if let Some(id) = raw.get("parent_id").and_then(|v| v.as_i64()) {
            Some(id)
        } else if let Some(parent_name) = raw.get("parent_name").and_then(|v| v.as_str()) {
            let hits: Vec<_> = existing
                .iter()
                .filter(|p| p.name.eq_ignore_ascii_case(parent_name.trim()))
                .collect();
            if hits.len() != 1 {
                return Err(format!(
                    "projects[{i}].parent_name {parent_name:?} did not match exactly one project"
                ));
            }
            Some(hits[0].id)
        } else {
            None
        };

        let project = db::insert_project(conn, name, &color, parent_id, pattern)
            .map_err(|e| e.to_string())?;
        created.push(json!({
            "id": project.id,
            "name": project.name,
            "color": project.color,
            "parent_id": project.parent_id,
            "sub_group_pattern": project.sub_group_pattern,
        }));
    }

    Ok(json!({
        "created": created,
        "note": "Projects alone do not classify anything — add match rules (create_match_rules) \
                 so activity is attributed to them.",
    }))
}

fn update_project(conn: &Connection, args: &Value) -> Result<Value, String> {
    let id = req_i64(args, "id")?;
    let before = project_by_id(conn, id)?;

    let name = arg_str(args, "name").unwrap_or_else(|| before.name.clone());
    let color = arg_str(args, "color").unwrap_or_else(|| before.color.clone());
    validate_color(&color)?;

    let parent_id = if args.get("parent_id").is_some() {
        args.get("parent_id").and_then(|v| v.as_i64())
    } else {
        before.parent_id
    };
    if parent_id == Some(id) {
        return Err("a project cannot be its own parent".to_string());
    }

    let pattern: Option<String> = if args.get("sub_group_pattern").is_some() {
        arg_str(args, "sub_group_pattern")
    } else {
        before.sub_group_pattern.clone()
    };
    validate_pattern(pattern.as_deref())?;

    db::update_project(conn, id, &name, &color, parent_id, pattern.as_deref())
        .map_err(|e| e.to_string())?;

    let after = project_by_id(conn, id)?;
    Ok(json!({
        "before": { "name": before.name, "color": before.color, "parent_id": before.parent_id,
                    "sub_group_pattern": before.sub_group_pattern },
        "after":  { "name": after.name, "color": after.color, "parent_id": after.parent_id,
                    "sub_group_pattern": after.sub_group_pattern },
    }))
}

fn archive_project(conn: &Connection, args: &Value, archive: bool) -> Result<Value, String> {
    let id = req_i64(args, "id")?;
    let project = project_by_id(conn, id)?;
    if archive {
        db::archive_project(conn, id).map_err(|e| e.to_string())?;
    } else {
        db::unarchive_project(conn, id).map_err(|e| e.to_string())?;
    }
    Ok(json!({
        "project": project.name,
        "archived": archive,
        "note": "Archiving hides a project but keeps its rules and entries.",
    }))
}

fn delete_project(conn: &Connection, args: &Value) -> Result<Value, String> {
    let id = req_i64(args, "id")?;
    if arg_bool(args, "confirm") != Some(true) {
        return Err(
            "Refusing to delete without `confirm: true`. This cascades to the project's time \
             entries and match rules. Call get_projects and get_time_entries first, or use \
             archive_project to keep the data."
                .to_string(),
        );
    }
    let project = project_by_id(conn, id)?;
    let rules = db::get_project_match_rules(conn).map_err(|e| e.to_string())?;
    let rule_ids: Vec<i64> = rules
        .iter()
        .filter(|r| r.project_id == id)
        .map(|r| r.id)
        .collect();

    db::delete_project(conn, id).map_err(|e| e.to_string())?;

    Ok(json!({
        "deleted": { "id": project.id, "name": project.name },
        "cascaded_rules": rule_ids,
        "note": "Time entries and match rules belonging to this project were deleted with it.",
    }))
}

fn create_match_rules(conn: &Connection, args: &Value) -> Result<Value, String> {
    let list = args.get("rules").and_then(|v| v.as_array()).ok_or(
        "`rules` must be an array of {project_id|project_name, name, conditions, sub_group_pattern?}",
    )?;

    let mut created = Vec::new();
    for (i, raw) in list.iter().enumerate() {
        let project_id = resolve_project_id(conn, raw).map_err(|e| format!("rules[{i}]: {e}"))?;
        let name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let conditions = parse_conditions(
            raw.get("conditions")
                .ok_or_else(|| format!("rules[{i}].conditions is required"))?,
        )
        .map_err(|e| format!("rules[{i}]: {e}"))?;

        let pattern = raw.get("sub_group_pattern").and_then(|v| v.as_str());
        validate_pattern(pattern).map_err(|e| format!("rules[{i}]: {e}"))?;

        let rule = db::insert_project_match_rule(conn, project_id, name, &conditions, pattern)
            .map_err(|e| format!("rules[{i}]: {e}"))?;

        created.push(json!({
            "id": rule.id,
            "position": rule.position,
            "name": rule.name,
            "project_id": rule.project_id,
            "sub_group_pattern": rule.sub_group_pattern,
        }));
    }

    Ok(json!({
        "created": created,
        "note": "Rules are appended to the END of the global precedence order, so an existing rule \
                 still wins on overlap. Check `warnings` from get_match_rules if classification \
                 looks wrong.",
    }))
}

fn update_match_rule(conn: &Connection, args: &Value) -> Result<Value, String> {
    let id = req_i64(args, "id")?;
    let before = db::get_project_match_rules(conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("no match rule with id {id}"))?;

    let name = arg_str(args, "name").unwrap_or_else(|| before.name.clone());
    let conditions = match args.get("conditions") {
        Some(raw) => parse_conditions(raw)?,
        None => before.conditions.clone(),
    };
    let pattern: Option<String> = if args.get("sub_group_pattern").is_some() {
        arg_str(args, "sub_group_pattern")
    } else {
        before.sub_group_pattern.clone()
    };
    validate_pattern(pattern.as_deref())?;

    db::update_project_match_rule(conn, id, &name, &conditions, pattern.as_deref())
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "before": { "name": before.name, "condition_count": before.conditions.len(),
                    "sub_group_pattern": before.sub_group_pattern },
        "after":  { "name": name, "condition_count": conditions.len(),
                    "sub_group_pattern": pattern },
        "note": "To undo, call update_match_rule again with the `before` values.",
    }))
}

fn delete_match_rule(conn: &Connection, args: &Value) -> Result<Value, String> {
    let id = req_i64(args, "id")?;
    if arg_bool(args, "confirm") != Some(true) {
        return Err("Refusing to delete without `confirm: true`.".to_string());
    }
    let rule = db::get_project_match_rules(conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("no match rule with id {id}"))?;

    let conditions: Vec<Value> = rule
        .conditions
        .iter()
        .map(|c| {
            json!({
                "field": c.field.as_str(),
                "operator": c.operator.as_str(),
                "value": c.value,
                "negate": c.negate,
            })
        })
        .collect();

    db::delete_project_match_rule(conn, id).map_err(|e| e.to_string())?;

    Ok(json!({
        "deleted": {
            "id": rule.id,
            "name": rule.name,
            "project_id": rule.project_id,
            "sub_group_pattern": rule.sub_group_pattern,
            "conditions": conditions,
        },
        "note": "Pass the `conditions` above to create_match_rules to restore it.",
    }))
}

fn reorder_match_rules(conn: &Connection, args: &Value) -> Result<Value, String> {
    let ids: Vec<i64> = args
        .get("ordered_ids")
        .and_then(|v| v.as_array())
        .ok_or("`ordered_ids` must be an array of every rule id, highest precedence first")?
        .iter()
        .map(|v| {
            v.as_i64()
                .ok_or_else(|| "`ordered_ids` must contain integers".to_string())
        })
        .collect::<Result<_, _>>()?;

    let existing: Vec<i64> = db::get_project_match_rules(conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|r| r.id)
        .collect();

    let mut sorted = ids.clone();
    sorted.sort_unstable();
    let mut reference = existing.clone();
    reference.sort_unstable();
    if sorted != reference {
        return Err(format!(
            "`ordered_ids` must list every existing rule exactly once. Existing ids: {existing:?}. \
             Call get_match_rules first."
        ));
    }

    db::reorder_project_match_rules(conn, &ids).map_err(|e| e.to_string())?;
    Ok(json!({ "order": ids }))
}

/// Compile-check candidate rules and, given a range, measure what they would
/// claim. Writes nothing.
fn validate_match_rules(conn: &Connection, args: &Value) -> Result<Value, String> {
    let list = args.get("rules").and_then(|v| v.as_array()).ok_or(
        "`rules` must be an array of {name, conditions, sub_group_pattern?}",
    )?;

    // Candidates get ids that cannot collide with real rules, and positions
    // after every existing rule, so the normal matching path can be exercised
    // without touching the database.
    let mut candidates: Vec<ProjectMatchRule> = Vec::new();
    for (i, raw) in list.iter().enumerate() {
        let conditions = match raw.get("conditions") {
            Some(v) => parse_conditions(v).map_err(|e| format!("rules[{i}]: {e}"))?,
            None => return Err(format!("rules[{i}].conditions is required")),
        };
        let pattern = raw.get("sub_group_pattern").and_then(|v| v.as_str());
        validate_pattern(pattern).map_err(|e| format!("rules[{i}]: {e}"))?;

        candidates.push(ProjectMatchRule {
            id: -1 - i as i64,
            project_id: raw.get("project_id").and_then(|v| v.as_i64()).unwrap_or(0),
            name: raw
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("(candidate)")
                .to_string(),
            position: 1_000_000 + i as i64,
            conditions,
            sub_group_pattern: pattern.map(str::to_string),
        });
    }

    let range = match (arg_str(args, "start_date"), arg_str(args, "end_date")) {
        (Some(s), Some(e)) => Some((s, e)),
        (None, None) => None,
        _ => return Err("provide both `start_date` and `end_date`, or neither".to_string()),
    };

    let Some((start, end)) = range else {
        return Ok(json!({
            "validated": candidates.iter().map(|c| json!({
                "name": c.name,
                "condition_count": c.conditions.len(),
                "sub_group_pattern": c.sub_group_pattern,
            })).collect::<Vec<_>>(),
            "note": "Every candidate compiles. Pass start_date and end_date to also measure what \
                     each would claim.",
        }));
    };

    let dates = reporting::date_range(&start, &end, reporting::MAX_RANGE_DAYS)
        .map_err(|e| e.to_string())?;
    let settings = db::get_settings(conn).map_err(|e| e.to_string())?;
    let filter_rules = db::get_filter_rules(conn).map_err(|e| e.to_string())?;

    let existing_rules = db::get_project_match_rules(conn).map_err(|e| e.to_string())?;
    let existing = matcher::compile(&existing_rules);
    let compiled_candidates: Vec<Vec<matcher::CompiledRule>> = candidates
        .iter()
        .map(|c| matcher::compile(std::slice::from_ref(c)))
        .collect();

    // (hits, secs) as if the batch were appended, plus seconds an existing rule
    // already claims before the candidate ever gets a look in.
    let mut claims = vec![(0i64, 0i64); candidates.len()];
    let mut shadowed = vec![0i64; candidates.len()];

    for date in &dates {
        for block in
            db::get_activity_for_date(conn, date, &settings, &filter_rules).map_err(|e| e.to_string())?
        {
            if matcher::first_match(&block, &existing, &settings).is_some() {
                for (i, compiled) in compiled_candidates.iter().enumerate() {
                    if matcher::first_match(&block, compiled, &settings).is_some() {
                        shadowed[i] += block.duration_secs;
                    }
                }
                continue;
            }
            // First candidate in the submitted order wins, mirroring precedence.
            for (i, compiled) in compiled_candidates.iter().enumerate() {
                if matcher::first_match(&block, compiled, &settings).is_some() {
                    claims[i].0 += 1;
                    claims[i].1 += block.duration_secs;
                    break;
                }
            }
        }
    }

    let measured: Vec<Value> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "name": c.name,
                "condition_count": c.conditions.len(),
                "sub_group_pattern": c.sub_group_pattern,
                "would_claim_secs": claims[i].1,
                "would_claim": fmt_dur(claims[i].1),
                "would_claim_blocks": claims[i].0,
                "shadowed_secs": shadowed[i],
                "shadowed": fmt_dur(shadowed[i]),
            })
        })
        .collect();

    Ok(json!({
        "start_date": start,
        "end_date": end,
        "validated": measured,
        "note": "Nothing was written. `would_claim` is what each candidate would capture if added \
                 after all existing rules, counting the first candidate that matches a block. \
                 `shadowed` is time an existing rule already takes. A `would_claim` of zero means \
                 the conditions never match real titles — call get_known_apps or \
                 unclassified_titles to find real values first.",
    }))
}

fn log_time_entries(conn: &Connection, args: &Value) -> Result<Value, String> {
    let list = args
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or("`entries` must be an array of {date, project_id|project_name, start, end, note?}")?;
    let dry_run = arg_bool(args, "dry_run").unwrap_or(false);

    struct Pending {
        date: String,
        project_id: i64,
        start: i64,
        end: i64,
        note: String,
    }

    // Everything is validated before anything is written, so one bad entry
    // cannot leave half a batch behind.
    let mut pending: Vec<Pending> = Vec::new();
    for (i, raw) in list.iter().enumerate() {
        let date = raw
            .get("date")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| is_valid_date(s))
            .ok_or_else(|| format!("entries[{i}].date must be YYYY-MM-DD"))?
            .to_string();

        let project_id = resolve_project_id(conn, raw).map_err(|e| format!("entries[{i}]: {e}"))?;

        let start =
            parse_bound(raw, "start_minutes", "start").map_err(|e| format!("entries[{i}]: {e}"))?;
        let end =
            parse_bound(raw, "end_minutes", "end").map_err(|e| format!("entries[{i}]: {e}"))?;
        if end <= start {
            return Err(format!(
                "entries[{i}]: `end` ({}) must be after `start` ({})",
                mins_to_hhmm(end),
                mins_to_hhmm(start)
            ));
        }

        if let Some(conflict) = find_overlap(conn, &date, start, end, None)? {
            return Err(format!(
                "entries[{i}] ({date} {}–{}) overlaps {conflict}. Timesheeps entries must not \
                 overlap — call get_time_entries for that date first.",
                mins_to_hhmm(start),
                mins_to_hhmm(end)
            ));
        }
        if let Some(conflict) = pending
            .iter()
            .find(|p| p.date == date && start < p.end && p.start < end)
        {
            return Err(format!(
                "entries[{i}] ({date} {}–{}) overlaps an earlier entry in this same batch ({}–{})",
                mins_to_hhmm(start),
                mins_to_hhmm(end),
                mins_to_hhmm(conflict.start),
                mins_to_hhmm(conflict.end)
            ));
        }

        pending.push(Pending {
            date,
            project_id,
            start,
            end,
            note: raw
                .get("note")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }

    if dry_run {
        return Ok(json!({
            "dry_run": true,
            "would_create": pending.iter().map(|p| json!({
                "date": p.date,
                "project_id": p.project_id,
                "start": mins_to_hhmm(p.start),
                "end": mins_to_hhmm(p.end),
                "duration_mins": p.end - p.start,
                "note": p.note,
            })).collect::<Vec<_>>(),
            "note": "Nothing was written. Call again without `dry_run` to create these.",
        }));
    }

    let mut created = Vec::new();
    for p in &pending {
        let entry = db::insert_time_entry(conn, &p.date, p.project_id, p.start, p.end, &p.note)
            .map_err(|e| e.to_string())?;
        created.push(json!({
            "id": entry.id,
            "date": entry.date,
            "project_id": entry.project_id,
            "start": mins_to_hhmm(entry.start_minutes),
            "end": mins_to_hhmm(entry.end_minutes),
        }));
    }

    Ok(json!({ "created": created }))
}

fn update_time_entry(conn: &Connection, args: &Value) -> Result<Value, String> {
    let id = req_i64(args, "id")?;
    let date = req_str(args, "date")?;
    let before = db::get_time_entries_for_date(conn, &date)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("no time entry with id {id} on {date}"))?;

    let project_id = match arg_i64(args, "project_id") {
        Some(v) => v,
        None => resolve_project_id(conn, args)?,
    };
    let start = arg_i64(args, "start_minutes").unwrap_or(before.start_minutes);
    let end = arg_i64(args, "end_minutes").unwrap_or(before.end_minutes);
    if end <= start {
        return Err(format!(
            "`end` ({}) must be after `start` ({})",
            mins_to_hhmm(end),
            mins_to_hhmm(start)
        ));
    }
    if let Some(conflict) = find_overlap(conn, &before.date, start, end, Some(id))? {
        return Err(format!(
            "the new range ({}–{}) overlaps {conflict}",
            mins_to_hhmm(start),
            mins_to_hhmm(end)
        ));
    }
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| before.note.clone());

    db::update_time_entry(conn, id, project_id, start, end, &note).map_err(|e| e.to_string())?;

    Ok(json!({
        "before": { "project_id": before.project_id, "start": mins_to_hhmm(before.start_minutes),
                    "end": mins_to_hhmm(before.end_minutes), "note": before.note },
        "after":  { "project_id": project_id, "start": mins_to_hhmm(start),
                    "end": mins_to_hhmm(end), "note": note },
    }))
}

fn delete_time_entry(conn: &Connection, args: &Value) -> Result<Value, String> {
    let id = req_i64(args, "id")?;
    if arg_bool(args, "confirm") != Some(true) {
        return Err("Refusing to delete without `confirm: true`.".to_string());
    }
    db::delete_time_entry(conn, id).map_err(|e| e.to_string())?;
    Ok(json!({ "deleted_id": id }))
}

// ── MCP protocol ──────────────────────────────────────────────────────────────

/// Every tool that mutates the database, so the `mcp_allow_writes` gate is
/// enforced in one place and a new write tool cannot forget to check it.
///
/// `validate_match_rules` is deliberately absent: it writes nothing, so it stays
/// available when writes are off — the user can still have Claude propose rules
/// and show what they would capture, then apply them by hand.
const WRITE_TOOLS: [&str; 12] = [
    "create_projects",
    "update_project",
    "archive_project",
    "unarchive_project",
    "delete_project",
    "create_match_rules",
    "update_match_rule",
    "delete_match_rule",
    "reorder_match_rules",
    "log_time_entries",
    "update_time_entry",
    "delete_time_entry",
];

fn write_gate_error() -> String {
    "Writes are disabled in Timesheeps. Turn on Settings → \"Allow Claude to change projects, \
     rules and time entries\", then try again. Reads still work."
        .to_string()
}

fn tools_schema() -> Value {
    let date = |what: &str| json!({ "type": "string", "description": format!("YYYY-MM-DD. {what}") });

    let condition_item = || {
        json!({
            "type": "object",
            "properties": {
                "field": { "type": "string", "enum": ["app_name", "window_title"] },
                "operator": { "type": "string", "enum": ["contains", "equals", "starts_with", "ends_with"] },
                "value": { "type": "string", "description": "Must not be blank — a blank value is dropped at match time." },
                "negate": { "type": "boolean", "description": "Inverts the condition (\"is not\")." }
            },
            "required": ["field", "operator", "value"]
        })
    };

    json!([
        {
            "name": "get_classified_totals",
            "description": "THE main tool for timesheet questions. Tracked time grouped by project (or by ticket, or by app) across a date range, with a per-day breakdown plus a tracked-vs-logged comparison. Classification uses the user's own match rules, so the numbers agree with the Timesheeps app. Also reports activity no rule claimed. Prefer this over fetching many single days.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start_date": date("First day, inclusive."),
                    "end_date": date("Last day, inclusive."),
                    "group_by": {
                        "type": "string",
                        "enum": ["project", "ticket", "app"],
                        "description": "`project` (default) is one bucket per project. `ticket` splits a project further by whatever its ticket regex captured (needs a sub_group_pattern). `app` ignores rules entirely and groups by application."
                    }
                },
                "required": ["start_date", "end_date"]
            }
        },
        {
            "name": "get_day_summary_range",
            "description": "Per-day totals across a range: tracked, logged, unclassified and a per-project split for each day. Use include: \"full\" to also get that day's time entries and unclassified titles.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start_date": date("First day, inclusive."),
                    "end_date": date("Last day, inclusive."),
                    "include": {
                        "type": "string",
                        "enum": ["summary", "full"],
                        "description": "`summary` (default) omits time entries and title lists."
                    }
                },
                "required": ["start_date", "end_date"]
            }
        },
        {
            "name": "get_day_summary",
            "description": "Everything about one day: tracked time per project, app usage and manually logged entries. Best for \"what did I work on today?\".",
            "inputSchema": {
                "type": "object",
                "properties": { "date": date("Defaults to today.") }
            }
        },
        {
            "name": "get_activity_summary",
            "description": "Per-app and per-window-title time for one day, after the user's merge and minimum-duration settings. Use for \"how long was I in VS Code?\".",
            "inputSchema": {
                "type": "object",
                "properties": { "date": date("Defaults to today.") }
            }
        },
        {
            "name": "get_window_summary_range",
            "description": "Window-level activity across a range, reaggregated by app and window title. Useful for spotting recurring non-project activity.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start_date": date("First day, inclusive."),
                    "end_date": date("Last day, inclusive."),
                    "min_secs": { "type": "integer", "description": "Drop rows below this total. Defaults to the app's window_summary_min_secs." }
                },
                "required": ["start_date", "end_date"]
            }
        },
        {
            "name": "unclassified_titles",
            "description": "Window titles over a range that no project match rule claimed, ranked by time. This is the raw material for new rules: ask the user which project each belongs to, then validate_match_rules, then create_match_rules.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start_date": date("First day, inclusive."),
                    "end_date": date("Last day, inclusive."),
                    "min_secs": { "type": "integer", "description": "Minimum total seconds across the whole range. Defaults to 60." }
                },
                "required": ["start_date", "end_date"]
            }
        },
        {
            "name": "get_match_rules",
            "description": "Every project match rule with its conditions, global precedence position and effective ticket pattern. Read this before adding rules: rules are evaluated in `position` order across all projects and the first match wins, so a new rule can be shadowed by an existing one.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_projects",
            "description": "Active projects (with their ticket patterns) plus archived ones.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_known_apps",
            "description": "Applications and sample window titles actually seen in recent activity. Use this to find real values for a rule's conditions instead of guessing.",
            "inputSchema": {
                "type": "object",
                "properties": { "days": { "type": "integer", "description": "How far back to look. Defaults to 90." } }
            }
        },
        {
            "name": "get_time_entries",
            "description": "Manually logged entries (project, note, start/end) for one day.",
            "inputSchema": {
                "type": "object",
                "properties": { "date": date("Defaults to today.") }
            }
        },
        {
            "name": "get_settings",
            "description": "Current tracking settings, including min_duration_secs and whether writes are currently allowed.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "create_projects",
            "description": "Create one or more projects. Batched and validated up front. Projects do not classify anything on their own — add rules with create_match_rules.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "projects": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "color": { "type": "string", "description": "Hex colour such as #6366f1. Defaults to the app's indigo." },
                                "parent_id": { "type": "integer" },
                                "parent_name": { "type": "string", "description": "Alternative to parent_id." },
                                "sub_group_pattern": { "type": "string", "description": "Optional regex applied to window titles to split this project's time by ticket, e.g. \"ELMS-\\\\d+\". Group 1 is used when the pattern has a capture group, otherwise the whole match." }
                            },
                            "required": ["name"]
                        }
                    }
                },
                "required": ["projects"]
            }
        },
        {
            "name": "update_project",
            "description": "Update a project. Omitted fields are left unchanged; returns before/after so a change can be undone.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "name": { "type": "string" },
                    "color": { "type": "string" },
                    "parent_id": { "type": ["integer", "null"], "description": "Pass null to make it top-level." },
                    "sub_group_pattern": { "type": ["string", "null"], "description": "Pass null to clear it." }
                },
                "required": ["id"]
            }
        },
        {
            "name": "archive_project",
            "description": "Archive a project. Hides it without deleting its rules or entries. Prefer this over delete_project.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer" } },
                "required": ["id"]
            }
        },
        {
            "name": "unarchive_project",
            "description": "Restore an archived project.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer" } },
                "required": ["id"]
            }
        },
        {
            "name": "delete_project",
            "description": "Permanently delete a project. Cascades to its time entries and match rules. Requires confirm: true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "confirm": { "type": "boolean", "description": "Must be true. Ask the user before setting it." }
                },
                "required": ["id", "confirm"]
            }
        },
        {
            "name": "create_match_rules",
            "description": "Create project match rules. A rule is a group of conditions AND-ed together; rules are OR-ed across projects and evaluated in global position order, first match wins. New rules are appended last, so they cannot steal time from an existing rule. Conditions should use `contains` on a distinctive fragment of a real title rather than exact titles, which change constantly.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rules": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "project_id": { "type": "integer" },
                                "project_name": { "type": "string", "description": "Alternative to project_id; must match exactly one project." },
                                "name": { "type": "string", "description": "A label for the rule, e.g. \"Lotteries Jira\"." },
                                "sub_group_pattern": { "type": "string", "description": "Optional regex overriding the project's ticket pattern." },
                                "conditions": { "type": "array", "minItems": 1, "items": condition_item() }
                            },
                            "required": ["name", "conditions"]
                        }
                    }
                },
                "required": ["rules"]
            }
        },
        {
            "name": "update_match_rule",
            "description": "Update one rule. Omitted fields are left unchanged; returns before/after.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "name": { "type": "string" },
                    "sub_group_pattern": { "type": ["string", "null"], "description": "Pass null to fall back to the project's pattern." },
                    "conditions": { "type": "array", "minItems": 1, "items": condition_item() }
                },
                "required": ["id"]
            }
        },
        {
            "name": "delete_match_rule",
            "description": "Delete one rule. Requires confirm: true; returns the deleted rule so it can be restored.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "confirm": { "type": "boolean", "description": "Must be true." }
                },
                "required": ["id", "confirm"]
            }
        },
        {
            "name": "reorder_match_rules",
            "description": "Set the global precedence order. Pass every rule id exactly once, highest precedence first — call get_match_rules for the current ids. Only matters where rules overlap.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ordered_ids": { "type": "array", "items": { "type": "integer" } }
                },
                "required": ["ordered_ids"]
            }
        },
        {
            "name": "validate_match_rules",
            "description": "Dry run: compile-check candidate rules and, if a date range is given, report how much unclaimed time each would capture and how much is shadowed by existing rules. Writes nothing. Use this to check your work before create_match_rules.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rules": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "sub_group_pattern": { "type": "string" },
                                "conditions": { "type": "array", "minItems": 1, "items": condition_item() }
                            },
                            "required": ["name", "conditions"]
                        }
                    },
                    "start_date": { "type": "string", "description": "Optional; needs end_date too." },
                    "end_date": { "type": "string", "description": "Optional; needs start_date too." }
                },
                "required": ["rules"]
            }
        },
        {
            "name": "log_time_entries",
            "description": "Create manual time entries. Validated up front: the whole batch is rejected if any entry is invalid or overlaps an existing one. Use dry_run to preview. These entries are what the user's timesheet is filled from, so confirm the split with the user before writing.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dry_run": { "type": "boolean", "description": "Validate and report without writing." },
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "date": { "type": "string", "description": "YYYY-MM-DD." },
                                "project_id": { "type": "integer" },
                                "project_name": { "type": "string", "description": "Alternative to project_id." },
                                "start": { "type": "string", "description": "24-hour \"HH:MM\"." },
                                "end": { "type": "string", "description": "24-hour \"HH:MM\"." },
                                "start_minutes": { "type": "integer", "description": "Minutes from midnight; alternative to start." },
                                "end_minutes": { "type": "integer", "description": "Minutes from midnight; alternative to end." },
                                "note": { "type": "string" }
                            },
                            "required": ["date"]
                        }
                    }
                },
                "required": ["entries"]
            }
        },
        {
            "name": "update_time_entry",
            "description": "Move or re-label an existing time entry. Requires the entry's current date plus its id. Returns before/after.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "date": { "type": "string", "description": "The date the entry is currently on." },
                    "project_id": { "type": "integer" },
                    "start_minutes": { "type": "integer" },
                    "end_minutes": { "type": "integer" },
                    "note": { "type": "string" }
                },
                "required": ["id", "date"]
            }
        },
        {
            "name": "delete_time_entry",
            "description": "Delete a time entry. Requires confirm: true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "confirm": { "type": "boolean", "description": "Must be true." }
                },
                "required": ["id", "confirm"]
            }
        }
    ])
}

fn dispatch(conn: &Connection, name: &str, args: &Value) -> Result<Value, String> {
    if WRITE_TOOLS.contains(&name) && !writes_allowed(conn) {
        return Err(write_gate_error());
    }

    let today = || arg_str(args, "date").unwrap_or_else(today_local);

    match name {
        // Reads
        "get_classified_totals" => get_classified_totals(conn, args),
        "get_day_summary_range" => get_day_summary_range(conn, args),
        "get_day_summary" => get_day_summary(conn, &today()),
        "get_activity_summary" => {
            let date = today();
            if !is_valid_date(&date) {
                return Err(format!("`date` must be YYYY-MM-DD, got {date:?}"));
            }
            activity_by_app(conn, &date)
        }
        "get_window_summary_range" => get_window_summary_range(conn, args),
        "unclassified_titles" => get_unclassified_titles(conn, args),
        "get_match_rules" => get_match_rules(conn),
        "get_projects" => get_projects(conn),
        "get_known_apps" => get_known_apps(conn, args),
        "get_time_entries" => {
            let date = today();
            if !is_valid_date(&date) {
                return Err(format!("`date` must be YYYY-MM-DD, got {date:?}"));
            }
            Ok(json!({ "date": date, "entries": time_entries_for(conn, &date)? }))
        }
        "get_settings" => get_settings(conn),

        // Writes
        "create_projects" => create_projects(conn, args),
        "update_project" => update_project(conn, args),
        "archive_project" => archive_project(conn, args, true),
        "unarchive_project" => archive_project(conn, args, false),
        "delete_project" => delete_project(conn, args),
        "create_match_rules" => create_match_rules(conn, args),
        "update_match_rule" => update_match_rule(conn, args),
        "delete_match_rule" => delete_match_rule(conn, args),
        "reorder_match_rules" => reorder_match_rules(conn, args),
        "validate_match_rules" => validate_match_rules(conn, args),
        "log_time_entries" => log_time_entries(conn, args),
        "update_time_entry" => update_time_entry(conn, args),
        "delete_time_entry" => delete_time_entry(conn, args),

        other => Err(format!("Unknown tool: {other}")),
    }
}

fn handle(msg: &Value) -> Option<Value> {
    let method = msg.get("method")?.as_str()?;
    let id = msg.get("id").cloned();

    // Notifications never get a response
    if method.starts_with("notifications/") {
        return None;
    }

    let result: Value = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "timesheeps", "version": "2.0.0" }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools_schema() }),
        "tools/call" => {
            let params = msg.get("params")?;
            let name = params.get("name")?.as_str()?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));

            // A failure has to be an MCP tool error, not data inside a
            // successful payload — otherwise the caller has to guess whether it
            // worked at all.
            match open_db().and_then(|conn| dispatch(&conn, name, &args)) {
                Ok(data) => {
                    let text = serde_json::to_string_pretty(&data).unwrap_or_default();
                    json!({ "content": [{ "type": "text", "text": text }] })
                }
                Err(message) => json!({
                    "content": [{ "type": "text", "text": message }],
                    "isError": true
                }),
            }
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            }));
        }
    };

    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_format_compactly() {
        assert_eq!(fmt_dur(0), "0s");
        assert_eq!(fmt_dur(59), "59s");
        assert_eq!(fmt_dur(60), "1m");
        assert_eq!(fmt_dur(3600), "1h");
        assert_eq!(fmt_dur(3660), "1h 1m");
        // Never render a negative duration as a negative number.
        assert_eq!(fmt_dur(-30), "0s");
    }

    #[test]
    fn clock_times_parse_and_validate() {
        assert_eq!(parse_hhmm("09:30"), Some(570));
        assert_eq!(parse_hhmm("00:00"), Some(0));
        assert_eq!(parse_hhmm("23:59"), Some(1439));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("9:5"), Some(545), "lenient on zero padding");
        assert_eq!(parse_hhmm("half nine"), None);
        assert_eq!(parse_hhmm(""), None);
    }

    #[test]
    fn entry_bounds_accept_either_shape() {
        let clock = json!({ "start": "09:00", "end": "10:30" });
        assert_eq!(parse_bound(&clock, "start_minutes", "start").unwrap(), 540);
        assert_eq!(parse_bound(&clock, "end_minutes", "end").unwrap(), 630);

        let minutes = json!({ "start_minutes": 0, "end_minutes": 1440 });
        assert_eq!(parse_bound(&minutes, "start_minutes", "start").unwrap(), 0);
        assert_eq!(parse_bound(&minutes, "end_minutes", "end").unwrap(), 1440);

        let out_of_range = json!({ "start_minutes": 2000 });
        assert!(parse_bound(&out_of_range, "start_minutes", "start").is_err());
        assert!(parse_bound(&json!({}), "start_minutes", "start").is_err());
    }

    #[test]
    fn colours_must_be_hex() {
        assert!(validate_color("#6366f1").is_ok());
        assert!(validate_color("#fff").is_ok());
        assert!(validate_color("#6366f1ff").is_ok());
        assert!(validate_color("red").is_err());
        assert!(validate_color("#12345").is_err());
        assert!(validate_color("6366f1").is_err());
        assert!(validate_color("#6366gz").is_err());
    }

    #[test]
    fn sub_group_patterns_are_compile_checked() {
        assert!(validate_pattern(None).is_ok());
        assert!(validate_pattern(Some("   ")).is_ok(), "blank means no pattern");
        assert!(validate_pattern(Some("ELMS-\\d+")).is_ok());
        assert!(validate_pattern(Some("ELMS-(\\d+")).is_err());
    }

    #[test]
    fn conditions_reject_blank_and_unknown_values() {
        // A blank value is dropped at match time, so accepting it would create a
        // rule that silently never matches.
        let blank = json!({ "field": "window_title", "operator": "contains", "value": "  " });
        assert!(parse_condition(&blank).is_err());

        let bad_field = json!({ "field": "banana", "operator": "contains", "value": "x" });
        let err = parse_condition(&bad_field).unwrap_err();
        assert!(err.contains("window_title"), "error should list valid fields: {err}");

        let bad_op = json!({ "field": "app_name", "operator": "regex", "value": "x" });
        let err = parse_condition(&bad_op).unwrap_err();
        assert!(err.contains("contains"), "error should list valid operators: {err}");

        let negated = json!({
            "field": "app_name", "operator": "equals", "value": "olk", "negate": true
        });
        let parsed = parse_condition(&negated).unwrap();
        assert!(parsed.negate);
        assert_eq!(parsed.value, "olk");

        assert!(parse_conditions(&json!([])).is_err(), "empty rule is not allowed");
    }

    #[test]
    fn every_declared_write_tool_exists_in_the_schema() {
        let schema = tools_schema();
        let names: Vec<&str> = schema
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();

        for tool in WRITE_TOOLS {
            assert!(names.contains(&tool), "{tool} is gated but not declared");
        }
    }

    #[test]
    fn tool_names_are_unique_and_every_tool_is_dispatched() {
        let schema = tools_schema();
        let names: Vec<String> = schema
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();

        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "duplicate tool name in the schema");

        // Dispatch must not fall through to the unknown-tool branch for any
        // declared tool. `None` means "no database", which is enough to prove
        // the name resolved to a real arm.
        let conn = Connection::open_in_memory().unwrap();
        for name in &names {
            let probe = match name.as_str() {
                "get_classified_totals" | "get_day_summary_range" | "get_window_summary_range"
                | "unclassified_titles" => json!({ "start_date": "2026-01-01", "end_date": "2026-01-01" }),
                "create_projects" => json!({ "projects": [{ "name": "x" }] }),
                "create_match_rules" | "validate_match_rules" => {
                    json!({ "rules": [{ "name": "x", "conditions": [] }] })
                }
                "log_time_entries" => json!({ "entries": [{ "date": "2026-01-01" }] }),
                "reorder_match_rules" => json!({ "ordered_ids": [] }),
                _ => json!({}),
            };
            let result = dispatch(&conn, name, &probe);
            if let Err(message) = result {
                assert!(
                    !message.starts_with("Unknown tool"),
                    "{name} is declared but not handled"
                );
            }
        }
    }

    #[test]
    fn range_tools_require_both_bounds() {
        assert!(req_range(&json!({})).is_err());
        assert!(req_range(&json!({ "start_date": "2026-01-01" })).is_err());
        assert!(req_range(&json!({ "start_date": "2026-01-01", "end_date": "2026-01-02" })).is_ok());
    }

    #[test]
    fn writes_default_on_and_the_gate_can_be_disabled() {
        let conn = Connection::open_in_memory().unwrap();
        // Only `settings` is needed to exercise the gate; the write tools
        // themselves are verified against a real database copy.
        conn.execute_batch("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();

        // No row yet means "never configured", which must mean allowed.
        assert!(writes_allowed(&conn));

        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('mcp_allow_writes', '0')",
            [],
        )
        .unwrap();
        assert!(!writes_allowed(&conn));

        // A mutating tool is refused before it can touch anything...
        let err = dispatch(&conn, "create_projects", &json!({ "projects": [{ "name": "x" }] }))
            .unwrap_err();
        assert!(err.contains("Writes are disabled"), "{err}");

        // ...but reads and the dry run stay available.
        for tool in ["get_projects", "validate_match_rules"] {
            let args = if tool == "validate_match_rules" {
                json!({ "rules": [{ "name": "x", "conditions": [] }] })
            } else {
                json!({})
            };

            if let Err(message) = dispatch(&conn, tool, &args) {
                assert!(
                    !message.contains("Writes are disabled"),
                    "{tool} must not be gated"
                );
            }
        }

        conn.execute(
            "UPDATE settings SET value = '1' WHERE key = 'mcp_allow_writes'",
            [],
        )
        .unwrap();
        assert!(writes_allowed(&conn));
    }
}

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut out = stdout.lock();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(msg) = serde_json::from_str::<Value>(trimmed) {
                    if let Some(resp) = handle(&msg) {
                        if let Ok(s) = serde_json::to_string(&resp) {
                            let _ = writeln!(out, "{s}");
                            let _ = out.flush();
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}
