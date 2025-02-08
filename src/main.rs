#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

use config::Config;
use dotenvy::dotenv;
use log::{error, info};
use logger::Logger;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tracker::{AppData, AppTracker, WindowStateManager};

pub mod config;
pub mod db;
pub mod logger;
pub mod platform;
pub mod tracker;

use db::connection::{upsert_app_usage, DbHandler};
use db::models::Sessions;
use platform::windows::WindowsHandle;
use platform::Platform;
use platform::WindowDetails;
use tracker::Result;

#[derive(Debug)]
pub struct WindowStateTracker {
    previous_state: Option<BTreeMap<String, WindowDetails>>,
    last_update: Instant,
}

impl WindowStateTracker {
    pub fn new() -> Self {
        Self {
            previous_state: None,
            last_update: Instant::now(),
        }
    }

    pub fn has_state_changed(&self, new_state: &BTreeMap<String, WindowDetails>) -> bool {
        self.previous_state.as_ref() != Some(new_state)
    }

    pub fn update_state(&mut self, new_state: BTreeMap<String, WindowDetails>) {
        self.previous_state = Some(new_state);
        self.last_update = Instant::now();
    }

    pub fn time_since_last_update(&self) -> Duration {
        self.last_update.elapsed()
    }

    pub fn needs_update(&self, update_interval: Duration) -> bool {
        self.time_since_last_update() >= update_interval
    }
}

type Sender = mpsc::UnboundedSender<AppData>;

const IDLE_THRESHOLD_SECS: u64 = 30;
const TRACKING_INTERVAL_MS: u64 = 1000;
const DB_UPDATE_INTERVAL: Duration = Duration::from_secs(30);

async fn track_application_usage(
    session_id: String,
    tx: Sender,
    mut ctrl_c_recv: mpsc::UnboundedReceiver<()>,
) {
    let mut tracker = AppTracker::new(session_id);
    let mut state_tracker = WindowStateTracker::new();

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

                if state_tracker.has_state_changed(&window_state) ||
                   state_tracker.needs_update(DB_UPDATE_INTERVAL) ||
                   idle_time_secs > IDLE_THRESHOLD_SECS {
                    state_tracker.update_state(window_state.clone());
                    tracker.update(&window_state);
                    should_update = true;
                }

                if should_update {
                    if let Err(err) = tx.send(tracker.get_state()) {
                        error!("Error sending updated data: {:?}", err);
                    }
                    tracker.reset_idle_map();
                }

                let sleep_duration = TRACKING_INTERVAL_MS.saturating_sub(start.elapsed().as_millis() as u64);
                tokio::time::sleep(Duration::from_millis(sleep_duration)).await;
            } => {}
        }
    }
}

fn main() {
    dotenv().ok();

    if !cfg!(target_os = "windows") {
        error!("This application is supported only on Windows.");
        return;
    }

    let config = Config::new().expect("Failed to load config");
    Logger::initialize(&config.log_path);

    let db_handler = DbHandler::new(config.db_path.clone());

    let tracker_runtime = Runtime::new().expect("Failed to create tracker runtime");
    let tracker_handle = thread::spawn(move || {
        tracker_runtime.block_on(async {
            if let Err(_) = tracker_service_main(db_handler, config).await {
                error!("Failed to start tracker service");
            }
        });
    });

    tracker_handle.join().expect("Tracker thread panicked");
}

async fn tracker_service_main(db_handler: DbHandler, config: Config) -> Result<()> {
    let (ctrl_c_tx, ctrl_c_rx) = mpsc::unbounded_channel();
    let (tx, rx) = mpsc::unbounded_channel();

    let signal_task = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        let _ = ctrl_c_tx.send(());
    });

    let session = Sessions::new(config.session_id.clone());

    let tracking_task = tokio::spawn(track_application_usage(
        config.session_id.clone(),
        tx,
        ctrl_c_rx,
    ));
    let db_task = tokio::spawn(upsert_app_usage(db_handler, session, rx));

    let (tracking_res, db_res, _) = tokio::join!(tracking_task, db_task, signal_task);

    if let Err(err) = tracking_res {
        error!("Tracking task failed: {:?}", err);
    }
    if let Err(err) = db_res {
        error!("Database task failed: {:?}", err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_state_tracker() {
        let mut tracker = WindowStateTracker::new();
        let mut state1 = BTreeMap::new();
        state1.insert("window1".to_string(), WindowDetails::default());

        assert!(tracker.has_state_changed(&state1));

        tracker.update_state(state1.clone());
        assert!(!tracker.has_state_changed(&state1));

        let mut state2 = BTreeMap::new();
        state2.insert("window2".to_string(), WindowDetails::default());
        assert!(tracker.has_state_changed(&state2));
    }

    #[test]
    fn test_update_interval() {
        let mut tracker = WindowStateTracker::new();
        let state = BTreeMap::new();

        tracker.update_state(state);
        assert!(!tracker.needs_update(Duration::from_secs(1)));

        thread::sleep(Duration::from_secs(2));
        assert!(tracker.needs_update(Duration::from_secs(1)));
    }
}
