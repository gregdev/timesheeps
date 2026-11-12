//! Background update *checks*.
//!
//! Deliberately checks only. The previous implementation ran once at startup and,
//! if it found a release, downloaded it, installed it and restarted the app with
//! no warning. Two problems with that:
//!
//! * this app is launched once and left running for weeks — the same premise the
//!   reminder, the auto-accept sweep and the calendar sync are built on — so a
//!   single check at startup meant the update was usually never seen at all;
//! * a silent restart throws away an in-flight stopwatch, because
//!   [`crate::timer::TimerManager`] keeps its state in memory and nothing writes
//!   it down.
//!
//! So the loop runs on a long interval, reports what it found through an
//! `update-available` event and through [`UpdaterState`], and leaves installing
//! to an explicit click. `install` refuses while a timer is running unless the
//! caller has confirmed, which is why it takes `force` rather than a boolean the
//! frontend could get wrong.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Give the window time to finish coming up, and the startup burst of database
/// and activity work time to settle, before touching the network.
const INITIAL_DELAY_SECS: u64 = 60;

/// Releases are cut by CI, not continuously. Four checks a day is plenty, and the
/// request is a single small JSON document.
const CHECK_INTERVAL_SECS: u64 = 6 * 60 * 60;

/// A release the app has found but not installed.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Release notes from `latest.json`, when the publisher included any.
    pub notes: Option<String>,
}

/// The last result of a background check, so the frontend can ask on mount
/// instead of relying on having been listening when the event fired.
#[derive(Default)]
pub struct UpdaterState {
    available: Mutex<Option<UpdateInfo>>,
}

impl UpdaterState {
    pub fn new() -> Self {
        UpdaterState::default()
    }

    pub fn get(&self) -> Option<UpdateInfo> {
        self.available.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn set(&self, info: Option<UpdateInfo>) {
        if let Ok(mut guard) = self.available.lock() {
            *guard = info;
        }
    }
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(INITIAL_DELAY_SECS)).await;

        loop {
            match check(&app).await {
                Ok(Some(info)) => {
                    // Record before emitting, so a frontend that mounts in
                    // response to the event can already read it back.
                    app.state::<UpdaterState>().set(Some(info.clone()));

                    // Only emit when the answer actually changed. A check every
                    // six hours must not stack a banner on top of one the user
                    // has already dismissed.
                    let _ = app.emit("update-available", &info);
                }
                Ok(None) => {
                    // Up to date, or the release was withdrawn. Clearing this is
                    // what makes a stale banner disappear after an update ships
                    // and is installed by other means.
                    app.state::<UpdaterState>().set(None);
                }
                Err(e) => {
                    // Offline, no endpoint yet, or a malformed manifest. None of
                    // those are the user's problem to solve at 3am.
                    eprintln!("[timesheeps] update check failed: {e}");
                }
            }

            tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;
        }
    });
}

/// Ask the configured endpoint what the latest release is. Writes nothing.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;

    Ok(update.map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
    }))
}

/// Download and install the pending release, then restart.
///
/// `force` exists for exactly one reason: an in-flight stopwatch cannot survive a
/// restart, so the frontend asks before overriding that. Everything else about
/// installing is unconditional — the user pressed the button.
pub async fn install(app: &AppHandle, force: bool) -> Result<(), String> {
    if !force && timer_is_running(app) {
        // A machine-readable marker: the frontend turns this into a confirmation
        // rather than an error message, so the wording belongs there.
        return Err("TIMER_RUNNING".to_string());
    }

    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No update is available any more.".to_string())?;

    update
        .download_and_install(|_downloaded, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;

    app.restart();
}

fn timer_is_running(app: &AppHandle) -> bool {
    let manager = app.state::<crate::timer::TimerManager>();
    manager
        .state
        .lock()
        .map(|s| s.status == crate::models::TimerStatus::Running)
        .unwrap_or(false)
}
