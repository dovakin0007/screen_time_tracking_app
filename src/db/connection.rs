use log::{debug, error};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio::time::Instant;

use super::models::{App, AppUsage, Classification, IdlePeriod, Sessions};

const APP_UPSERT_QUERY: &'static str = r#"
    INSERT INTO apps (name, path)
    VALUES (?1, ?2)
    ON CONFLICT(name) DO UPDATE SET
        path = excluded.path
"#;

const USAGE_UPSERT_QUERY: &'static str = r#"
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

const SESSION_UPSET_QUERY: &'static str = r#"
        INSERT INTO sessions (id, date)
        VALUES (?1, ?2)
    "#;

const CLASSIFICATION_UPSET_QUERY: &'static str = r#"
        INSERT INTO activity_classifications (application_name, current_screen_title, classification)
        VALUES (?1, ?2, NULL)
        ON CONFLICT(current_screen_title)
        DO NOTHING;
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

    async fn update_classifications(
        &self,
        classifications: &HashMap<String, Classification>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        for (usage_name, classification) in classifications {
            match conn.execute(
                CLASSIFICATION_UPSET_QUERY,
                params![classification.name, classification.window_title],
            ) {
                Ok(_) => debug!("Successfully updated usage: {}", usage_name),
                Err(err) => {
                    error!("Error updating app usage '{}': {}", usage_name, err);
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    async fn update_session(&self, session: Sessions) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        match conn.execute(
            SESSION_UPSET_QUERY,
            params![session.session_id, session.session_date],
        ) {
            Ok(_) => debug!("Successfully updated session: {}", session.session_id),
            Err(err) => {
                error!("Error updating app usage '{}': {}", session.session_id, err);
                return Err(err);
            }
        }
        Ok(())
    }

    async fn update_idle_periods(
        &self,
        idle_periods: &HashMap<String, IdlePeriod>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().await;

        for idle_period in idle_periods.values() {
            // Ensure referenced rows exist

            // Insert into idle_periods
            conn.execute(
                r#"INSERT INTO idle_periods (id, app_id, session_id, app_name, start_time, end_time, idle_type)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(id) DO UPDATE SET
                    end_time = excluded.end_time"#,
                params![
                    idle_period.id,
                    idle_period.app_id,
                    idle_period.session_id,
                    idle_period.app_name,
                    idle_period.start_time,
                    idle_period.end_time,
                    idle_period.idle_type,
                ],
            )?;
        }
        println!("worked");
        Ok(())
    }
}

/// Metrics for database operations
#[derive(Debug)]
struct DbMetrics {
    apps_count: usize,
    usages_count: usize,
    classifications_count: usize,
    idle_state_count: usize,
    duration: std::time::Duration,
}

impl DbMetrics {
    fn new(
        apps_count: usize,
        usages_count: usize,
        classifications_count: usize,
        idle_state_count: usize,
        duration: std::time::Duration,
    ) -> Self {
        Self {
            apps_count,
            usages_count,
            classifications_count,
            idle_state_count,
            duration,
        }
    }

    fn log(&self) {
        debug!(
            "DB Update Metrics - Apps: {}, Usages: {}, Classifications: {}, Idle: {}, Duration: {:?}",
            self.apps_count, self.usages_count, self.classifications_count, self.idle_state_count, self.duration
        );
    }
}

/// Process database updates for apps, their usage, and classifications
pub async fn upsert_app_usage(
    conn: Arc<Mutex<Connection>>,
    session: Sessions,
    mut rx: mpsc::UnboundedReceiver<(
        HashMap<String, App>,
        HashMap<String, AppUsage>,
        HashMap<String, Classification>,
        HashMap<String, IdlePeriod>, // Added idle periods
    )>,
) {
    let db_handler = DbHandler::new(conn);
    let _ = db_handler.update_session(session).await;
    while let Some((apps, app_usages, classifications, idle_periods)) = rx.recv().await {
        let start = Instant::now();

        // Process updates
        let result = process_updates(
            &db_handler,
            &apps,
            &app_usages,
            &classifications,
            &idle_periods,
        )
        .await;

        // Log metrics
        let metrics = DbMetrics::new(
            apps.len(),
            app_usages.len(),
            classifications.len(),
            idle_periods.len(), // Added to metrics
            start.elapsed(),
        );
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
    classifications: &HashMap<String, Classification>,
    idle_periods: &HashMap<String, IdlePeriod>, // Added idle periods
) -> SqliteResult<()> {
    // Update apps first as they are referenced by usages
    println!(
        "app usage {:?},\n idle periods{:?}",
        app_usages, idle_periods
    );
    db_handler.update_apps(apps).await?;
    db_handler.update_app_usages(app_usages).await?;
    db_handler.update_classifications(classifications).await?;

    db_handler.update_idle_periods(idle_periods).await?; // Added idle period updates
    Ok(())
}
