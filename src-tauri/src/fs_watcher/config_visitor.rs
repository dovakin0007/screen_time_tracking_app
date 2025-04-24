use serde::de::{Error, Visitor};

use super::config_watcher::AppConfig;

pub(crate) struct AppConfigVisitor;

impl<'de> Visitor<'de> for AppConfigVisitor {
    type Value = AppConfig;
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut cpu_threshold = None;
        let mut gpu_threshold = None;
        let mut ram_usage = None;
        let mut gpu_ram = None;
        let mut timeout = None;
        let mut db_update_interval = None;
        let mut idle_threshold_period = None;

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "cpu_threshold" => cpu_threshold = Some(map.next_value::<f32>()?.clamp(1.0, 100.0)),
                "gpu_threshold" => gpu_threshold = Some(map.next_value::<f32>()?.clamp(1.0, 100.0)),
                "ram_usage" => ram_usage = Some(map.next_value::<f32>()?.clamp(1.0, 100.0)),
                "gpu_ram" => gpu_ram = Some(map.next_value::<f32>()?.clamp(1.0, 100.0)),
                "timeout" => timeout = Some(map.next_value::<u64>()?.clamp(900, 21600)),
                "db_update_interval" => {
                    db_update_interval = Some(map.next_value::<u64>()?.clamp(1, 900))
                }
                "idle_threshold_period" => {
                    idle_threshold_period = Some(map.next_value::<u64>()?.clamp(30, 3600))
                }
                &_ => {
                    let _: serde::de::IgnoredAny = map.next_value()?;
                }
            }
        }
        let cpu_threshold =
            cpu_threshold.ok_or_else(|| A::Error::missing_field("cpu_threshold"))?;
        let gpu_threshold =
            gpu_threshold.ok_or_else(|| A::Error::missing_field("gpu_threshold"))?;
        let ram_usage = ram_usage.ok_or_else(|| A::Error::missing_field("ram_usage"))?;
        let gpu_ram = gpu_ram.ok_or_else(|| A::Error::missing_field("gpu_ram"))?;
        let timeout = timeout.ok_or_else(|| A::Error::missing_field("timeout"))?;
        let db_update_interval =
            db_update_interval.ok_or_else(|| A::Error::missing_field("db_update_interval"))?;
        let idle_threshold_period = idle_threshold_period
            .ok_or_else(|| A::Error::missing_field("idle_threshold_period"))?;

        Ok(AppConfig {
            cpu_threshold,
            gpu_threshold,
            ram_usage,
            gpu_ram,
            timeout,
            db_update_interval,
            idle_threshold_period,
        })
    }

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a map representing AppConfig")
    }
}
