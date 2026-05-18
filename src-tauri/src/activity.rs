//! Background task that polls the active window every 5 seconds.
//! On Windows: uses Win32 APIs via the `windows` crate.
//! On other platforms: no-op (stub for development on Linux/macOS).

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::models::Settings;

/// Emitted to the frontend when the user has been idle and returns.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdleReturnEvent {
    pub idle_secs: u64,
    pub idle_started_at: String, // ISO8601
    pub idle_ended_at: String,
}

/// State shared between the polling loop and IPC commands.
pub struct PollerState {
    /// The window/app that is currently active.
    current_app: String,
    current_title: String,
    current_window_id: u64,
    /// Row id of the open activity_raw row being extended on each poll.
    current_row_id: Option<i64>,
    is_idle: bool,
    idle_started: Option<chrono::DateTime<chrono::Utc>>,
}

impl PollerState {
    fn new() -> Self {
        PollerState {
            current_app: String::new(),
            current_title: String::new(),
            current_window_id: 0,
            current_row_id: None,
            is_idle: false,
            idle_started: None,
        }
    }
}

const POLL_INTERVAL_SECS: u64 = 20;

pub async fn start_polling(app: AppHandle) {
    let state = Arc::new(Mutex::new(PollerState::new()));

    loop {
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
        poll_once(&app, &state);
    }
}

fn poll_once(app: &AppHandle, state: &Arc<Mutex<PollerState>>) {
    let db_state = app.state::<crate::AppState>();
    let db_guard = db_state.db.lock().unwrap();
    let settings = db::get_settings(&db_guard).unwrap_or_default();
    drop(db_guard);

    let idle_secs = get_idle_seconds();

    if idle_secs >= settings.idle_timeout_secs as u64 {
        handle_idle(app, state, &settings, idle_secs);
    } else {
        handle_active(app, state, &settings);
    }
}

fn handle_active(app: &AppHandle, state: &Arc<Mutex<PollerState>>, _settings: &Settings) {
    let Some((new_app, new_title, new_window_id)) = get_foreground_window_info() else {
        return;
    };

    let mut s = state.lock().unwrap();
    let now = chrono::Utc::now();

    // Return from idle: emit event then fall through to start a new session.
    if s.is_idle {
        if let Some(idle_start) = s.idle_started {
            let idle_secs = (now - idle_start).num_seconds() as u64;
            let _ = app.emit(
                "idle-return",
                IdleReturnEvent {
                    idle_secs,
                    idle_started_at: idle_start.to_rfc3339(),
                    idle_ended_at: now.to_rfc3339(),
                },
            );
        }
        s.is_idle = false;
        s.idle_started = None;
        s.current_row_id = None;
        // fall through to start a new session below
    }

    let window_changed = new_window_id != s.current_window_id || new_app != s.current_app;

    if window_changed || s.current_row_id.is_none() {
        // New window (or first poll): open a fresh row.
        s.current_app = new_app.clone();
        s.current_title = new_title.clone();
        s.current_window_id = new_window_id;
        s.current_row_id = start_session(app, &new_app, &new_title, new_window_id, &now);
    } else {
        // Same window — just extend the existing row's ended_at.
        if new_title != s.current_title {
            s.current_title = new_title;
        }
        extend_session(app, s.current_row_id, &now);
    }
}

fn handle_idle(
    app: &AppHandle,
    state: &Arc<Mutex<PollerState>>,
    _settings: &Settings,
    idle_secs: u64,
) {
    let mut s = state.lock().unwrap();
    if s.is_idle {
        return;
    }
    let now = chrono::Utc::now();
    let idle_started = now - chrono::Duration::seconds(idle_secs as i64);

    // Stamp the open row's ended_at precisely at when idle began.
    if let Some(id) = s.current_row_id {
        let db_state = app.state::<crate::AppState>();
        let db_guard = db_state.db.lock().unwrap();
        let _ = db::update_activity_end(&db_guard, id, &idle_started);
    }

    s.is_idle = true;
    s.idle_started = Some(idle_started);
    s.current_app = String::new();
    s.current_title = String::new();
    s.current_window_id = 0;
    s.current_row_id = None;
}

fn start_session(
    app: &AppHandle,
    app_name: &str,
    window_title: &str,
    window_id: u64,
    now: &chrono::DateTime<chrono::Utc>,
) -> Option<i64> {
    let db_state = app.state::<crate::AppState>();
    let db_guard = db_state.db.lock().unwrap();
    match db::insert_activity(&db_guard, app_name, window_title, window_id, now, now) {
        Ok(id) => {
            drop(db_guard);
            let _ = app.emit("activity-updated", ());
            Some(id)
        }
        Err(_) => None,
    }
}

fn extend_session(
    app: &AppHandle,
    row_id: Option<i64>,
    now: &chrono::DateTime<chrono::Utc>,
) {
    let Some(id) = row_id else { return; };
    let db_state = app.state::<crate::AppState>();
    let db_guard = db_state.db.lock().unwrap();
    if db::update_activity_end(&db_guard, id, now).is_ok() {
        drop(db_guard);
        let _ = app.emit("activity-updated", ());
    }
}

// ── Platform-specific: get foreground window ──────────────────────────────────

#[cfg(target_os = "windows")]
fn get_foreground_window_info() -> Option<(String, String, u64)> {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::ProcessStatus::GetModuleFileNameExW,
        System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId},
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        // Window title
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        if title_len == 0 {
            return None;
        }
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        // Process name
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return Some((String::from("unknown"), title, hwnd.0 as usize as u64));
        }

        let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return Some((String::from("unknown"), title, hwnd.0 as usize as u64)),
        };

        let mut name_buf = [0u16; 512];
        let name_len = GetModuleFileNameExW(process, None, &mut name_buf);
        let _ = CloseHandle(process);

        let app_name = if name_len > 0 {
            let full_path = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            std::path::Path::new(&full_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            String::from("unknown")
        };

        Some((app_name, title, hwnd.0 as usize as u64))
    }
}

#[cfg(not(target_os = "windows"))]
fn get_foreground_window_info() -> Option<(String, String, u64)> {
    // Stub: returns nothing on non-Windows platforms.
    // Real activity tracking only happens on Windows.
    None
}

// ── Platform-specific: idle detection ────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_idle_seconds() -> u64 {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut last_input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut last_input).as_bool() {
            let current_tick = GetTickCount();
            let idle_ms = current_tick.wrapping_sub(last_input.dwTime);
            (idle_ms / 1000) as u64
        } else {
            0
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_idle_seconds() -> u64 {
    0
}
