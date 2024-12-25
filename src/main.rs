use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::time;
use std::time::Instant;

use chrono::Local;
use db::connection::upset_app_usage;
use db::models::{App, AppUsage};
use dirs;
use env_logger::Builder;
use dotenvy::dotenv;
use platform::Platform;
use platform::WindowDetails;
use rusqlite::Connection;
use std::env;
use tokio::sync::mpsc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::Mutex;
use uuid::Uuid;
use std::io::Write;
use log::{error, info};

mod db;
mod platform;
use platform::windows::{self, WindowsHandle};

type Sender = mpsc::UnboundedSender<(HashMap<String, App>, HashMap<String, AppUsage>)>;

async fn track_application_usage(
    session_id: String,
    tx: Sender,
    mut ctrl_c_recv: tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    let mut previous_app_map: HashMap<String, App> = HashMap::new();
    let mut previous_app_usage_map: HashMap<String, AppUsage> = HashMap::new();

    loop {
        tokio::select! {
            Some(_) = ctrl_c_recv.recv() => {
                if let Err(err) = tx.send((previous_app_map.clone(), previous_app_usage_map.clone())) {
                    error!("Failed to send data on shutdown: {:?}", err);
                }
                info!("Shutdown signal received. Exiting tracking loop.");
                break;
            },
            _ = async {
                let start = Instant::now();

                let window_state = windows::WindowsHandle::get_window_titles();
                let idle_time_secs = WindowsHandle::get_last_input_info()
                    .unwrap_or_default()
                    .as_secs();
                let idle_state = idle_time_secs >= 300;

                let mut modified_window_state = window_state.clone();

                if idle_state {
                    if let Some(first_entry) = modified_window_state.first_entry() {
                        let value = first_entry.get().clone();
                        let mut key = String::from("Idle Time");
                        key.push_str(&value.app_name.clone().unwrap_or("Unknown app".to_owned()));
                        modified_window_state.insert(
                            key,
                            WindowDetails {
                                window_title: "Idle".to_owned(),
                                app_name: value.app_name.clone(),
                                app_path: value.app_path.clone(),
                                is_active: false,
                            },
                        );
                    }
                }

                let before_retain_count = previous_app_usage_map.len();

                for (_, value) in modified_window_state.clone().into_iter() {
                    let app_name = value
                        .app_name
                        .clone()
                        .unwrap_or_else(|| "Unknown App".to_string());
                    let app_path = value
                        .app_path
                        .clone()
                        .unwrap_or_else(|| "Unknown Path".to_string());

                    let app = App {
                        name: app_name.clone(),
                        path: app_path,
                    };

                    let current_time = Local::now().naive_utc();
                    let entry = previous_app_usage_map.entry(value.window_title.clone());
                    match entry {
                        std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
                            let app_usage = occupied_entry.get_mut();

                            app_usage.last_updated_time = current_time;
                        }
                        std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                            let app_usage = AppUsage {
                                session_id: session_id.clone(),
                                app_id: Uuid::new_v4().to_string(),
                                application_name: app_name.clone(),
                                current_screen_title: value.window_title.clone(),
                                start_time: current_time,
                                last_updated_time: current_time,
                            };
                            vacant_entry.insert(app_usage);
                            previous_app_map.insert(app_name.clone(), app);
                        }
                    }
                }

                previous_app_usage_map.retain(|key, _| modified_window_state.contains_key(key));
                let after_retain_count = modified_window_state.len();
                if before_retain_count != after_retain_count {
                    if let Err(err) = tx
                        .send((previous_app_map.clone(), previous_app_usage_map.clone()))
                    {
                        error!("Failed to send data: {:?}", err);
                        return;
                    }
                }

                let duration = start.elapsed();
                info!("Time elapsed in tracking loop: {:?}", duration);
                let time_delay_for_function = 1000 - duration.as_millis();
                let sleep_duration =
                    time::Duration::from_millis(time_delay_for_function.try_into().unwrap_or(1000));
                tokio::time::sleep(sleep_duration).await;
            } => {}
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    if cfg!(target_os = "windows") {
        let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        // Determine database path
        let db_path = if db_url.contains("%AppData%") {
            let app_data_path = dirs::config_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
            db_url.replace("%AppData%", app_data_path.to_str().unwrap())
        } else {
            db_url
        };
        let db_path = Path::new(&db_path);
        let db_dir = db_path.parent().unwrap_or_else(|| Path::new("."));

        // Configure logger
        let log_file_path = db_dir.join("application.log");

        // Conditional logging based on build mode
        #[cfg(debug_assertions)]
        {
            env_logger::Builder::from_default_env()
                .format(|buf, record| {
                    writeln!(
                        buf,
                        "{} [{}] - {}",
                        Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.level(),
                        record.args()
                    )
                })
                .filter(None, log::LevelFilter::Debug) // Debug-level logging for development
                .init();
            info!("Debug mode: Logger initialized to log to console.");
        }

        #[cfg(not(debug_assertions))]
        {
            let log_file = File::create(&log_file_path).unwrap_or_else(|err| {
                panic!("Failed to create log file at {:?}: {:?}", log_file_path, err);
            });

            env_logger::Builder::from_default_env()
                .format(move |buf, record| {
                    writeln!(
                        buf,
                        "{} [{}] - {}",
                        Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.level(),
                        record.args()
                    )
                })
                .target(env_logger::Target::Pipe(Box::new(log_file))) // Logs to file in release mode
                .filter(None, log::LevelFilter::Info) // Info-level logging for release
                .init();
            println!("Release mode: Logger initialized to write to file at {:?}", log_file_path);
        }

        info!("Logger configured successfully.");
        println!("called");

        let conn = Arc::new(Mutex::new(
            Connection::open(&db_path).unwrap_or_else(|err| {
                error!("Failed to open database connection at {:?}: {:?}", db_path, err);
                panic!("Cannot proceed without database connection");
            }),
        ));
        info!("Database connection established at: {:?}", db_path);

        let (ctrl_c_tx, ctrl_c_rx) = unbounded_channel::<()>();
        let (tx, rx) = mpsc::unbounded_channel();

        let handle3 = tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            info!("Ctrl+C detected. Sending shutdown signal...");
            let _ = ctrl_c_tx.send(());
        });

        let session_id = Uuid::new_v4().to_string();
        let handle1 = tokio::spawn(track_application_usage(session_id.clone(), tx, ctrl_c_rx));
        let handle2 = tokio::spawn(upset_app_usage(conn, rx));

        let (r1, r2, _) = tokio::join!(handle1, handle2, handle3);
        if let Err(err) = r1 {
            error!("Tracking task failed: {:?}", err);
        }
        if let Err(err) = r2 {
            error!("Database update task failed: {:?}", err);
        }
    } else {
        error!("This program is only supported on Windows.");
    }
}