use anyhow::Result;
use std::ffi::OsStr;
use std::os::windows::prelude::*;
use std::time::Duration;
use std::{ffi::OsString, path::Path};
use windows::Win32::{
    Foundation::{CloseHandle, FALSE, HINSTANCE, HWND},
    System::{
        ProcessStatus::GetModuleFileNameExW,
        SystemInformation::GetTickCount,
        Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    },
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextA, GetWindowTextLengthA, GetWindowThreadProcessId,
        },
    },
};

use super::Platform;

pub struct WindowsHandle;

impl Platform for WindowsHandle {
    fn get_window_title() -> (String, String, Option<String>) {
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
            let p_name = get_process_name(current_window).unwrap_or("Invalid App Path".to_string());
            let path = resolve_path(&p_name);

            let app_name = path
            .and_then(|p| p.file_name())
            .unwrap_or_else(|| OsStr::new(&title))
            .to_string_lossy()
            .to_string();
            let path_as_string = path_to_string(path);
            println!("{:?}", title);
            return (title.trim().to_string(), app_name, path_as_string);
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

fn get_process_name(current_window: HWND) -> Result<String, ()> {
    let length = unsafe { GetWindowTextLengthA(current_window) };

    let mut title: Vec<u8> = vec![0; (length + 1) as usize];
    let _ = unsafe { GetWindowTextA(current_window, title.as_mut_slice()) };
    let mut process_id: u32 = 0;

    let _ = unsafe { GetWindowThreadProcessId(current_window, Some(&mut process_id)) };
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            FALSE,
            process_id,
        )
    };

    let h = handle.map_err(|e| {
        eprintln!("{:?}", e);
    })?;

    let mut buffer: [u16; 260] = [0; 260];

    let result = unsafe { GetModuleFileNameExW(h, HINSTANCE::default(), buffer.as_mut_slice()) };

    unsafe {
        let _ = CloseHandle(h);
    };

    let path = OsString::from_wide(&buffer[..result as usize])
        .to_string_lossy()
        .into_owned();

    return Ok(path);
}


fn resolve_path(p_name: &str) -> Option<&Path> {
    match p_name {
        "Invalid App Path" => None,
        _ => Some(Path::new(p_name)),
    }
}

fn path_to_string(path: Option<&Path>) -> Option<String> {
    match path {
        Some(p) => Some(p.to_string_lossy().to_string()),
        None => None
    }
}