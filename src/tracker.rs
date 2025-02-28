use chrono::Timelike;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

use crate::{
    db::models::{App, AppUsage, IdlePeriod, WindowUsage},
    platform::{windows::WindowsHandle, Platform, WindowDetails},
};

type AppMap = HashMap<String, App>;
type WindowUsageMap = HashMap<String, WindowUsage>;
type ClassificationSet = HashSet<String>;
type IdleMap = HashMap<String, IdlePeriod>;
type AppUsageMap = HashMap<String, AppUsage>;
pub type AppData = (
    AppMap,
    WindowUsageMap,
    ClassificationSet,
    IdleMap,
    AppUsageMap,
);

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const IDLE_THRESHOLD_SECS: u64 = 30;

pub struct AppTracker {
    session_id: String,
    previous_app_map: HashMap<String, App>,
    previous_window_usage_map: HashMap<String, WindowUsage>,
    previous_classification_map: HashSet<String>,
    previous_idle_map: HashMap<String, IdlePeriod>,
    previous_app_usage_map: HashMap<String, AppUsage>,
}

impl AppTracker {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            previous_app_map: HashMap::new(),
            previous_window_usage_map: HashMap::new(),
            previous_classification_map: HashSet::new(),
            previous_idle_map: HashMap::new(),
            previous_app_usage_map: HashMap::new(),
        }
    }

    pub fn update(
        &mut self,
        window_state: &(
            BTreeMap<String, WindowDetails>,
            BTreeMap<String, WindowDetails>,
        ),
    ) {
        let current_time = chrono::Local::now()
            .naive_local()
            .with_nanosecond(0)
            .unwrap();
        let start_time = chrono::Local::now()
            .naive_local()
            .with_nanosecond(0)
            .unwrap();
        for (_, details) in window_state.0.iter() {
            let app_name = details
                .app_name
                .clone()
                .unwrap_or_else(|| "Unknown App".to_string());
            let app_path = details
                .app_path
                .clone()
                .unwrap_or_else(|| "Unknown Path".to_string());

            self.update_app(&app_name, &app_path);
            self.update_usage(&details.window_title, &app_name, current_time, start_time);
            self.update_classification(&app_name);
        }

        self.cleanup_old_entries(window_state);
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
        start_time: chrono::NaiveDateTime,
    ) {
        let mut window_id = Uuid::new_v4().to_string();
        let mut app_time_id = Uuid::new_v4().to_string();
        let idle_time_secs = WindowsHandle::get_last_input_info()
            .unwrap_or_default()
            .as_secs();

        match self.previous_app_usage_map.entry(app_name.to_string()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().end_time = current_time;
                app_time_id = entry.get().id.clone();
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(AppUsage {
                    id: app_time_id.clone(),
                    app_name: app_name.to_string(),
                    start_time,
                    end_time: current_time,
                });
            }
        }
        match self
            .previous_window_usage_map
            .entry(window_title.to_string())
        {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().last_updated_time = current_time;
                window_id = entry.get().app_id.clone();
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(WindowUsage {
                    session_id: self.session_id.clone(),
                    app_id: window_id.clone(),
                    application_name: app_name.to_string(),
                    current_screen_title: window_title.to_string(),
                    start_time: current_time,
                    last_updated_time: current_time,
                    app_time_id: app_time_id.clone(),
                });
            }
        }

        if idle_time_secs > IDLE_THRESHOLD_SECS {
            match self.previous_idle_map.entry(window_title.to_owned()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().end_time = current_time;
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let idle_period = IdlePeriod {
                        app_id: app_time_id,
                        window_id,
                        session_id: self.session_id.clone(),
                        app_name: app_name.to_string(),
                        start_time: current_time,
                        end_time: current_time,
                        id: Uuid::new_v4().to_string(),
                    };
                    entry.insert(idle_period);
                }
            }
        }
    }

    fn update_classification(&mut self, app_name: &str) {
        self.previous_classification_map.insert(app_name.to_owned());
    }

    fn cleanup_old_entries(
        &mut self,
        window_state: &(
            BTreeMap<String, WindowDetails>,
            BTreeMap<String, WindowDetails>,
        ),
    ) {
        self.previous_app_usage_map
            .retain(|key, _| window_state.1.contains_key(key));
        self.previous_window_usage_map
            .retain(|key, _| window_state.0.contains_key(key));
        self.previous_idle_map
            .retain(|key, _| window_state.0.contains_key(key));
    }

    pub fn get_state(&self) -> AppData {
        (
            self.previous_app_map.clone(),
            self.previous_window_usage_map.clone(),
            self.previous_classification_map.clone(),
            self.previous_idle_map.clone(),
            self.previous_app_usage_map.clone(),
        )
    }

    pub fn reset_idle_map(&mut self) {
        let idle_time_secs = WindowsHandle::get_last_input_info()
            .unwrap_or_default()
            .as_secs();
        if idle_time_secs < IDLE_THRESHOLD_SECS && self.previous_idle_map.is_empty() == false {
            self.previous_idle_map.clear();
        }
    }
}
