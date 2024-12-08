use std::time::Duration;

#[cfg(windows)]
pub mod windows;

#[derive(Debug, Clone)]
pub struct WindowDetails {
    pub window_title: String,
    pub app_name: Option<String>,
    pub app_path: Option<String>,
    pub is_active: bool,
}

pub trait Platform {
    fn get_window_titles() -> Vec<WindowDetails>;
    fn get_last_input_info() -> Result<Duration, ()>;
}
