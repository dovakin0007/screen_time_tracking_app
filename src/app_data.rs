
use std::{result::Result, collections::HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::Local ;
use chrono::NaiveDate;
use mysql_async::{prelude::*, Opts};


pub type AppTimeSpentMap = Arc<Mutex<HashMap<String,AppData>>>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AppData{ 
    main_key: String,
    app_name: String,
    seconds_spent: u32,
    hours_spent: u32,
    minutes_spent: u32,
    current_day: String,
}


impl AppData {
    pub fn new(app_name: String, seconds_spent: u32, date: String, main_key:String) -> Self {
        Self {main_key, app_name, seconds_spent, hours_spent: 0, minutes_spent:0 , current_day:date}
    }

    pub fn store_data(app_id: String, app_name: String, seconds_spent: u32, hours_spent:u32, minutes_spent: u32) -> Self {
        let current_day= Local::now();
        let today_date = current_day.date_naive();
        Self { main_key:app_id , app_name, seconds_spent, hours_spent, minutes_spent, current_day: today_date.to_string()}
    }

    pub fn get_date(&self) -> &str {
        &self.current_day
    }

    pub fn update_seconds(&mut self, seconds: u32) {
        self.seconds_spent += seconds;
        if self.seconds_spent == 60{
         self.update_minutes();
        }
        if self.minutes_spent == 60 && self.seconds_spent == 60 {
            self.update_hours();
        }
    }  

   fn update_minutes(&mut self) {
        self.minutes_spent +=1;
        self.seconds_spent = 0;
    }

    fn  update_hours(&mut self) {
        self.hours_spent +=1;
        self.minutes_spent = 0;
        self.seconds_spent = 0;

    }

    pub fn reset_time(&mut self, date: String) {
        self.seconds_spent = 0;
        self.hours_spent = 0;
        self.minutes_spent = 0;
        self.current_day = date;
    }

}

pub async fn update_db(data :&HashMap<String, AppData>) -> Result<(), std::io::Error>{
    let database_url = Opts::from_url("mysql://root:dOVAKIN03!@localhost/screen_time_monitoring").unwrap();
    let pool = mysql_async::Pool::new(database_url);
    let mut conn = pool.get_conn().await.unwrap();
    let data_vec = data.values().clone().collect::<Vec<_>>();
    println!("connected");
    // let dt1= Local::now();
    // let today = dt1.date_naive();
    r"REPLACE INTO monitoring_table VALUES (
        :app_id,
        :app_name,
        :seconds_spent,
        :hours_spent,
        :minutes_spent,
        :current_day);".with(data_vec.iter().map(|curr_data| params!{
            "app_id" => curr_data.main_key.as_str(),
            "app_name" => curr_data.app_name.as_str(),
            "seconds_spent" => curr_data.seconds_spent,
            "hours_spent" => curr_data.hours_spent,
            "minutes_spent" => curr_data.minutes_spent,
            "current_day" => curr_data.current_day.as_str(),

        })).batch(&mut conn)
        .await.map_err(|e|{
            eprintln!("Error: {:?}", e)
        }).unwrap();
        
    drop(conn);


    pool.disconnect().await.unwrap();

    Ok(())
}

pub async fn get_data_from_db<'a>(data_map: &'a mut HashMap<String,AppData>, today_date: &NaiveDate) -> Result<&'a mut HashMap<String,AppData>, std::io::Error>{
    let database_url = Opts::from_url("mysql://root:dOVAKIN03!@localhost/screen_time_monitoring").unwrap();
    let curr_day = today_date.to_string();
    
    let pool = mysql_async::Pool::new(database_url);
    let mut conn = pool.get_conn().await.unwrap();

    let loaded_data =  r"Select app_id, app_name, seconds_spent, hours_spent, minutes_spent 
    from monitoring_table where current_day=:today".with(params!{
        "today" => curr_day
    }).map(&mut conn, |(app_id, app_name, seconds_spent, hours_spent, minutes_spent)|{
        println!("{app_id}:{app_name}:{seconds_spent}:{hours_spent}:{minutes_spent}");
        AppData::store_data(app_id, app_name, seconds_spent, hours_spent, minutes_spent)
    }).await.unwrap();
    
    let _ = loaded_data.iter().map(|app_data|{
        data_map.insert(app_data.app_name.clone(), app_data.to_owned())
    }).collect::<Vec<_>>();

    drop(conn);


    pool.disconnect().await.unwrap();

    Ok(data_map)
    
}
