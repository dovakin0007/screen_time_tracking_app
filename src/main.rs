#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Local;
use config::Config;
use dirs;
use dotenvy::dotenv;
use env_logger::Builder;
use log::{error, info};
use logger::Logger;
use rusqlite::Connection;
use tokio::sync::{mpsc, Mutex};
use tracker::{AppTracker, WindowStateManager};
use uuid::Uuid;

pub mod config;
mod db;
pub mod logger;
mod platform;
pub mod tracker;

use db::connection::upsert_app_usage;
use db::models::{App, AppUsage, Classification, IdlePeriod, Sessions};
use platform::windows::{self, WindowsHandle};
use platform::{Platform, WindowDetails};

// Types
type AppMap = HashMap<String, App>;
type UsageMap = HashMap<String, AppUsage>;
type ClassificationMap = HashMap<String, Classification>;
type IdleMap = HashMap<String, IdlePeriod>;
type AppData = (AppMap, UsageMap, ClassificationMap, IdleMap);
type Sender = mpsc::UnboundedSender<AppData>;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Constants
const IDLE_THRESHOLD_SECS: u64 = 30;
const TRACKING_INTERVAL_MS: u64 = 1000;

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
    let mut last_db_update = Instant::now();
    const DB_UPDATE_INTERVAL: Duration = Duration::from_secs(30);

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

                let mut should_update = false;

                let idle_time_secs = WindowsHandle::get_last_input_info()
                .unwrap_or_default()
                .as_secs();

                // Update tracker if state changed
                if previous_state.as_ref() != Some(&window_state) {
                    previous_state = Some(window_state.clone());
                    tracker.update(&window_state);
                    should_update = true;
                }

                // Check if 30 seconds have elapsed
                if start.duration_since(last_db_update) >= DB_UPDATE_INTERVAL || idle_time_secs > IDLE_THRESHOLD_SECS  {
                    previous_state = Some(window_state.clone());
                    tracker.update(&window_state);
                    should_update = true;
                }


                // Send update if either condition is met
                if should_update {
                    if let Err(err) = tx.send(tracker.get_state()) {
                        error!("Error sending updated data: {:?}", err);
                    }
                    tracker.reset_idle_map();
                    last_db_update = start;
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
    let session = Sessions {
        session_id: config.session_id.clone(),
        session_date: Local::now().date_naive(),
    };
    let tracking_task = tokio::spawn(track_application_usage(
        config.session_id.clone(),
        tx,
        ctrl_c_rx,
    ));
    let db_task = tokio::spawn(upsert_app_usage(conn, session, rx));

    let (tracking_res, db_res, _) = tokio::join!(tracking_task, db_task, signal_task);

    if let Err(err) = tracking_res {
        error!("Tracking task failed: {:?}", err);
    }
    if let Err(err) = db_res {
        error!("Database task failed: {:?}", err);
    }

    Ok(())
}
