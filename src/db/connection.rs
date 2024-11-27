use diesel::PgConnection;
use diesel::prelude::*;
use dotenvy::dotenv;
use tokio::sync::Mutex;
use std::future::Future;
use std::sync::Arc;
use std::env;

use diesel::dsl::now;
use super::models::{AddApp, NewAppUsage};
use super::schema::app::dsl::*;
use super::schema::app_usage::dsl::{app_usage, duration_in_seconds, updated_at, screen_title_name};

pub fn connect() -> PgConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set"); PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub fn insert_app_data(conn: Arc<Mutex<PgConnection>>, app_data: AddApp) ->  impl Future<Output = ()> {
    async move {
        let mut x = conn.lock().await;
        let _ = diesel::insert_into(app).values(&app_data).on_conflict(app_name).do_nothing().execute(&mut *x);
    }
}

pub fn insert_app_usage_data(conn: Arc<Mutex<PgConnection>>, usage_data: NewAppUsage) -> impl Future<Output = ()> {
    async move{
        let mut x = conn.lock().await;
        let _ = diesel::insert_into(app_usage)
        .values(&usage_data)
        .on_conflict(screen_title_name)
        .do_update()
        .set((
            duration_in_seconds.eq(duration_in_seconds + 1),
            updated_at.eq(now),
        ))
        .execute(&mut *x).map_err(|e| {
            eprintln!("{:?}", e);
        });
    }
}

// pub fn populate_app_usage(conn: &mut PgConnection) -> HashMap<App, Vec<AppUsage>> {
//     let mut db_map: HashMap<App, Vec<AppUsage>> = HashMap::new();
//     let vec_app_data = app.select(App::as_select()).load(conn).unwrap_or(vec![App::default()]);
//     for val in vec_app_data {
//         db_map.insert()
//     }
//     return db_map;
//     /* `HashMap<App, Vec<AppUsage>>` value */
// }