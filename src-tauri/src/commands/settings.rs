use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::db;
use crate::models::Settings;
use crate::AppState;

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<Settings, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_settings(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(
    settings: Settings,
    state: State<AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::save_settings(&conn, &settings).map_err(|e| e.to_string())?;

    let autolaunch = app.autolaunch();
    if settings.start_on_login {
        autolaunch.enable().map_err(|e| e.to_string())?;
    } else {
        autolaunch.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Finds the compiled `timesheeps-mcp` binary.
/// In production it lives in the Tauri resource dir (bundle.resources).
/// In dev mode both executables land in target/debug/ from the same `cargo build`.
fn find_mcp_binary(app: &AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;

    let name = if cfg!(windows) { "timesheeps-mcp.exe" } else { "timesheeps-mcp" };

    // Production: Tauri copies bundle.resources files to the resource dir
    if let Ok(resource_dir) = app.path().resource_dir() {
        let p = resource_dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }

    // Dev mode: both timesheeps.exe and timesheeps-mcp.exe land in target/debug/
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

#[tauri::command]
pub fn setup_claude_mcp(app: AppHandle) -> Result<(), String> {
    let binary = find_mcp_binary(&app).ok_or_else(|| {
        "Cannot find timesheeps-mcp binary. Build the project first with `pnpm tauri dev` or `pnpm tauri build`.".to_string()
    })?;

    let binary_path = binary.to_string_lossy().to_string();

    // Read existing Claude Desktop config or start fresh
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "APPDATA environment variable not set".to_string())?;
    let claude_dir = std::path::Path::new(&appdata).join("Claude");
    std::fs::create_dir_all(&claude_dir)
        .map_err(|e| format!("Cannot create Claude config dir: {}", e))?;

    let config_path = claude_dir.join("claude_desktop_config.json");
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Cannot read existing config: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if config.get("mcpServers").is_none() {
        config["mcpServers"] = serde_json::json!({});
    }

    config["mcpServers"]["timesheeps"] = serde_json::json!({
        "command": binary_path
    });

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize config: {}", e))?;
    std::fs::write(&config_path, json)
        .map_err(|e| format!("Cannot write Claude config: {}", e))?;

    Ok(())
}
