use std::time::Instant;
use std::time;

use chrono::Local;
use db::connection::{insert_app_data, insert_app_usage_data};
use db::models::{AddApp, NewAppUsage};
use diesel::PgConnection;
use platform::Platform;
use std::sync::Arc;
use tokio::sync::Mutex;

mod platform;
mod rpc_service;
mod db;
use platform::windows::{self, *};

pub async fn app(pg_conn: Arc<Mutex<PgConnection>>) {
    let start = Instant::now();

    let (mut window_activity, app_name, path) = windows::WindowsHandle::get_window_title();


    let dt1 = Local::now();
    let today = dt1.date_naive();


    let idle_time = WindowsHandle::get_last_input_info().unwrap().as_secs();
    if idle_time >= 300 {
        window_activity = "Idle Time".parse().unwrap()
    }


    let app_data = AddApp {
        id: uuid::Uuid::new_v4(),
        app_name: app_name.clone(),
        app_path: path,
    };
    let new_app_usage = NewAppUsage {
        id: uuid::Uuid::new_v4(),
        app_name: app_name.clone(),
        screen_title_name: Some(window_activity),
        duration_in_seconds: 1,
    };


    insert_app_data(pg_conn.clone(), app_data).await;
    insert_app_usage_data(pg_conn.clone(), new_app_usage).await;
    let duration = start.elapsed();
    println!("Time elapsed in expensive_function() is: {:?}", duration);
    let time_delay_for_function = 1000 - duration.as_millis();
    let delay = time::Duration::from_millis(time_delay_for_function.try_into().unwrap_or(1000));
    tokio::time::sleep(delay).await;
}

#[tokio::main]
async fn main() {
    let pg_conn = db::connection::connect();
    let postgres_diesel = Arc::new(Mutex::new(pg_conn));
    // let current_day = Local::now();
    // let today_date = current_day.date_naive();

    loop {
        tokio::spawn(app(postgres_diesel.clone())).await.unwrap();
    }
}
