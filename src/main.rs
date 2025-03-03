#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
    thread,
    time::{Duration, Instant},
};

use config::Config;
use config_watcher::{open_or_create_file, watcher, ConfigFile};
use dotenvy::dotenv;
use log::{error, info};
use logger::Logger;
use tokio::{
    runtime::Runtime,
    sync::{mpsc, RwLock},
};
use tracker::{AppData, AppTracker};

pub mod config;
pub mod config_watcher;
pub mod db;
pub mod logger;
pub mod platform;
pub mod system_usage;
pub mod tracker;
pub mod zero_mq_service;

use db::{
    connection::{upsert_app_usage, DbHandler},
    models::Sessions,
};
use platform::{windows::WindowsHandle, Platform, WindowDetails};
use tracker::Result;
use zero_mq_service::start_server;

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

impl Default for WindowStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

type Sender = mpsc::UnboundedSender<AppData>;

const TRACKING_INTERVAL_MS: u64 = 1000;

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
                let app_config = &APP_CONFIG.read().await.config_message;
                let start = Instant::now();
                let window_state = WindowsHandle::get_window_titles();

                let mut should_update = false;

                let idle_time_secs = WindowsHandle::get_last_input_info()
                    .as_secs();

                if state_tracker.has_state_changed(&window_state.0) ||
                   state_tracker.needs_update(Duration::from_secs(app_config.db_update_interval)) ||
                   idle_time_secs > app_config.idle_threshold_period {
                    state_tracker.update_state(window_state.0.clone());
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

static APP_CONFIG: LazyLock<RwLock<ConfigFile>> =
    LazyLock::new(|| RwLock::new(ConfigFile::default()));

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenv().ok();

    if !cfg!(target_os = "windows") {
        error!("This application is supported only on Windows.");
        return;
    }

    let config = Config::new().expect("Failed to load config");
    Logger::initialize(&config.log_path);

    let db_handler = Arc::new(DbHandler::new(config.db_path.clone()));

    let tracker_runtime = Runtime::new().expect("Failed to create tracker runtime");
    let server_runtime = Runtime::new().expect("Failed to create server runtime");
    let file_notifier_runtime = Runtime::new().expect("Failed to create watcher runtime");
    let file_handle = thread::spawn(move || {
        file_notifier_runtime.block_on(async {
            let _ = std::mem::replace(
                &mut APP_CONFIG.write().await.config_message,
                open_or_create_file().await.config_message,
            );
            watcher(&APP_CONFIG).await;
        });
    });
    let tracker_db = Arc::clone(&db_handler);
    let tracker_config = config;
    let tracker_handle = thread::spawn(move || {
        tracker_runtime.block_on(async {
            if let Err(e) = tracker_service_main(tracker_db, tracker_config).await {
                error!("Failed to start tracker service:{:?}", e);
            }
        });
    });
    let server_db = Arc::clone(&db_handler);
    let server_handle = thread::spawn(move || {
        let (control_sender, control_recv) = tokio::sync::mpsc::channel::<bool>(30);
        server_runtime.block_on(start_server(
            server_db,
            control_sender,
            control_recv,
            &APP_CONFIG,
        ))
    });

    if let Err(e) = tracker_handle.join() {
        error!("Tracker thread panicked: {:?}", e);
    }

    if let Err(e) = file_handle.join() {
        error!("File config listener panicked: {:?}", e);
    }

    if let Err(e) = server_handle.join() {
        error!("Server thread panicked: {:?}", e);
        std::process::exit(1)
    }
}

async fn tracker_service_main(db_handler: Arc<DbHandler>, config: Config) -> Result<()> {
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
