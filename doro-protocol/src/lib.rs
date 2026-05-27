use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "EnrollmentToken.ts")]
pub struct EnrollmentToken {
    pub id: Uuid,
    pub label: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "Host.ts")]
pub struct Host {
    pub id: Uuid,
    pub hostname: String,
    pub labels: Vec<String>,
    pub status: HostStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub capabilities: Vec<AgentCapability>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "HostStatus.ts")]
pub enum HostStatus {
    Pending,
    Online,
    Degraded,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AgentCapability.ts")]
pub struct AgentCapability {
    pub name: CapabilityName,
    pub risk: CapabilityRisk,
    pub description: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "CapabilityName.ts")]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "CapabilityRisk.ts")]
pub enum CapabilityRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "Task.ts")]
pub struct Task {
    pub id: Uuid,
    pub host_id: Option<Uuid>,
    pub title: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub steps: Vec<TaskStep>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "TaskStatus.ts")]
pub enum TaskStatus {
    Draft,
    WaitingApproval,
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "TaskStep.ts")]
pub struct TaskStep {
    pub id: Uuid,
    pub capability: CapabilityName,
    pub risk: CapabilityRisk,
    pub summary: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ApprovalRequest.ts")]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub reason: String,
    pub status: ApprovalStatus,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ApprovalStatus.ts")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "MetricSnapshot.ts")]
pub struct MetricSnapshot {
    pub host_id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub disk_percent: f32,
    pub load_average: f32,
    pub extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "HostContainer.ts")]
pub struct HostContainer {
    pub id: Uuid,
    pub host_id: Uuid,
    pub runtime: String,
    pub container_ref: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Value,
    pub labels: Value,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
#[ts(export_to = "AgentEvent.ts")]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateTaskRequest.ts")]
pub struct CreateTaskRequest {
    pub title: String,
    pub host_id: Option<Uuid>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AuthStatusResponse.ts")]
pub struct AuthStatusResponse {
    pub registration_open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "RegisterRequest.ts")]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "LoginRequest.ts")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "RefreshTokenRequest.ts")]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "UserSummary.ts")]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AuthTokenResponse.ts")]
pub struct AuthTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub user: UserSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CurrentUserResponse.ts")]
pub struct CurrentUserResponse {
    pub user: UserSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListHostsResponse.ts")]
pub struct ListHostsResponse {
    pub items: Vec<Host>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "LatestMetricResponse.ts")]
pub struct LatestMetricResponse {
    pub item: Option<MetricSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "ListMetricSnapshotsResponse.ts")]
pub struct ListMetricSnapshotsResponse {
    pub items: Vec<MetricSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListHostContainersResponse.ts")]
pub struct ListHostContainersResponse {
    pub items: Vec<HostContainer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListTasksResponse.ts")]
pub struct ListTasksResponse {
    pub items: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListApprovalsResponse.ts")]
pub struct ListApprovalsResponse {
    pub items: Vec<ApprovalRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AppSummary.ts")]
pub struct AppSummary {
    pub id: String,
    pub name: String,
    pub category: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListAppsResponse.ts")]
pub struct ListAppsResponse {
    pub items: Vec<AppSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "SettingsResponse.ts")]
pub struct SettingsResponse {
    pub approval_policy: String,
    pub agent_transport: String,
    pub database: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "HealthResponse.ts")]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
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
    use ts_rs::Config;
    use ts_rs::TS;

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

    #[test]
    fn exports_typescript_bindings_for_ui_rest_contract() {
        let cfg = Config::new().with_out_dir("../doro-ui/types/generated");

        assert!(EnrollmentToken::export_all(&cfg).is_ok());
        assert!(Host::export_all(&cfg).is_ok());
        assert!(HostStatus::export_all(&cfg).is_ok());
        assert!(AgentCapability::export_all(&cfg).is_ok());
        assert!(CapabilityName::export_all(&cfg).is_ok());
        assert!(CapabilityRisk::export_all(&cfg).is_ok());
        assert!(Task::export_all(&cfg).is_ok());
        assert!(TaskStatus::export_all(&cfg).is_ok());
        assert!(TaskStep::export_all(&cfg).is_ok());
        assert!(ApprovalRequest::export_all(&cfg).is_ok());
        assert!(ApprovalStatus::export_all(&cfg).is_ok());
        assert!(MetricSnapshot::export_all(&cfg).is_ok());
        assert!(HostContainer::export_all(&cfg).is_ok());
        assert!(AgentEvent::export_all(&cfg).is_ok());
        assert!(CreateTaskRequest::export_all(&cfg).is_ok());
        assert!(AuthStatusResponse::export_all(&cfg).is_ok());
        assert!(RegisterRequest::export_all(&cfg).is_ok());
        assert!(LoginRequest::export_all(&cfg).is_ok());
        assert!(RefreshTokenRequest::export_all(&cfg).is_ok());
        assert!(UserSummary::export_all(&cfg).is_ok());
        assert!(AuthTokenResponse::export_all(&cfg).is_ok());
        assert!(CurrentUserResponse::export_all(&cfg).is_ok());
        assert!(ListHostsResponse::export_all(&cfg).is_ok());
        assert!(LatestMetricResponse::export_all(&cfg).is_ok());
        assert!(ListMetricSnapshotsResponse::export_all(&cfg).is_ok());
        assert!(ListHostContainersResponse::export_all(&cfg).is_ok());
        assert!(ListTasksResponse::export_all(&cfg).is_ok());
        assert!(ListApprovalsResponse::export_all(&cfg).is_ok());
        assert!(AppSummary::export_all(&cfg).is_ok());
        assert!(ListAppsResponse::export_all(&cfg).is_ok());
        assert!(SettingsResponse::export_all(&cfg).is_ok());
        assert!(HealthResponse::export_all(&cfg).is_ok());
    }
}
