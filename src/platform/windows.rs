use anyhow::Result;
use log::error;
use regex::Regex;
use std::collections::BTreeMap;
use std::os::windows::prelude::*;
use std::time::Duration;
use std::{ffi::OsString, path::Path};
use unicode_segmentation::UnicodeSegmentation;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Foundation::{BOOL, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongW, GetWindowPlacement, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, IsWindowVisible, GWL_EXSTYLE, SW_SHOWMINIMIZED, WINDOWPLACEMENT,
    WS_EX_TOOLWINDOW,
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
        WindowsAndMessaging::GetWindowThreadProcessId,
    },
};

use super::Platform;
use crate::platform::WindowDetails;

const FILTERED_WINDOWS: [&str; 6] = [
    "Windows Input Experience",
    "Program Manager",
    "Settings",
    "Microsoft Text Input Application",
    "Windows Shell Experience Host",
    "Application Frame Host",
];

pub struct WindowsHandle;

impl Platform for WindowsHandle {
    fn get_window_titles() -> BTreeMap<String, WindowDetails> {
        let mut state = BTreeMap::new();
        let state_ptr = &mut state as *mut _ as isize;
        let result = unsafe { EnumWindows(Some(enumerate_windows), LPARAM(state_ptr)) };

        if result.is_err() {
            error!("Unable to get the window titles.");
        }

        state
    }

    fn get_last_input_info() -> Result<Duration, ()> {
        unsafe {
            let now = GetTickCount();
            let mut last_input_info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };

            if !GetLastInputInfo(&mut last_input_info).as_bool() {
                error!("Failed to retrieve the last input time.");
                return Err(());
            }

            let millis = now - last_input_info.dwTime;
            Ok(Duration::from_millis(millis as u64))
        }
    }
}

unsafe extern "system" fn enumerate_windows(window: HWND, state: LPARAM) -> BOOL {
    let state = &mut *(state.0 as *mut BTreeMap<String, WindowDetails>);

    if !IsWindowVisible(window).as_bool() {
        return BOOL::from(true);
    }

    if is_window_minimized(window) {
        if let Some(details) = get_window_details(window) {
            state.insert(details.window_title.clone(), details);
        }
    }

    if !is_valid_window(window) {
        return BOOL::from(true);
    }

    if let Some(details) = get_window_details(window) {
        state.insert(details.window_title.clone(), details);
    }

    BOOL::from(true)
}

unsafe fn is_valid_window(window: HWND) -> bool {
    let mut rect = RECT::default();
    if GetWindowRect(window, &mut rect).is_err() {
        return false;
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if rect.left <= -32000
        || rect.top <= -32000
        || width <= 100
        || height <= 100
        || (rect.top > 0 && height < 200)
    {
        return false;
    }

    let ex_style = GetWindowLongW(window, GWL_EXSTYLE);
    (ex_style & (WS_EX_TOOLWINDOW.0) as i32) == 0
}

fn get_window_details(window: HWND) -> Option<WindowDetails> {
    let title = unsafe { get_window_title(window)? };
    let (app_name, app_path) = get_app_details(window);
    let sanitized_title = sanitize_title(&title);

    if should_include_window(&sanitized_title, &app_path) {
        Some(WindowDetails {
            window_title: sanitized_title,
            app_name: Some(app_name),
            app_path: Some(app_path),
            is_active: false,
        })
    } else {
        None
    }
}

unsafe fn get_window_title(window: HWND) -> Option<String> {
    let length = GetWindowTextLengthW(window);
    if length == 0 {
        return None;
    }

    let mut buffer = vec![0u16; (length + 1) as usize];
    let len = GetWindowTextW(window, &mut buffer);
    buffer.truncate(len as usize);

    String::from_utf16(&buffer).ok()
}

fn get_app_details(window: HWND) -> (String, String) {
    let path = get_process_path(window).unwrap_or_else(|_| {
        error!("Failed to get process path");
        "Unknown".into()
    });

    let app_name = Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    (app_name, path)
}

fn get_process_path(window: HWND) -> Result<String, ()> {
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };

    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            FALSE,
            process_id,
        )
    }
    .map_err(|e| {
        error!("OpenProcess failed: {:?}", e);
    })?;

    let mut buffer = [0u16; 260];
    let len = unsafe { GetModuleFileNameExW(handle, HINSTANCE::default(), &mut buffer) };
    unsafe {
        if CloseHandle(handle).is_err() {
            error!("Unable Close the handle")
        }
    };

    if len == 0 {
        error!("GetModuleFileNameExW failed");
        return Err(());
    }

    Ok(OsString::from_wide(&buffer[..len as usize])
        .to_string_lossy()
        .into_owned())
}

fn sanitize_title(title: &str) -> String {
    let emoji_pattern = Regex::new(r"[\p{Emoji}]|●|[^\x00-\x7F]").unwrap();
    title
        .graphemes(true)
        .filter(|g| !emoji_pattern.is_match(g))
        .collect::<String>()
        .trim()
        .to_string()
}

fn should_include_window(title: &str, path: &str) -> bool {
    !title.is_empty()
        && !FILTERED_WINDOWS.contains(&title)
        && !title.to_lowercase().contains("notification")
        && !title.starts_with('_')
        && !title.contains("Task View")
        && !title.contains("Start")
        && !path.contains("SystemSettings.exe")
        && !path.contains("ShellExperienceHost.exe")
}

fn is_window_minimized(hwnd: HWND) -> bool {
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };

    unsafe {
        GetWindowPlacement(hwnd, &mut placement)
            .map(|_| placement.showCmd == SW_SHOWMINIMIZED.0 as u32)
            .unwrap_or(false)
    }
}
