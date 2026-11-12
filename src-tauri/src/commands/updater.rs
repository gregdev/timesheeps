use tauri::{AppHandle, Manager, State};

use crate::updater::{self, UpdateInfo, UpdaterState};

/// The result of the last background check, or `None` when there is nothing to
/// install. Lets the frontend ask on mount instead of relying on having been
/// listening when the `update-available` event fired.
#[tauri::command]
pub fn get_update_info(state: State<'_, UpdaterState>) -> Option<UpdateInfo> {
    state.get()
}

/// Check now rather than waiting for the next background pass.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let result = updater::check(&app).await?;
    app.state::<UpdaterState>().set(result.clone());
    Ok(result)
}

/// Download, install and restart.
///
/// Returns `TIMER_RUNNING` when a stopwatch is going, so the frontend can ask
/// before throwing it away; pass `force` to go ahead anyway. On success this
/// never returns — the process is replaced.
#[tauri::command]
pub async fn install_update(app: AppHandle, force: bool) -> Result<(), String> {
    updater::install(&app, force).await
}
