use db::models::AppUsageQuery;
use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State,
};

pub mod config;
pub mod db;
pub mod error;
pub mod fs_watcher;
pub mod logger;
pub mod platform;
pub mod system_usage;
pub mod tracker;
pub mod zero_mq_service;

use crate::db::connection::DbHandler;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn fetch_app_usage_info(
    state: State<'_, Arc<DbHandler>>,
) -> Result<Vec<AppUsageQuery>, error::TuariError> {
    Ok(state.get_app_usage_details().await?)
}

#[tauri::command]
async fn set_daily_limit(app_name: String, total_minutes: u64) -> Result<String, String> {
    println!("Setting daily limit for app: {}", app_name);
    println!("Limit: minutes {}", total_minutes);

    Ok(format!(
        "Daily limit set for {} {} minutes",
        app_name, total_minutes
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(db_handler: Arc<DbHandler>) {
    tauri::Builder::default()
        .setup(|app| {
            let quit = MenuItemBuilder::with_id("quit", "Quit Program").build(app)?;
            let hide = MenuItemBuilder::with_id("hide", "Close to tray").build(app)?;
            let show = MenuItemBuilder::with_id("show", "Show").build(app)?;
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
        .manage(Arc::clone(&db_handler))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, fetch_app_usage_info, set_daily_limit])
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
