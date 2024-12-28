#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Local;
use dirs;
use dotenvy::dotenv;
use env_logger::Builder;
use log::{error, info};
use rusqlite::Connection;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

mod db;
mod platform;

use db::connection::upset_app_usage;
use db::models::{App, AppUsage};
use platform::windows::{self, WindowsHandle};
use platform::{Platform, WindowDetails};

// Types
type AppMap = HashMap<String, App>;
type UsageMap = HashMap<String, AppUsage>;
type AppData = (AppMap, UsageMap);
type Sender = mpsc::UnboundedSender<AppData>;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Constants
const IDLE_THRESHOLD_SECS: u64 = 300;
const TRACKING_INTERVAL_MS: u64 = 1000;

/// Application configuration structure
struct Config {
    session_id: String,
    db_path: PathBuf,
    log_path: PathBuf,
}

impl Config {
    fn new() -> Result<Self> {
        let db_path = get_database_path()?;
        let log_path = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("application.log");

        Ok(Config {
            session_id: Uuid::new_v4().to_string(),
            db_path,
            log_path,
        })
    }
}

/// Logger configuration and initialization
struct Logger;

impl Logger {
    fn initialize(log_path: &Path) {
        let mut binding = Builder::from_default_env();
        let builder = binding.format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        });

        #[cfg(debug_assertions)]
        {
            builder.filter(None, log::LevelFilter::Debug).init();
            info!("Debug mode: Logging to console.");
        }

        #[cfg(not(debug_assertions))]
        {
            let log_file = std::fs::File::create(log_path).unwrap_or_else(|err| {
                panic!("Failed to create log file at {:?}: {:?}", log_path, err);
            });
            builder
                .target(env_logger::Target::Pipe(Box::new(log_file)))
                .filter(None, log::LevelFilter::Debug)
                .init();
            println!("Release mode: Logging to file at {:?}", log_path);
        }
    }
}

/// Application state tracker
struct AppTracker {
    session_id: String,
    previous_app_map: AppMap,
    previous_app_usage_map: UsageMap,
}

impl AppTracker {
    fn new(session_id: String) -> Self {
        Self {
            session_id,
            previous_app_map: HashMap::new(),
            previous_app_usage_map: HashMap::new(),
        }
    }

    fn update(&mut self, window_state: &BTreeMap<String, WindowDetails>) {
        let current_time = Local::now().naive_utc();

        for (_, details) in window_state.iter() {
            let app_name = details
                .app_name
                .clone()
                .unwrap_or_else(|| "Unknown App".to_string());
            let app_path = details
                .app_path
                .clone()
                .unwrap_or_else(|| "Unknown Path".to_string());

            self.update_app(&app_name, &app_path);
            self.update_usage(&details.window_title, &app_name, current_time);
        }

        self.previous_app_usage_map
            .retain(|key, _| window_state.contains_key(key));
    }

    fn update_app(&mut self, app_name: &str, app_path: &str) {
        self.previous_app_map.insert(
            app_name.to_string(),
            App {
                name: app_name.to_string(),
                path: app_path.to_string(),
            },
        );
    }

    fn update_usage(
        &mut self,
        window_title: &str,
        app_name: &str,
        current_time: chrono::NaiveDateTime,
    ) {
        match self.previous_app_usage_map.entry(window_title.to_string()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().last_updated_time = current_time;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(AppUsage {
                    session_id: self.session_id.clone(),
                    app_id: Uuid::new_v4().to_string(),
                    application_name: app_name.to_string(),
                    current_screen_title: window_title.to_string(),
                    start_time: current_time,
                    last_updated_time: current_time,
                });
            }
        }
    }

    fn get_state(&self) -> AppData {
        (
            self.previous_app_map.clone(),
            self.previous_app_usage_map.clone(),
        )
    }
}

/// Window state management
struct WindowStateManager;

impl WindowStateManager {
    fn get_current_state() -> BTreeMap<String, WindowDetails> {
        let window_state = windows::WindowsHandle::get_window_titles();
        let idle_time_secs = WindowsHandle::get_last_input_info()
            .unwrap_or_default()
            .as_secs();

        if idle_time_secs >= IDLE_THRESHOLD_SECS {
            Self::augment_with_idle_state(window_state)
        } else {
            window_state
        }
    }

    fn augment_with_idle_state(
        mut window_state: BTreeMap<String, WindowDetails>,
    ) -> BTreeMap<String, WindowDetails> {
        if let Some(first_entry) = window_state.first_entry() {
            let value = first_entry.get().clone();
            let key = format!(
                "Idle Time{}",
                value
                    .app_name
                    .clone()
                    .unwrap_or_else(|| "Unknown app".to_string())
            );
            window_state.insert(
                key,
                WindowDetails {
                    window_title: "Idle".to_owned(),
                    app_name: value.app_name,
                    app_path: value.app_path,
                    is_active: false,
                },
            );
        }
        window_state
    }
}

/// Database path resolution
fn get_database_path() -> Result<PathBuf> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or("%AppData%\\screen_time_tracking_app\\stop_procastinating.sqlite3".to_owned());
    Ok(if db_url.contains("%AppData%") {
        let app_data_path = dirs::config_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        PathBuf::from(db_url.replace("%AppData%", app_data_path.to_str().unwrap()))
    } else {
        PathBuf::from(db_url)
    })
}

/// Main tracking loop
async fn track_application_usage(
    session_id: String,
    tx: Sender,
    mut ctrl_c_recv: mpsc::UnboundedReceiver<()>,
) {
    let mut tracker = AppTracker::new(session_id);
    let mut previous_state = None;
    loop {
        tokio::select! {
            Some(_) = ctrl_c_recv.recv() => {
                info!("Shutdown signal received.");
                if let Err(err) = tx.send(tracker.get_state()) {
                    error!("Error sending data on shutdown: {:?}", err);
                }
                break;
            }
            _ = async {
                let start = Instant::now();
                let window_state = WindowStateManager::get_current_state();
                if previous_state.as_ref() != Some(&window_state) {
                    previous_state = Some(window_state.clone());
                    tracker.update(&window_state);
                    if let Err(err) = tx.send(tracker.get_state()) {
                        error!("Error sending updated data: {:?}", err);
                    }
                }
                let sleep_duration = TRACKING_INTERVAL_MS.saturating_sub(start.elapsed().as_millis() as u64);
                tokio::time::sleep(Duration::from_millis(sleep_duration)).await;
            } => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    if !cfg!(target_os = "windows") {
        error!("This application is supported only on Windows.");
        return Ok(());
    }

    let config = Config::new()?;
    Logger::initialize(&config.log_path);

    let conn = Arc::new(Mutex::new(
        Connection::open(&config.db_path).unwrap_or_else(|err| {
            panic!(
                "Failed to open database connection at {:?}: {:?}",
                config.db_path, err
            );
        }),
    ));
    info!("Database connected at {:?}", config.db_path);

    let (ctrl_c_tx, ctrl_c_rx) = mpsc::unbounded_channel();
    let (tx, rx) = mpsc::unbounded_channel();

    let signal_task = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        let _ = ctrl_c_tx.send(());
    });

    let tracking_task = tokio::spawn(track_application_usage(
        config.session_id.clone(),
        tx,
        ctrl_c_rx,
    ));
    let db_task = tokio::spawn(upset_app_usage(conn, rx));

    let (tracking_res, db_res, _) = tokio::join!(tracking_task, db_task, signal_task);

    if let Err(err) = tracking_res {
        error!("Tracking task failed: {:?}", err);
    }
    if let Err(err) = db_res {
        error!("Database task failed: {:?}", err);
    }

    Ok(())
}
