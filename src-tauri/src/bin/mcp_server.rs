//! Timesheeps MCP server — standalone binary that speaks the Model Context Protocol
//! over stdio. Claude Desktop launches this directly; no Node.js required.
//!
//! DB path: %APPDATA%\app.timesheeps.Timesheeps\timesheeps.db
//! Override: TIMESHEEPS_DB environment variable.

use std::io::{BufRead, BufReader, Write};

use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};

// ── Database ──────────────────────────────────────────────────────────────────

fn db_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("TIMESHEEPS_DB") {
        return std::path::PathBuf::from(p);
    }
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

fn open_db() -> Result<Connection, String> {
    let path = db_path();
    Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Cannot open database at {}: {}", path.display(), e))
}

// ── Formatting helpers ────────────────────────────────────────────────────────

fn today_local() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn fmt_dur(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        return format!("{}s", secs);
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    match (h, m) {
        (0, m) => format!("{}m", m),
        (h, 0) => format!("{}h", h),
        (h, m) => format!("{}h {}m", h, m),
    }
}

fn mins_to_hhmm(m: i64) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

// ── Tool: activity summary ────────────────────────────────────────────────────

fn get_activity_summary(date: &str) -> Value {
    let db = match open_db() {
        Ok(c) => c,
        Err(e) => return json!({ "error": e }),
    };

    let mut stmt = match db.prepare(
        "SELECT app_name, window_title,
                SUM(CAST((julianday(ended_at) - julianday(started_at)) * 86400 AS INTEGER)) AS total_secs
         FROM activity_raw
         WHERE date(started_at, 'localtime') = ?1
         GROUP BY app_name, window_title
         ORDER BY total_secs DESC",
    ) {
        Ok(s) => s,
        Err(e) => return json!({ "error": e.to_string() }),
    };

    use std::collections::HashMap;
    let mut by_app: HashMap<String, (i64, Vec<Value>)> = HashMap::new();

    if let Ok(rows) = stmt.query_map([date], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2).unwrap_or(0),
        ))
    }) {
        for (app, title, secs) in rows.flatten() {
            let e = by_app.entry(app).or_insert((0, vec![]));
            e.0 += secs;
            e.1.push(json!({ "title": title, "duration": fmt_dur(secs), "total_secs": secs }));
        }
    }

    let mut apps: Vec<Value> = by_app
        .into_iter()
        .map(|(name, (total, windows))| {
            json!({
                "app_name": name,
                "total_secs": total,
                "duration": fmt_dur(total),
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

    json!({ "date": date, "activity_by_app": apps })
}

// ── Tool: time entries ────────────────────────────────────────────────────────

fn get_time_entries(date: &str) -> Value {
    let db = match open_db() {
        Ok(c) => c,
        Err(e) => return json!({ "error": e }),
    };

    let mut stmt = match db.prepare(
        "SELECT te.id, te.start_minutes, te.end_minutes, te.note, p.name, p.color
         FROM time_entries te
         JOIN projects p ON p.id = te.project_id
         WHERE te.date = ?1
         ORDER BY te.start_minutes",
    ) {
        Ok(s) => s,
        Err(e) => return json!({ "error": e.to_string() }),
    };

    let entries: Vec<Value> = match stmt.query_map([date], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    }) {
        Ok(rows) => rows
            .flatten()
            .map(|(id, start, end, note, project, color)| {
                json!({
                    "id":            id,
                    "project":       project,
                    "color":         color,
                    "start":         mins_to_hhmm(start),
                    "end":           mins_to_hhmm(end),
                    "duration":      fmt_dur((end - start) * 60),
                    "duration_mins": end - start,
                    "note":          note,
                })
            })
            .collect(),
        Err(_) => vec![],
    };

    json!({ "date": date, "entries": entries })
}

// ── Tool: projects ────────────────────────────────────────────────────────────

fn get_projects() -> Value {
    let db = match open_db() {
        Ok(c) => c,
        Err(e) => return json!({ "error": e }),
    };

    let mut stmt = match db.prepare(
        "SELECT id, name, color, parent_id
         FROM projects
         WHERE archived_at IS NULL
         ORDER BY name",
    ) {
        Ok(s) => s,
        Err(e) => return json!({ "error": e.to_string() }),
    };

    let projects: Vec<Value> = match stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let color: String = row.get(2)?;
        let parent_id: Option<i64> = row.get(3)?;
        Ok(json!({ "id": id, "name": name, "color": color, "parent_id": parent_id }))
    }) {
        Ok(rows) => rows.flatten().collect(),
        Err(_) => vec![],
    };

    json!({ "projects": projects })
}

// ── Tool: day summary ─────────────────────────────────────────────────────────

fn get_day_summary(date: &str) -> Value {
    let activity = get_activity_summary(date);
    let entries = get_time_entries(date);

    let tracked_secs = activity["activity_by_app"]
        .as_array()
        .map(|v| v.iter().filter_map(|a| a["total_secs"].as_i64()).sum::<i64>())
        .unwrap_or(0);

    let logged_mins = entries["entries"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|e| e["duration_mins"].as_i64())
                .sum::<i64>()
        })
        .unwrap_or(0);

    json!({
        "date":            date,
        "total_tracked":   fmt_dur(tracked_secs),
        "total_logged":    fmt_dur(logged_mins * 60),
        "activity_by_app": activity["activity_by_app"],
        "time_entries":    entries["entries"],
    })
}

// ── MCP protocol ──────────────────────────────────────────────────────────────

fn tools_schema() -> Value {
    json!([
        {
            "name": "get_day_summary",
            "description": "Get a combined summary of computer activity (apps used) and logged time entries for a day. Best for 'what did I work on today?' or 'how did I spend Tuesday?'",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "date": { "type": "string", "description": "YYYY-MM-DD. Defaults to today." }
                }
            }
        },
        {
            "name": "get_activity_summary",
            "description": "Time spent per app and window title on a given day, from continuous background window tracking.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "date": { "type": "string", "description": "YYYY-MM-DD. Defaults to today." }
                }
            }
        },
        {
            "name": "get_time_entries",
            "description": "Manually logged time entries (project, note, start/end time) for a given day.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "date": { "type": "string", "description": "YYYY-MM-DD. Defaults to today." }
                }
            }
        },
        {
            "name": "get_projects",
            "description": "List all active (non-archived) projects in Timesheeps.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

fn dispatch_tool(name: &str, args: &Value) -> Value {
    let date = args
        .get("date")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(today_local);

    match name {
        "get_day_summary"      => get_day_summary(&date),
        "get_activity_summary" => get_activity_summary(&date),
        "get_time_entries"     => get_time_entries(&date),
        "get_projects"         => get_projects(),
        _                      => json!({ "error": format!("Unknown tool: {}", name) }),
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
            "serverInfo": { "name": "timesheeps", "version": "1.0.0" }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools_schema() }),
        "tools/call" => {
            let params = msg.get("params")?;
            let name = params.get("name")?.as_str()?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let data = dispatch_tool(name, &args);
            let text = serde_json::to_string_pretty(&data).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": text }] })
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            }));
        }
    };

    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

// ── Entry point ───────────────────────────────────────────────────────────────

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
                            let _ = writeln!(out, "{}", s);
                            let _ = out.flush();
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}
