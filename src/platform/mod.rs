use std::{collections::BTreeMap, time::Duration};

use internment::ArcIntern;

#[cfg(windows)]
pub mod windows;

pub type AppName = ArcIntern<String>;
pub type WindowName = ArcIntern<String>;

pub type WindowDetailsTuple = (
    BTreeMap<WindowName, ArcIntern<WindowDetails>>,
    BTreeMap<AppName, ArcIntern<WindowDetails>>,
);

#[derive(Debug, Clone, PartialEq, Default, Eq, Hash)]
pub struct WindowDetails {
    pub window_title: ArcIntern<String>,
    pub app_name: Option<ArcIntern<String>>,
    pub app_path: Option<ArcIntern<String>>,
    pub is_active: bool,
}

pub trait Platform {
    fn get_window_titles() -> WindowDetailsTuple;
    fn get_last_input_info() -> Duration;
}
