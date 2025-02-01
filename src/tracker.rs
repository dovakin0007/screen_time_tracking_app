use chrono::NaiveDateTime;
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

use crate::{
    db::models::{App, AppUsage, Classification, IdlePeriod},
    platform::{windows::WindowsHandle, Platform, WindowDetails},
    AppData,
};

const IDLE_THRESHOLD_SECS: u64 = 30;

pub struct AppTracker {
    session_id: String,
    previous_app_map: HashMap<String, App>,
    previous_app_usage_map: HashMap<String, AppUsage>,
    previous_classification_map: HashMap<String, Classification>,
    previous_idle_map: HashMap<String, IdlePeriod>,
}

impl AppTracker {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            previous_app_map: HashMap::new(),
            previous_app_usage_map: HashMap::new(),
            previous_classification_map: HashMap::new(),
            previous_idle_map: HashMap::new(),
        }
    }

    pub fn update(&mut self, window_state: &BTreeMap<String, WindowDetails>) {
        let current_time = chrono::Local::now().naive_local();

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
            self.update_classification(&details.window_title, &app_name);
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
    ) {
        let mut app_id = Uuid::new_v4().to_string();
        let idle_time_secs = WindowsHandle::get_last_input_info()
            .unwrap_or_default()
            .as_secs();

        match self.previous_app_usage_map.entry(window_title.to_string()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().last_updated_time = current_time;
                app_id = entry.get().app_id.clone();
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(AppUsage {
                    session_id: self.session_id.clone(),
                    app_id: app_id.clone(),
                    application_name: app_name.to_string(),
                    current_screen_title: window_title.to_string(),
                    start_time: current_time,
                    last_updated_time: current_time,
                });
            }
        }

        if idle_time_secs > IDLE_THRESHOLD_SECS {
            match self.previous_idle_map.entry(window_title.to_owned()) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    // Update existing idle period's end time
                    entry.get_mut().end_time = current_time;
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    // Create new idle period
                    let idle_period = IdlePeriod {
                        app_id: app_id.clone(),
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

    fn update_classification(&mut self, window_title: &str, app_name: &str) {
        self.previous_classification_map.insert(
            window_title.to_owned(),
            Classification {
                name: app_name.to_owned(),
                window_title: window_title.to_owned(),
            },
        );
    }

    fn cleanup_old_entries(&mut self, window_state: &BTreeMap<String, WindowDetails>) {
        self.previous_app_usage_map
            .retain(|key, _| window_state.contains_key(key));
        self.previous_idle_map
            .retain(|key, _| window_state.contains_key(key));
    }

    pub fn get_state(&self) -> AppData {
        (
            self.previous_app_map.clone(),
            self.previous_app_usage_map.clone(),
            self.previous_classification_map.clone(),
            self.previous_idle_map.clone(),
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

pub struct WindowStateManager;

impl WindowStateManager {
    pub fn get_current_state() -> BTreeMap<String, WindowDetails> {
        let window_state = WindowsHandle::get_window_titles();
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
                "Idle Time - {}",
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
