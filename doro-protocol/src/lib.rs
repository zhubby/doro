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
    protobuf_timestamp_from_utc(Utc::now())
}

pub fn protobuf_timestamp_from_utc(value: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: value.timestamp(),
        nanos: value.timestamp_subsec_nanos() as i32,
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
#[ts(export_to = "CreateEnrollmentTokenRequest.ts")]
pub struct CreateEnrollmentTokenRequest {
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateEnrollmentTokenResponse.ts")]
pub struct CreateEnrollmentTokenResponse {
    pub item: EnrollmentToken,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "Host.ts")]
pub struct Host {
    pub id: Uuid,
    pub hostname: String,
    pub display_name: String,
    pub labels: Vec<String>,
    pub status: HostStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub capabilities: Vec<AgentCapability>,
    pub system_profile: Value,
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
    AgentRun,
    ServicesManage,
    ContainersManage,
    VirtualMachinesManage,
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
    pub status: TaskStepStatus,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "TaskStepStatus.ts")]
pub enum TaskStepStatus {
    Pending,
    WaitingApproval,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ScheduledTaskKind.ts")]
pub enum ScheduledTaskKind {
    Script,
    AgentRun,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ScheduledTaskStatus.ts")]
pub enum ScheduledTaskStatus {
    PendingApproval,
    Active,
    Paused,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ScheduledTaskRunStatus.ts")]
pub enum ScheduledTaskRunStatus {
    Running,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ScheduledTask.ts")]
pub struct ScheduledTask {
    pub id: Uuid,
    pub name: String,
    pub kind: ScheduledTaskKind,
    pub schedule: String,
    pub status: ScheduledTaskStatus,
    pub required_capability: CapabilityName,
    pub label_selector: Vec<String>,
    pub task_template: Value,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<ScheduledTaskRunStatus>,
    pub approval_task_id: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ScheduledTaskRun.ts")]
pub struct ScheduledTaskRun {
    pub id: Uuid,
    pub scheduled_task_id: Uuid,
    pub task_id: Option<Uuid>,
    pub status: ScheduledTaskRunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub message: Option<String>,
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
    pub expires_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
    pub decision_note: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateApprovalRequest.ts")]
pub struct CreateApprovalRequest {
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub reason: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateApprovalResponse.ts")]
pub struct CreateApprovalResponse {
    pub item: ApprovalRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ResolveApprovalRequest.ts")]
pub struct ResolveApprovalRequest {
    pub decision_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ResolveApprovalResponse.ts")]
pub struct ResolveApprovalResponse {
    pub item: ApprovalRequest,
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
    pub created_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "DockerContainerSummary.ts")]
pub struct DockerContainerSummary {
    pub host_id: Uuid,
    pub runtime: String,
    pub id: Option<String>,
    pub names: Vec<String>,
    pub image: Option<String>,
    pub image_id: Option<String>,
    pub command: Option<String>,
    pub created: Option<i64>,
    pub ports: Value,
    pub labels: Value,
    pub state: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "DockerImageSummary.ts")]
pub struct DockerImageSummary {
    pub host_id: Uuid,
    pub id: Option<String>,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub created: Option<i64>,
    pub size: Option<i64>,
    pub labels: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "DockerNetworkSummary.ts")]
pub struct DockerNetworkSummary {
    pub host_id: Uuid,
    pub id: Option<String>,
    pub name: Option<String>,
    pub driver: Option<String>,
    pub scope: Option<String>,
    pub internal: Option<bool>,
    pub attachable: Option<bool>,
    pub ingress: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "DockerVolumeSummary.ts")]
pub struct DockerVolumeSummary {
    pub host_id: Uuid,
    pub name: String,
    pub driver: Option<String>,
    pub mountpoint: Option<String>,
    pub labels: Value,
    pub usage_size: Option<i64>,
    pub usage_ref_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "DockerComposeProject.ts")]
pub struct DockerComposeProject {
    pub host_id: Uuid,
    pub name: String,
    pub status: String,
    pub path: String,
    pub services: Vec<String>,
    pub compose_yaml: Option<String>,
    pub env_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "DockerActionRequest.ts")]
pub struct DockerActionRequest {
    pub host_id: Option<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "DockerContainerCreateRequest.ts")]
pub struct DockerContainerCreateRequest {
    pub host_id: Uuid,
    pub name: String,
    pub image: String,
    pub command: Vec<String>,
    pub env: Vec<String>,
    pub labels: Value,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "DockerImagePullRequest.ts")]
pub struct DockerImagePullRequest {
    pub host_id: Uuid,
    pub reference: String,
    pub tag: Option<String>,
    pub platform: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "DockerImageRemoveRequest.ts")]
pub struct DockerImageRemoveRequest {
    pub host_id: Uuid,
    pub reference: String,
    pub force: bool,
    pub noprune: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "DockerNetworkCreateRequest.ts")]
pub struct DockerNetworkCreateRequest {
    pub host_id: Uuid,
    pub name: String,
    pub driver: String,
    pub internal: bool,
    pub attachable: bool,
    pub labels: Value,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "DockerNetworkContainerRequest.ts")]
pub struct DockerNetworkContainerRequest {
    pub host_id: Uuid,
    pub container: String,
    pub force: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "DockerVolumeCreateRequest.ts")]
pub struct DockerVolumeCreateRequest {
    pub host_id: Uuid,
    pub name: String,
    pub driver: String,
    pub driver_opts: Value,
    pub labels: Value,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "DockerComposeProjectRequest.ts")]
pub struct DockerComposeProjectRequest {
    pub host_id: Uuid,
    pub name: String,
    pub compose_yaml: String,
    pub env_file: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "DockerActionResponse.ts")]
pub struct DockerActionResponse {
    pub task: Task,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "WebsiteStatus.ts")]
pub enum WebsiteStatus {
    Stopped,
    Running,
    Warning,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "WebsiteKind.ts")]
pub enum WebsiteKind {
    ReverseProxy,
    StaticSite,
    TcpProxy,
    UdpProxy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "WebsiteProtocol.ts")]
pub enum WebsiteProtocol {
    Http,
    Https,
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "WebsiteProxyTarget.ts")]
pub struct WebsiteProxyTarget {
    pub url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "WebsitePlannedCapability.ts")]
pub enum WebsitePlannedCapability {
    Https,
    CertificateBinding,
    StaticSite,
    UpstreamPool,
    RewriteRules,
    TcpProxy,
    UdpProxy,
    RealIp,
    AccessControl,
    PasswordGate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "Website.ts")]
pub struct Website {
    pub id: Uuid,
    pub host_id: Option<Uuid>,
    pub name: String,
    pub primary_domain: String,
    pub aliases: Vec<String>,
    pub status: WebsiteStatus,
    pub kind: WebsiteKind,
    pub protocol: WebsiteProtocol,
    pub listen_port: u16,
    pub upstream: WebsiteProxyTarget,
    pub app_install_id: Option<Uuid>,
    pub tls_certificate_id: Option<Uuid>,
    pub config: Value,
    pub notes: Option<String>,
    pub last_runtime_error: Option<String>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateWebsiteRequest.ts")]
pub struct CreateWebsiteRequest {
    pub host_id: Uuid,
    pub name: String,
    pub primary_domain: String,
    pub aliases: Vec<String>,
    pub listen_port: u16,
    pub upstream_url: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "UpdateWebsiteRequest.ts")]
pub struct UpdateWebsiteRequest {
    pub host_id: Uuid,
    pub name: String,
    pub primary_domain: String,
    pub aliases: Vec<String>,
    pub listen_port: u16,
    pub upstream_url: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "WebsiteActionRequest.ts")]
pub struct WebsiteActionRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "WebsiteActionResponse.ts")]
pub struct WebsiteActionResponse {
    pub item: Website,
    pub task: Option<Task>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "VirtualMachineStatus.ts")]
pub enum VirtualMachineStatus {
    Unknown,
    Stopped,
    Starting,
    Running,
    Paused,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "VirtualMachineNetworkMode.ts")]
pub enum VirtualMachineNetworkMode {
    UserNat,
    BridgeTap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "VirtualMachinePortForward.ts")]
pub struct VirtualMachinePortForward {
    pub host_port: u16,
    pub guest_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "VirtualMachineNetwork.ts")]
pub struct VirtualMachineNetwork {
    pub mode: VirtualMachineNetworkMode,
    pub bridge: Option<String>,
    pub mac_address: Option<String>,
    pub port_forwards: Vec<VirtualMachinePortForward>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "VirtualMachineDisk.ts")]
pub struct VirtualMachineDisk {
    pub path: String,
    pub size_gb: u32,
    pub format: String,
    pub boot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "VirtualMachineImage.ts")]
pub struct VirtualMachineImage {
    pub host_id: Option<Uuid>,
    pub id: String,
    pub name: String,
    pub path: String,
    pub os_family: Option<String>,
    pub architecture: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "VirtualMachine.ts")]
pub struct VirtualMachine {
    pub id: Uuid,
    pub host_id: Uuid,
    pub vm_ref: String,
    pub name: String,
    pub status: VirtualMachineStatus,
    pub provider: String,
    pub image: String,
    pub cpu_cores: u16,
    pub memory_mib: u32,
    pub disk_gb: u32,
    pub networks: Vec<VirtualMachineNetwork>,
    pub console: Option<Value>,
    pub metadata: Value,
    pub created_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "VirtualMachineTemplate.ts")]
pub struct VirtualMachineTemplate {
    pub id: String,
    pub name: String,
    pub image_id: String,
    pub cpu_cores: u16,
    pub memory_mib: u32,
    pub disk_gb: u32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "VirtualMachineSnapshot.ts")]
pub struct VirtualMachineSnapshot {
    pub id: Uuid,
    pub vm_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "CreateVirtualMachineRequest.ts")]
pub struct CreateVirtualMachineRequest {
    pub host_id: Uuid,
    pub name: String,
    pub image_id: String,
    pub cpu_cores: u16,
    pub memory_mib: u32,
    pub disk_gb: u32,
    pub networks: Vec<VirtualMachineNetwork>,
    pub cloud_init: Value,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "VirtualMachineActionRequest.ts")]
pub struct VirtualMachineActionRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "VirtualMachineActionResponse.ts")]
pub struct VirtualMachineActionResponse {
    pub task: Task,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateVirtualMachineSnapshotRequest.ts")]
pub struct CreateVirtualMachineSnapshotRequest {
    pub name: String,
    pub description: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "VirtualMachineConsoleResponse.ts")]
pub struct VirtualMachineConsoleResponse {
    pub item: Value,
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
    pub ai_provider_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AiModelProvider.ts")]
pub struct AiModelProvider {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout_seconds: u32,
    pub enabled: bool,
    pub has_api_key: bool,
    pub api_key_hint: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateAiModelProviderRequest.ts")]
pub struct CreateAiModelProviderRequest {
    pub name: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout_seconds: u32,
    pub api_key: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "UpdateAiModelProviderRequest.ts")]
pub struct UpdateAiModelProviderRequest {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub timeout_seconds: Option<u32>,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AiModelProviderResponse.ts")]
pub struct AiModelProviderResponse {
    pub item: AiModelProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListAiModelProvidersResponse.ts")]
pub struct ListAiModelProvidersResponse {
    pub items: Vec<AiModelProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AiConversation.ts")]
pub struct AiConversation {
    pub id: Uuid,
    pub title: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "AiChatMessageRole.ts")]
pub enum AiChatMessageRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "AiChatMessageStatus.ts")]
pub enum AiChatMessageStatus {
    Pending,
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "AiChatEventKind.ts")]
pub enum AiChatEventKind {
    TextDelta,
    ToolCall,
    ApprovalRequired,
    ToolResult,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AiChatMessage.ts")]
pub struct AiChatMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: AiChatMessageRole,
    pub status: AiChatMessageStatus,
    pub content: String,
    pub task_id: Option<Uuid>,
    pub host_id: Option<Uuid>,
    pub ai_provider_id: Option<Uuid>,
    pub model: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AiChatEvent.ts")]
pub struct AiChatEvent {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub message_id: Uuid,
    pub kind: AiChatEventKind,
    pub content: Option<String>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateAiConversationRequest.ts")]
pub struct CreateAiConversationRequest {
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateAiChatTurnRequest.ts")]
pub struct CreateAiChatTurnRequest {
    pub host_id: Uuid,
    pub ai_provider_id: Uuid,
    pub model: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AiConversationResponse.ts")]
pub struct AiConversationResponse {
    pub item: AiConversation,
    pub messages: Vec<AiChatMessage>,
    pub events: Vec<AiChatEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListAiConversationsResponse.ts")]
pub struct ListAiConversationsResponse {
    pub items: Vec<AiConversation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateAiChatTurnResponse.ts")]
pub struct CreateAiChatTurnResponse {
    pub user_message: AiChatMessage,
    pub assistant_message: AiChatMessage,
    pub task: Task,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "AiChatStreamEvent.ts")]
pub struct AiChatStreamEvent {
    pub event_id: Uuid,
    pub conversation_id: Uuid,
    pub message_id: Uuid,
    pub kind: AiChatEventKind,
    pub content: Option<String>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateScheduledTaskRequest.ts")]
pub struct CreateScheduledTaskRequest {
    pub name: String,
    pub kind: ScheduledTaskKind,
    pub schedule: String,
    pub label_selector: Vec<String>,
    pub script: Option<String>,
    pub prompt: Option<String>,
    pub timeout_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "UpdateScheduledTaskRequest.ts")]
pub struct UpdateScheduledTaskRequest {
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub label_selector: Option<Vec<String>>,
    pub script: Option<String>,
    pub prompt: Option<String>,
    pub timeout_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "TerminalCommandRequest.ts")]
pub struct TerminalCommandRequest {
    pub host_id: Uuid,
    pub input: String,
    pub cols: Option<u32>,
    pub rows: Option<u32>,
    pub timeout_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "TerminalCommandResponse.ts")]
pub struct TerminalCommandResponse {
    pub command_id: String,
    pub host_id: Uuid,
    pub status: String,
    pub output: String,
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "FileEntryKind.ts")]
pub enum FileEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "FileEntry.ts")]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub kind: FileEntryKind,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub readonly: bool,
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "FileDirectoryResponse.ts")]
pub struct FileDirectoryResponse {
    pub path: String,
    pub parent_path: Option<String>,
    pub items: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "FileSearchResponse.ts")]
pub struct FileSearchResponse {
    pub items: Vec<FileEntry>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "FileOperationKind.ts")]
pub enum FileOperationKind {
    CreateDirectory,
    Rename,
    Move,
    Copy,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "FileOperationRequest.ts")]
pub struct FileOperationRequest {
    pub operation: FileOperationKind,
    pub path: String,
    pub target_path: Option<String>,
    pub name: Option<String>,
    pub overwrite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "FileOperationResponse.ts")]
pub struct FileOperationResponse {
    pub item: Option<FileEntry>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "FileUploadRequest.ts")]
pub struct FileUploadRequest {
    pub path: String,
    pub content_base64: String,
    pub overwrite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "FileUploadResponse.ts")]
pub struct FileUploadResponse {
    pub item: FileEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "FileDownloadResponse.ts")]
pub struct FileDownloadResponse {
    pub path: String,
    pub name: String,
    pub content_base64: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "RuntimeLogEntry.ts")]
pub struct RuntimeLogEntry {
    pub id: Uuid,
    pub source: String,
    pub host_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: Value,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "ListRuntimeLogsResponse.ts")]
pub struct ListRuntimeLogsResponse {
    pub items: Vec<RuntimeLogEntry>,
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
#[ts(export_to = "UpdateCurrentUserRequest.ts")]
pub struct UpdateCurrentUserRequest {
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "UpdateCurrentUserResponse.ts")]
pub struct UpdateCurrentUserResponse {
    pub user: UserSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ChangeCurrentUserPasswordRequest.ts")]
pub struct ChangeCurrentUserPasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListHostsResponse.ts")]
pub struct ListHostsResponse {
    pub items: Vec<Host>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "UpdateHostRequest.ts")]
pub struct UpdateHostRequest {
    pub display_name: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "UpdateHostResponse.ts")]
pub struct UpdateHostResponse {
    pub item: Host,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "ListDockerContainersResponse.ts")]
pub struct ListDockerContainersResponse {
    pub items: Vec<DockerContainerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "ListDockerImagesResponse.ts")]
pub struct ListDockerImagesResponse {
    pub items: Vec<DockerImageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListDockerNetworksResponse.ts")]
pub struct ListDockerNetworksResponse {
    pub items: Vec<DockerNetworkSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "ListDockerVolumesResponse.ts")]
pub struct ListDockerVolumesResponse {
    pub items: Vec<DockerVolumeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListDockerComposeProjectsResponse.ts")]
pub struct ListDockerComposeProjectsResponse {
    pub items: Vec<DockerComposeProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "DockerComposeProjectResponse.ts")]
pub struct DockerComposeProjectResponse {
    pub item: DockerComposeProject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "ListWebsitesResponse.ts")]
pub struct ListWebsitesResponse {
    pub items: Vec<Website>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export_to = "ListVirtualMachinesResponse.ts")]
pub struct ListVirtualMachinesResponse {
    pub items: Vec<VirtualMachine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListVirtualMachineImagesResponse.ts")]
pub struct ListVirtualMachineImagesResponse {
    pub items: Vec<VirtualMachineImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListVirtualMachineTemplatesResponse.ts")]
pub struct ListVirtualMachineTemplatesResponse {
    pub items: Vec<VirtualMachineTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListVirtualMachineSnapshotsResponse.ts")]
pub struct ListVirtualMachineSnapshotsResponse {
    pub items: Vec<VirtualMachineSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ControlPlaneEnvironment.ts")]
pub struct ControlPlaneEnvironment {
    pub hostname: String,
    pub os_version: String,
    pub kernel_version: String,
    pub architecture: String,
    pub host_address: String,
    pub booted_at: Option<DateTime<Utc>>,
    pub uptime_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ControlPlaneEnvironmentResponse.ts")]
pub struct ControlPlaneEnvironmentResponse {
    pub item: ControlPlaneEnvironment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListTasksResponse.ts")]
pub struct ListTasksResponse {
    pub items: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListScheduledTasksResponse.ts")]
pub struct ListScheduledTasksResponse {
    pub items: Vec<ScheduledTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "CreateScheduledTaskResponse.ts")]
pub struct CreateScheduledTaskResponse {
    pub item: ScheduledTask,
    pub approval_task: Option<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "UpdateScheduledTaskResponse.ts")]
pub struct UpdateScheduledTaskResponse {
    pub item: ScheduledTask,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ScheduledTaskActionResponse.ts")]
pub struct ScheduledTaskActionResponse {
    pub item: ScheduledTask,
    pub task: Option<Task>,
    pub runs: Vec<ScheduledTaskRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export_to = "ListScheduledTaskRunsResponse.ts")]
pub struct ListScheduledTaskRunsResponse {
    pub items: Vec<ScheduledTaskRun>,
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
        CapabilityName::VirtualMachinesManage,
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
        assert!(!high_risk_capabilities().contains(&CapabilityName::AgentRun));
    }

    #[test]
    fn generated_grpc_types_are_available() {
        let command = grpc::ControlPlaneCommand {
            command_id: "command-1".to_string(),
            issued_at: None,
            command: Some(grpc::control_plane_command::Command::Shutdown(
                grpc::ShutdownCommand {
                    reason: "control-plane shutting down".to_string(),
                },
            )),
        };

        assert!(matches!(
            command.command,
            Some(grpc::control_plane_command::Command::Shutdown(_))
        ));
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
        assert!(TaskStepStatus::export_all(&cfg).is_ok());
        assert!(ScheduledTaskKind::export_all(&cfg).is_ok());
        assert!(ScheduledTaskStatus::export_all(&cfg).is_ok());
        assert!(ScheduledTaskRunStatus::export_all(&cfg).is_ok());
        assert!(ScheduledTask::export_all(&cfg).is_ok());
        assert!(ScheduledTaskRun::export_all(&cfg).is_ok());
        assert!(ApprovalRequest::export_all(&cfg).is_ok());
        assert!(ApprovalStatus::export_all(&cfg).is_ok());
        assert!(CreateApprovalRequest::export_all(&cfg).is_ok());
        assert!(CreateApprovalResponse::export_all(&cfg).is_ok());
        assert!(ResolveApprovalRequest::export_all(&cfg).is_ok());
        assert!(ResolveApprovalResponse::export_all(&cfg).is_ok());
        assert!(MetricSnapshot::export_all(&cfg).is_ok());
        assert!(HostContainer::export_all(&cfg).is_ok());
        assert!(DockerContainerSummary::export_all(&cfg).is_ok());
        assert!(DockerImageSummary::export_all(&cfg).is_ok());
        assert!(DockerNetworkSummary::export_all(&cfg).is_ok());
        assert!(DockerVolumeSummary::export_all(&cfg).is_ok());
        assert!(DockerComposeProject::export_all(&cfg).is_ok());
        assert!(DockerActionRequest::export_all(&cfg).is_ok());
        assert!(DockerContainerCreateRequest::export_all(&cfg).is_ok());
        assert!(DockerImagePullRequest::export_all(&cfg).is_ok());
        assert!(DockerImageRemoveRequest::export_all(&cfg).is_ok());
        assert!(DockerNetworkCreateRequest::export_all(&cfg).is_ok());
        assert!(DockerNetworkContainerRequest::export_all(&cfg).is_ok());
        assert!(DockerVolumeCreateRequest::export_all(&cfg).is_ok());
        assert!(DockerComposeProjectRequest::export_all(&cfg).is_ok());
        assert!(DockerActionResponse::export_all(&cfg).is_ok());
        assert!(VirtualMachineStatus::export_all(&cfg).is_ok());
        assert!(VirtualMachineNetworkMode::export_all(&cfg).is_ok());
        assert!(VirtualMachinePortForward::export_all(&cfg).is_ok());
        assert!(VirtualMachineNetwork::export_all(&cfg).is_ok());
        assert!(VirtualMachineDisk::export_all(&cfg).is_ok());
        assert!(VirtualMachineImage::export_all(&cfg).is_ok());
        assert!(VirtualMachine::export_all(&cfg).is_ok());
        assert!(VirtualMachineTemplate::export_all(&cfg).is_ok());
        assert!(VirtualMachineSnapshot::export_all(&cfg).is_ok());
        assert!(CreateVirtualMachineRequest::export_all(&cfg).is_ok());
        assert!(VirtualMachineActionRequest::export_all(&cfg).is_ok());
        assert!(VirtualMachineActionResponse::export_all(&cfg).is_ok());
        assert!(CreateVirtualMachineSnapshotRequest::export_all(&cfg).is_ok());
        assert!(VirtualMachineConsoleResponse::export_all(&cfg).is_ok());
        assert!(AgentEvent::export_all(&cfg).is_ok());
        assert!(CreateTaskRequest::export_all(&cfg).is_ok());
        assert!(AiModelProvider::export_all(&cfg).is_ok());
        assert!(CreateAiModelProviderRequest::export_all(&cfg).is_ok());
        assert!(UpdateAiModelProviderRequest::export_all(&cfg).is_ok());
        assert!(AiModelProviderResponse::export_all(&cfg).is_ok());
        assert!(ListAiModelProvidersResponse::export_all(&cfg).is_ok());
        assert!(AiConversation::export_all(&cfg).is_ok());
        assert!(AiChatMessageRole::export_all(&cfg).is_ok());
        assert!(AiChatMessageStatus::export_all(&cfg).is_ok());
        assert!(AiChatEventKind::export_all(&cfg).is_ok());
        assert!(AiChatMessage::export_all(&cfg).is_ok());
        assert!(AiChatEvent::export_all(&cfg).is_ok());
        assert!(CreateAiConversationRequest::export_all(&cfg).is_ok());
        assert!(CreateAiChatTurnRequest::export_all(&cfg).is_ok());
        assert!(AiConversationResponse::export_all(&cfg).is_ok());
        assert!(ListAiConversationsResponse::export_all(&cfg).is_ok());
        assert!(CreateAiChatTurnResponse::export_all(&cfg).is_ok());
        assert!(AiChatStreamEvent::export_all(&cfg).is_ok());
        assert!(CreateScheduledTaskRequest::export_all(&cfg).is_ok());
        assert!(UpdateScheduledTaskRequest::export_all(&cfg).is_ok());
        assert!(TerminalCommandRequest::export_all(&cfg).is_ok());
        assert!(TerminalCommandResponse::export_all(&cfg).is_ok());
        assert!(FileEntryKind::export_all(&cfg).is_ok());
        assert!(FileEntry::export_all(&cfg).is_ok());
        assert!(FileDirectoryResponse::export_all(&cfg).is_ok());
        assert!(FileSearchResponse::export_all(&cfg).is_ok());
        assert!(FileOperationKind::export_all(&cfg).is_ok());
        assert!(FileOperationRequest::export_all(&cfg).is_ok());
        assert!(FileOperationResponse::export_all(&cfg).is_ok());
        assert!(FileUploadRequest::export_all(&cfg).is_ok());
        assert!(FileUploadResponse::export_all(&cfg).is_ok());
        assert!(FileDownloadResponse::export_all(&cfg).is_ok());
        assert!(RuntimeLogEntry::export_all(&cfg).is_ok());
        assert!(ListRuntimeLogsResponse::export_all(&cfg).is_ok());
        assert!(WebsiteStatus::export_all(&cfg).is_ok());
        assert!(WebsiteKind::export_all(&cfg).is_ok());
        assert!(WebsiteProtocol::export_all(&cfg).is_ok());
        assert!(WebsiteProxyTarget::export_all(&cfg).is_ok());
        assert!(WebsitePlannedCapability::export_all(&cfg).is_ok());
        assert!(Website::export_all(&cfg).is_ok());
        assert!(CreateWebsiteRequest::export_all(&cfg).is_ok());
        assert!(UpdateWebsiteRequest::export_all(&cfg).is_ok());
        assert!(WebsiteActionRequest::export_all(&cfg).is_ok());
        assert!(WebsiteActionResponse::export_all(&cfg).is_ok());
        assert!(CreateEnrollmentTokenRequest::export_all(&cfg).is_ok());
        assert!(CreateEnrollmentTokenResponse::export_all(&cfg).is_ok());
        assert!(AuthStatusResponse::export_all(&cfg).is_ok());
        assert!(RegisterRequest::export_all(&cfg).is_ok());
        assert!(LoginRequest::export_all(&cfg).is_ok());
        assert!(RefreshTokenRequest::export_all(&cfg).is_ok());
        assert!(UserSummary::export_all(&cfg).is_ok());
        assert!(AuthTokenResponse::export_all(&cfg).is_ok());
        assert!(CurrentUserResponse::export_all(&cfg).is_ok());
        assert!(UpdateCurrentUserRequest::export_all(&cfg).is_ok());
        assert!(UpdateCurrentUserResponse::export_all(&cfg).is_ok());
        assert!(ChangeCurrentUserPasswordRequest::export_all(&cfg).is_ok());
        assert!(ListHostsResponse::export_all(&cfg).is_ok());
        assert!(UpdateHostRequest::export_all(&cfg).is_ok());
        assert!(UpdateHostResponse::export_all(&cfg).is_ok());
        assert!(LatestMetricResponse::export_all(&cfg).is_ok());
        assert!(ListMetricSnapshotsResponse::export_all(&cfg).is_ok());
        assert!(ListHostContainersResponse::export_all(&cfg).is_ok());
        assert!(ListDockerContainersResponse::export_all(&cfg).is_ok());
        assert!(ListDockerImagesResponse::export_all(&cfg).is_ok());
        assert!(ListDockerNetworksResponse::export_all(&cfg).is_ok());
        assert!(ListDockerVolumesResponse::export_all(&cfg).is_ok());
        assert!(ListDockerComposeProjectsResponse::export_all(&cfg).is_ok());
        assert!(DockerComposeProjectResponse::export_all(&cfg).is_ok());
        assert!(ListWebsitesResponse::export_all(&cfg).is_ok());
        assert!(ListVirtualMachinesResponse::export_all(&cfg).is_ok());
        assert!(ListVirtualMachineImagesResponse::export_all(&cfg).is_ok());
        assert!(ListVirtualMachineTemplatesResponse::export_all(&cfg).is_ok());
        assert!(ListVirtualMachineSnapshotsResponse::export_all(&cfg).is_ok());
        assert!(ControlPlaneEnvironment::export_all(&cfg).is_ok());
        assert!(ControlPlaneEnvironmentResponse::export_all(&cfg).is_ok());
        assert!(ListTasksResponse::export_all(&cfg).is_ok());
        assert!(ListScheduledTasksResponse::export_all(&cfg).is_ok());
        assert!(CreateScheduledTaskResponse::export_all(&cfg).is_ok());
        assert!(UpdateScheduledTaskResponse::export_all(&cfg).is_ok());
        assert!(ScheduledTaskActionResponse::export_all(&cfg).is_ok());
        assert!(ListScheduledTaskRunsResponse::export_all(&cfg).is_ok());
        assert!(ListApprovalsResponse::export_all(&cfg).is_ok());
        assert!(AppSummary::export_all(&cfg).is_ok());
        assert!(ListAppsResponse::export_all(&cfg).is_ok());
        assert!(SettingsResponse::export_all(&cfg).is_ok());
        assert!(HealthResponse::export_all(&cfg).is_ok());
    }
}
