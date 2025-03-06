use std::{collections::BTreeMap, time::Duration};

use internment::ArcIntern;

#[cfg(windows)]
pub mod windows;

pub type WindowDetailsTuple = (
    BTreeMap<String, ArcIntern<WindowDetails>>,
    BTreeMap<String, ArcIntern<WindowDetails>>,
);

#[derive(Debug, Clone, PartialEq, Default, Eq, Hash)]
pub struct WindowDetails {
    pub window_title: String,
    pub app_name: Option<String>,
    pub app_path: Option<String>,
    pub is_active: bool,
}

pub trait Platform {
    fn get_window_titles() -> WindowDetailsTuple;
    fn get_last_input_info() -> Duration;
}
