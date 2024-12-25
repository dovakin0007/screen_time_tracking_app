use chrono::{NaiveDate, NaiveDateTime};

#[derive(Debug, Default, Clone)]
pub struct App {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Default, Clone)]
pub struct AppUsage {
    pub session_id: String,
    pub app_id: String,
    pub application_name: String,
    pub current_screen_title: String,
    pub start_time: NaiveDateTime,
    pub last_updated_time: NaiveDateTime,
}

#[derive(Debug, Default)]
pub struct Sessions {
    pub id: String,
    pub session_date: NaiveDate,
}
