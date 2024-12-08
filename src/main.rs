// #![windows_subsystem = "windows"]
use std::time;
use std::time::Instant;

use db::connection::{upsert_app_usages, upsert_apps};
use db::models::{NewApp, NewAppUsage};
use diesel::SqliteConnection;
use platform::Platform;
use spin_sleep::SpinSleeper;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use winreg::enums::*;
use winreg::RegKey;

mod db;
mod platform;
mod rpc_service;
use platform::windows::{self, *};

pub async fn app(pg_conn: Arc<Mutex<SqliteConnection>>, session_id: String) {
    loop {
        let start = Instant::now();
        let (new_apps, new_app_usages): (Vec<NewApp>, Vec<NewAppUsage>) =
            windows::WindowsHandle::get_window_titles()
                .into_iter()
                .map(|details| {
                    // Get idle time
                    let idle_time = WindowsHandle::get_last_input_info()
                        .unwrap_or_default()
                        .as_secs();

                    // Determine the window activity
                    let window_activity = if idle_time >= 300 {
                        "Idle Time".to_string()
                    } else {
                        details.window_title.clone()
                    };

                    // Create NewApp
                    let new_app = NewApp {
                        app_name: details
                            .app_name
                            .clone()
                            .unwrap_or_else(|| "Unknown App".to_string()),
                        app_path: details.app_path.clone(),
                    };

                    // Create NewAppUsage
                    let new_app_usage = NewAppUsage {
                        session_id: session_id.to_string(), // Ensure session_id is available in scope
                        app_name: details
                            .app_name
                            .clone()
                            .unwrap_or_else(|| "Unknown App".to_string()),
                        screen_title_name: window_activity,
                        duration_in_seconds: 1, // Initialize with 1 second
                        is_active: if details.is_active { 1 } else { 0 },
                    };

                    (new_app, new_app_usage)
                })
                .unzip(); // Unzip into separate vectors for NewApp and NewAppUsage

        upsert_apps(pg_conn.clone(), new_apps).await;
        upsert_app_usages(pg_conn.clone(), new_app_usages).await;
        let duration = start.elapsed();
        println!("Time elapsed in expensive_function() is: {:?}", duration);
        let time_delay_for_function = 1000 - duration.as_millis();
        let sleep_duration =
            time::Duration::from_millis(time_delay_for_function.try_into().unwrap_or(1000));
        let sleeper = SpinSleeper::new(1_000_000);
        sleeper.sleep(sleep_duration);
    }
}

#[tokio::main]
async fn main() {
    if cfg!(target_os = "windows") {
        let exe_path = env::current_exe().expect("Failed to get current executable path");
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                KEY_WRITE,
            )
            .expect("Failed to open registry key");
        run_key
            .set_value("screen-time-tracker", &exe_path.to_str().unwrap())
            .expect("Failed to set registry value");
        let pg_conn = db::connection::connect();
        let postgres_diesel = Arc::new(Mutex::new(pg_conn));
        let session_id = Uuid::new_v4().to_string();
        tokio::spawn(app(postgres_diesel.clone(), session_id))
            .await
            .unwrap();
    } else {
        todo!()
    }
}
