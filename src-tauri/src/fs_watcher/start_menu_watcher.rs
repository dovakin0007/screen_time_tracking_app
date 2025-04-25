use std::{
    collections::HashSet,
    ffi::OsString,
    fs::File,
    io::{Cursor, Read},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use base64::Engine;
use ico::IconDir;
use image::{ImageBuffer, RgbaImage};
use log::error;
use notify::{Config, PollWatcher, RecursiveMode, Watcher};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use walkdir::WalkDir;

use windows::{
    core::{Interface, HSTRING, PCWSTR},
    Win32::{
        Foundation::MAX_PATH,
        Graphics::Gdi::{
            CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, SelectObject, BITMAPINFO,
            BITMAPINFOHEADER, DIB_RGB_COLORS,
        },
        Storage::FileSystem::{self, WIN32_FIND_DATAW},
        System::{
            Com::{CoCreateInstance, CoInitialize, CoUninitialize, IPersistFile, STGM},
            Ole::{OleInitialize, OleUninitialize},
        },
        UI::{
            Shell::{
                ExtractIconExW, IShellLinkW, SHGetFolderPathW, CSIDL_COMMON_PROGRAMS,
                CSIDL_PROGRAMS, SLGP_RAWPATH,
            },
            WindowsAndMessaging::{DestroyIcon, GetIconInfoExW, HICON, ICONINFOEXW},
        },
    },
};

use crate::{db::connection::DbHandler, StartMenuStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellLinkInfo {
    pub link: Option<String>,
    pub target_path: String,
    pub arguments: Option<String>,
    pub icon_base64_image: Option<String>,
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
            }
            LinkType::Unknown => None,
        }
    }
}

// https://stackoverflow.com/questions/78190248/extract-icons-from-exe-in-rust

//https://github.com/TeamDman/Cursor-Hero/blob/51611380997d74f74f76fa776be4892a9906c005/crates/winutils/src/win_icons.rs
pub fn get_image_from_exe(executable_path: &str) -> anyhow::Result<Option<RgbaImage>> {
    unsafe {
        let hstr = HSTRING::from(executable_path);
        let path_pcwstr = PCWSTR::from_raw(hstr.as_wide().as_ptr());
        let mut hicon = HICON::default();
        let num_icons = ExtractIconExW(path_pcwstr, 0, Some(&mut hicon), None, 1);

        if num_icons == 0 || hicon.is_invalid() {
            return Ok(None);
        }

        let image_result = convert_hicon_to_rgba_image(&hicon);

        if let Err(e) = DestroyIcon(hicon) {
            eprintln!("Failed to destroy icon: {:?}", e);
        }

        Ok(image_result.ok())
    }
}

pub fn convert_hicon_to_rgba_image(hicon: &HICON) -> anyhow::Result<RgbaImage> {
    unsafe {
        let mut icon_info = ICONINFOEXW::default();
        icon_info.cbSize = std::mem::size_of::<ICONINFOEXW>() as u32;

        if !GetIconInfoExW(*hicon, &mut icon_info).as_bool() {
            return Err(anyhow::anyhow!(format!(
                "icon • GetIconInfoExW: {} {}:{}",
                file!(),
                line!(),
                column!()
            )));
        }
        let hdc_screen = CreateCompatibleDC(None);
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbm_old = SelectObject(hdc_mem, icon_info.hbmColor);

        let mut bmp_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: icon_info.xHotspot as i32 * 2,
                biHeight: -(icon_info.yHotspot as i32 * 2),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: DIB_RGB_COLORS.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut buffer: Vec<u8> =
            vec![0; (icon_info.xHotspot * 2 * icon_info.yHotspot * 2 * 4) as usize];

        if GetDIBits(
            hdc_mem,
            icon_info.hbmColor,
            0,
            icon_info.yHotspot * 2,
            Some(buffer.as_mut_ptr() as *mut _),
            &mut bmp_info,
            DIB_RGB_COLORS,
        ) == 0
        {
            return Err(anyhow::anyhow!(format!(
                "GetDIBits: {} {}:{}",
                file!(),
                line!(),
                column!()
            )));
        }
        // Clean up
        SelectObject(hdc_mem, hbm_old);
        _ = DeleteDC(hdc_mem);
        _ = DeleteDC(hdc_screen);
        _ = DeleteObject(icon_info.hbmColor);
        _ = DeleteObject(icon_info.hbmMask);

        bgra_to_rgba(buffer.as_mut_slice());

        let image = ImageBuffer::from_raw(icon_info.xHotspot * 2, icon_info.yHotspot * 2, buffer)
            .ok_or_else(|| anyhow::anyhow!("Failed to create RgbaImage from raw buffer"))?;
        return Ok(image);
    }
}

//https://github.com/TeamDman/Cursor-Hero/blob/51611380997d74f74f76fa776be4892a9906c005/crates/math/src/shuffle.rs
#[cfg(target_arch = "x86")]
use std::arch::x86::_mm_shuffle_epi8;
use std::arch::x86_64::__m128i;
use std::arch::x86_64::_mm_loadu_si128;
use std::arch::x86_64::_mm_setr_epi8;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::_mm_shuffle_epi8;
use std::arch::x86_64::_mm_storeu_si128;

/// Convert BGRA to RGBA
///
/// Uses SIMD to go fast
pub fn bgra_to_rgba(data: &mut [u8]) {
    // The shuffle mask for converting BGRA -> RGBA
    let mask: __m128i = unsafe {
        _mm_setr_epi8(
            2, 1, 0, 3, // First pixel
            6, 5, 4, 7, // Second pixel
            10, 9, 8, 11, // Third pixel
            14, 13, 12, 15, // Fourth pixel
        )
    };
    for chunk in data.chunks_exact_mut(16) {
        let mut vector = unsafe { _mm_loadu_si128(chunk.as_ptr() as *const __m128i) };
        vector = unsafe { _mm_shuffle_epi8(vector, mask) };
        unsafe { _mm_storeu_si128(chunk.as_mut_ptr() as *mut __m128i, vector) };
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

pub fn get_icon_base64_from_exe(executable_path: &str) -> anyhow::Result<Option<String>> {
    if let Some(image) = get_image_from_exe(executable_path)? {
        let mut buf = vec![];
        image.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)?;
        let b64 = base64::prelude::BASE64_STANDARD.encode(&buf);
        return anyhow::Result::Ok(Some(b64));
    } else {
        anyhow::Result::Ok(None)
    }
}

pub fn ico_to_base64_png(path: &str) -> anyhow::Result<String> {
    // Load the ICO file
    let mut file = File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    let icon_dir = IconDir::read(Cursor::new(&data))?;

    let entry = icon_dir
        .entries()
        .last()
        .ok_or_else(|| anyhow::anyhow!("No icons found in .ico file"))?;
    let decoded = entry.decode()?;
    let h = decoded.height();
    let w = decoded.width();
    // Convert to RgbaImage
    let image_data = decoded.rgba_data();
    let image: RgbaImage = ImageBuffer::from_raw(w, h, image_data.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Failed to create RgbaImage from raw buffer"))?;

    let mut buf = vec![];
    image.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)?;
    let b64 = base64::prelude::BASE64_STANDARD.encode(&buf);
    return anyhow::Result::Ok(b64);
}

pub fn normalize_file_uri(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("file:///") {
        let decoded = percent_decode_str(stripped).decode_utf8_lossy();
        return decoded.into_owned();
    }
    path.to_string()
}

pub fn get_icon_base64_from_icon_base64_image(
    icon_base64_image: Option<String>,
    exe_path: String,
) -> anyhow::Result<Option<String>> {
    if let Some(loc) = icon_base64_image {
        let normalized = normalize_file_uri(&loc);
        let resolved = resolve_path(&normalized);

        if resolved.to_lowercase().ends_with(".ico") {
            let base64 = ico_to_base64_png(&resolved)?;
            return Ok(Some(base64));
        } else if resolved.to_lowercase().ends_with(".exe") {
            return get_icon_base64_from_exe(&resolved);
        } else if !exe_path.is_empty() {
            let normalized = normalize_file_uri(&exe_path);
            let resolved = resolve_path(&normalized);
            return get_icon_base64_from_exe(&resolved);
        }
    }
    Ok(None)
}

async fn resolve_shortcut<T: AsRef<Path>>(shortcut_path: T) -> Option<ShellLinkInfo> {
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

        let icon_base64_image = shell_link
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
        // match icon
        OleUninitialize();
        CoUninitialize();
        Some(ShellLinkInfo {
            link: path.into(),
            target_path: resolve_path(&target),
            arguments,
            icon_base64_image: get_icon_base64_from_icon_base64_image(icon_base64_image, target)
                .unwrap_or(None),
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

async fn sync_removed_shortcuts(db_handler: Arc<DbHandler>, menu_paths: &[PathBuf]) {
    let mut existing_files = HashSet::new();
    for dir in menu_paths {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .map_or(false, |ext| ext == "lnk" || ext == "url")
                {
                    existing_files.insert(path);
                }
            }
        }
    }

    match db_handler.get_all_menu_paths().await {
        Ok(db_paths) => {
            for db_path in db_paths {
                if !existing_files.contains(&db_path) {
                    if let Err(e) = db_handler.delete_menu_shell_link(&db_path).await {
                        error!("Failed to delete stale shortcut from DB: {:?}", e);
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch shortcut paths from DB: {:?}", e);
        }
    }
}

fn expand_windows_env_vars(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '%' {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != '%' {
                end += 1;
            }
            if end < chars.len() && chars[end] == '%' {
                let var_name: String = chars[start..end].iter().collect();
                // Skip expansion if it contains %20
                if var_name.contains("20") {
                    result.push('%');
                    result.push_str(&var_name);
                    result.push('%');
                } else {
                    let var_value = std::env::var(&var_name).unwrap_or_default();
                    result.push_str(&var_value);
                }
                i = end + 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

fn resolve_path(path: &str) -> String {
    expand_windows_env_vars(path)
}

pub async fn start_menu_watcher(
    db_handler: Arc<DbHandler>,
    programs_watcher_status: Arc<StartMenuStatus>,
) {
    let runtime_handle = tokio::runtime::Handle::current();
    let start_menu_paths = get_start_menu_paths();
    sync_removed_shortcuts(Arc::clone(&db_handler), &start_menu_paths).await;
    let user_menu_path = start_menu_paths[0].clone();
    let common_menu_path = start_menu_paths[1].clone();
    let (tx, mut rx) = mpsc::channel(1);
    let db_handler_menu = Arc::clone(&db_handler);
    let programs_watcher_status_user = Arc::clone(&programs_watcher_status);

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
        move |result: std::result::Result<std::path::PathBuf, notify::Error>| {
            if let Ok(p) = result {
                let db_handler_menu = Arc::clone(&db_handler_menu);
                let paths: Vec<_> = if p.is_dir() {
                    WalkDir::new(&p)
                        .into_iter()
                        .filter_map(Result::ok)
                        .map(|entry| entry.path().to_path_buf())
                        .collect()
                } else {
                    vec![p]
                };

                for path in paths {
                    if path.is_file()
                        && path
                            .extension()
                            .map_or(false, |ext| ext == "lnk" || ext == "url")
                    {
                        let db_handler_menu = Arc::clone(&db_handler_menu);
                        let path_clone = path.clone();
                        tokio::task::block_in_place(|| {
                            let rt = tokio::runtime::Handle::current();
                            rt.block_on(async {
                                if let Some(target) = resolve_shortcut(&path_clone).await {
                                    if let Err(e) = db_handler_menu.insert_menu_shell_links(target).await {
                                        error!("Unable to insert / update the shell link info: {:?}", e);
                                    }
                                } else {
                                    error!("Failed to resolve: {:?}", path_clone);
                                }
                            });
                        });
                    }
                }
            } else if let Err(e) = result {
                error!("Initial listing shortcuts error: {}", e);
            }
            programs_watcher_status_user.set_atomic_bool_one(true);
        },
    )
    .unwrap();
    let db_handler_menu = Arc::clone(&db_handler);
    let runtime_handle = tokio::runtime::Handle::current();
    let programs_watcher_status_common = Arc::clone(&programs_watcher_status);
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
        move |result: std::result::Result<std::path::PathBuf, notify::Error>| {
            match result {
                Ok(p) => {
                    let db_handler_menu = Arc::clone(&db_handler_menu);
                    let paths: Vec<_> = if p.is_dir() {
                        WalkDir::new(&p)
                            .into_iter()
                            .filter_map(Result::ok)
                            .map(|entry| entry.path().to_path_buf())
                            .collect()
                    } else {
                        vec![p]
                    };

                    for path in paths {
                        if path.is_file()
                            && path
                                .extension()
                                .map_or(false, |ext| ext == "lnk" || ext == "url")
                        {
                            let db_handler_menu = Arc::clone(&db_handler_menu);
                            let path_clone = path.clone();
                            tokio::task::block_in_place(|| {
                                let rt = tokio::runtime::Handle::current();
                                rt.block_on(async {
                                    if let Some(target) = resolve_shortcut(&path_clone).await {
                                        if let Err(e) = db_handler_menu.insert_menu_shell_links(target).await {
                                            error!("Unable to insert / update the shell link info: {:?}", e);
                                        }
                                    } else {
                                        error!("Failed to resolve: {:?}", path_clone);
                                    }
                                });
                            });
                        }
                    }
                }
                Err(e) => {
                    error!("Initial listing shortcuts error: {}", e);
                }
            }
            programs_watcher_status_common.set_atomic_bool_two(true);
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
                let info = resolve_shortcut(&path).await;
                if let Some(v) = info {
                    if let Err(e) = db_handler.insert_menu_shell_links(v).await {
                        error!("unable to insert / update the shell link info: {:?}", e);
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_windows_env_vars() {
        // Set an environment variable for testing
        std::env::set_var("TEST_VAR", "C:\\TestPath");

        let input = "%TEST_VAR%\\subdir\\file.txt";
        let expected = "C:\\TestPath\\subdir\\file.txt";

        let result = expand_windows_env_vars(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_windows_env_vars_with_missing_var() {
        // Unset the environment variable just in case
        std::env::remove_var("NON_EXISTENT_VAR");

        let input = "%NON_EXISTENT_VAR%\\default";
        let expected = "\\default"; // Since the variable doesn't exist, it gets replaced with an empty string

        let result = expand_windows_env_vars(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_windows_env_vars_partial_no_closing_percent() {
        let input = "%PARTIAL_VAR\\file.txt";
        let expected = "%PARTIAL_VAR\\file.txt"; // No closing %, should be treated as literal

        let result = expand_windows_env_vars(input);
        assert_eq!(result, expected);
    }

    #[ignore]
    #[test]
    fn test_get_icon_base64_from_icon_base64_image_with_ico() {
        // Manually specify an .ico file for reliable testing
        let test_ico_path = "C:/Users/dovak/anaconda3/Menu/jupyter.ico";

        let base64_result =
            get_icon_base64_from_icon_base64_image(Some(test_ico_path.to_string()), String::new());
        assert!(base64_result.is_ok());
        let base64 = base64_result.unwrap();
        assert!(base64.is_some());
    }

    #[test]
    fn test_get_icon_base64_from_icon_base64_image_with_exe() {
        let test_exe_path = r"C:\Windows\System32\notepad.exe";
        assert!(
            std::fs::metadata(test_exe_path).is_ok(),
            "Test EXE file doesn't exist"
        );

        let base64_result =
            get_icon_base64_from_icon_base64_image(Some(test_exe_path.to_string()), String::new());
        assert!(base64_result.is_ok());
        let base64 = base64_result.unwrap();
        assert!(base64.is_some());
        println!("{:?}", base64);
        assert!(
            base64.unwrap().starts_with("iVBOR"),
            "Expected base64 PNG to start with PNG signature"
        );
    }

    #[ignore]
    #[test]
    fn test_get_icon_base64_from_icon_as_url_file_path() {
        let test_exe_path = r"file:///C:/Program%20Files%20(x86)/Steam/steam/games/5b3bd0d9e5800b933245b5308089a688804787eb.ico";

        let base64_result =
            get_icon_base64_from_icon_base64_image(Some(test_exe_path.to_string()), String::new());
        assert!(base64_result.is_ok());
        let base64 = base64_result.unwrap();
        assert!(base64.is_some());
        println!("{:?}", base64);
    }
}
