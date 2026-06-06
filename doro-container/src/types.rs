use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerProviderConfig {
    pub socket_path: Option<String>,
    pub config_dir: Option<PathBuf>,
}

impl DockerProviderConfig {
    pub fn new(socket_path: Option<String>) -> Self {
        Self {
            socket_path,
            config_dir: None,
        }
    }

    pub fn with_config_dir(mut self, config_dir: Option<PathBuf>) -> Self {
        self.config_dir = config_dir;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerRuntimeInfo {
    pub id: Option<String>,
    pub server_version: Option<String>,
    pub docker_root_dir: Option<String>,
    pub containers: Option<i64>,
    pub images: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerRuntimeSnapshot {
    pub runtime: String,
    pub daemon: Option<ContainerRuntimeInfo>,
    pub containers: Vec<ContainerSummary>,
    pub networks: Vec<NetworkSummary>,
    pub volumes: Vec<VolumeSummary>,
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
    pub platform: Option<String>,
    pub hostname: Option<String>,
    pub domainname: Option<String>,
    pub user: Option<String>,
    pub working_dir: Option<String>,
    pub entrypoint: Vec<String>,
    pub command: Vec<String>,
    pub env: Vec<String>,
    pub labels: HashMap<String, String>,
    pub network_mode: Option<String>,
    pub network_name: Option<String>,
    pub aliases: Vec<String>,
    pub ipv4_address: Option<String>,
    pub mac_address: Option<String>,
    pub ports: Vec<ContainerPortBinding>,
    pub dns: Vec<String>,
    pub dns_search: Vec<String>,
    pub extra_hosts: Vec<String>,
    pub binds: Vec<String>,
    pub volumes: Vec<String>,
    pub tmpfs: Vec<String>,
    pub shm_size: Option<String>,
    pub restart_policy: Option<ContainerRestartPolicyName>,
    pub restart_max_retries: Option<i64>,
    pub auto_remove: bool,
    pub privileged: bool,
    pub init: bool,
    pub tty: bool,
    pub open_stdin: bool,
    pub read_only_rootfs: bool,
    pub cap_add: Vec<String>,
    pub cap_drop: Vec<String>,
    pub devices: Vec<ContainerDevice>,
    pub memory: Option<String>,
    pub memory_swap: Option<String>,
    pub cpus: Option<String>,
    pub cpu_shares: Option<i64>,
    pub cpuset_cpus: Option<String>,
    pub pids_limit: Option<i64>,
    pub healthcheck: Option<ContainerHealthcheck>,
    pub log_driver: Option<String>,
    pub log_options: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerRestartPolicyName {
    No,
    Always,
    UnlessStopped,
    OnFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerPortBinding {
    pub container_port: String,
    pub protocol: Option<String>,
    pub host_ip: Option<String>,
    pub host_port: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerDevice {
    pub host_path: String,
    pub container_path: Option<String>,
    pub permissions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerHealthcheck {
    pub disabled: bool,
    pub command: Option<String>,
    pub interval_seconds: Option<i64>,
    pub timeout_seconds: Option<i64>,
    pub retries: Option<i64>,
    pub start_period_seconds: Option<i64>,
    pub start_interval_seconds: Option<i64>,
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
    pub architecture: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryCredentialSummary {
    pub registry: String,
    pub username: Option<String>,
    pub source: String,
    pub has_secret: bool,
    pub config_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpsertRegistryCredentialRequest {
    pub registry: String,
    pub username: String,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoveRegistryCredentialRequest {
    pub registry: String,
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
pub enum ContainerRuntimeCommand {
    Image(ContainerImageCommand),
    Container(ContainerCommand),
    Network(ContainerNetworkCommand),
    Volume(ContainerVolumeCommand),
    Compose(ContainerComposeCommand),
    Registry(ContainerRegistryCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ContainerImageCommand {
    List,
    Inspect { reference: String },
    Pull(PullImageRequest),
    Remove(RemoveImageRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ContainerRegistryCommand {
    List,
    Upsert(UpsertRegistryCredentialRequest),
    Remove(RemoveRegistryCredentialRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ContainerCommand {
    List { filter: ContainerListFilter },
    Inspect { id_or_name: String },
    Create(Box<CreateContainerRequest>),
    Start { id_or_name: String },
    Stop(StopContainerRequest),
    Restart(RestartContainerRequest),
    Remove(RemoveContainerRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ContainerNetworkCommand {
    List,
    Create(CreateNetworkRequest),
    Remove { name_or_id: String },
    Connect(NetworkContainerRequest),
    Disconnect(NetworkContainerRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ContainerVolumeCommand {
    List,
    Inspect { name: String },
    Create(CreateVolumeRequest),
    Remove(RemoveVolumeRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ContainerComposeCommand {
    List,
    Read {
        project: String,
    },
    CreateOrUpdate {
        project: String,
        compose_yaml: String,
        env_file: Option<String>,
    },
    Up {
        project: String,
    },
    Down {
        project: String,
    },
    Restart {
        project: String,
    },
    Pull {
        project: String,
    },
    Delete {
        project: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerRuntimeCommandEnvelope {
    pub command_id: Uuid,
    pub task_id: Option<Uuid>,
    pub step_id: Option<Uuid>,
    pub command: ContainerRuntimeCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContainerCommandStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerCommandResult {
    pub command_id: Uuid,
    pub status: ContainerCommandStatus,
    pub message: String,
    pub details: Value,
}
