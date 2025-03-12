use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use log::error;
use notify::{Config, PollWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        Foundation::MAX_PATH,
        Storage::FileSystem::{self, WIN32_FIND_DATAW},
        System::{
            Com::{CoCreateInstance, CoInitialize, CoUninitialize, IPersistFile, STGM},
            Ole::{OleInitialize, OleUninitialize},
        },
        UI::Shell::{
            CSIDL_COMMON_PROGRAMS, CSIDL_PROGRAMS, IShellLinkW, SHGetFolderPathW, SLGP_RAWPATH,
        },
    },
};

use crate::db::connection::DbHandler;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellLinkInfo {
    pub link: Option<String>,
    pub target_path: String,
    pub arguments: Option<String>,
    pub icon_location: Option<String>,
    pub working_directory: Option<String>,
    pub description: Option<String>,
}

pub enum LinkType {
    InternetShortCut(PathBuf),
    LinkShortCut(PathBuf),
    Unknown,
}

impl<P: AsRef<Path>> From<P> for LinkType {
    fn from(value: P) -> Self {
        match value.as_ref().extension() {
            None => return LinkType::Unknown,
            Some(os_str) => match os_str.to_str() {
                Some("lnk") => return LinkType::LinkShortCut(value.as_ref().to_path_buf()),
                Some("url") => return LinkType::InternetShortCut(value.as_ref().to_path_buf()),
                _ => return LinkType::Unknown,
            },
        }
    }
}

impl Into<Option<String>> for LinkType {
    fn into(self) -> Option<String> {
        match self {
            LinkType::InternetShortCut(path_buf) | LinkType::LinkShortCut(path_buf) => {
                Some(path_buf.into_os_string().into_string().unwrap_or_default())
            },
            LinkType::Unknown => None,
        }
    }
}

fn get_start_menu_paths() -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(2);
    let mut add_path = |csidl| {
        let mut path_buf = [0u16; MAX_PATH as usize];
        unsafe {
            if SHGetFolderPathW(None, csidl as i32, None, 0, &mut path_buf).is_ok() {
                let end = path_buf
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(path_buf.len());
                let os_string = OsString::from_wide(&path_buf[..end]);
                paths.push(PathBuf::from(os_string));
            }
        }
    };
    add_path(CSIDL_PROGRAMS);
    add_path(CSIDL_COMMON_PROGRAMS);
    paths
}

fn resolve_shortcut<T: AsRef<Path>>(shortcut_path: T) -> Option<ShellLinkInfo> {
    unsafe {
        let _ = CoInitialize(None);
        OleInitialize(None).ok()?;
        let path = LinkType::from(shortcut_path.as_ref());
        let shell_link: IShellLinkW = match &path {
            LinkType::InternetShortCut(_) => CoCreateInstance(
                &windows::Win32::UI::Shell::CLSID_InternetShortcut,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
            .ok()?,
            LinkType::LinkShortCut(_) => CoCreateInstance(
                &windows::Win32::UI::Shell::ShellLink,
                None,
                windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
            )
            .ok()?,
            LinkType::Unknown => return None,
        };
        let persist_file: IPersistFile = shell_link.cast::<IPersistFile>().ok()?;
        let shortcut_wide: Vec<u16> = shortcut_path
            .as_ref()
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        persist_file
            .Load(PCWSTR(shortcut_wide.as_ptr()), STGM(0))
            .ok()?;
        let mut path_buffer = [0u16; MAX_PATH as usize];
        let dataw = FileSystem::WIN32_FIND_DATAW::default();
        shell_link
            .GetPath(
                &mut path_buffer,
                &dataw as *const WIN32_FIND_DATAW as *mut WIN32_FIND_DATAW,
                SLGP_RAWPATH.0 as u32,
            )
            .ok()?;
        let end = path_buffer
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(path_buffer.len());
        let target = OsString::from_wide(&path_buffer[..end])
            .to_string_lossy()
            .into_owned();
        let mut icon_path = [0u16; MAX_PATH as usize];
        let mut icon_index = 0;
        let mut arguments_buffer = [0u16; MAX_PATH as usize];
        let mut working_dir_buffer = [0u16; MAX_PATH as usize];
        let mut description_buffer = [0u16; MAX_PATH as usize];

        let arguments = shell_link
            .GetArguments(&mut arguments_buffer)
            .ok()
            .map(|_| extract_wide_string(&arguments_buffer));

        let icon_location = shell_link
            .GetIconLocation(&mut icon_path, &mut icon_index)
            .ok()
            .map(|_| extract_wide_string(&icon_path));

        let working_directory = shell_link
            .GetWorkingDirectory(&mut working_dir_buffer)
            .ok()
            .map(|_| extract_wide_string(&working_dir_buffer));

        let description = shell_link
            .GetDescription(&mut description_buffer)
            .ok()
            .map(|_| extract_wide_string(&description_buffer));

        OleUninitialize();
        CoUninitialize();

        Some(ShellLinkInfo {
            link: path.into(),
            target_path: target,
            arguments,
            icon_location,
            working_directory,
            description,
        })
    }
}

fn extract_wide_string(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    OsString::from_wide(&buffer[..end])
        .to_string_lossy()
        .into_owned()
}

pub async fn start_menu_watcher(db_handler: Arc<DbHandler>) {
    let runtime_handle = tokio::runtime::Handle::current();
    let start_menu_paths = get_start_menu_paths();
    let user_menu_path = start_menu_paths[0].clone();
    let common_menu_path = start_menu_paths[1].clone();
    let (tx, mut rx) = mpsc::channel(1);
    let db_handler_menu = Arc::clone(&db_handler);
    let mut user_menu_watcher = PollWatcher::with_initial_scan(
        {
            let user_menu_path = user_menu_path.clone();
            let tx = tx.clone();
            move |result: Result<notify::Event, notify::Error>| {
                let user_menu_path = user_menu_path.clone();
                let tx = tx.clone();
                runtime_handle.spawn(async move {
                    match result {
                        Ok(event) => {
                            let _ = tx.send(event.clone()).await;
                        }
                        Err(e) => {
                            error!("Error listening for events on {:?}: {}", user_menu_path, e)
                        }
                    }
                });
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(1)),
        {
            move |result| match result {
                Ok(p) => {
                    if let Some(target) = resolve_shortcut(&p) {
                        let db_handler_menu = Arc::clone(&db_handler_menu);
                        tokio::task::spawn(async move {
                            let db_handler = Arc::clone(&db_handler_menu);
                            if let Err(e) = db_handler.insert_menu_shell_links(target).await {
                                error!("unable to insert / update the shell link info: {:?}", e);
                            };
                        });
                    } else {
                        error!("Failed to resolve: {:?}", &p);
                    }
                }
                Err(e) => error!("Initial listing shortcuts error: {}", e),
            }
        },
    )
    .unwrap();
    let db_handler_menu = Arc::clone(&db_handler);
    let runtime_handle = tokio::runtime::Handle::current();
    let mut common_menu_watcher = PollWatcher::with_initial_scan(
        {
            let common_menu_path = common_menu_path.clone();
            let tx = tx.clone();
            move |result: Result<notify::Event, notify::Error>| {
                let common_menu_path = common_menu_path.clone();
                let tx = tx.clone();
                runtime_handle.spawn(async move {
                    match result {
                        Ok(event) => {
                            let _ = tx.send(event).await;
                        }
                        Err(e) => {
                            error!(
                                "Error listening for events on {:?}: {}",
                                common_menu_path, e
                            )
                        }
                    }
                });
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(1)),
        {
            move |result| match result {
                Ok(p) => {
                    if let Some(target) = resolve_shortcut(&p) {
                        let db_handler_menu = Arc::clone(&db_handler_menu);
                        tokio::task::spawn(async move {
                            let db_handler = Arc::clone(&db_handler_menu);
                            if let Err(e) = db_handler.insert_menu_shell_links(target).await {
                                error!("unable to insert / update the shell link info: {:?}", e);
                            };
                        });
                    } else {
                        error!("Failed to resolve: {:?}", &p);
                    }
                }
                Err(e) => error!("Initial listing shortcuts error: {}", e),
            }
        },
    )
    .unwrap();

    if let Err(e) = user_menu_watcher.watch(&user_menu_path, RecursiveMode::Recursive) {
        error!("Failed to watch {:?}: {:?}", user_menu_path, e);
    }

    if let Err(e) = common_menu_watcher.watch(&common_menu_path, RecursiveMode::Recursive) {
        error!("Failed to watch {:?}: {:?}", common_menu_path, e);
    }
    while let Some(event) = rx.recv().await {
        let event_paths = event.paths;
        let menu_paths = get_start_menu_paths();
        for path in event_paths {
            let db_handler = Arc::clone(&db_handler);
            if menu_paths.contains(&path) {
                continue;
            } else {
                let info = resolve_shortcut(&path);
                if let Some(v) = info {
                    if let Err(e) = db_handler.insert_menu_shell_links(v).await {
                        error!("unable to insert / update the shell link info: {:?}", e);
                    };
                }
            }
        }
    }
}
