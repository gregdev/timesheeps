mod activity;
mod appicon;
mod autoaccept;
mod calendar;
mod commands;
// `db`, `matcher` and `models` are public so the `timesheeps-mcp` binary (a
// separate crate target) can reuse the real merge/match pipeline instead of
// reimplementing it in SQL. Note a bin target cannot see `pub(crate)` items.
pub mod db;
pub mod matcher;
pub mod models;
pub mod reporting;
mod nl_query;
mod reminder;
mod timer;
mod updater;
mod watchdog;

use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, PhysicalPosition, Size, LogicalSize,
};
use tauri_plugin_autostart::MacosLauncher;

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
}

/// Route panics to the log.
///
/// A Windows GUI build has no console, so a panic used to disappear without a
/// trace: no WER report (that covers native faults only), no stderr, nothing at
/// all. On 2026-11-18 the app started from the Run key, created its WebView2
/// profile and then exited, and there was no way to tell why. The logging
/// plugin is initialised before `setup` runs, so `log::error!` from here always
/// reaches the log file.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        previous(info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let start_hidden = std::env::args().any(|a| a == "--hidden");

    tauri::Builder::default()
        // **Must be first.** The watchdog task re-launches this executable every
        // 15 minutes; each of those invocations has to hand its arguments to the
        // running copy and exit, so that ticking costs one short-lived process
        // and nothing else.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // `--hidden` is how autostart and the watchdog launch us. The user
            // did not ask for a window in that case, so do not steal focus from
            // whatever they are actually doing.
            if argv.iter().any(|a| a == "--hidden") {
                log::debug!("second instance launched with --hidden; leaving the window alone");
                return;
            }
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                // 5 MB x 5 keeps a couple of weeks of a long-running app without
                // letting the folder grow without bound.
                .max_file_size(5 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(5))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("timesheeps".to_string()),
                    },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let handle = app.handle().clone();

            install_panic_hook();
            log::info!(
                "Timesheeps {} starting (pid {}, args {:?})",
                app.package_info().version,
                std::process::id(),
                std::env::args().skip(1).collect::<Vec<_>>()
            );
            if let Ok(dir) = app.path().app_log_dir() {
                log::info!("logs: {}", dir.display());
            }

            // Initialise the database
            let conn = db::open(&handle)?;

            // Sync autostart state from saved settings
            if let Ok(settings) = db::get_settings(&conn) {
                use tauri_plugin_autostart::ManagerExt;
                let autolaunch = app.handle().autolaunch();
                if settings.start_on_login {
                    let _ = autolaunch.enable();
                } else {
                    let _ = autolaunch.disable();
                }

                // A `Run` key is evaluated once per logon and never retried, so
                // it cannot recover from a launch that died seconds later. The
                // scheduled task is the retry. `schtasks` is a blocking spawn,
                // so keep it off the startup path.
                let watchdog_handle = handle.clone();
                let watchdog_enabled = settings.start_on_login;
                tauri::async_runtime::spawn_blocking(move || {
                    watchdog::sync(&watchdog_handle, watchdog_enabled);
                });
            }

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
            });

            // Initialise timer state
            app.manage(timer::TimerManager::new());

            // Last known update state, so the frontend can read it on mount
            app.manage(updater::UpdaterState::new());

            // Set up system tray
            let show = MenuItem::with_id(app, "show", "Show Timesheeps", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Timesheeps")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        // Restore main window to full size
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.set_size(Size::Logical(LogicalSize::new(1100.0, 780.0)));
                            let _ = win.center();
                            // Navigate back to timeline
                            let _ = win.eval("window.location.replace('/')");
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        position,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            // Navigate to timer popup view
                            let _ = win.eval("window.location.replace('/timer-popup')");
                            // Resize to compact popup size (tall enough for the
                            // "Open full app" footer button)
                            let _ = win.set_size(Size::Logical(LogicalSize::new(280.0, 364.0)));
                            // Position near the click (tray icon location)
                            let x = (position.x - 280.0).max(0.0);
                            let y = (position.y - 364.0 - 40.0).max(0.0);
                            let _ = win.set_position(PhysicalPosition::new(x, y));
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Hide window on autostart launch
            if start_hidden {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
            }

            // Start activity polling in a background task
            let polling_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                activity::start_polling(polling_handle).await;
            });

            // Start the timer ticker (taskbar progress + frontend events)
            timer::start_ticker(handle.clone());

            // Daily timesheet reminder (fires once per local day, when enabled)
            reminder::start(handle.clone());

            // Auto-accept suggestions for the recent past, independent of which
            // day the UI happens to be showing.
            autoaccept::start(handle.clone());

            // Keep the local calendar cache warm, so period reviews and the MCP
            // can see meetings without a network call.
            calendar::start_sync(handle.clone());

            // Check for updates periodically, and *only* check: installing is a
            // deliberate action. The previous version downloaded and restarted
            // unprompted, once, at startup — so in practice it either never ran
            // or discarded an in-flight stopwatch.
            updater::start(handle.clone());

            // Claude runs a staged copy of the sidecar, not the installed one, so
            // the installer can replace the latter while Claude is open. Repoint
            // an existing config at this build's copy.
            commands::settings::prune_staged_mcp_binaries(&handle);
            commands::settings::refresh_claude_mcp_if_configured(&handle);

            Ok(())
        })
        // Intercept close → minimise to tray instead of quitting (main window only)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::activity::get_activity_for_day,
            commands::activity::get_window_summary_for_day,
            commands::activity::search,
            commands::activity::delete_activity_block,
            commands::activity::delete_activity_by_app_title,
            commands::calendar::start_m365_login,
            commands::calendar::get_m365_status,
            commands::calendar::get_calendar_events,
            commands::calendar::refresh_calendar,
            commands::calendar::disconnect_m365,
            commands::projects::get_projects,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::archive_project,
            commands::projects::unarchive_project,
            commands::projects::delete_project,
            commands::time_entries::get_time_entries_for_day,
            commands::time_entries::create_time_entry,
            commands::time_entries::update_time_entry,
            commands::time_entries::delete_time_entry,
            commands::day_notes::get_day_note,
            commands::day_notes::set_day_note,
            commands::review::get_period_review,
            commands::review::accept_suggestions_for_day,
            commands::review::set_meeting_project,
            commands::review::hide_meeting,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::check_claude_mcp,
            commands::settings::setup_claude_mcp,
            commands::filter_rules::get_filter_rules,
            commands::filter_rules::create_filter_rule,
            commands::filter_rules::delete_filter_rule,
            commands::project_match_rules::get_project_match_rules,
            commands::project_match_rules::create_project_match_rule,
            commands::project_match_rules::update_project_match_rule,
            commands::project_match_rules::delete_project_match_rule,
            commands::project_match_rules::reorder_project_match_rules,
            commands::project_match_rules::get_suggested_entries_for_day,
            commands::project_match_rules::get_rule_stats,
            commands::project_match_rules::get_known_apps,
            commands::apps::get_app_icon,
            commands::permissions::check_screen_recording_permission,
            commands::permissions::request_screen_recording_permission,
            commands::timer::start_timer,
            commands::timer::pause_timer,
            commands::timer::resume_timer,
            commands::timer::stop_timer,
            commands::timer::get_timer_state,
            commands::window::hide_main_window,
            commands::window::show_main_window,
            commands::updater::get_update_info,
            commands::updater::check_for_updates,
            commands::updater::install_update,
        ])
        .run(tauri::generate_context!())
        // Logged rather than `.expect`ed: this is one of the paths that used to
        // exit silently, and a failed `setup` (above) surfaces here.
        .unwrap_or_else(|e| {
            log::error!("fatal: Timesheeps exited: {e}");
            std::process::exit(1);
        });
}
