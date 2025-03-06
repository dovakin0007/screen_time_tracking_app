use internment::ArcIntern;
use log::{debug, error};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};
use tokio::{
    sync::{mpsc, Mutex},
    time::Instant,
};

use super::models::{App, AppUsage, ClassificationSerde, IdlePeriod, Sessions, WindowUsage};

const APP_UPSERT_QUERY: &str = r#"
    INSERT INTO apps (name, path)
    VALUES (?1, ?2)
    ON CONFLICT(name) DO UPDATE SET
        path = excluded.path
"#;

const USAGE_UPSERT_QUERY: &str = r#"
    INSERT INTO window_activity_usage (
        id, 
        session_id, 
        app_time_id, 
        application_name, 
        current_screen_title, 
        start_time,
        last_updated_time
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
    ON CONFLICT(id) DO UPDATE SET
        last_updated_time = excluded.last_updated_time
"#;

const SESSION_UPSET_QUERY: &str = r#"
        INSERT INTO sessions (id, date)
        VALUES (?1, ?2)
    "#;

const CLASSIFICATION_UPSET_QUERY: &str = r#"
        INSERT INTO app_classifications (application_name, classification)
        VALUES (?1, NULL)
        ON CONFLICT(application_name)
        DO NOTHING;
    "#;

type ReceiveUsageInfo = mpsc::UnboundedReceiver<(
    HashMap<ArcIntern<String>, App>,
    HashMap<ArcIntern<String>, WindowUsage>,
    HashSet<ArcIntern<String>>,
    HashMap<ArcIntern<String>, IdlePeriod>,
    HashMap<ArcIntern<String>, AppUsage>,
)>;

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

    pub async fn fetch_all_classification(&self) -> SqliteResult<VecDeque<ClassificationSerde>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT ac.application_name, ap.path, ac.classification
             FROM app_classifications ac
             LEFT JOIN apps as ap ON ac.application_name = ap.name
             WHERE ac.classification IS NULL OR ac.classification = 'Unclassified'
             LIMIT 50;",
        )?;
        let classification_iter = stmt.query_map([], |row| {
            Ok(ClassificationSerde {
                name: row.get(0)?,
                classification: row.get(2)?,
                path: row.get(1)?,
            })
        })?;

        let mut classifications = VecDeque::with_capacity(50);
        for (i, classification) in classification_iter.enumerate() {
            classifications.insert(i, classification?);
        }
        Ok(classifications)
    }

    pub async fn update_classification(&self, content: ClassificationSerde) -> SqliteResult<()> {
        const MAX_RETRIES: u64 = 5;
        const RETRY_DELAY_MS: u64 = 100;

        let mut attempts = 0;
        loop {
            let conn = self.conn.lock().await;
            let result = conn
                .prepare(
                    "UPDATE app_classifications SET classification = ? WHERE application_name = ?;",
                )
                .and_then(|mut stmt| stmt.execute(params![content.classification, content.name,]));
            match result {
                Ok(_) => return Ok(()),
                Err(rusqlite::Error::SqliteFailure(err, s)) => {
                    if err.code == rusqlite::ffi::ErrorCode::DatabaseLocked
                        && attempts < MAX_RETRIES
                    {
                        attempts += 1;
                        drop(conn);
                        tokio::time::sleep(std::time::Duration::from_millis(
                            RETRY_DELAY_MS * attempts,
                        ))
                        .await;
                        continue;
                    }
                    return Err(rusqlite::Error::SqliteFailure(err, s));
                }
                Err(err) => return Err(err),
            }
        }
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
    db_handler: Arc<DbHandler>,
    session: Sessions,
    mut rx: ReceiveUsageInfo,
) {
    let _ = db_handler.update_session(session).await;
    while let Some((apps, window_usages, classifications, idle_periods, app_usages)) =
        rx.recv().await
    {
        let start = Instant::now();

        let result = process_updates(
            &db_handler,
            &apps,
            &window_usages,
            &classifications,
            &idle_periods,
            &app_usages,
        )
        .await;

        let metrics = DbMetrics::new(
            apps.len(),
            window_usages.len(),
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
    apps: &HashMap<ArcIntern<String>, App>,
    window_usages: &HashMap<ArcIntern<String>, WindowUsage>,
    classifications: &HashSet<ArcIntern<String>>,
    idle_periods: &HashMap<ArcIntern<String>, IdlePeriod>,
    app_usages: &HashMap<ArcIntern<String>, AppUsage>,
) -> SqliteResult<()> {
    debug!("Starting batch database update process");
    let start = std::time::Instant::now();

    let mut conn = db_handler.conn.lock().await;
    debug!("Database connection locked");

    let tx = conn.transaction()?;
    debug!("Transaction started");

    debug!("Processing {} apps", apps.len());
    for app in apps.values() {
        match tx.execute(APP_UPSERT_QUERY, params![app.name.to_string(), app.path.to_string()]) {
            Ok(_) => debug!("Successfully upserted app: {}", app.name),
            Err(err) => {
                error!("Failed to upsert app '{}': {}", app.name, err);
                return Err(err);
            }
        }
    }

    for app_time in app_usages.values() {
        match tx.execute(
            r#"INSERT INTO app_usage_time_period (id, app_name, start_time, end_time)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
            end_time = excluded.end_time"#,
            params![
                app_time.id,
                app_time.app_name.to_string(),
                app_time.start_time,
                app_time.end_time,
            ],
        ) {
            Ok(_) => debug!(
                "Successfully upserted app time for app: {}",
                app_time.app_name
            ),
            Err(err) => {
                error!(
                    "Failed to upsert app time for '{}': {}",
                    app_time.app_name, err
                );
                return Err(err);
            }
        }
    }

    debug!("Processing {} app usages", window_usages.len());
    for usage in window_usages.values() {
        match tx.execute(
            USAGE_UPSERT_QUERY,
            params![
                usage.app_id,
                usage.session_id,
                usage.app_time_id,
                usage.application_name.to_string(),
                usage.current_screen_title.to_string(),
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
    for classification in classifications {
        match tx.execute(CLASSIFICATION_UPSET_QUERY, params![classification.to_string()]) {
            Ok(_) => debug!(
                "Successfully upserted classification for: {}",
                classification
            ),
            Err(err) => {
                error!(
                    "Failed to upsert classification for '{}': {}",
                    classification, err
                );
                return Err(err);
            }
        }
    }

    debug!("Processing {} idle periods", idle_periods.len());
    for idle_period in idle_periods.values() {
        match tx.execute(
            r#"INSERT INTO app_idle_time_period (id, app_id, window_id ,session_id, app_name, start_time, end_time)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
            end_time = excluded.end_time"#,
            params![
                idle_period.id,
                idle_period.app_id,
                idle_period.window_id,
                idle_period.session_id,
                idle_period.app_name.to_string(),
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
        "Batch update completed in {:?}. Processed: {} apps, {} usages, {} classifications, {} idle periods, {} app times",
        start.elapsed(),
        apps.len(),
        window_usages.len(),
        classifications.len(),
        idle_periods.len(),
        app_usages.len(),
    );

    Ok(())
}
