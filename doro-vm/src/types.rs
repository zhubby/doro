use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VmId(pub String);

impl VmId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Display for VmId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmStatus {
    Unknown,
    Stopped,
    Starting,
    Running,
    Paused,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmNetworkMode {
    UserNat,
    BridgeTap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmStopMode {
    Graceful,
    Force,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmDeleteMode {
    KeepDisks,
    DeleteDisks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmImageRef {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub os_family: Option<String>,
    pub architecture: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmDiskSpec {
    pub path: PathBuf,
    pub size_gb: u32,
    pub format: String,
    pub boot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmPortForward {
    pub host_port: u16,
    pub guest_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmNetworkSpec {
    pub mode: VmNetworkMode,
    pub bridge: Option<String>,
    pub mac_address: Option<String>,
    pub port_forwards: Vec<VmPortForward>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmConsoleEndpoint {
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub path: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmSpec {
    pub id: VmId,
    pub name: String,
    pub image: VmImageRef,
    pub cpu_cores: u16,
    pub memory_mib: u32,
    pub disks: Vec<VmDiskSpec>,
    pub networks: Vec<VmNetworkSpec>,
    pub cloud_init: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VmRuntimeState {
    pub id: VmId,
    pub name: String,
    pub status: VmStatus,
    pub cpu_cores: u16,
    pub memory_mib: u32,
    pub disk_gb: u32,
    pub networks: Vec<VmNetworkSpec>,
    pub console: Option<VmConsoleEndpoint>,
    pub pid: Option<u32>,
    pub qmp_socket: Option<PathBuf>,
    pub serial_log: Option<PathBuf>,
    pub created_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmSnapshot {
    pub id: Uuid,
    pub vm_id: VmId,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmSnapshotRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmCommandStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VmCommandResult {
    pub command_id: Uuid,
    pub vm_id: Option<VmId>,
    pub status: VmCommandStatus,
    pub message: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum VmCommand {
    Create {
        spec: Box<VmSpec>,
    },
    Start {
        id: VmId,
    },
    Stop {
        id: VmId,
        mode: VmStopMode,
    },
    Restart {
        id: VmId,
    },
    Delete {
        id: VmId,
        mode: VmDeleteMode,
    },
    Snapshot {
        id: VmId,
        request: VmSnapshotRequest,
    },
    Console {
        id: VmId,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VmCommandEnvelope {
    pub command_id: Uuid,
    pub task_id: Option<Uuid>,
    pub step_id: Option<Uuid>,
    pub command: VmCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmProviderStatus {
    pub provider: String,
    pub available: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum VmProviderError {
    #[error("virtual machine provider is unavailable: {0}")]
    Unavailable(String),
    #[error("virtual machine not found: {0}")]
    NotFound(VmId),
    #[error("invalid virtual machine request: {0}")]
    InvalidRequest(String),
    #[error("virtual machine provider I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("virtual machine provider serialization failed: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("virtual machine provider command failed: {0}")]
    CommandFailed(String),
}
