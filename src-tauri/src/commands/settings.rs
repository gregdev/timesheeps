use tauri::{AppHandle, Manager, State};
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
///
/// No `externalBin` or `bundle.resources` entry is needed: Tauri bundles the
/// extra `[[bin]]` target automatically, placing it beside the main executable.
/// On Windows `resource_dir()` *is* the executable's directory, so the first
/// branch below is the one that normally hits in an installed build; the second
/// covers `tauri dev`, where both binaries land in `target/debug/`.
fn find_mcp_binary(app: &AppHandle) -> Option<std::path::PathBuf> {
    let name = mcp_file_name();

    // Production: Tauri copies bundle resources (and the sidecar) to the resource dir
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

fn mcp_file_name() -> &'static str {
    if cfg!(windows) {
        "timesheeps-mcp.exe"
    } else {
        "timesheeps-mcp"
    }
}

/// Stage a runnable **copy** of the sidecar, one directory per app version, and
/// return the path to it.
///
/// Windows locks the image of a running executable. `timesheeps-mcp.exe` sits
/// beside `timesheeps.exe` and the installer overwrites it on every upgrade — so
/// while Claude Desktop has a process open on that exact file, the installer
/// cannot replace it and the install fails until Claude is killed by hand.
///
/// Running a per-version copy breaks that alias. An upgrade writes a fresh
/// `timesheeps-mcp.exe` that no process has mapped, and the copy Claude is still
/// holding belongs to the *previous* version, which stops being referenced as
/// soon as its config entry is rewritten. The stale directory is pruned later,
/// once Claude has restarted and released the file.
///
/// It also fixes the developer case: `cargo build` cannot overwrite
/// `target/debug/timesheeps-mcp.exe` while Claude runs it, so a staged copy means
/// rebuilds stop failing too.
pub fn stage_mcp_binary(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let source = find_mcp_binary(app).ok_or_else(|| {
        "Cannot find timesheeps-mcp binary. Build the project first with `pnpm tauri dev` or `pnpm tauri build`.".to_string()
    })?;

    let version = app.package_info().version.to_string();
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Cannot resolve the app data directory: {e}"))?
        .join("mcp")
        .join(&version);

    let dest = dir.join(mcp_file_name());

    // Already staged for this version: nothing to do. Checking the size as well
    // as existence catches a truncated copy from an interrupted upgrade.
    if let (Ok(meta_src), Ok(meta_dst)) = (std::fs::metadata(&source), std::fs::metadata(&dest)) {
        if meta_src.len() == meta_dst.len() && meta_dst.len() > 0 {
            return Ok(dest);
        }
    }

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create {}: {}", dir.display(), e))?;
    std::fs::copy(&source, &dest).map_err(|e| {
        format!(
            "Cannot stage the MCP server to {}: {}",
            dest.display(),
            e
        )
    })?;

    Ok(dest)
}

/// Delete staged copies of other versions.
///
/// Best-effort by design: a copy Claude is still running cannot be deleted, and
/// that is expected — it is cleaned up on whichever launch follows Claude's next
/// restart. Failures are therefore ignored rather than reported.
pub fn prune_staged_mcp_binaries(app: &AppHandle) {
    let Ok(base) = app.path().app_local_data_dir() else {
        return;
    };
    let base = base.join("mcp");
    let current = app.package_info().version.to_string();

    let Ok(entries) = std::fs::read_dir(&base) else {
        return;
    };

    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy() == current {
            continue;
        }
        if entry.path().is_dir() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
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
    // Stage rather than pointing at the installed binary: see `stage_mcp_binary`
    // for why Claude must not hold the file the installer overwrites.
    let binary = stage_mcp_binary(&app)?;
    let binary_path = binary.to_string_lossy().to_string();

    let dirs = find_claude_config_dirs();
    for dir in &dirs {
        write_claude_config(dir, &binary_path)?;
    }

    Ok(())
}

/// Repoint an existing Claude config at this version's staged binary.
///
/// Called on every launch. Without it the config would keep naming the version
/// that was current when the user last pressed "Set up Claude MCP", so an upgrade
/// would leave Claude running a stale server — which then refuses every call once
/// the database schema moves ahead of it.
///
/// Deliberately conservative: only an entry that already names a
/// `timesheeps-mcp` binary is touched. A config the user pointed at something
/// else, or removed, is left exactly as it is.
pub fn refresh_claude_mcp_if_configured(app: &AppHandle) {
    let Ok(binary) = stage_mcp_binary(app) else {
        return;
    };
    let binary_path = binary.to_string_lossy().to_string();

    for dir in find_claude_config_dirs() {
        let config_path = dir.join("claude_desktop_config.json");
        let Ok(content) = std::fs::read_to_string(&config_path) else {
            continue;
        };
        let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };

        let Some(command) = config["mcpServers"]["timesheeps"]["command"].as_str() else {
            continue;
        };

        if !command.contains("timesheeps-mcp") || command == binary_path {
            continue;
        }

        if let Err(e) = write_claude_config(&dir, &binary_path) {
            eprintln!("[timesheeps] could not update the Claude MCP config: {e}");
        }
    }
}
