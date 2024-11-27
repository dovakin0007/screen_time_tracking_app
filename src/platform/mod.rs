use std::time::Duration;

#[cfg(windows)]
pub mod windows;

pub trait Platform {
    fn get_window_title() ->(String, String, Option<String>);
    fn get_last_input_info() -> Result<Duration, ()>;
}
