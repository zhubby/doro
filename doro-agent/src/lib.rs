use chrono::Utc;
use doro_protocol::AgentCapability;
use doro_protocol::AgentEvent;
use doro_protocol::CapabilityName;
use doro_protocol::CapabilityRisk;
use doro_protocol::Host;
use doro_protocol::HostStatus;
use doro_protocol::MetricSnapshot;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub host_id: Uuid,
    pub hostname: String,
    pub control_plane_url: String,
}

impl AgentConfig {
    pub fn local(control_plane_url: impl Into<String>) -> Self {
        Self {
            host_id: Uuid::new_v4(),
            hostname: "doro-local-agent".to_string(),
            control_plane_url: control_plane_url.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Agent {
    config: AgentConfig,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    pub fn host(&self) -> Host {
        Host {
            id: self.config.host_id,
            hostname: self.config.hostname.clone(),
            labels: vec!["agent".to_string()],
            status: HostStatus::Online,
            last_seen_at: Some(Utc::now()),
            capabilities: self.capabilities(),
        }
    }

    pub fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability {
                name: CapabilityName::MetricsRead,
                risk: CapabilityRisk::Low,
                description: "Collect local host metrics".to_string(),
            },
            AgentCapability {
                name: CapabilityName::LogsRead,
                risk: CapabilityRisk::Low,
                description: "Read local service logs".to_string(),
            },
            AgentCapability {
                name: CapabilityName::ShellExecute,
                risk: CapabilityRisk::High,
                description: "Execute approved shell commands".to_string(),
            },
        ]
    }

    pub fn heartbeat(&self) -> AgentEvent {
        AgentEvent::Heartbeat {
            host_id: self.config.host_id,
            at: Utc::now(),
        }
    }

    pub fn metrics(&self) -> MetricSnapshot {
        MetricSnapshot {
            host_id: self.config.host_id,
            captured_at: Utc::now(),
            cpu_percent: 0.0,
            memory_percent: 0.0,
            disk_percent: 0.0,
            load_average: 0.0,
        }
    }
}
