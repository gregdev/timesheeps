mod activity;
mod commands;
mod db;
mod models;
mod nl_query;
mod timer;

use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, PhysicalPosition, Size, LogicalSize,
};
use tauri_plugin_autostart::MacosLauncher;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let start_hidden = std::env::args().any(|a| a == "--hidden");

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let handle = app.handle().clone();

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
            }

            app.manage(AppState {
                db: Mutex::new(conn),
            });

            // Initialise timer state
            app.manage(timer::TimerManager::new());

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
                            // Resize to compact popup size
                            let _ = win.set_size(Size::Logical(LogicalSize::new(280.0, 320.0)));
                            // Position near the click (tray icon location)
                            let x = (position.x - 280.0).max(0.0);
                            let y = (position.y - 320.0 - 40.0).max(0.0);
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

            // Check for updates silently in the background
            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use tauri_plugin_updater::UpdaterExt;
                if let Ok(updater) = update_handle.updater() {
                    if let Ok(Some(update)) = updater.check().await {
                        let _ = update
                            .download_and_install(|_downloaded, _total| {}, || {})
                            .await;
                        update_handle.restart();
                    }
                }
            });

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
            commands::projects::get_projects,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::archive_project,
            commands::time_entries::get_time_entries_for_day,
            commands::time_entries::create_time_entry,
            commands::time_entries::update_time_entry,
            commands::time_entries::delete_time_entry,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::check_claude_mcp,
            commands::settings::setup_claude_mcp,
            commands::filter_rules::get_filter_rules,
            commands::filter_rules::create_filter_rule,
            commands::filter_rules::delete_filter_rule,
            commands::project_match_rules::get_project_match_rules,
            commands::project_match_rules::create_project_match_rule,
            commands::project_match_rules::delete_project_match_rule,
            commands::project_match_rules::get_suggested_entries_for_day,
            commands::permissions::check_screen_recording_permission,
            commands::permissions::request_screen_recording_permission,
            commands::timer::start_timer,
            commands::timer::pause_timer,
            commands::timer::resume_timer,
            commands::timer::stop_timer,
            commands::timer::get_timer_state,
            commands::window::hide_main_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running timesheeps");
}
