use anyhow::Result;
use log::error;
use std::collections::BTreeMap;
use std::os::windows::prelude::*;
use std::time::Duration;
use std::{ffi::OsString, path::Path};
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Foundation::{BOOL, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
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
        WindowsAndMessaging::{GetWindowTextA, GetWindowTextLengthA, GetWindowThreadProcessId},
    },
};

use crate::platform::WindowDetails;

use super::Platform;

pub struct WindowsHandle;

impl Platform for WindowsHandle {
    fn get_window_titles() -> BTreeMap<String, WindowDetails> {
        let state: Box<BTreeMap<String, WindowDetails>> = Box::new(BTreeMap::new());
        let state_ptr = Box::into_raw(state);
        let state;
        let result = unsafe { EnumWindows(Some(enumerate_windows), LPARAM(state_ptr as isize)) };
        if result.is_err() {
            error!("Unable to get the window titles.");
        }
        state = unsafe { Box::from_raw(state_ptr) };
        *state
    }

    fn get_last_input_info() -> Result<Duration, ()> {
        unsafe {
            let now = GetTickCount();
            let mut last_input_info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            let time_ok = GetLastInputInfo(&mut last_input_info);
            if !time_ok.as_bool() {
                error!("Failed to retrieve the last input time.");
                return Err(());
            }
            let millis = now - last_input_info.dwTime;
            Ok(Duration::from_millis(millis as u64))
        }
    }
}

fn get_process_name(current_window: HWND) -> Result<String, ()> {
    let length = unsafe { GetWindowTextLengthA(current_window) };
    let mut title: Vec<u8> = vec![0; (length + 1) as usize];
    let _ = unsafe { GetWindowTextA(current_window, &mut title) };
    let mut process_id: u32 = 0;
    unsafe { GetWindowThreadProcessId(current_window, Some(&mut process_id)) };
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            FALSE,
            process_id,
        )
    };
    let h = handle.map_err(|e| {
        error!("Failed to open process: {:?}", e);
    })?;
    let mut buffer: [u16; 260] = [0; 260];
    let result = unsafe { GetModuleFileNameExW(h, HINSTANCE::default(), &mut buffer) };
    let _ = unsafe { CloseHandle(h) };
    // if close_result.is_err() == false {
    //     error!("Failed to close handle: {:?}", h);
    // }
    if result == 0 {
        error!("Failed to retrieve the module file name.");
        return Err(());
    }
    let path = OsString::from_wide(&buffer[..result as usize])
        .to_string_lossy()
        .into_owned();
    Ok(path)
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
    let mut rect = RECT::default();
    if GetWindowRect(window, &mut rect).is_err() {
        error!("Failed to get window rectangle.");
        return BOOL::from(true);
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if rect.left <= -32000 && rect.top <= -32000 || width <= 1 || height <= 1 {
        return BOOL::from(true);
    }
    let state = state.0 as *mut BTreeMap<String, WindowDetails>;
    let length = GetWindowTextLengthW(window);
    if length == 0 {
        return BOOL::from(true);
    }
    let mut title: Vec<u16> = vec![0; (length + 1) as usize];
    let text_len = GetWindowTextW(window, &mut title);
    if text_len > 0 {
        if let Ok(title) = String::from_utf16(&title[0..text_len as usize]) {
            let path_name = get_process_name(window).unwrap_or_else(|_| {
                error!("Unable to get process name.");
                "Invalid path".to_string()
            });
            let app_name = get_app_name_from_path(&path_name)
                .unwrap_or_else(|| "Invalid app name".to_string());
            if title != "Windows Input Experience" && title != "Program Manager" {
                (*state).insert(
                    title.clone(),
                    WindowDetails {
                        window_title: title,
                        app_name: Some(app_name),
                        app_path: Some(path_name),
                        is_active: false,
                    },
                );
            }
        }
    }
    BOOL::from(true)
}