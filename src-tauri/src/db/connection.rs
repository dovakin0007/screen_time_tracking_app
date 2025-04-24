use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{Local, NaiveDate};
use internment::ArcIntern;
use log::{debug, error};
use rusqlite::{params, Connection, Result as SqliteResult};
use tokio::{
    sync::{mpsc, Mutex},
    time::Instant,
};

use super::models::{
    App, AppUsage, AppUsageQuery, ClassificationSerde, IdlePeriod, Sessions, WindowUsage,
};
use crate::fs_watcher::start_menu_watcher::ShellLinkInfo;

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

const UPSET_TIME_LIMIT: &str = r#"
    INSERT INTO daily_limits (
            app_name, time_limit, should_alert, should_close, alert_before_close, alert_duration
        ) VALUES (?1, 60, 0, 0, 0, 360)
        ON CONFLICT(app_name) DO NOTHING
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

const APP_USAGE_QUERY: &str = r#"
    WITH app_total AS (
        SELECT 
            app_name,
            SUM(
                CASE 
                    WHEN end_time IS NULL THEN 
                        strftime('%s', 'now') - strftime('%s', start_time)
                    ELSE 
                        strftime('%s', end_time) - strftime('%s', start_time)
                END
            ) AS total_seconds
        FROM app_usage_time_period
        WHERE DATE(start_time) BETWEEN :previous_date AND :current_date
        GROUP BY app_name
    ),
    app_idle AS (
        SELECT 
            app_name,
            COUNT(*) AS idle_count,
            SUM(strftime('%s', end_time) - strftime('%s', start_time)) AS idle_seconds
        FROM app_idle_time_period
        WHERE DATE(start_time) BETWEEN :previous_date AND :current_date
        GROUP BY app_name
    )
    SELECT 
        t.app_name AS AppName,
        ROUND(t.total_seconds / 3600.0, 2) AS TotalHours,
        ROUND(COALESCE(i.idle_seconds, 0) / 3600.0, 2) AS IdleHours,
        CASE 
            WHEN t.total_seconds > 0 
            THEN ROUND(((t.total_seconds - COALESCE(i.idle_seconds, 0)) * 100.0 / t.total_seconds), 2) 
            ELSE NULL 
        END AS ActivePercentage,
        dl.time_limit AS TimeLimit,
        dl.should_alert AS ShouldAlert,
        dl.should_close AS ShouldClose,
        dl.alert_before_close AS AlertBeforeClose,
        dl.alert_duration AS AlertDuration
    FROM app_total t
    LEFT JOIN app_idle i ON t.app_name = i.app_name
    LEFT JOIN daily_limits dl ON t.app_name = dl.app_name
    ORDER BY TotalHours DESC;
    "#;

const APP_USAGE_QUERY_APP_NAME: &str = r#"
    WITH app_total AS (
        SELECT 
            app_name,
            SUM(
                CASE 
                    WHEN end_time IS NULL THEN 
                        strftime('%s', 'now') - strftime('%s', start_time)
                    ELSE 
                        strftime('%s', end_time) - strftime('%s', start_time)
                END
            ) AS total_seconds
        FROM app_usage_time_period
        WHERE DATE(start_time) BETWEEN :previous_date AND :current_date
          AND (:app_name IS NULL OR app_name = :app_name)
        GROUP BY app_name
    ),
    app_idle AS (
        SELECT 
            app_name,
            COUNT(*) AS idle_count,
            SUM(strftime('%s', end_time) - strftime('%s', start_time)) AS idle_seconds
        FROM app_idle_time_period
        WHERE DATE(start_time) BETWEEN :previous_date AND :current_date
          AND (:app_name IS NULL OR app_name = :app_name)
        GROUP BY app_name
    )
    SELECT 
        t.app_name AS AppName,
        ROUND(t.total_seconds / 3600.0, 2) AS TotalHours,
        ROUND(COALESCE(i.idle_seconds, 0) / 3600.0, 2) AS IdleHours,
        CASE 
            WHEN t.total_seconds > 0 
            THEN ROUND(((t.total_seconds - COALESCE(i.idle_seconds, 0)) * 100.0 / t.total_seconds), 2) 
            ELSE NULL 
        END AS ActivePercentage,
        dl.time_limit AS TimeLimit,
        dl.should_alert AS ShouldAlert,
        dl.should_close AS ShouldClose,
        dl.alert_before_close AS AlertBeforeClose,
        dl.alert_duration AS AlertDuration
    FROM app_total t
    LEFT JOIN app_idle i ON t.app_name = i.app_name
    LEFT JOIN daily_limits dl ON t.app_name = dl.app_name
    ORDER BY TotalHours DESC;
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
    pub async fn get_app_usage_details(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> SqliteResult<Vec<AppUsageQuery>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(APP_USAGE_QUERY)?;

        let app_usage_iter = stmt.query_map(
            &[
                (":current_date", end_date.to_string().as_str()),
                (":previous_date", start_date.to_string().as_str()),
            ],
            |row| {
                Ok(AppUsageQuery {
                    app_name: row.get(0)?,
                    total_hours: row.get(1)?,
                    idle_hours: row.get(2)?,
                    active_percentage: row.get(3).ok(),
                    time_limit: row.get(4).ok(),
                    should_alert: row.get(5).ok(),
                    should_close: row.get(6).ok(),
                    alert_before_close: row.get(7).ok(),
                    alert_duration: row.get(8).ok(),
                })
            },
        )?;

        app_usage_iter.collect()
    }

    pub async fn get_current_app_usage_details(&self) -> SqliteResult<Vec<AppUsageQuery>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(APP_USAGE_QUERY_APP_NAME)?;
        let current_date = Local::now().date_naive();
        let seven_days_ago = current_date;

        let app_usage_iter = stmt.query_map(
            &[
                (":current_date", current_date.to_string().as_str()),
                (":previous_date", seven_days_ago.to_string().as_str()),
            ],
            |row| {
                Ok(AppUsageQuery {
                    app_name: row.get(0)?,
                    total_hours: row.get(1)?,
                    idle_hours: row.get(2)?,
                    active_percentage: row.get(3).ok(),
                    time_limit: row.get(4).ok(),
                    should_alert: row.get(5).ok(),
                    should_close: row.get(6).ok(),
                    alert_before_close: row.get(7).ok(),
                    alert_duration: row.get(8).ok(),
                })
            },
        )?;

        app_usage_iter.collect()
    }

    pub async fn insert_update_app_limits(
        &self,
        app_name: &str,
        time_limit: u32,
        should_alert: bool,
        should_close: bool,
        alert_before_close: bool,
        alert_duration: u32,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO daily_limits (app_name, time_limit, should_alert, should_close, alert_before_close, alert_duration)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(app_name)
             DO UPDATE SET
                time_limit = excluded.time_limit,
                should_alert = excluded.should_alert,
                should_close = excluded.should_close,
                alert_before_close = excluded.alert_before_close,
                alert_duration = excluded.alert_duration",
            params![app_name, time_limit, should_alert, should_close, alert_before_close, alert_duration],
        )?;
        Ok(())
    }

    pub async fn get_specific_app_details(
        &self,
        app_name: &str,
    ) -> Result<AppUsageQuery, rusqlite::Error> {
        let app_name = app_name;
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(APP_USAGE_QUERY_APP_NAME)?;
        let current_date = Local::now().date_naive();
        let seven_days_ago = current_date;
        let mut app_usage_iter = stmt.query_map(
            &[
                (":app_name", app_name),
                (":current_date", current_date.to_string().as_str()),
                (":previous_date", seven_days_ago.to_string().as_str()),
            ],
            |row| {
                Ok(AppUsageQuery {
                    app_name: row.get(0)?,
                    total_hours: row.get(1)?,
                    idle_hours: row.get(2)?,
                    active_percentage: row.get(3).ok(),
                    time_limit: row.get(4).ok(),
                    should_alert: row.get(5).ok(),
                    should_close: row.get(6).ok(),
                    alert_before_close: row.get(7).ok(),
                    alert_duration: row.get(8).ok(),
                })
            },
        )?;

        match app_usage_iter.next() {
            Some(Ok(v)) => Ok(v),
            Some(Err(e)) => Err(e),
            None => Err(rusqlite::Error::InvalidQuery),
        }
    }

    pub async fn insert_menu_shell_links(&self, apps: ShellLinkInfo) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            INSERT INTO shell_link_info (
                link,
                target_path,
                arguments,
                icon_base64_image,
                working_directory,
                description
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(link) DO UPDATE SET
                target_path = excluded.target_path,
                arguments = excluded.arguments,
                icon_base64_image = excluded.icon_base64_image,
                working_directory = excluded.working_directory,
                description = excluded.description
            "#,
            params![
                apps.link,
                apps.target_path,
                apps.arguments,
                apps.icon_base64_image,
                apps.working_directory,
                apps.description
            ],
        )?;
        Ok(())
    }

    pub async fn get_all_menu_paths(&self) -> SqliteResult<Vec<PathBuf>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT link FROM shell_link_info")?;

        let rows = stmt.query_map([], |row| {
            let link_path: String = row.get(0)?;
            Ok(PathBuf::from(link_path))
        })?;

        let mut paths = Vec::new();
        for row in rows {
            paths.push(row?);
        }

        Ok(paths)
    }

    pub async fn delete_menu_shell_link(&self, path: &Path) -> SqliteResult<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM shell_link_info WHERE link = ?1",
            params![path.to_string_lossy()],
        )?;
        Ok(())
    }

    pub async fn get_all_shell_links(&self) -> SqliteResult<Vec<ShellLinkInfo>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
        .prepare("SELECT link, target_path, arguments, icon_base64_image, working_directory, description FROM shell_link_info")?;

        let shell_links_iter = stmt.query_map([], |row| {
            Ok(ShellLinkInfo {
                link: row.get(0)?,
                target_path: row.get(1)?,
                arguments: row.get(2)?,
                icon_base64_image: row.get(3)?,
                working_directory: row.get(4)?,
                description: row.get(5)?,
            })
        })?;

        let mut shell_links = Vec::new();
        for link in shell_links_iter {
            shell_links.push(link?);
        }
        return Ok(shell_links);
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
        match tx.execute(
            APP_UPSERT_QUERY,
            params![app.name.to_string(), app.path.to_string()],
        ) {
            Ok(_) => debug!("Successfully upserted app: {}", app.name),
            Err(err) => {
                error!("Failed to upsert app '{}': {}", app.name, err);
                return Err(err);
            }
        }

        match tx.execute(UPSET_TIME_LIMIT, params![app.name.to_string()]) {
            Ok(_) => debug!("Successfully upserted app: {}", app.name),
            Err(err) => {
                error!("Failed to upsert limit info '{}': {}", app.name, err);
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
        match tx.execute(
            CLASSIFICATION_UPSET_QUERY,
            params![classification.to_string()],
        ) {
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
