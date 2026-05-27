use anyhow::Context;
use bollard::Docker;
use bollard::container::ListContainersOptions;
use chrono::Utc;
use doro_protocol::MetricSnapshot;
use serde_json::Value;
use serde_json::json;
use std::collections::HashSet;
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
    Containers(Value),
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
        }
    }

    pub async fn collect(&mut self, host_id: uuid::Uuid) -> Vec<CollectorEvent> {
        let mut events = vec![CollectorEvent::Metrics(self.collect_metrics(host_id))];

        if self.config.container_metrics_enabled {
            match collect_containers(self.config.docker_socket_path.as_deref()).await {
                Ok(payload) => events.push(CollectorEvent::Containers(payload)),
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
                "system": {
                    "kernel_version": System::kernel_version(),
                    "long_os_version": System::long_os_version(),
                    "os_name": System::name(),
                    "host_name": System::host_name(),
                    "cpu_arch": System::cpu_arch(),
                    "physical_core_count": System::physical_core_count(),
                    "logical_core_count": self.system.cpus().len(),
                    "uptime_seconds": System::uptime(),
                    "process_count": self.system.processes().len(),
                    "load_average": {
                        "one": load_average.one,
                        "five": load_average.five,
                        "fifteen": load_average.fifteen,
                    },
                    "memory": {
                        "total_bytes": total_memory,
                        "used_bytes": used_memory,
                        "available_bytes": self.system.available_memory(),
                    }
                },
                "cpus": self.cpu_payload(),
                "disks": self.disk_payload(),
                "networks": self.network_payload(),
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
                        "mount_point": disk.mount_point().to_string_lossy(),
                        "total_bytes": total,
                        "available_bytes": available,
                        "used_bytes": total.saturating_sub(available),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn network_payload(&self) -> Value {
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

fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    ((used as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
}

fn merge_extra(extra: &mut Value, key: &str, value: Value) {
    if let Some(map) = extra.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

async fn collect_containers(socket_path: Option<&str>) -> anyhow::Result<Value> {
    let docker = match socket_path {
        Some(path) => Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION),
        None => Docker::connect_with_unix_defaults(),
    }
    .context("failed to connect to Docker socket")?;

    let containers = docker
        .list_containers::<String>(Some(ListContainersOptions {
            all: true,
            ..Default::default()
        }))
        .await
        .context("failed to list Docker containers")?;
    let system = docker.info().await.ok();
    let networks = docker.list_networks::<String>(None).await.ok();
    let volumes = docker.list_volumes::<String>(None).await.ok();

    Ok(json!({
        "runtime": "docker",
        "daemon": system.map(|info| json!({
            "id": info.id,
            "containers": info.containers,
            "containers_running": info.containers_running,
            "containers_paused": info.containers_paused,
            "containers_stopped": info.containers_stopped,
            "images": info.images,
            "driver": info.driver,
            "docker_root_dir": info.docker_root_dir,
            "kernel_version": info.kernel_version,
            "operating_system": info.operating_system,
            "architecture": info.architecture,
            "ncpu": info.ncpu,
            "mem_total": info.mem_total,
            "server_version": info.server_version,
        })),
        "containers": containers.into_iter().map(|container| {
            json!({
                "id": container.id,
                "names": container.names,
                "image": container.image,
                "image_id": container.image_id,
                "command": container.command,
                "created": container.created,
                "ports": container.ports,
                "labels": container.labels,
                "state": container.state,
                "status": container.status,
            })
        }).collect::<Vec<_>>(),
        "networks": networks.unwrap_or_default().into_iter().map(|network| {
            json!({
                "id": network.id,
                "name": network.name,
                "driver": network.driver,
                "scope": network.scope,
                "internal": network.internal,
                "attachable": network.attachable,
                "ingress": network.ingress,
            })
        }).collect::<Vec<_>>(),
        "volumes": volumes.and_then(|volumes| volumes.volumes).unwrap_or_default().into_iter().map(|volume| {
            json!({
                "name": volume.name,
                "driver": volume.driver,
                "mountpoint": volume.mountpoint,
                "usage_size": volume.usage_data.as_ref().map(|usage| usage.size),
                "usage_ref_count": volume.usage_data.as_ref().map(|usage| usage.ref_count),
            })
        }).collect::<Vec<_>>(),
    }))
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
        assert!(metrics.extra.get("system").is_some());
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
