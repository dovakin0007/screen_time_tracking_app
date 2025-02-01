use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{get_database_path, tracker};
use tracker::Result;

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
