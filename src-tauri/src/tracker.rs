use chrono::Timelike;
use internment::ArcIntern;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

use crate::{
    db::models::{App, AppUsage, IdlePeriod, WindowUsage},
    platform::{
        windows::WindowsHandle, AppName, Platform, WindowDetails, WindowDetailsTuple, WindowName,
    },
};

type AppMap = HashMap<ArcIntern<String>, App>;
type WindowUsageMap = HashMap<ArcIntern<String>, WindowUsage>;
type ClassificationSet = HashSet<ArcIntern<String>>;
type IdleMap = HashMap<ArcIntern<String>, IdlePeriod>;
type AppUsageMap = HashMap<ArcIntern<String>, AppUsage>;
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
    previous_app_map: HashMap<ArcIntern<String>, App>,
    previous_window_usage_map: HashMap<ArcIntern<String>, WindowUsage>,
    previous_classification_map: HashSet<ArcIntern<String>>,
    previous_idle_map: HashMap<ArcIntern<String>, IdlePeriod>,
    previous_app_usage_map: HashMap<ArcIntern<String>, AppUsage>,
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
            BTreeMap<WindowName, ArcIntern<WindowDetails>>,
            BTreeMap<AppName, ArcIntern<WindowDetails>>,
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
                .unwrap_or_else(|| ArcIntern::new("Unknown App".to_string()));
            let app_path = details
                .app_path
                .clone()
                .unwrap_or_else(|| ArcIntern::new("Unknown Path".to_string()));

            self.update_app(&app_name, app_path);
            self.update_usage(&details.window_title, &app_name, current_time, start_time);
            self.update_classification(&app_name);
        }

        self.cleanup_old_entries(window_state);
    }

    fn update_app(&mut self, app_name: &ArcIntern<String>, app_path: ArcIntern<String>) {
        self.previous_app_map.insert(
            app_name.clone(),
            App {
                name: app_name.clone(),
                path: app_path.clone(),
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
        let idle_time_secs = WindowsHandle::get_last_input_info().as_secs();
        let app_name = ArcIntern::new(app_name.to_owned());
        let window_title = ArcIntern::new(window_title.to_owned());
        match self.previous_app_usage_map.entry(app_name.clone()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().end_time = current_time;
                app_time_id = entry.get().id.clone();
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(AppUsage {
                    id: app_time_id.clone(),
                    app_name: app_name.clone(),
                    start_time,
                    end_time: current_time,
                });
            }
        }
        match self.previous_window_usage_map.entry(window_title.clone()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().last_updated_time = current_time;
                window_id = entry.get().app_id.clone();
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(WindowUsage {
                    session_id: self.session_id.clone(),
                    app_id: window_id.clone(),
                    application_name: app_name.clone(),
                    current_screen_title: window_title.clone(),
                    start_time: current_time,
                    last_updated_time: current_time,
                    app_time_id: app_time_id.clone(),
                });
            }
        }

        if idle_time_secs > IDLE_THRESHOLD_SECS {
            match self.previous_idle_map.entry(window_title.clone()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().end_time = current_time;
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let idle_period = IdlePeriod {
                        app_id: app_time_id,
                        window_id,
                        session_id: self.session_id.clone(),
                        app_name: app_name,
                        start_time: current_time,
                        end_time: current_time,
                        id: Uuid::new_v4().to_string(),
                    };
                    entry.insert(idle_period);
                }
            }
        }
    }

    fn update_classification(&mut self, app_name: &ArcIntern<String>) {
        self.previous_classification_map.insert(app_name.clone());
    }

    fn cleanup_old_entries(&mut self, window_state: &WindowDetailsTuple) {
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
        let idle_time_secs = WindowsHandle::get_last_input_info().as_secs();
        if idle_time_secs < IDLE_THRESHOLD_SECS && !self.previous_idle_map.is_empty() {
            self.previous_idle_map.clear();
        }
    }
}
