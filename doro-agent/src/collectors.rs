use chrono::Utc;
use doro_container::ContainerRuntimeSnapshot;
use doro_container::DockerProvider;
use doro_container::DockerProviderConfig;
use doro_protocol::MetricSnapshot;
use serde_json::Value;
use serde_json::json;
use std::collections::HashSet;
use std::time::Duration;
use std::time::Instant;
use sysinfo::Components;
use sysinfo::Disks;
use sysinfo::Networks;
use sysinfo::ProcessRefreshKind;
use sysinfo::ProcessesToUpdate;
use sysinfo::System;

#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub process_names: Vec<String>,
    pub container_metrics_enabled: bool,
    pub docker_socket_path: Option<String>,
    pub gpu_metrics_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct MetricsCapture {
    pub snapshot: MetricSnapshot,
    pub extra: Value,
}

#[derive(Debug, Clone)]
pub enum CollectorEvent {
    Metrics(MetricsCapture),
    Containers(ContainerRuntimeSnapshot),
    Error {
        collector: &'static str,
        message: String,
    },
}

#[derive(Debug)]
pub struct LocalCollectors {
    config: CollectorConfig,
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    last_sample_at: Option<Instant>,
}

impl LocalCollectors {
    pub fn new(config: CollectorConfig) -> Self {
        let mut system = System::new_all();
        system.refresh_cpu_all();
        Self {
            config,
            system,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            last_sample_at: None,
        }
    }

    pub async fn collect(&mut self, host_id: uuid::Uuid) -> Vec<CollectorEvent> {
        let mut events = vec![CollectorEvent::Metrics(self.collect_metrics(host_id))];

        if self.config.container_metrics_enabled {
            match collect_container_snapshot(self.config.docker_socket_path.clone()).await {
                Ok(snapshot) => events.push(CollectorEvent::Containers(snapshot)),
                Err(error) => events.push(CollectorEvent::Error {
                    collector: "containers",
                    message: error.to_string(),
                }),
            }
        }

        if self.config.gpu_metrics_enabled {
            match collect_gpu() {
                Ok(gpus) => {
                    if let Some(CollectorEvent::Metrics(metrics)) = events.first_mut() {
                        merge_extra(&mut metrics.extra, "gpus", gpus);
                    }
                }
                Err(error) => events.push(CollectorEvent::Error {
                    collector: "gpu",
                    message: error.to_string(),
                }),
            }
        }

        events
    }

    fn collect_metrics(&mut self, host_id: uuid::Uuid) -> MetricsCapture {
        self.system.refresh_memory();
        self.system.refresh_cpu_all();
        if !self.config.process_names.is_empty() {
            self.system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::everything(),
            );
        }
        self.disks.refresh(true);
        self.networks.refresh(true);
        self.components.refresh(true);

        let captured_at = Utc::now();
        let sampled_at = Instant::now();
        let sample_interval = self
            .last_sample_at
            .map(|last_sample_at| sampled_at.saturating_duration_since(last_sample_at));
        self.last_sample_at = Some(sampled_at);
        let total_memory = self.system.total_memory();
        let used_memory = self.system.used_memory();
        let memory_percent = percent(used_memory, total_memory);
        let (used_disk, total_disk) = self.disks.list().iter().fold((0_u64, 0_u64), |acc, disk| {
            let total = disk.total_space();
            let available = disk.available_space();
            (acc.0 + total.saturating_sub(available), acc.1 + total)
        });
        let load_average = System::load_average();
        let cpu_percent = self.system.global_cpu_usage().clamp(0.0, 100.0);
        let disk_percent = percent(used_disk, total_disk);

        MetricsCapture {
            snapshot: MetricSnapshot {
                host_id,
                captured_at,
                cpu_percent,
                memory_percent,
                disk_percent,
                load_average: load_average.one as f32,
                extra: json!({}),
            },
            extra: json!({
                "cpus": self.cpu_payload(),
                "disks": self.disk_payload(),
                "disk_io": self.disk_io_payload(sample_interval),
                "networks": self.network_payload(sample_interval),
                "components": self.component_payload(),
                "processes": self.process_payload(),
            }),
        }
    }

    fn cpu_payload(&self) -> Value {
        json!(
            self.system
                .cpus()
                .iter()
                .map(|cpu| {
                    json!({
                        "name": cpu.name(),
                        "usage_percent": cpu.cpu_usage(),
                        "frequency_mhz": cpu.frequency(),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn disk_payload(&self) -> Value {
        json!(
            self.disks
                .list()
                .iter()
                .map(|disk| {
                    let total = disk.total_space();
                    let available = disk.available_space();
                    json!({
                        "name": disk.name().to_string_lossy(),
                        "kind": disk.kind().to_string(),
                        "mount_point": disk.mount_point().to_string_lossy(),
                        "total_bytes": total,
                        "available_bytes": available,
                        "used_bytes": total.saturating_sub(available),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn disk_io_payload(&self, sample_interval: Option<Duration>) -> Value {
        json!(
            self.disks
                .list()
                .iter()
                .map(|disk| {
                    let usage = disk.usage();
                    json!({
                        "name": disk.name().to_string_lossy(),
                        "kind": disk.kind().to_string(),
                        "mount_point": disk.mount_point().to_string_lossy(),
                        "read_bytes": usage.read_bytes,
                        "written_bytes": usage.written_bytes,
                        "total_read_bytes": usage.total_read_bytes,
                        "total_written_bytes": usage.total_written_bytes,
                        "read_bytes_per_second": bytes_per_second(usage.read_bytes, sample_interval),
                        "write_bytes_per_second": bytes_per_second(usage.written_bytes, sample_interval),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn network_payload(&self, sample_interval: Option<Duration>) -> Value {
        json!(
            self.networks
                .iter()
                .map(|(name, data)| {
                    json!({
                        "name": name,
                        "received_bytes": data.received(),
                        "transmitted_bytes": data.transmitted(),
                        "total_received_bytes": data.total_received(),
                        "total_transmitted_bytes": data.total_transmitted(),
                        "received_bytes_per_second": bytes_per_second(data.received(), sample_interval),
                        "transmitted_bytes_per_second": bytes_per_second(data.transmitted(), sample_interval),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn component_payload(&self) -> Value {
        json!(
            self.components
                .list()
                .iter()
                .map(|component| {
                    json!({
                        "label": component.label(),
                        "temperature_celsius": component.temperature(),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn process_payload(&self) -> Value {
        if self.config.process_names.is_empty() {
            return json!([]);
        }

        let names = self
            .config
            .process_names
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        json!(
            self.system
                .processes()
                .iter()
                .filter_map(|(pid, process)| {
                    let process_name = process.name().to_string_lossy().to_string();
                    let command_name = process
                        .cmd()
                        .first()
                        .map(|value| value.to_string_lossy().to_string());
                    if !names.contains(&process_name)
                        && !command_name
                            .as_ref()
                            .is_some_and(|command| names.contains(command))
                    {
                        return None;
                    }
                    let disk_usage = process.disk_usage();
                    Some(json!({
                        "pid": pid.as_u32(),
                        "name": process_name,
                        "command": command_name,
                        "cpu_percent": process.cpu_usage(),
                        "memory_bytes": process.memory(),
                        "disk_read_bytes": disk_usage.read_bytes,
                        "disk_written_bytes": disk_usage.written_bytes,
                        "disk_total_read_bytes": disk_usage.total_read_bytes,
                        "disk_total_written_bytes": disk_usage.total_written_bytes,
                        "start_time": process.start_time(),
                    }))
                })
                .collect::<Vec<_>>()
        )
    }
}

async fn collect_container_snapshot(
    socket_path: Option<String>,
) -> Result<ContainerRuntimeSnapshot, doro_container::ContainerProviderError> {
    let provider = DockerProvider::connect(&DockerProviderConfig::new(socket_path))?;
    provider.snapshot().await
}

pub fn system_profile() -> Value {
    let system = System::new_all();
    json!({
        "kernel_version": System::kernel_version(),
        "long_os_version": System::long_os_version(),
        "os_name": System::name(),
        "host_name": System::host_name(),
        "cpu_arch": System::cpu_arch(),
        "physical_core_count": System::physical_core_count(),
        "logical_core_count": system.cpus().len(),
        "memory": {
            "total_bytes": system.total_memory(),
        },
    })
}

fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    ((used as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
}

fn bytes_per_second(bytes: u64, interval: Option<Duration>) -> f64 {
    let Some(interval) = interval else {
        return 0.0;
    };
    let seconds = interval.as_secs_f64();
    if seconds <= 0.0 {
        return 0.0;
    }
    bytes as f64 / seconds
}

fn merge_extra(extra: &mut Value, key: &str, value: Value) {
    if let Some(map) = extra.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

fn collect_gpu() -> anyhow::Result<Value> {
    collect_gpu_inner()
}

#[cfg(all(feature = "gpu", target_os = "linux"))]
fn collect_gpu_inner() -> anyhow::Result<Value> {
    let nvml = nvml_wrapper::Nvml::init().context("failed to initialize NVML")?;
    let device_count = nvml.device_count().context("failed to count NVIDIA GPUs")?;
    let mut devices = Vec::with_capacity(device_count as usize);
    for index in 0..device_count {
        let device = nvml.device_by_index(index)?;
        let memory = device.memory_info().ok();
        let utilization = device.utilization_rates().ok();
        let pci_info = device.pci_info().ok();
        devices.push(json!({
            "index": index,
            "name": device.name().ok(),
            "memory_total": memory.as_ref().map(|memory| memory.total),
            "memory_used": memory.as_ref().map(|memory| memory.used),
            "memory_free": memory.as_ref().map(|memory| memory.free),
            "pci_bus_id": pci_info.as_ref().map(|info| info.bus_id.clone()),
            "utilization_gpu": utilization.as_ref().map(|rates| rates.gpu),
            "utilization_memory": utilization.as_ref().map(|rates| rates.memory),
            "temperature": device
                .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                .ok(),
        }));
    }
    Ok(json!(devices))
}

#[cfg(not(all(feature = "gpu", target_os = "linux")))]
fn collect_gpu_inner() -> anyhow::Result<Value> {
    anyhow::bail!("GPU collector support requires Linux and the agent gpu feature")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_handles_zero_total_and_clamps() {
        assert_eq!(percent(1, 0), 0.0);
        assert_eq!(percent(2, 1), 100.0);
    }

    #[test]
    fn bytes_per_second_handles_missing_or_zero_interval() {
        assert_eq!(bytes_per_second(10, None), 0.0);
        assert_eq!(bytes_per_second(10, Some(Duration::from_secs(0))), 0.0);
        assert_eq!(bytes_per_second(10, Some(Duration::from_secs(2))), 5.0);
    }

    #[tokio::test]
    async fn system_sampling_produces_metric_snapshot() {
        let mut collectors = LocalCollectors::new(CollectorConfig {
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            gpu_metrics_enabled: false,
        });
        let host_id = uuid::Uuid::new_v4();
        let events = collectors.collect(host_id).await;

        let Some(CollectorEvent::Metrics(metrics)) = events.first() else {
            panic!("first collector event should be metrics");
        };
        assert_eq!(metrics.snapshot.host_id, host_id);
        assert!((0.0..=100.0).contains(&metrics.snapshot.cpu_percent));
        assert!((0.0..=100.0).contains(&metrics.snapshot.memory_percent));
        assert!((0.0..=100.0).contains(&metrics.snapshot.disk_percent));
        assert!(metrics.extra.get("system").is_none());
        assert!(metrics.extra.get("cpus").is_some());
        assert!(metrics.extra.get("networks").is_some());
        assert!(metrics.extra.get("disk_io").is_some());
        assert!(
            metrics.extra["networks"]
                .as_array()
                .is_some_and(|networks| networks.iter().all(|network| {
                    network.get("received_bytes").is_some()
                        && network.get("transmitted_bytes").is_some()
                        && network.get("received_bytes_per_second").is_some()
                        && network.get("transmitted_bytes_per_second").is_some()
                }))
        );
        assert!(
            metrics.extra["disk_io"]
                .as_array()
                .is_some_and(|disks| disks.iter().all(|disk| {
                    disk.get("read_bytes").is_some()
                        && disk.get("written_bytes").is_some()
                        && disk.get("read_bytes_per_second").is_some()
                        && disk.get("write_bytes_per_second").is_some()
                }))
        );
    }

    #[tokio::test]
    async fn repeated_sampling_reports_non_negative_io_rates() {
        let mut collectors = LocalCollectors::new(CollectorConfig {
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            gpu_metrics_enabled: false,
        });
        let host_id = uuid::Uuid::new_v4();
        let _ = collectors.collect(host_id).await;
        tokio::time::sleep(Duration::from_millis(1)).await;
        let events = collectors.collect(host_id).await;

        let Some(CollectorEvent::Metrics(metrics)) = events.first() else {
            panic!("first collector event should be metrics");
        };

        assert!(
            metrics.extra["networks"]
                .as_array()
                .is_some_and(|networks| networks.iter().all(|network| {
                    network["received_bytes_per_second"]
                        .as_f64()
                        .is_some_and(|value| value >= 0.0)
                        && network["transmitted_bytes_per_second"]
                            .as_f64()
                            .is_some_and(|value| value >= 0.0)
                }))
        );
        assert!(
            metrics.extra["disk_io"]
                .as_array()
                .is_some_and(|disks| disks.iter().all(|disk| {
                    disk["read_bytes_per_second"]
                        .as_f64()
                        .is_some_and(|value| value >= 0.0)
                        && disk["write_bytes_per_second"]
                            .as_f64()
                            .is_some_and(|value| value >= 0.0)
                }))
        );
    }

    #[test]
    fn system_profile_is_collected_for_registration() {
        let profile = system_profile();

        assert!(profile.get("cpu_arch").is_some());
        assert!(profile.get("memory").is_some());
    }

    #[tokio::test]
    async fn process_names_empty_skips_process_detail() {
        let mut collectors = LocalCollectors::new(CollectorConfig {
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            gpu_metrics_enabled: false,
        });
        let events = collectors.collect(uuid::Uuid::new_v4()).await;
        let Some(CollectorEvent::Metrics(metrics)) = events.first() else {
            panic!("first collector event should be metrics");
        };

        assert_eq!(metrics.extra["processes"], json!([]));
    }

    #[tokio::test]
    async fn process_names_filter_out_unmatched_processes() {
        let mut collectors = LocalCollectors::new(CollectorConfig {
            process_names: vec!["doro-process-that-should-not-exist".to_string()],
            container_metrics_enabled: false,
            docker_socket_path: None,
            gpu_metrics_enabled: false,
        });
        let events = collectors.collect(uuid::Uuid::new_v4()).await;
        let Some(CollectorEvent::Metrics(metrics)) = events.first() else {
            panic!("first collector event should be metrics");
        };

        assert_eq!(metrics.extra["processes"], json!([]));
    }

    #[tokio::test]
    async fn unavailable_docker_socket_returns_collector_error() {
        let mut collectors = LocalCollectors::new(CollectorConfig {
            process_names: Vec::new(),
            container_metrics_enabled: true,
            docker_socket_path: Some("/tmp/doro-missing-docker.sock".to_string()),
            gpu_metrics_enabled: false,
        });
        let events = collectors.collect(uuid::Uuid::new_v4()).await;

        assert!(events.iter().any(|event| matches!(
            event,
            CollectorEvent::Error {
                collector: "containers",
                ..
            }
        )));
    }

    #[tokio::test]
    async fn unavailable_gpu_collector_returns_error_without_stopping_metrics() {
        let mut collectors = LocalCollectors::new(CollectorConfig {
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            gpu_metrics_enabled: true,
        });
        let events = collectors.collect(uuid::Uuid::new_v4()).await;

        assert!(
            events
                .iter()
                .any(|event| matches!(event, CollectorEvent::Metrics(_)))
        );
        assert!(events.iter().any(|event| matches!(
            event,
            CollectorEvent::Error {
                collector: "gpu",
                ..
            }
        )));
    }
}
