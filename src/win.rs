use std::time::Duration;

use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::{
    System::SystemInformation::GetTickCount,
    UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextA, GetWindowTextLengthA},
};

pub trait Platform {
    fn get_window_title() -> String;
    fn get_last_input_info() -> Result<Duration, ()>;
}
pub struct WindowsHandle;

impl Platform for WindowsHandle {
    fn get_window_title() -> String {
        unsafe {
            let current_window = GetForegroundWindow();

            let length = GetWindowTextLengthA(current_window);
            let mut title = vec![0; (length + 1) as usize];
            let text_len = GetWindowTextA(current_window, &mut title);
            let title = match String::from_utf8(title[0..(text_len as usize)].to_vec()) {
                Ok(valid_utf8) => valid_utf8,
                Err(_) => {
                    let utf16_bytes: Vec<u16> = title[0..(text_len as usize)]
                        .iter()
                        .map(|&b| b as u16)
                        .collect();
                    String::from_utf16(&utf16_bytes)
                        .unwrap_or_else(|_| "Invalid_app_Name".to_string())
                }
            };
            let title_name_vec = title.split('-').map(|v| v.to_owned()).collect::<Vec<_>>();
            return title_name_vec.last().unwrap().trim().to_string();
        }
    }
    fn get_last_input_info() -> Result<Duration, ()> {
        unsafe {
            let now = GetTickCount();
            let mut last_input_info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            let p_last_input_info = &mut last_input_info as *mut LASTINPUTINFO;

            let time_ok = GetLastInputInfo(p_last_input_info);

            match time_ok.as_bool() {
                true => {
                    let millis = now - last_input_info.dwTime;
                    Ok(Duration::from_millis(millis as u64))
                }
                false => Err(()),
            }
        }
    }
}

// fn get_process_name() -> () {
//     let current_window = unsafe { GetForegroundWindow() };
//     let length = unsafe { GetWindowTextLengthA(current_window) };

//     let mut title: Vec<u8> = vec![0; (length + 1) as usize];
//     let _ = unsafe { GetWindowTextA(current_window, title.as_mut_ptr() as LPSTR, length + 1) };
//     let mut process_id: u32 = 0;

//     // Unsafe block to call the WinAPI function
//     let _ = unsafe { GetWindowThreadProcessId(current_window, &mut process_id) };
//     let handle = unsafe {
//         OpenProcess(
//             winapi::um::winnt::PROCESS_QUERY_INFORMATION | winapi::um::winnt::PROCESS_VM_READ,
//             FALSE,
//             process_id,
//         )
//     };

//     if handle.is_null() {
//         eprintln!("Error getting window handle");
//         return;
//     }

//     let mut buffer: [u16; 260] = [0; 260];

//     // Retrieve the module file name
//     let result = unsafe {
//         GetModuleFileNameExW(
//             handle,
//             std::ptr::null_mut(),
//             buffer.as_mut_ptr(),
//             buffer.len() as DWORD,
//         )
//     };

//     // Close the process handle
//     unsafe { CloseHandle(handle) };

//     // Check if the function call succeeded
//     if result == 0 {
//         return;
//     }
//     let path = OsString::from_wide(&buffer[..result as usize])
//         .to_string_lossy()
//         .into_owned();

//     println!("{:?}", path);
//     return;
// }
