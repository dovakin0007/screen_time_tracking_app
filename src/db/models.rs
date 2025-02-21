use chrono::{Local, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct App {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Default, Clone)]
pub struct AppUsage {
    pub session_id: String,
    pub app_time_id: String,
    pub app_id: String,
    pub application_name: String,
    pub current_screen_title: String,
    pub start_time: NaiveDateTime,
    pub last_updated_time: NaiveDateTime,
}

#[derive(Debug, Default, Clone)]
pub struct Classification {
    pub name: String,
    pub window_title: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClassificationSerde {
    pub name: String,
    pub window_title: String,
    pub path: String,
    pub classification: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Sessions {
    pub session_id: String,
    pub session_date: NaiveDate,
}

impl Sessions {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            session_date: Local::now().date_naive(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct IdlePeriod {
    pub id: String,
    pub app_id: String,
    pub window_id: String,
    pub session_id: String,
    pub app_name: String,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
}

#[derive(Debug, Default, Clone)]
pub struct AppTime {
    pub id: String,
    pub app_name: String,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
}
