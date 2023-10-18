use winapi::um::winuser::{GetWindowTextA, GetWindowTextLengthA, GetForegroundWindow};
use winapi::um::winuser::{
    LASTINPUTINFO,
    PLASTINPUTINFO,
    GetLastInputInfo,
};
use winapi::um::winnt::LPSTR;
use winapi::um::sysinfoapi::GetTickCount;
use std::time::Duration;

pub fn get_title_vec<'a>() -> String{
    let current_widow = unsafe{ GetForegroundWindow() };

    let length = unsafe { GetWindowTextLengthA(current_widow) };

    let mut title:Vec<u8> = vec![0; (length + 1) as usize];
    let textw = unsafe { GetWindowTextA(current_widow, title.as_mut_ptr() as LPSTR , length + 1) };


    let mut title= String::from_utf8(title[0..(textw  as usize)].as_ref().to_vec()).map_err(|_e|{
        // eprintln!("ERROR: Failed to get window title : {e}")
    }).unwrap_or("Invalid_app_Name".parse().unwrap());
    if textw == 0  {
        title = "Home screen".to_owned()
    }


    let title_name_vec = title.split('-').map(|v| v.to_owned()).collect::<Vec<_>>();
    return title_name_vec.last().unwrap().trim().to_string();
}
pub fn get_last_input_info() -> Result<Duration, ()> {
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