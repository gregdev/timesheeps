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

/// Returns all candidate Claude Desktop config paths that exist on this machine.
/// Covers both the traditional install (%APPDATA%\Claude) and the Microsoft
/// Store sandboxed install (%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude).
/// If neither directory exists yet, falls back to the traditional path (creates it).
fn find_claude_config_dirs() -> Vec<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();

        // Traditional install
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = std::path::PathBuf::from(appdata).join("Claude");
            if p.exists() {
                dirs.push(p);
            }
        }

        // Store install: %LOCALAPPDATA%\Packages\Claude_<publisher-id>\LocalCache\Roaming\Claude
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let packages = std::path::PathBuf::from(localappdata).join("Packages");
            if let Ok(entries) = std::fs::read_dir(&packages) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("Claude_") {
                        let p = entry.path().join("LocalCache").join("Roaming").join("Claude");
                        if p.exists() {
                            dirs.push(p);
                        }
                    }
                }
            }
        }

        // Fallback: create the traditional path if nothing was found
        if dirs.is_empty() {
            if let Ok(appdata) = std::env::var("APPDATA") {
                dirs.push(std::path::PathBuf::from(appdata).join("Claude"));
            }
        }

        dirs
    }

    #[cfg(target_os = "macos")]
    {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();

        if let Ok(home) = std::env::var("HOME") {
            let p = std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Claude");
            // Push whether or not it exists — write_claude_config will create it
            dirs.push(p);
        }

        dirs
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Vec::new()
    }
}

fn write_claude_config(dir: &std::path::Path, binary_path: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;

    let config_path = dir.join("claude_desktop_config.json");
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Cannot read {}: {}", config_path.display(), e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if config.get("mcpServers").is_none() {
        config["mcpServers"] = serde_json::json!({});
    }
    config["mcpServers"]["timesheeps"] = serde_json::json!({ "command": binary_path });

    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize config: {}", e))?;
    std::fs::write(&config_path, json)
        .map_err(|e| format!("Cannot write {}: {}", config_path.display(), e))?;

    Ok(())
}

#[tauri::command]
pub fn check_claude_mcp() -> bool {
    for dir in find_claude_config_dirs() {
        let config_path = dir.join("claude_desktop_config.json");
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                if config["mcpServers"]["timesheeps"].is_object() {
                    return true;
                }
            }
        }
    }
    false
}

#[tauri::command]
pub fn setup_claude_mcp(app: AppHandle) -> Result<(), String> {
    let binary = find_mcp_binary(&app).ok_or_else(|| {
        "Cannot find timesheeps-mcp binary. Build the project first with `pnpm tauri dev` or `pnpm tauri build`.".to_string()
    })?;

    let binary_path = binary.to_string_lossy().to_string();

    let dirs = find_claude_config_dirs();
    for dir in &dirs {
        write_claude_config(dir, &binary_path)?;
    }

    Ok(())
}
