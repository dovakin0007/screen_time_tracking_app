use std::{io::Write, path::Path};

use chrono::Local;
use env_logger::Builder;
use log::info;

pub struct Logger;

impl Logger {
    pub fn initialize(log_path: &Path) {
        let mut binding = Builder::from_default_env();
        let builder = binding.format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        });

        #[cfg(debug_assertions)]
        {
            _ = log_path;
            builder.filter(None, log::LevelFilter::Error).init();
            info!("Debug mode: Logging to console.");
        }

        #[cfg(not(debug_assertions))]
        {
            let log_file = std::fs::File::create(log_path).unwrap_or_else(|err| {
                panic!("Failed to create log file at {:?}: {:?}", log_path, err);
            });
            builder
                .target(env_logger::Target::Pipe(Box::new(log_file)))
                .filter(None, log::LevelFilter::Info)
                .init();
            println!("Release mode: Logging to file at {:?}", log_path);
        }
    }
}
