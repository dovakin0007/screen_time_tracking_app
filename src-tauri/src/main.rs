// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use dotenvy::dotenv;
use internment::ArcIntern;
use log::{debug, error, info};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use tokio::{
    sync::{mpsc, RwLock},
    task,
};

use screen_time_tracking_front_end_lib::{
    config::Config,
    db::{
        connection::{upsert_app_usage, DbHandler},
        models::Sessions,
    },
    fs_watcher::{
        config_watcher::{open_or_create_file, watcher, ConfigFile},
        start_menu_watcher::start_menu_watcher,
    },
    logger::Logger,
    platform::{
        windows::{spawn_toast_notification, WindowsHandle},
        Platform, WindowDetails,
    },
    tracker::{AppData, AppTracker},
    zero_mq_service::start_server,
    StartMenuStatus,
};

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

async fn main2(
    db_handler: Arc<DbHandler>,
    config: Config,
    programs_watcher_status: Arc<StartMenuStatus>,
) {
    let db_handler_2 = Arc::clone(&db_handler);
    let file_notifier_task = task::spawn(async move {
        let _ = std::mem::replace(
            &mut APP_CONFIG.write().await.config_message,
            open_or_create_file().await.config_message,
        );
        let db_handler = Arc::clone(&db_handler_2);
        let file_watcher = tokio::task::spawn(watcher(&APP_CONFIG));
        let menu_watcher = task::spawn(start_menu_watcher(
            Arc::clone(&db_handler),
            programs_watcher_status,
        ));
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
    let app_task_db_handler = Arc::clone(&db_handler);

    let app_manager_task = task::spawn(async move {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );
        // let process_count = HashMap::new();
        let mut seconds = 0;
        loop {
            let start = Instant::now();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

            let app_details = app_task_db_handler
                .get_current_app_usage_details()
                .await
                .unwrap();
            for app_detail in app_details {
                let exe_name = OsStr::new(&app_detail.app_name);

                let process_list = sys.processes_by_exact_name(exe_name);

                for process in process_list {
                    if exe_name == process.name() {
                        let limit = app_detail.time_limit.unwrap_or(0) as f64;
                        let total_spent = app_detail.total_hours * 60.0;
                        if total_spent >= limit {
                            if app_detail.should_close.unwrap_or(false) {
                                let result = process.kill();
                                debug!(
                                    "process killed successfully {:?}: {}",
                                    app_detail.app_name, result
                                );
                            } else if app_detail.should_alert.unwrap_or(false)
                                && (seconds % app_detail.alert_duration.unwrap_or(300) == 0)
                            {
                                let exe_name_str = exe_name.to_str().unwrap().to_string();
                                _ = spawn_toast_notification(
                                    exe_name_str,
                                    Arc::clone(&app_task_db_handler),
                                )
                                .await;
                            }
                        }
                    }
                }
            }

            let sleep_duration =
                TRACKING_INTERVAL_MS.saturating_sub(start.elapsed().as_millis() as u64);
            tokio::time::sleep(Duration::from_millis(sleep_duration)).await;
            seconds += 1;
        }
    });

    let (task1, task2, task3, task4) = tokio::join!(
        file_notifier_task,
        tracker_task,
        server_task,
        app_manager_task,
    );
    if let Err(e) = task1 {
        error!("{:?}", e);
    }
    if let Err(e) = task2 {
        error!("{:?}", e);
    }
    if let Err(e) = task3 {
        error!("{:?}", e);
    }

    if let Err(e) = task4 {
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
    if !cfg!(target_os = "windows") {
        error!("This application is supported only on Windows.");
        return;
    }

    let config = Config::new().expect("Failed to load config");
    Logger::initialize(&config.log_path);

    let db_handler = Arc::new(DbHandler::new(config.db_path.clone()));
    let db_handler_clone = Arc::clone(&db_handler);
    let programs_watcher_status = Arc::new(StartMenuStatus::new());
    let backend_runtime = tokio::runtime::Runtime::new().expect("Failed to create backend runtime");
    let program_watch_status_clone = Arc::clone(&programs_watcher_status);
    std::thread::spawn(move || {
        backend_runtime.block_on(async move {
            main2(db_handler_clone, config, program_watch_status_clone).await
        });
    });

    let tauri_runtime =
        tokio::runtime::Runtime::new().expect("Failed to create seperate runtime for tauri");
    tauri::async_runtime::set(tauri_runtime.handle().to_owned());

    screen_time_tracking_front_end_lib::run(Arc::clone(&db_handler), programs_watcher_status);
}
