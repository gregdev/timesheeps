mod activity;
mod commands;
mod db;
mod models;

use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_autostart::MacosLauncher;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
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
                        if let Some(win) = app.get_webview_window("main") {
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
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Start activity polling in a background task
            tauri::async_runtime::spawn(async move {
                activity::start_polling(handle).await;
            });

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
        // Intercept close → minimise to tray instead of quitting
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::activity::get_activity_for_day,
            commands::activity::get_window_summary_for_day,
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
            commands::filter_rules::get_filter_rules,
            commands::filter_rules::create_filter_rule,
            commands::filter_rules::delete_filter_rule,
            commands::project_match_rules::get_project_match_rules,
            commands::project_match_rules::create_project_match_rule,
            commands::project_match_rules::delete_project_match_rule,
            commands::project_match_rules::get_suggested_entries_for_day,
        ])
        .run(tauri::generate_context!())
        .expect("error while running timesheeps");
}
