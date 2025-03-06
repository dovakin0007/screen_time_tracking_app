use chrono::{Local, NaiveDate, NaiveDateTime};
use internment::ArcIntern;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Eq, Hash, PartialEq)]
pub struct App {
    pub name: ArcIntern<String>,
    pub path: ArcIntern<String>,
}

#[derive(Debug, Default, Clone, Eq, Hash, PartialEq)]
pub struct WindowUsage {
    pub session_id: String,
    pub app_time_id: String,
    pub app_id: String,
    pub application_name: ArcIntern<String>,
    pub current_screen_title: ArcIntern<String>,
    pub start_time: NaiveDateTime,
    pub last_updated_time: NaiveDateTime,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ClassificationSerde {
    pub name: String,
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

#[derive(Debug, Default, Clone, Eq, Hash, PartialEq)]
pub struct IdlePeriod {
    pub id: String,
    pub app_id: String,
    pub window_id: String,
    pub session_id: String,
    pub app_name: ArcIntern<String>,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
}

#[derive(Debug, Default, Clone, Eq, Hash, PartialEq)]
pub struct AppUsage {
    pub id: String,
    pub app_name: ArcIntern<String>,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
}
