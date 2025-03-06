use dirs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::tracker;
use tracker::Result;

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub session_id: String,
    pub db_path: PathBuf,
    pub log_path: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self> {
        let db_path = get_database_path()?;
        let log_path = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("application.log");

        Ok(Config {
            session_id: Uuid::new_v4().to_string(),
            db_path,
            log_path,
        })
    }
}

fn get_database_path() -> Result<PathBuf> {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or("%AppData%\\screen_time_tracking_app\\stop_procastinating.sqlite3".to_owned());
    Ok(if db_url.contains("%AppData%") {
        let app_data_path = dirs::config_dir().unwrap_or_else(|| Path::new(".").to_path_buf());
        PathBuf::from(db_url.replace("%AppData%", app_data_path.to_str().unwrap()))
    } else {
        PathBuf::from(db_url)
    })
}
