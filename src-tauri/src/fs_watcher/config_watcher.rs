use std::{env, io::ErrorKind, path::Path, sync::LazyLock};

use log::error;
use notify::{Config, Error, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, RwLock},
};

use super::config_visitor::AppConfigVisitor;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub cpu_threshold: f32,
    pub gpu_threshold: f32,
    pub ram_usage: f32,
    pub gpu_ram: f32,
    pub timeout: u64,
    pub db_update_interval: u64,
    pub idle_threshold_period: u64,
}

impl<'de> Deserialize<'de> for AppConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "AppConfig",
            &[
                "cpu_threshold",
                "gpu_threshold",
                "ram_usage",
                "gpu_ram",
                "timeout",
                "db_update_interval",
                "idle_threshold_period",
            ],
            AppConfigVisitor,
        )
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cpu_threshold: 25.0,
            gpu_threshold: 15.0,
            ram_usage: 75.0,
            gpu_ram: 10.0,
            timeout: 900,
            db_update_interval: 30,
            idle_threshold_period: 60,
        }
    }
}

#[derive(Default, Debug)]
pub struct ConfigFile {
    pub config_message: AppConfig,
}

impl ConfigFile {
    async fn new(config_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config_file = File::create(config_path).await?;
        let default_config = AppConfig::default();
        let default_config_string = serde_json::to_string(&default_config).unwrap();
        config_file
            .write_all(default_config_string.as_bytes())
            .await?;
        Ok(Self {
            config_message: default_config,
        })
    }
}

pub async fn open_or_create_file() -> ConfigFile {
    let config_path = env::var("CONFIG_PATH")
        .unwrap_or("%AppData%\\screen_time_tracking_app\\config.json".to_owned());

    let config_path = if config_path.contains("%AppData%") {
        match dirs::config_dir() {
            Some(app_data_path) => {
                config_path.replace("%AppData%", app_data_path.to_str().unwrap())
            }
            None => {
                error!("Failed to resolve %AppData%. Using default.");
                return ConfigFile::default();
            }
        }
    } else {
        config_path
    };

    let path = Path::new(&config_path);
    let file_result = File::open(path).await;
    let mut json_string = String::new();

    match file_result {
        Ok(mut file) => {
            if let Err(err) = file.read_to_string(&mut json_string).await {
                error!("Failed to read config file: {}. Using default.", err);
                return ConfigFile::default();
            }

            match serde_json::from_str(&json_string) {
                Ok(app_config) => ConfigFile {
                    config_message: app_config,
                },
                Err(err) => {
                    error!("Failed to parse config file: {}. Using default.", err);
                    ConfigFile::default()
                }
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => match ConfigFile::new(path).await {
            Ok(new_config) => new_config,
            Err(err) => {
                error!("Failed to create new config file: {}. Using default.", err);
                ConfigFile::default()
            }
        },
        Err(err) => {
            error!(
                "Unexpected error opening config file: {}. Using default.",
                err
            );
            ConfigFile::default()
        }
    }
}

pub async fn watcher(config: &'static LazyLock<RwLock<ConfigFile>>) {
    let runtime_handle = tokio::runtime::Handle::current();
    let (sender, mut receiver) = mpsc::channel(1);

    let mut watcher = RecommendedWatcher::new(
        move |result: Result<Event, Error>| {
            let sender_clone = sender.clone();
            runtime_handle.spawn(async move {
                match result {
                    Ok(event) => {
                        if event.kind.is_modify() {
                            if let Err(e) = sender_clone.send(open_or_create_file().await).await {
                                error!("Unable to send Config details {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Watch error: {:?}", e);
                    }
                }
            });
        },
        Config::default(),
    )
    .unwrap();

    let config_path = match env::var("CONFIG_PATH") {
        Ok(path) => path,
        Err(_) => {
            error!("CONFIG_PATH environment variable is not set. Using default.");
            String::default()
        }
    };

    let config_path = if config_path.contains("%AppData%") {
        match dirs::config_dir() {
            Some(app_data_path) => {
                config_path.replace("%AppData%", app_data_path.to_str().unwrap())
            }
            None => {
                error!("Failed to resolve %AppData%. Using default.");
                String::default()
            }
        }
    } else {
        config_path
    };

    let path = Path::new(&config_path);
    if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
        error!("Unable to watch for config file: {:?}", e);
    }

    while let Some(res) = receiver.recv().await {
        *config.write().await = res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_app_config_deserialization() {
        let json_data = r#"
        {
            "cpu_threshold": 50.0,
            "gpu_threshold": 75.0,
            "ram_usage": 60.0,
            "gpu_ram": 80.0,
            "timeout": 1800,
            "db_update_interval": 300,
            "idle_threshold_period": 600
        }
        "#;

        let config: AppConfig = serde_json::from_str(json_data).expect("Failed to deserialize");

        assert_eq!(config.cpu_threshold, 50.0);
        assert_eq!(config.gpu_threshold, 75.0);
        assert_eq!(config.ram_usage, 60.0);
        assert_eq!(config.gpu_ram, 80.0);
        assert_eq!(config.timeout, 1800);
        assert_eq!(config.db_update_interval, 300);
        assert_eq!(config.idle_threshold_period, 600);
    }

    #[test]
    fn test_clamping_behavior() {
        let json_data = r#"
        {
            "cpu_threshold": 0.0,
            "gpu_threshold": 150.0,
            "ram_usage": -10.0,
            "gpu_ram": 101.0,
            "timeout": 100,
            "db_update_interval": 1000,
            "idle_threshold_period": 5
        }
        "#;

        let config: AppConfig = serde_json::from_str(json_data).expect("Failed to deserialize");

        assert_eq!(config.cpu_threshold, 1.0); // min clamp
        assert_eq!(config.gpu_threshold, 100.0); // max clamp
        assert_eq!(config.ram_usage, 1.0); // min clamp
        assert_eq!(config.gpu_ram, 100.0); // max clamp
        assert_eq!(config.timeout, 900); // min clamp
        assert_eq!(config.db_update_interval, 900); // max clamp
        assert_eq!(config.idle_threshold_period, 30); // min clamp
    }

    #[test]
    fn test_missing_field_error() {
        let json_data = r#"
        {
            "cpu_threshold": 50.0
            // missing other fields
        }
        "#;

        let result: Result<AppConfig, _> = serde_json::from_str(json_data);

        assert!(result.is_err());
    }
}
