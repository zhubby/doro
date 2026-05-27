use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "v1";

pub mod grpc {
    tonic::include_proto!("doro.agent.v1");
}

pub fn protobuf_timestamp_now() -> prost_types::Timestamp {
    let now = Utc::now();
    prost_types::Timestamp {
        seconds: now.timestamp(),
        nanos: now.timestamp_subsec_nanos() as i32,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnrollmentToken {
    pub id: Uuid,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Host {
    pub id: Uuid,
    pub hostname: String,
    pub labels: Vec<String>,
    pub status: HostStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub capabilities: Vec<AgentCapability>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostStatus {
    Pending,
    Online,
    Degraded,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCapability {
    pub name: CapabilityName,
    pub risk: CapabilityRisk,
    pub description: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityName {
    MetricsRead,
    LogsRead,
    ServicesManage,
    ContainersManage,
    FilesRead,
    FilesWrite,
    ShellExecute,
    NetworkExpose,
    DatabaseRestore,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub id: Uuid,
    pub host_id: Option<Uuid>,
    pub title: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub steps: Vec<TaskStep>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Draft,
    WaitingApproval,
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskStep {
    pub id: Uuid,
    pub capability: CapabilityName,
    pub risk: CapabilityRisk,
    pub summary: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub reason: String,
    pub status: ApprovalStatus,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricSnapshot {
    pub host_id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub disk_percent: f32,
    pub load_average: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AgentEvent {
    Connected {
        host_id: Uuid,
    },
    Heartbeat {
        host_id: Uuid,
        at: DateTime<Utc>,
    },
    CapabilitiesDeclared {
        host_id: Uuid,
        capabilities: Vec<AgentCapability>,
    },
    MetricsCaptured(MetricSnapshot),
    TaskStarted {
        task_id: Uuid,
    },
    TaskFinished {
        task_id: Uuid,
        status: TaskStatus,
    },
    ApprovalRequired(ApprovalRequest),
}

pub fn high_risk_capabilities() -> Vec<CapabilityName> {
    vec![
        CapabilityName::ShellExecute,
        CapabilityName::FilesWrite,
        CapabilityName::ContainersManage,
        CapabilityName::ServicesManage,
        CapabilityName::NetworkExpose,
        CapabilityName::DatabaseRestore,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_risk_capabilities_include_shell_execution() {
        assert!(high_risk_capabilities().contains(&CapabilityName::ShellExecute));
    }

    #[test]
    fn generated_grpc_types_are_available() {
        let command = grpc::ControlPlaneCommand {
            command_id: "command-1".to_string(),
            kind: "ack".to_string(),
            payload_json: "{}".to_string(),
            requires_approval: false,
        };

        assert_eq!(command.kind, "ack");
    }

    #[test]
    fn protobuf_timestamp_uses_current_epoch_time() {
        assert!(protobuf_timestamp_now().seconds > 0);
    }
}
