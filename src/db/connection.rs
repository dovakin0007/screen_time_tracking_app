use diesel::prelude::*;
use diesel::SqliteConnection;
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::models::AppUsage2;
use crate::db::models::{NewApp, NewAppUsage};
use crate::db::schema::{app, app_usage};

pub fn connect() -> SqliteConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub async fn upsert_apps(conn: Arc<Mutex<SqliteConnection>>, apps: Vec<NewApp>) {
    let mut conn = conn.lock().await;
    for app_data in apps {
        let _ = diesel::insert_into(app::table)
            .values(&app_data)
            .on_conflict(app::app_name) // Handle conflicts based on `app_name`
            .do_nothing()
            .execute(&mut *conn);
    }
}

pub async fn upsert_app_usages(conn: Arc<Mutex<SqliteConnection>>, usages: Vec<NewAppUsage>) {
    let mut conn = conn.lock().await;

    for usage in usages {
        let existing_app_usage = app_usage::table
            .filter(app_usage::session_id.eq(&usage.session_id))
            .filter(app_usage::app_name.eq(&usage.app_name))
            .select(AppUsage2::as_select()) // Explicitly select fields
            .first::<AppUsage2>(&mut *conn)
            .optional();

        match existing_app_usage {
            Ok(Some(existing_usage)) => {
                let result =
                    if existing_usage.is_active == 1 {
                        let result = diesel::update(app_usage::table.filter(
                            app_usage::screen_title_name.eq(existing_usage.screen_title_name),
                        ))
                        .set((
                            app_usage::is_active.eq(1),
                            app_usage::duration_in_seconds.eq(app_usage::duration_in_seconds + 1),
                        ))
                        .execute(&mut *conn);
                        result
                    } else {
                        let result = diesel::update(app_usage::table.filter(
                            app_usage::screen_title_name.eq(existing_usage.screen_title_name),
                        ))
                        .set((
                            app_usage::is_active.eq(0),
                            app_usage::duration_in_seconds.eq(app_usage::duration_in_seconds + 1),
                        ))
                        .execute(&mut *conn);
                        result
                    };
                if let Err(e) = result {
                    eprintln!("Failed to update app usage: {}", e);
                }
            }
            Ok(None) => {
                let result = diesel::insert_into(app_usage::table)
                    .values(&usage)
                    .execute(&mut *conn);

                if let Err(e) = result {
                    eprintln!("Failed to insert app usage: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to query app usage: {}", e);
            }
        }
    }
}
