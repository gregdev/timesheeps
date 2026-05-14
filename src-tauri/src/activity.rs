//! Background task that polls the active window every 5 seconds.
//! On Windows: uses Win32 APIs via the `windows` crate.
//! On other platforms: no-op (stub for development on Linux/macOS).

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

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
    /// The window/app that is currently active (in progress, not yet written to DB).
    current_app: String,
    current_title: String,
    session_started: chrono::DateTime<chrono::Utc>,
    is_idle: bool,
    idle_started: Option<chrono::DateTime<chrono::Utc>>,
}

impl PollerState {
    fn new() -> Self {
        PollerState {
            current_app: String::new(),
            current_title: String::new(),
            session_started: chrono::Utc::now(),
            is_idle: false,
            idle_started: None,
        }
    }
}

const POLL_INTERVAL_SECS: u64 = 5;

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

fn handle_active(app: &AppHandle, state: &Arc<Mutex<PollerState>>, settings: &Settings) {
    let Some((new_app, new_title)) = get_foreground_window_info() else {
        return;
    };

    let mut s = state.lock().unwrap();
    let now = chrono::Utc::now();

    // If we were idle, fire idle-return event
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
        // Start fresh session with current window
        s.current_app = new_app;
        s.current_title = new_title;
        s.session_started = now;
        return;
    }

    // Window changed: flush old session to DB, start new one
    if new_app != s.current_app || new_title != s.current_title {
        let duration = (now - s.session_started).num_seconds();
        // Only persist if longer than poll interval (avoids tiny blips)
        if duration >= POLL_INTERVAL_SECS as i64 && !s.current_app.is_empty() {
            flush_session(app, &s.current_app, &s.current_title, &s.session_started, &now);
        }
        s.current_app = new_app;
        s.current_title = new_title;
        s.session_started = now;
    }
    // else: same window, session continues — nothing to do until it changes
}

fn handle_idle(
    app: &AppHandle,
    state: &Arc<Mutex<PollerState>>,
    _settings: &Settings,
    idle_secs: u64,
) {
    let mut s = state.lock().unwrap();
    if s.is_idle {
        return; // already in idle state
    }
    let now = chrono::Utc::now();
    let idle_started = now - chrono::Duration::seconds(idle_secs as i64);

    // Flush the active session up to where idle started
    if !s.current_app.is_empty() {
        let active_duration = (idle_started - s.session_started).num_seconds();
        if active_duration >= POLL_INTERVAL_SECS as i64 {
            flush_session(app, &s.current_app, &s.current_title, &s.session_started, &idle_started);
        }
    }

    s.is_idle = true;
    s.idle_started = Some(idle_started);
    s.current_app = String::new();
    s.current_title = String::new();
}

fn flush_session(
    app: &AppHandle,
    app_name: &str,
    window_title: &str,
    started_at: &chrono::DateTime<chrono::Utc>,
    ended_at: &chrono::DateTime<chrono::Utc>,
) {
    let db_state = app.state::<crate::AppState>();
    let db_guard = db_state.db.lock().unwrap();
    let _ = db::insert_activity(&db_guard, app_name, window_title, started_at, ended_at);
}

// ── Platform-specific: get foreground window ──────────────────────────────────

#[cfg(target_os = "windows")]
fn get_foreground_window_info() -> Option<(String, String)> {
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
            return Some((String::from("unknown"), title));
        }

        let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return Some((String::from("unknown"), title)),
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

        Some((app_name, title))
    }
}

#[cfg(not(target_os = "windows"))]
fn get_foreground_window_info() -> Option<(String, String)> {
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
