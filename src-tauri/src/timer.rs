use std::sync::Mutex;

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager};

use crate::models::{TimerState, TimerStatus};

/// Shared timer state managed by the app.
pub struct TimerManager {
    pub state: Mutex<TimerState>,
}

impl TimerManager {
    pub fn new() -> Self {
        TimerManager {
            state: Mutex::new(TimerState::default()),
        }
    }

    /// Start (or restart) the timer for a given project.
    pub fn start(
        &self,
        project_id: i64,
        project_name: Option<String>,
        project_color: Option<String>,
        note: String,
    ) -> TimerState {
        let mut s = self.state.lock().unwrap();
        s.status = TimerStatus::Running;
        s.project_id = Some(project_id);
        s.project_name = project_name;
        s.project_color = project_color;
        s.note = note;
        s.started_at = Some(Utc::now());
        s.accumulated_ms = 0;
        s.paused_at = None;
        s.elapsed_ms = 0;
        s.clone()
    }

    /// Pause the running timer (preserves accumulated time).
    pub fn pause(&self) -> Option<TimerState> {
        let mut s = self.state.lock().unwrap();
        if s.status != TimerStatus::Running {
            return None;
        }
        let now = Utc::now();
        s.accumulated_ms += now
            .signed_duration_since(s.paused_at.unwrap_or(s.started_at.unwrap_or(now)))
            .num_milliseconds()
            .max(0);
        s.elapsed_ms = s.accumulated_ms;
        s.status = TimerStatus::Paused;
        s.paused_at = Some(now);
        Some(s.clone())
    }

    /// Resume a paused timer.
    pub fn resume(&self) -> Option<TimerState> {
        let mut s = self.state.lock().unwrap();
        if s.status != TimerStatus::Paused {
            return None;
        }
        s.status = TimerStatus::Running;
        s.paused_at = Some(Utc::now()); // reset the "last tick" reference
        Some(s.clone())
    }

    /// Stop the timer and return the final state (for creating a time entry).
    pub fn stop(&self) -> Option<TimerState> {
        let mut s = self.state.lock().unwrap();
        if s.status == TimerStatus::Stopped {
            return None;
        }
        // Finalize elapsed
        let now = Utc::now();
        if s.status == TimerStatus::Running {
            s.accumulated_ms += now
                .signed_duration_since(s.paused_at.unwrap_or(s.started_at.unwrap_or(now)))
                .num_milliseconds()
                .max(0);
        }
        s.elapsed_ms = s.accumulated_ms;
        s.status = TimerStatus::Stopped;
        s.paused_at = None;
        let result = s.clone();
        // Reset accumulated for next use
        s.accumulated_ms = 0;
        s.project_id = None;
        s.project_name = None;
        s.project_color = None;
        s.note = String::new();
        s.started_at = None;
        s.elapsed_ms = 0;
        Some(result)
    }

    /// Get the current timer state with up-to-date elapsed time.
    pub fn get_state(&self) -> TimerState {
        let s = self.state.lock().unwrap();
        let mut state = s.clone();
        if s.status == TimerStatus::Running {
            let now = Utc::now();
            let since = s.paused_at.unwrap_or(s.started_at.unwrap_or(now));
            let extra = now
                .signed_duration_since(since)
                .num_milliseconds()
                .max(0);
            state.elapsed_ms = s.accumulated_ms + extra;
        }
        state
    }
}

/// Spawn a background task that emits `timer:tick` events to the frontend
/// and updates the Windows taskbar progress bar every second while the timer is running.
pub fn start_ticker(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));

            let timer = match app.try_state::<TimerManager>() {
                Some(t) => t,
                None => continue,
            };

            let state = timer.get_state();
            let _ = app.emit("timer:tick", &state);

            // Update Windows taskbar progress
            #[cfg(target_os = "windows")]
            update_taskbar(&app, &state);
        }
    });
}

#[cfg(target_os = "windows")]
fn update_taskbar(app: &AppHandle, state: &TimerState) {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
    use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList, TBPF_NORMAL, TBPF_NOPROGRESS, TBPF_PAUSED};

    // Get the HWND of the main window
    let hwnd = match app.get_webview_window("main") {
        Some(win) => {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match win.window_handle() {
                Ok(wh) => match wh.as_raw() {
                    RawWindowHandle::Win32(h) => {
                        windows::Win32::Foundation::HWND(h.hwnd.get() as *mut _)
                    }
                    _ => return,
                },
                Err(_) => return,
            }
        }
        None => return,
    };

    // Create ITaskbarList3 COM instance
    let Ok(taskbar) = (unsafe {
        CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_ALL)
    }) else {
        return;
    };

    match state.status {
        TimerStatus::Stopped => {
            let _ = unsafe { taskbar.SetProgressState(hwnd, TBPF_NOPROGRESS) };
        }
        TimerStatus::Running => {
            let _ = unsafe { taskbar.SetProgressState(hwnd, TBPF_NORMAL) };
            // Show elapsed time as progress. Use 8 hours (28800000 ms) as a full "day" reference.
            let total = 28_800_000u64;
            let elapsed = (state.elapsed_ms as u64).min(total);
            let _ = unsafe { taskbar.SetProgressValue(hwnd, elapsed, total) };
        }
        TimerStatus::Paused => {
            let _ = unsafe { taskbar.SetProgressState(hwnd, TBPF_PAUSED) };
            let total = 28_800_000u64;
            let elapsed = (state.elapsed_ms as u64).min(total);
            let _ = unsafe { taskbar.SetProgressValue(hwnd, elapsed, total) };
        }
    }
}
