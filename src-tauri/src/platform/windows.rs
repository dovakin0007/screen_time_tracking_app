// Standard library
use std::{
    collections::BTreeMap,
    ffi::OsString,
    os::windows::prelude::*,
    path::Path,
    sync::{mpsc, Arc},
    time::Duration,
};

// External crates
use anyhow::{Context, Result};
use internment::ArcIntern;
use log::{debug, error};
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

// Windows APIs
use windows::{
    core::{IInspectable, Interface, HSTRING},
    Data::Xml::Dom::XmlDocument,
    Foundation::{IPropertyValue, TypedEventHandler},
    Win32::{
        Foundation::{CloseHandle, BOOL, FALSE, HINSTANCE, HWND, LPARAM, RECT},
        System::{
            ProcessStatus::GetModuleFileNameExW,
            SystemInformation::GetTickCount,
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
        UI::{
            Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
            WindowsAndMessaging::{
                EnumWindows, GetWindowLongW, GetWindowPlacement, GetWindowRect,
                GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
                GWL_EXSTYLE, SW_SHOWMINIMIZED, WINDOWPLACEMENT, WS_EX_TOOLWINDOW,
            },
        },
    },
    UI::Notifications::{
        ToastActivatedEventArgs, ToastDismissalReason, ToastDismissedEventArgs, ToastNotification,
        ToastNotificationManager,
    },
};

use super::{AppName, Platform, WindowDetailsTuple, WindowName};
use crate::{db::connection::DbHandler, platform::WindowDetails};

#[allow(unused_macros)]
macro_rules! sys_time_to_local_time {
    ($($systime: ident),*) => {
        ( $(chrono::Local.with_ymd_and_hms($systime.wYear.into(), $systime.wMonth.into(), $systime.wDay.into(), $systime.wHour.into(), $systime.wMinute.into(), $systime.wSecond.into()).unwrap()),* )
    };
}

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
    fn get_window_titles() -> WindowDetailsTuple {
        let mut window_title_map = BTreeMap::new();
        let mut app_name_map = BTreeMap::new();
        let state = (&mut window_title_map, &mut app_name_map);
        let state_ptr = &state as *const _ as isize;

        let result = unsafe { EnumWindows(Some(enumerate_windows), LPARAM(state_ptr)) };

        if result.is_err() {
            error!("Unable to get the window titles.");
        }

        (window_title_map, app_name_map)
    }

    fn get_last_input_info() -> Duration {
        unsafe {
            let now = GetTickCount();
            let mut last_input_info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };

            if !GetLastInputInfo(&mut last_input_info).as_bool() {
                error!("Failed to retrieve the last input time.");
            }

            let millis = now - last_input_info.dwTime;
            Duration::from_millis(millis as u64)
        }
    }
}

unsafe extern "system" fn enumerate_windows(window: HWND, state: LPARAM) -> BOOL {
    let state = &mut *(state.0
        as *mut (
            &mut BTreeMap<WindowName, ArcIntern<WindowDetails>>,
            &mut BTreeMap<AppName, ArcIntern<WindowDetails>>,
        ));

    if !IsWindowVisible(window).as_bool() {
        return BOOL::from(true);
    }

    if is_window_minimized(window) {
        if let Some(details) = get_window_details(window) {
            let details = ArcIntern::new(details);
            state
                .0
                .insert(details.window_title.clone(), details.clone());
            if let Some(app_name) = &details.app_name {
                state.1.insert(app_name.clone(), details);
            }
        }
    }

    if !is_valid_window(window) {
        return BOOL::from(true);
    }

    if let Some(details) = get_window_details(window) {
        let details = ArcIntern::new(details);
        state
            .0
            .insert(details.window_title.clone(), details.clone());
        if let Some(app_name) = &details.app_name {
            state.1.insert(app_name.clone(), details);
        }
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
            window_title: ArcIntern::new(sanitized_title),
            app_name: Some(ArcIntern::new(app_name)),
            app_path: Some(ArcIntern::new(app_path)),
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

#[derive(Debug)]
pub enum ToastResult {
    Accept(u32),
    Dismiss(Option<ToastDismissalReason>),
    Failed,
}
// unwraps shouldn't fail here!
pub fn create_toast_xml(app_name: &str, time_spent: &str, usage_limit: &str) -> XmlDocument {
    let toast_xml = XmlDocument::new().unwrap();

    let toast_element = toast_xml.CreateElement(&HSTRING::from("toast")).unwrap();
    toast_element
        .SetAttribute(
            &HSTRING::from("launch"),
            &HSTRING::from("app-defined-string"),
        )
        .unwrap();

    let visual_element = toast_xml.CreateElement(&HSTRING::from("visual")).unwrap();

    let binding_element = toast_xml.CreateElement(&HSTRING::from("binding")).unwrap();
    binding_element
        .SetAttribute(&HSTRING::from("template"), &HSTRING::from("ToastGeneric"))
        .unwrap();

    let text1 = toast_xml.CreateElement(&HSTRING::from("text")).unwrap();
    let text1_node = toast_xml
        .CreateTextNode(&HSTRING::from("App Usage Alert"))
        .unwrap();
    text1.AppendChild(&text1_node).unwrap();

    let text2 = toast_xml.CreateElement(&HSTRING::from("text")).unwrap();
    let text2_node = toast_xml
        .CreateTextNode(&HSTRING::from(format!(
            "{app_name} used for {time_spent}. Limit: {usage_limit}."
        )))
        .unwrap();
    text2.AppendChild(&text2_node).unwrap();

    binding_element.AppendChild(&text1).unwrap();
    binding_element.AppendChild(&text2).unwrap();
    visual_element.AppendChild(&binding_element).unwrap();

    // Actions section with dropdown and arguments
    let actions_element = toast_xml.CreateElement(&HSTRING::from("actions")).unwrap();

    // Input element for selection
    let input_element = toast_xml.CreateElement(&HSTRING::from("input")).unwrap();
    input_element
        .SetAttribute(&HSTRING::from("id"), &HSTRING::from("options"))
        .unwrap();
    input_element
        .SetAttribute(&HSTRING::from("type"), &HSTRING::from("selection"))
        .unwrap();
    input_element
        .SetAttribute(&HSTRING::from("defaultInput"), &HSTRING::from("15"))
        .unwrap();
    input_element
        .SetAttribute(&HSTRING::from("title"), &HSTRING::from("Alert After"))
        .unwrap();

    const VALUES: [(&str, &str); 4] = [
        ("15", "15 mins"),
        ("30", "30 mins"),
        ("45", "45 mins"),
        ("60", "1 hour"),
    ];

    for value in VALUES {
        let option = toast_xml
            .CreateElement(&HSTRING::from("selection"))
            .unwrap();
        option
            .SetAttribute(&HSTRING::from("id"), &HSTRING::from(value.0))
            .unwrap();
        option
            .SetAttribute(&HSTRING::from("content"), &HSTRING::from(value.1))
            .unwrap();
        input_element.AppendChild(&option).unwrap();
    }

    actions_element.AppendChild(&input_element).unwrap();

    // Accept action
    let action_accept = toast_xml.CreateElement(&HSTRING::from("action")).unwrap();
    action_accept
        .SetAttribute(&HSTRING::from("content"), &HSTRING::from("Accept"))
        .unwrap();
    action_accept
        .SetAttribute(&HSTRING::from("arguments"), &HSTRING::from("accept"))
        .unwrap();
    action_accept
        .SetAttribute(
            &HSTRING::from("activationType"),
            &HSTRING::from("foreground"),
        )
        .unwrap();

    actions_element.AppendChild(&action_accept).unwrap();

    // Dismiss action
    let action_dismiss = toast_xml.CreateElement(&HSTRING::from("action")).unwrap();
    action_dismiss
        .SetAttribute(&HSTRING::from("content"), &HSTRING::from("Dismiss"))
        .unwrap();
    action_dismiss
        .SetAttribute(&HSTRING::from("arguments"), &HSTRING::from("dismiss"))
        .unwrap();
    action_dismiss
        .SetAttribute(
            &HSTRING::from("activationType"),
            &HSTRING::from("foreground"),
        )
        .unwrap();

    actions_element.AppendChild(&action_dismiss).unwrap();

    // Audio element (optional)
    let audio_element = toast_xml.CreateElement(&HSTRING::from("audio")).unwrap();
    audio_element
        .SetAttribute(
            &HSTRING::from("src"),
            &HSTRING::from("ms-winsoundevent:Notification.Default"),
        )
        .unwrap();

    toast_element.AppendChild(&visual_element).unwrap();
    toast_element.AppendChild(&actions_element).unwrap();
    toast_element.AppendChild(&audio_element).unwrap();

    toast_xml.AppendChild(&toast_element).unwrap();

    toast_xml
}

pub async fn spawn_toast_notification(app_name: String, db_handler: Arc<DbHandler>) -> Result<()> {
    unsafe {
        _ = windows::Win32::System::Com::CoInitialize(None);
    }

    let app_usage = db_handler
        .get_specific_app_details(&app_name)
        .await
        .context("Failed to get app usage details")?;

    let total_minutes = (app_usage.total_hours
        .max(u32::MIN as f64)
        .min((u32::MAX / 60) as f64)  // avoid overflow when converting to minutes
        * 60.0)
        .round() as u32;

    let usage_details = app_usage.time_limit.unwrap_or_default();

    let mut buffer = itoa::Buffer::new();
    let total_minutes_str = buffer.format(total_minutes);

    let mut buffer = itoa::Buffer::new();
    let time_limit = buffer.format(usage_details);

    let toast_xml = create_toast_xml(&app_name, total_minutes_str, time_limit);

    let app_id = HSTRING::from("com.screen-time-tracker.app");
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&app_id)
        .context("Failed to create toast notifier")?;

    let toast = ToastNotification::CreateToastNotification(&toast_xml)
        .context("Failed to create toast notification")?;

    let (tx, rx) = mpsc::channel::<ToastResult>();
    let tx_clone = tx.clone();

    let activated_token = toast
        .Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_, args| {
                if let Some(args) = args.clone() {
                    if let Ok(t) = args.cast::<ToastActivatedEventArgs>() {
                        let input_value = t
                            .UserInput()
                            .and_then(|i| i.Lookup(&HSTRING::from("options")))
                            .and_then(|o| o.cast::<IPropertyValue>())
                            .ok();

                        let input_args = t
                            .Arguments()
                            .map(|a| a.to_string_lossy())
                            .unwrap_or_default();
                        let input_string = input_value
                            .and_then(|v| v.GetString().ok())
                            .map(|s| s.to_string_lossy());

                        if input_args == "accept" {
                            if let Some(s) = input_string {
                                if let Some(mins) = atoi::atoi::<u32>(s.as_bytes()) {
                                    let _ = tx_clone.send(ToastResult::Accept(mins));
                                }
                            }
                        } else {
                            let _ = tx_clone.send(ToastResult::Dismiss(None));
                        }
                    }
                }
                Ok(())
            },
        ))
        .context("Failed to attach Activated handler")?;

    let tx_clone = tx.clone();
    let dismissed_token = toast
        .Dismissed(&TypedEventHandler::<
            ToastNotification,
            ToastDismissedEventArgs,
        >::new(move |_, args| {
            if let Some(args) = args.clone() {
                let reason = args.Reason()?;
                _ = tx_clone.send(ToastResult::Dismiss(Some(reason)));
            }
            Ok(())
        }))
        .context("Failed to attach Dismissed handler")?;

    let failed_token = toast
        .Failed(&TypedEventHandler::new(move |_, _| {
            let _ = tx.send(ToastResult::Failed);
            Ok(())
        }))
        .context("Failed to attach Failed handler")?;

    notifier
        .Show(&toast)
        .context("Failed to show toast notification")?;
    while let Ok(recv) = rx.recv() {
        match recv {
            ToastResult::Accept(v) => {
                let _ = &db_handler
                    .insert_update_app_limits(
                        &app_usage.app_name,
                        app_usage.time_limit.unwrap_or_default() + v,
                        app_usage.should_alert.unwrap_or_default(),
                        app_usage.should_close.unwrap_or_default(),
                        app_usage.alert_before_close.unwrap_or_default(),
                        app_usage.alert_duration.unwrap_or_default(),
                    )
                    .await?;
                break;
            }
            ToastResult::Dismiss(reason) => {
                debug!("Toast has been dismissed, Reason: {:?}", reason);
                break;
            }
            ToastResult::Failed => {
                error!("Failed to create toast");
                break;
            }
        }
    }
    let _ = activated_token;
    let _ = dismissed_token;
    let _ = failed_token;

    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }

    Ok(())
}
