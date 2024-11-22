use std::time::Instant;
use std::{collections::HashMap, time};

use chrono::Local;
use std::sync::Arc;
use tokio::sync::Mutex;

mod app_data;
mod rpc_service;
mod win;

use app_data::*;
use win::*;

pub async fn app(app_spent_time_map: Arc<Mutex<HashMap<String, AppData>>>) {
    let start = Instant::now();

    let mut app_spent_time_map = (app_spent_time_map.lock()).await;

    let mut last_val = WindowsHandle::get_window_title();

    let dt1 = Local::now();
    let today = dt1.date_naive();

    let current_date = today.to_string();

    let idle_time = WindowsHandle::get_last_input_info().unwrap().as_secs();

    if idle_time >= 300 {
        last_val = "Idle Time".parse().unwrap()
    }

    if app_spent_time_map.contains_key(last_val.as_str()) {
        app_spent_time_map
            .get_mut(last_val.as_str())
            .unwrap()
            .update_seconds(1);
    } else {
        let mut main_key = last_val.clone().to_owned();
        main_key.push_str(&current_date);
        app_spent_time_map.insert(
            last_val.to_owned(),
            AppData::new(
                last_val.clone().to_owned(),
                1,
                current_date.clone(),
                main_key,
            ),
        );
    }

    if app_spent_time_map
        .get(&*last_val)
        .unwrap()
        .get_date()
        .to_string()
        != current_date.clone()
    {
        app_spent_time_map
            .get_mut(&*last_val)
            .unwrap()
            .reset_time(current_date.clone())
    }

    let duration = start.elapsed();

    update_db(&app_spent_time_map).await.unwrap();
    println!("Time elapsed in expensive_function() is: {:?}", duration);
    let time_delay_for_function = 1000 - duration.as_millis();
    let delay = time::Duration::from_millis(time_delay_for_function.try_into().unwrap_or(1000));
    tokio::time::sleep(delay).await;
}

#[tokio::main]
async fn main() {
    let mut app_time_spent_map = HashMap::new();
    let current_day = Local::now();
    let today_date = current_day.date_naive();
    let app_spent_time_new: &mut HashMap<String, AppData> =
        get_data_from_db(&mut app_time_spent_map, &today_date)
            .await
            .unwrap();
    let app_spent_time: Arc<Mutex<HashMap<String, AppData>>> =
        Arc::new(Mutex::new(app_spent_time_new.to_owned()));

    loop {
        let app_spent_time = app_spent_time.clone();
        tokio::spawn(app(app_spent_time)).await.unwrap();
    }
}
