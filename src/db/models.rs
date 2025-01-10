use chrono::{NaiveDate, NaiveDateTime};

// TODO: Add App Classification with unique names
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

#[derive(Debug, Default, Clone)]
pub struct Classification {
    pub name: String,
    pub window_title: String,
}

#[derive(Debug, Default, Clone)]
pub struct Sessions {
    pub session_id: String,
    pub session_date: NaiveDate,
}
