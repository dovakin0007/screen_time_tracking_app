use std::{collections::BTreeMap, time::Duration};

#[cfg(windows)]
pub mod windows;

pub type WindowDetailsTuple = (
    BTreeMap<String, WindowDetails>,
    BTreeMap<String, WindowDetails>,
);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WindowDetails {
    pub window_title: String,
    pub app_name: Option<String>,
    pub app_path: Option<String>,
    pub is_active: bool,
}

pub trait Platform {
    fn get_window_titles() -> (
        BTreeMap<String, WindowDetails>,
        BTreeMap<String, WindowDetails>,
    );
    fn get_last_input_info() -> Result<Duration, ()>;
}
