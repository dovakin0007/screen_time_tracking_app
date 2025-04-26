use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use chrono::NaiveDate;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use db::models::AppUsageQuery;
use error::TuariError;
use fs_watcher::start_menu_watcher::ShellLinkInfo;

use crate::db::connection::DbHandler;

pub mod config;
pub mod db;
pub mod error;
pub mod fs_watcher;
pub mod logger;
pub mod platform;
pub mod system_usage;
pub mod tracker;
pub mod zero_mq_service;

#[derive(Debug)]
pub struct StartMenuStatus(AtomicBool, AtomicBool);

impl StartMenuStatus {
    pub fn new() -> Self {
        Self(AtomicBool::new(false), AtomicBool::new(false))
    }

    pub fn set_atomic_bool_one(&self, val: bool) {
        self.0.store(val, Ordering::Release);
    }

    pub fn set_atomic_bool_two(&self, val: bool) {
        self.1.store(val, Ordering::Release);
    }

    pub fn get_atomic_bools(&self) -> (bool, bool) {
        return (
            self.0.load(Ordering::Acquire),
            self.1.load(Ordering::Acquire),
        );
    }
}

#[tauri::command]
async fn fetch_app_usage_info(
    start_date: NaiveDate,
    end_date: NaiveDate,
    state: State<'_, Arc<DbHandler>>,
) -> Result<Vec<AppUsageQuery>, error::TuariError> {
    Ok(state.get_app_usage_details(start_date, end_date).await?)
}

#[tauri::command]
async fn fetch_shell_links(
    state: State<'_, Arc<DbHandler>>,
    state2: State<'_, Arc<StartMenuStatus>>,
) -> Result<Vec<ShellLinkInfo>, error::TuariError> {
    loop {
        match state2.get_atomic_bools() {
            (true, true) => break,
            _ => tokio::time::sleep(tokio::time::Duration::from_millis(1)).await,
        }
    }

    Ok(state.get_all_shell_links().await?)
}

#[tauri::command]
async fn start_app(link: &str) -> Result<(), error::TuariError> {
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", link])
        .spawn()
        .map_err(|e| error::TuariError::LaunchError(e.to_string()))?;
    _ = status;
    Ok(())
}

#[tauri::command]
async fn set_daily_limit(
    app_name: String,
    total_minutes: u32,
    should_alert: bool,
    should_close: bool,
    alert_before_close: bool,
    alert_duration: u32,
    state: State<'_, Arc<DbHandler>>,
) -> Result<String, TuariError> {
    match (should_alert, should_close) {
        (true, true) => Err(TuariError::OptionError(String::from(
            "can't set alert and close at the same time",
        ))),
        (true, false) => {
            state
                .insert_update_app_limits(
                    &app_name,
                    total_minutes,
                    should_alert,
                    false,
                    false,
                    alert_duration,
                )
                .await?;
            Ok(format!(
                "Daily limit set for {} {} minutes",
                app_name, total_minutes
            ))
        }
        (false, true) => {
            state
                .insert_update_app_limits(
                    &app_name,
                    total_minutes,
                    false,
                    should_close,
                    alert_before_close,
                    alert_duration,
                )
                .await?;
            Ok(format!(
                "Daily limit set for {} {} minutes",
                app_name, total_minutes
            ))
        }
        (false, false) => {
            state
                .insert_update_app_limits(
                    &app_name,
                    total_minutes,
                    should_alert,
                    false,
                    alert_before_close,
                    alert_duration,
                )
                .await?;
            Ok(format!("Removed Daily for {}", app_name))
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(db_handler: Arc<DbHandler>, program_watcher_status: Arc<StartMenuStatus>) {
    tauri::Builder::default()
        .any_thread()
        .plugin(tauri_plugin_store::Builder::new().build())
        .any_thread()
        .setup(|app| {
            #[cfg(desktop)]
            let quit = MenuItemBuilder::with_id("quit", "Quit Program").build(app)?;
            let hide = MenuItemBuilder::with_id("hide", "Close to tray").build(app)?;
            let show = MenuItemBuilder::with_id("show", "Show").build(app)?;
            app.handle()
                .plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    Some(vec![]),
                ))
                .unwrap();
            let autostart_manager = app.autolaunch();
            let _ = autostart_manager.enable();
            let menu = MenuBuilder::new(app)
                .items(&[&quit, &hide, &show])
                .build()?;
            TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => {
                        let window = app.get_webview_window("main").unwrap();
                        window.hide().unwrap();
                        std::process::exit(1);
                    }
                    "hide" => {
                        let window = app.get_webview_window("main").unwrap();
                        window.hide().unwrap();
                    }
                    "show" => {
                        let window = app.get_webview_window("main").unwrap();
                        window.show().unwrap();
                        window.set_focus().unwrap();
                    }
                    _ => (),
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(webview_window) = app.get_webview_window("main") {
                            let _ = webview_window.show();
                            let _ = webview_window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .manage(db_handler)
        .manage(program_watcher_status)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            fetch_app_usage_info,
            set_daily_limit,
            fetch_shell_links,
            start_app,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri app");
}
