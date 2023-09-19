
use std::{time , thread::sleep, collections::HashMap};
use std::time::{Duration, Instant};
use winapi::um::winuser::{GetWindowTextA, GetWindowTextLengthA, GetForegroundWindow};
use winapi::um::winuser::{
        LASTINPUTINFO,
        PLASTINPUTINFO,
        GetLastInputInfo,
    };
use winapi::um::winnt::LPSTR;
use winapi::um::sysinfoapi::GetTickCount;
use chrono::Local;


mod app_data;

use app_data::*;





pub fn get_last_input_info()-> Result<Duration, ()> {
    let now = unsafe {
        GetTickCount()
    };

    let mut last_input_info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0
    };
    let p_last_input_info: PLASTINPUTINFO = &mut last_input_info as *mut LASTINPUTINFO;

    let time_ok = unsafe { GetLastInputInfo(p_last_input_info) } != 0;

    match time_ok {
        true => {
            let millis =now -  last_input_info.dwTime;
            Ok(Duration::from_millis(millis as u64))
        }
        false => {
            Err(())
        }
    }
}

// fn _print_type_of<T>(_: &T){
//     println!("{}", std::any::type_name::<T>())
// }


#[tokio::main]
async fn main()-> Result<(), ()>{
    let mut app_spent_time_map:AppTimeSpentMap = HashMap::new(); 
 
    loop{

    let start = Instant::now();
    
    let current_widow = unsafe{ GetForegroundWindow() };
 
    let length = unsafe { GetWindowTextLengthA(current_widow) };

    let mut title:Vec<u8> = vec![0; (length + 1) as usize];
    let textw = unsafe { GetWindowTextA(current_widow, title.as_mut_ptr() as LPSTR , length + 1) };
    

    let mut title= String::from_utf8(title[0..(textw  as usize)].as_ref().to_vec()).map_err(|e|{
        eprintln!("ERROR: Failed to get window title : {e}")
    })?; 
    if textw == 0  {
        title = "Home screen".to_owned()
    }


    let x = title.split('-').into_iter().collect::<Vec<_>>();
    let mut last_val = x.last().unwrap().trim();

    let dt1= Local::now();
    let today = dt1.date_naive();
    
    
    let current_date = today.to_string();

    let idle_time  =get_last_input_info().unwrap().as_secs();

    if idle_time >= 300 {
        last_val = "Idle Time"
    }    

    if app_spent_time_map.contains_key(last_val) {
        app_spent_time_map.get_mut(last_val).unwrap().update_seconds(1) ;
    }else {
       let mut main_key = last_val.clone().to_owned(); 
       main_key.push_str(&current_date);
        app_spent_time_map.insert(last_val.to_owned(), AppData::new(last_val.clone().to_owned(), 1, current_date.clone(), main_key));
    }

    let (key, value) = &app_spent_time_map.get_key_value(last_val).unwrap();

    println!("{key}: {value:?}");

    if app_spent_time_map.get(last_val).unwrap().get_date().to_string() != current_date.clone(){
        println!("got called");
        app_spent_time_map.get_mut(last_val).unwrap().reset_time(current_date.clone())
    }
    
    
    
    
    
    let duration = start.elapsed();

    update_db(app_spent_time_map.clone()).await.unwrap();
    println!("Time elapsed in expensive_function() is: {:?}", duration);
    let time_delay_for_function = 10000 - duration.as_micros();
    let delay = time::Duration::from_micros(time_delay_for_function.try_into().unwrap_or(10000));
    sleep(delay);
    }

}
