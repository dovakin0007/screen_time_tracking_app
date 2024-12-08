use anyhow::Result;
use std::os::windows::prelude::*;
use std::time::Duration;
use std::{ffi::OsString, path::Path};
use windows::Win32::Foundation::BOOL;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
};
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
            GetWindowTextA, GetWindowTextLengthA, GetWindowThreadProcessId,
        },
    },
};

use crate::platform::WindowDetails;

use super::Platform;
pub struct WindowsHandle;

impl Platform for WindowsHandle {
    fn get_window_titles() -> Vec<WindowDetails> {
        let state: Box<Vec<WindowDetails>> = Box::new(Vec::new());
        let state_ptr = Box::into_raw(state);
        let state;
        let result = unsafe { EnumWindows(Some(enumerate_windows), LPARAM(state_ptr as isize)) };
        let _ = result.inspect_err(|e| {
            eprintln!("Unable to get the window titles, {:?}", e);
        });
        state = unsafe { Box::from_raw(state_ptr) };
        return *state;
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

fn get_app_name_from_path(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
}

unsafe extern "system" fn enumerate_windows(window: HWND, state: LPARAM) -> BOOL {
    if IsWindowVisible(window).as_bool() == false {
        return BOOL::from(true);
    }
    let state = state.0 as *mut Vec<WindowDetails>;

    let length = GetWindowTextLengthW(window);
    if length == 0 {
        return BOOL::from(true);
    }

    let mut title: Vec<u16> = vec![0; (length + 1) as usize];
    let text_len = GetWindowTextW(window, &mut title);
    if text_len > 0 {
        if let Ok(title) = String::from_utf16(&title[0..text_len as usize]) {
            let path_name = get_process_name(window)
                .inspect_err(|_| {
                    eprintln!("unable to get process name");
                })
                .unwrap_or(String::from("Invalid path"));
            let app_name = get_app_name_from_path(&path_name);
            let app_name2 = if let Some(name) = app_name {
                name
            } else {
                String::from("Invalid app name")
            };
            (*state).push(WindowDetails {
                window_title: title,
                app_name: Some(app_name2),
                app_path: Some(path_name),
                is_active: false,
            });
        }
    }
    BOOL::from(true)
}
