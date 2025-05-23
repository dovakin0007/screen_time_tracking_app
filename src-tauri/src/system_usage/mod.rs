use anyhow::Result;
use nvml_wrapper::Nvml;
use sysinfo::{MemoryRefreshKind, System};
use tokio::time;

use crate::fs_watcher::config_watcher::AppConfig;

#[derive(Debug, Clone, Copy)]
pub struct SystemUsage {
    pub gpu_usage: f32,
    pub gpu_mem_usage: f32,
    pub cpu_usage: f32,
    pub ram_usage: f32,
}

pub struct Machine {
    sys_info: System,
    nvml: Option<Nvml>,
}

impl Machine {
    pub fn new() -> Self {
        let nvml = Nvml::init().ok();
        Self {
            sys_info: System::new(),
            nvml,
        }
    }

    fn memory_usage(&mut self) -> f32 {
        let system_total_memory = self.sys_info.total_memory() as f32;
        let available_memory = self.sys_info.available_memory() as f32;

        (1.0 - (available_memory / system_total_memory)) * 100.0
    }

    async fn cpu_usage(&mut self) -> f32 {
        self.sys_info.refresh_cpu_all();
        time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;

        self.sys_info.refresh_cpu_all();

        self.sys_info.global_cpu_usage()
    }

    fn gpu_usage(&self) -> Result<(f32, f32)> {
        if let Some(nvml) = &self.nvml {
            let gpu_count = nvml.device_count()?;
            if gpu_count == 0 {
                return Err(nvml_wrapper::error::NvmlError::NotFound.into());
            }

            let mut total_gpu_util = 0.0;
            let mut total_mem_util = 0.0;

            for index in 0..gpu_count {
                let device = nvml.device_by_index(index)?;
                let utilization = device.utilization_rates()?;
                total_gpu_util += utilization.gpu as f32;
                total_mem_util += utilization.memory as f32;
            }

            Ok((
                total_gpu_util / gpu_count as f32,
                total_mem_util / gpu_count as f32,
            ))
        } else {
            Err(nvml_wrapper::error::NvmlError::NotSupported.into())
        }
    }

    async fn get_system_usage(&mut self) -> SystemUsage {
        self.sys_info
            .refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());

        let cpu_usage = self.cpu_usage().await;
        let ram_usage = self.memory_usage();
        let (gpu_usage, gpu_mem_usage) = self.gpu_usage().unwrap_or((0.0, 0.0));

        SystemUsage {
            gpu_usage,
            gpu_mem_usage,
            cpu_usage,
            ram_usage,
        }
    }

    pub async fn check_system_usage(&mut self, is_idle: bool, app_config: &AppConfig) -> bool {
        self.sys_info.refresh_cpu_usage();
        self.sys_info
            .refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());

        let metrics = self.get_system_usage().await;

        metrics.cpu_usage <= app_config.cpu_threshold
            && metrics.ram_usage < app_config.ram_usage
            && is_idle
            && metrics.gpu_usage < app_config.gpu_threshold
            && metrics.gpu_mem_usage < 150.0
    }
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_system_usage() {
        let mut machine = Machine::new();
        let usage = machine.get_system_usage().await;

        println!(
            "CPU Usage: {:.2}% | RAM Usage: {:.2}% | GPU Usage: {:.2}% | GPU Memory Usage: {:.2}%",
            usage.cpu_usage, usage.ram_usage, usage.gpu_usage, usage.gpu_mem_usage
        );

        assert!(
            usage.cpu_usage >= 0.0 && usage.cpu_usage <= 100.0,
            "CPU usage out of range"
        );
        assert!(
            usage.ram_usage >= 0.0 && usage.ram_usage <= 100.0,
            "RAM usage out of range"
        );
        assert!(
            usage.gpu_usage >= 0.0 && usage.gpu_usage <= 100.0,
            "GPU usage out of range"
        );
        assert!(
            usage.gpu_mem_usage >= 0.0 && usage.gpu_mem_usage <= 100.0,
            "GPU memory usage out of range"
        );
    }
}
