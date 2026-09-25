use tauri::{LogicalSize, Manager, Size};

#[tauri::command]
pub async fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    win.hide().map_err(|e| e.to_string())
}

/// Restore the main window from the compact tray popup into the full-size app view.
#[tauri::command]
pub async fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    win.set_size(Size::Logical(LogicalSize::new(1100.0, 780.0)))
        .map_err(|e| e.to_string())?;
    win.center().map_err(|e| e.to_string())?;
    // Leave the /timer-popup route for the normal app shell
    win.eval("window.location.replace('/')")
        .map_err(|e| e.to_string())?;
    win.show().map_err(|e| e.to_string())?;
    win.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}
