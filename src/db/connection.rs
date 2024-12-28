use log::{debug, error};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio::time::Instant;

use super::models::{App, AppUsage};

const APP_UPSERT_QUERY: &str = r#"
    INSERT INTO apps (name, path) 
    VALUES (?1, ?2)
    ON CONFLICT(name) DO UPDATE SET 
        path = excluded.path
"#;

const USAGE_UPSERT_QUERY: &str = r#"
    INSERT INTO app_usages (
        id, 
        session_id, 
        application_name, 
        current_screen_title, 
        start_time,
        last_updated_time
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
    ON CONFLICT(id) DO UPDATE SET
        last_updated_time = excluded.last_updated_time
"#;

/// Database operations handler
struct DbHandler {
    conn: Arc<Mutex<Connection>>,
}

impl DbHandler {
    fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Update app information in the database
    async fn update_apps(&self, apps: &HashMap<String, App>) -> SqliteResult<()> {
        let conn = self.conn.lock().await;

        for (app_id, app) in apps {
            match conn.execute(APP_UPSERT_QUERY, params![app.name, app.path]) {
                Ok(_) => debug!("Successfully updated app: {}", app_id),
                Err(err) => {
                    error!("Error updating app '{}': {}", app_id, err);
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    /// Update app usage information in the database
    async fn update_app_usages(&self, app_usages: &HashMap<String, AppUsage>) -> SqliteResult<()> {
        let conn = self.conn.lock().await;

        for (usage_id, usage) in app_usages {
            match conn.execute(
                USAGE_UPSERT_QUERY,
                params![
                    usage.app_id,
                    usage.session_id,
                    usage.application_name,
                    usage.current_screen_title,
                    usage.start_time,
                    usage.last_updated_time,
                ],
            ) {
                Ok(_) => debug!("Successfully updated usage: {}", usage_id),
                Err(err) => {
                    error!("Error updating app usage '{}': {}", usage_id, err);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}

/// Metrics for database operations
#[derive(Debug)]
struct DbMetrics {
    apps_count: usize,
    usages_count: usize,
    duration: std::time::Duration,
}

impl DbMetrics {
    fn new(apps_count: usize, usages_count: usize, duration: std::time::Duration) -> Self {
        Self {
            apps_count,
            usages_count,
            duration,
        }
    }

    fn log(&self) {
        debug!(
            "DB Update Metrics - Apps: {}, Usages: {}, Duration: {:?}",
            self.apps_count, self.usages_count, self.duration
        );
    }
}

/// Process database updates for apps and their usage
pub async fn upset_app_usage(
    conn: Arc<Mutex<Connection>>,
    mut rx: mpsc::UnboundedReceiver<(HashMap<String, App>, HashMap<String, AppUsage>)>,
) {
    let db_handler = DbHandler::new(conn);

    while let Some((apps, app_usages)) = rx.recv().await {
        let start = Instant::now();

        // Process updates
        let result = process_updates(&db_handler, &apps, &app_usages).await;

        // Log metrics
        let metrics = DbMetrics::new(apps.len(), app_usages.len(), start.elapsed());
        metrics.log();

        // Handle any errors
        if let Err(err) = result {
            error!("Failed to process database updates: {}", err);
        }
    }
}

/// Process both app and usage updates in a single transaction
async fn process_updates(
    db_handler: &DbHandler,
    apps: &HashMap<String, App>,
    app_usages: &HashMap<String, AppUsage>,
) -> SqliteResult<()> {
    // Update apps first as they are referenced by usages
    db_handler.update_apps(apps).await?;
    db_handler.update_app_usages(app_usages).await?;
    Ok(())
}
