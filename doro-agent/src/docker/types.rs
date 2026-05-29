use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerConfig {
    pub socket_path: Option<String>,
}

impl DockerConfig {
    pub fn new(socket_path: Option<String>) -> Self {
        Self { socket_path }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockerHealth {
    pub id: Option<String>,
    pub server_version: Option<String>,
    pub docker_root_dir: Option<String>,
    pub containers: Option<i64>,
    pub images: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerListFilter {
    pub all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerSummary {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerDetail {
    pub id: Option<String>,
    pub name: Option<String>,
    pub image: Option<String>,
    pub state: Value,
    pub config: Value,
    pub host_config: Value,
    pub network_settings: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateContainerRequest {
    pub name: String,
    pub image: String,
    pub command: Vec<String>,
    pub env: Vec<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopContainerRequest {
    pub id_or_name: String,
    pub timeout_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestartContainerRequest {
    pub id_or_name: String,
    pub timeout_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoveContainerRequest {
    pub id_or_name: String,
    pub force: bool,
    pub remove_volumes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerOperationResult {
    pub id: Option<String>,
    pub name: Option<String>,
    pub action: String,
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageSummary {
    pub id: Option<String>,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub created: Option<i64>,
    pub size: Option<i64>,
    pub labels: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageDetail {
    pub id: Option<String>,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub created: Option<String>,
    pub size: Option<i64>,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullImageRequest {
    pub reference: String,
    pub tag: Option<String>,
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoveImageRequest {
    pub reference: String,
    pub force: bool,
    pub noprune: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageOperationResult {
    pub reference: String,
    pub action: String,
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkSummary {
    pub id: Option<String>,
    pub name: Option<String>,
    pub driver: Option<String>,
    pub scope: Option<String>,
    pub internal: Option<bool>,
    pub attachable: Option<bool>,
    pub ingress: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateNetworkRequest {
    pub name: String,
    pub driver: String,
    pub internal: bool,
    pub attachable: bool,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkContainerRequest {
    pub network: String,
    pub container: String,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkOperationResult {
    pub id: Option<String>,
    pub name: Option<String>,
    pub action: String,
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VolumeSummary {
    pub name: String,
    pub driver: Option<String>,
    pub mountpoint: Option<String>,
    pub labels: Value,
    pub usage_size: Option<i64>,
    pub usage_ref_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VolumeDetail {
    pub name: String,
    pub driver: Option<String>,
    pub mountpoint: Option<String>,
    pub labels: Value,
    pub options: Value,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateVolumeRequest {
    pub name: String,
    pub driver: String,
    pub driver_opts: HashMap<String, String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoveVolumeRequest {
    pub name: String,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VolumeOperationResult {
    pub name: String,
    pub action: String,
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "resource", content = "command", rename_all = "snake_case")]
pub enum DockerCommand {
    Image(DockerImageCommand),
    Container(DockerContainerCommand),
    Network(DockerNetworkCommand),
    Volume(DockerVolumeCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DockerImageCommand {
    List,
    Inspect { reference: String },
    Pull(PullImageRequest),
    Remove(RemoveImageRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DockerContainerCommand {
    List { filter: ContainerListFilter },
    Inspect { id_or_name: String },
    Create(CreateContainerRequest),
    Start { id_or_name: String },
    Stop(StopContainerRequest),
    Restart(RestartContainerRequest),
    Remove(RemoveContainerRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DockerNetworkCommand {
    List,
    Create(CreateNetworkRequest),
    Remove { name_or_id: String },
    Connect(NetworkContainerRequest),
    Disconnect(NetworkContainerRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DockerVolumeCommand {
    List,
    Inspect { name: String },
    Create(CreateVolumeRequest),
    Remove(RemoveVolumeRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DockerCommandEnvelope {
    pub command_id: Uuid,
    pub task_id: Option<Uuid>,
    pub step_id: Option<Uuid>,
    pub command: DockerCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DockerCommandStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DockerCommandResult {
    pub command_id: Uuid,
    pub status: DockerCommandStatus,
    pub message: String,
    pub details: Value,
}
