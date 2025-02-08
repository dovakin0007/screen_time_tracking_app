use log::{debug, error};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::PathBuf;
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

pub struct DbHandler {
    conn: Arc<Mutex<Connection>>,
}

impl DbHandler {
    pub fn new(connection_string: PathBuf) -> Self {
        let conn = Arc::new(Mutex::new(
            Connection::open(&connection_string).unwrap_or_else(|err| {
                panic!(
                    "Failed to open database connection at {:?}: {:?}",
                    connection_string, err
                );
            }),
        ));
        Self { conn }
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
}

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

pub async fn upsert_app_usage(
    db_handler: DbHandler,
    session: Sessions,
    mut rx: mpsc::UnboundedReceiver<(
        HashMap<String, App>,
        HashMap<String, AppUsage>,
        HashMap<String, Classification>,
        HashMap<String, IdlePeriod>,
    )>,
) {
    let _ = db_handler.update_session(session).await;
    while let Some((apps, app_usages, classifications, idle_periods)) = rx.recv().await {
        let start = Instant::now();

        let result = process_updates(
            &db_handler,
            &apps,
            &app_usages,
            &classifications,
            &idle_periods,
        )
        .await;

        let metrics = DbMetrics::new(
            apps.len(),
            app_usages.len(),
            classifications.len(),
            idle_periods.len(),
            start.elapsed(),
        );
        metrics.log();

        if let Err(err) = result {
            error!("Failed to process database updates: {}", err);
        }
    }
}

async fn process_updates(
    db_handler: &DbHandler,
    apps: &HashMap<String, App>,
    app_usages: &HashMap<String, AppUsage>,
    classifications: &HashMap<String, Classification>,
    idle_periods: &HashMap<String, IdlePeriod>,
) -> SqliteResult<()> {
    debug!("Starting batch database update process");
    let start = std::time::Instant::now();

    let mut conn = db_handler.conn.lock().await;
    debug!("Database connection locked");

    let tx = conn.transaction()?;
    debug!("Transaction started");

    debug!("Processing {} apps", apps.len());
    for (_, app) in apps {
        match tx.execute(APP_UPSERT_QUERY, params![app.name, app.path]) {
            Ok(_) => debug!("Successfully upserted app: {}", app.name),
            Err(err) => {
                error!("Failed to upsert app '{}': {}", app.name, err);
                return Err(err);
            }
        }
    }

    debug!("Processing {} app usages", app_usages.len());
    for (_, usage) in app_usages {
        match tx.execute(
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
            Ok(_) => debug!(
                "Successfully upserted usage for app: {}",
                usage.application_name
            ),
            Err(err) => {
                error!(
                    "Failed to upsert usage for '{}': {}",
                    usage.application_name, err
                );
                return Err(err);
            }
        }
    }

    debug!("Processing {} classifications", classifications.len());
    for (_, classification) in classifications {
        match tx.execute(
            CLASSIFICATION_UPSET_QUERY,
            params![classification.name, classification.window_title],
        ) {
            Ok(_) => debug!(
                "Successfully upserted classification for: {}",
                classification.name
            ),
            Err(err) => {
                error!(
                    "Failed to upsert classification for '{}': {}",
                    classification.name, err
                );
                return Err(err);
            }
        }
    }

    debug!("Processing {} idle periods", idle_periods.len());
    for idle_period in idle_periods.values() {
        match tx.execute(
            r#"INSERT INTO idle_periods (id, app_id, session_id, app_name, start_time, end_time)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
            end_time = excluded.end_time"#,
            params![
                idle_period.id,
                idle_period.app_id,
                idle_period.session_id,
                idle_period.app_name,
                idle_period.start_time,
                idle_period.end_time,
            ],
        ) {
            Ok(_) => debug!(
                "Successfully upserted idle period for app: {}",
                idle_period.app_name
            ),
            Err(err) => {
                error!(
                    "Failed to upsert idle period for '{}': {}",
                    idle_period.app_name, err
                );
                return Err(err);
            }
        }
    }

    match tx.commit() {
        Ok(_) => debug!("Transaction successfully committed"),
        Err(err) => {
            error!("Failed to commit transaction: {}", err);
            return Err(err);
        }
    }

    debug!(
        "Batch update completed in {:?}. Processed: {} apps, {} usages, {} classifications, {} idle periods",
        start.elapsed(),
        apps.len(),
        app_usages.len(),
        classifications.len(),
        idle_periods.len()
    );

    Ok(())
}
