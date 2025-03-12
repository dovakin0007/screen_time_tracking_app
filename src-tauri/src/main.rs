// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use dotenvy::dotenv;
use internment::ArcIntern;
use log::{error, info};
use screen_time_tracking_front_end_lib::fs_watcher::config_watcher::{
    open_or_create_file, watcher, ConfigFile,
};
use screen_time_tracking_front_end_lib::logger::Logger;
use screen_time_tracking_front_end_lib::tracker::{AppData, AppTracker};
use screen_time_tracking_front_end_lib::{
    config::Config, fs_watcher::start_menu_watcher::start_menu_watcher,
};
use tokio::{
    sync::{mpsc, RwLock},
    task,
};

use screen_time_tracking_front_end_lib::db::{
    connection::{upsert_app_usage, DbHandler},
    models::Sessions,
};
use screen_time_tracking_front_end_lib::platform::{
    windows::WindowsHandle, Platform, WindowDetails,
};
use screen_time_tracking_front_end_lib::zero_mq_service::start_server;

#[derive(Debug)]
pub struct WindowStateTracker {
    previous_state: Option<BTreeMap<ArcIntern<String>, ArcIntern<WindowDetails>>>,
    last_update: Instant,
}

impl WindowStateTracker {
    pub fn new() -> Self {
        Self {
            previous_state: None,
            last_update: Instant::now(),
        }
    }

    pub fn has_state_changed(
        &self,
        new_state: &BTreeMap<ArcIntern<String>, ArcIntern<WindowDetails>>,
    ) -> bool {
        self.previous_state.as_ref() != Some(new_state)
    }

    pub fn update_state(
        &mut self,
        new_state: BTreeMap<ArcIntern<String>, ArcIntern<WindowDetails>>,
    ) {
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

async fn main2(db_handler: Arc<DbHandler>, config: Config) {
    println!("called");
    let db_handler_2 = Arc::clone(&db_handler);
    let file_notifier_task = task::spawn(async move {
        let _ = std::mem::replace(
            &mut APP_CONFIG.write().await.config_message,
            open_or_create_file().await.config_message,
        );
        let db_handler = Arc::clone(&db_handler_2);
        let file_watcher = tokio::task::spawn(watcher(&APP_CONFIG));
        let menu_watcher = task::spawn(start_menu_watcher(Arc::clone(&db_handler)));
        let (join1, join2) = tokio::join!(file_watcher, menu_watcher);
        if let Err(e) = join1 {
            error!("{:?}", e);
        }
        if let Err(e) = join2 {
            error!("{:?}", e);
        }
    });

    let tracker_db = Arc::clone(&db_handler);
    let tracker_config = config.clone();
    let tracker_task = task::spawn(async move {
        if let Err(e) = tracker_service_main(tracker_db, tracker_config).await {
            error!("Failed to start tracker service:{:?}", e);
        }
    });

    let server_db = Arc::clone(&db_handler);
    let (control_sender, control_recv) = tokio::sync::mpsc::channel::<bool>(30);
    let server_task = task::spawn(async move {
        let result = start_server(server_db, control_sender, control_recv, &APP_CONFIG).await;
        result
    });

    // Run all tasks concurrently and wait for them to complete
    let (task1, task2, task3) = tokio::join!(file_notifier_task, tracker_task, server_task);
    if let Err(e) = task1 {
        error!("{:?}", e);
    }
    if let Err(e) = task2 {
        error!("{:?}", e);
    }
    if let Err(e) = task3 {
        error!("{:?}", e);
    }
}

async fn tracker_service_main(db_handler: Arc<DbHandler>, config: Config) -> anyhow::Result<()> {
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

#[tokio::main]
async fn main() {
    dotenv().ok();
    // Check operating system
    if !cfg!(target_os = "windows") {
        error!("This application is supported only on Windows.");
        return;
    }

    let config = Config::new().expect("Failed to load config");
    Logger::initialize(&config.log_path);

    let db_handler = Arc::new(DbHandler::new(config.db_path.clone()));
    let db_handler_clone = Arc::clone(&db_handler);

    let backend_runtime = tokio::runtime::Runtime::new().expect("Failed to create backend runtime");

    std::thread::spawn(move || {
        backend_runtime.block_on(async move { main2(db_handler_clone, config).await });
    });

    tauri::async_runtime::set(tokio::runtime::Handle::current());
    screen_time_tracking_front_end_lib::run(Arc::clone(&db_handler));
}
