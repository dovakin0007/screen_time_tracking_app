use log::error;
use rusqlite::{params, Connection};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, Mutex};

use super::models::{App, AppUsage};

pub async fn upset_app_usage(
    conn: Arc<Mutex<Connection>>,
    mut rx: mpsc::UnboundedReceiver<(HashMap<String, App>, HashMap<String, AppUsage>)>,
) {
    let conn = conn.lock().await;

    while let Some(recv) = rx.recv().await {
        let (apps, app_usages) = recv;

        for (app_id, app) in apps {
            if let Err(err) = conn.execute(
                "INSERT INTO apps (name, path) VALUES (?1, ?2)
                 ON CONFLICT(name) DO UPDATE SET path = excluded.path;",
                params![app.name, app.path],
            ) {
                error!("Error inserting or updating app '{}': {}", app_id, err);
            }
        }

        for (usage_id, usage) in app_usages {
            if let Err(err) = conn.execute(
                "INSERT INTO app_usages (
                    id, session_id, application_name, current_screen_title, start_time,
                    last_updated_time
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                    last_updated_time = excluded.last_updated_time;",
                params![
                    usage.app_id,
                    usage.session_id,
                    usage.application_name,
                    usage.current_screen_title,
                    usage.start_time,
                    usage.last_updated_time,
                ],
            ) {
                error!(
                    "Error inserting or updating app usage '{}': {}",
                    usage_id, err
                );
            }
        }
    }
}
