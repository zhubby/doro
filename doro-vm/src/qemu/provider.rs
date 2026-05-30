use super::QemuPaths;
use super::build_qemu_argv;
use super::qemu_binary;
use crate::VirtualMachineConsoleProvider;
use crate::VirtualMachineImageStore;
use crate::VirtualMachineInventory;
use crate::VirtualMachineLifecycle;
use crate::VirtualMachineSnapshotStore;
use crate::VmCommandResult;
use crate::VmCommandStatus;
use crate::VmConsoleEndpoint;
use crate::VmDeleteMode;
use crate::VmId;
use crate::VmImageRef;
use crate::VmProviderError;
use crate::VmProviderStatus;
use crate::VmRuntimeState;
use crate::VmSnapshot;
use crate::VmSnapshotRequest;
use crate::VmSpec;
use crate::VmStatus;
use crate::VmStopMode;
use crate::console::vnc_endpoint;
use crate::images::LocalImageStore;
use crate::network::NetworkPolicy;
use crate::state_store::FileStateStore;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct QemuProviderConfig {
    pub binary_dir: Option<PathBuf>,
    pub state_dir: PathBuf,
    pub image_dir: PathBuf,
    pub network_policy: NetworkPolicy,
    pub vnc_bind_host: String,
    pub vnc_display_base: u16,
}

impl Default for QemuProviderConfig {
    fn default() -> Self {
        Self {
            binary_dir: None,
            state_dir: PathBuf::from(".doro/vms"),
            image_dir: PathBuf::from(".doro/vm-images"),
            network_policy: NetworkPolicy::default(),
            vnc_bind_host: "127.0.0.1".to_string(),
            vnc_display_base: 10,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QemuProvider {
    config: QemuProviderConfig,
    states: FileStateStore,
    images: LocalImageStore,
}

impl QemuProvider {
    pub fn new(config: QemuProviderConfig) -> Self {
        Self {
            states: FileStateStore::new(config.state_dir.clone()),
            images: LocalImageStore::new(config.image_dir.clone()),
            config,
        }
    }

    pub fn command_args(&self, spec: &VmSpec) -> Result<Vec<String>, VmProviderError> {
        let paths = self.paths(&spec.id)?;
        build_qemu_argv(spec, &paths, self.vnc_display(&spec.id))
    }

    fn paths(&self, id: &VmId) -> Result<QemuPaths, VmProviderError> {
        let dir = self.states.vm_dir(id)?;
        Ok(QemuPaths {
            binary: qemu_binary(self.config.binary_dir.as_deref()),
            qmp_socket: dir.join("qmp.sock"),
            qga_socket: dir.join("qga.sock"),
            serial_log: dir.join("serial.log"),
        })
    }

    fn vnc_display(&self, id: &VmId) -> u16 {
        let hash =
            id.0.bytes()
                .fold(0_u16, |acc, byte| acc.wrapping_add(byte as u16));
        self.config.vnc_display_base + (hash % 80)
    }

    fn command_result(
        &self,
        vm_id: Option<VmId>,
        status: VmCommandStatus,
        message: impl Into<String>,
    ) -> VmCommandResult {
        VmCommandResult {
            command_id: Uuid::new_v4(),
            vm_id,
            status,
            message: message.into(),
            details: json!({}),
        }
    }
}

#[async_trait]
impl VirtualMachineInventory for QemuProvider {
    async fn probe(&self) -> Result<VmProviderStatus, VmProviderError> {
        let binary = qemu_binary(self.config.binary_dir.as_deref());
        let output = Command::new(&binary)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| {
                VmProviderError::Unavailable(format!(
                    "{} is not executable: {error}",
                    binary.display()
                ))
            })?;
        let version = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .map(str::to_string);
        Ok(VmProviderStatus {
            provider: "qemu".to_string(),
            available: output.status.success(),
            version,
            message: if output.status.success() {
                "qemu is available".to_string()
            } else {
                String::from_utf8_lossy(&output.stderr).to_string()
            },
        })
    }

    async fn list(&self) -> Result<Vec<VmRuntimeState>, VmProviderError> {
        self.states.list()
    }
}

#[async_trait]
impl VirtualMachineLifecycle for QemuProvider {
    async fn create(&self, spec: VmSpec) -> Result<VmRuntimeState, VmProviderError> {
        for network in &spec.networks {
            self.config.network_policy.validate(network)?;
        }
        let paths = self.paths(&spec.id)?;
        std::fs::create_dir_all(self.states.vm_dir(&spec.id)?)?;
        let args = build_qemu_argv(&spec, &paths, self.vnc_display(&spec.id))?;
        let mut metadata = spec.metadata;
        metadata["image"] = json!(spec.image.name);
        metadata["qemu"] = json!({
            "binary": paths.binary,
            "args": args,
        });
        let disk_gb = spec.disks.iter().map(|disk| disk.size_gb).sum();
        let state = VmRuntimeState {
            id: spec.id,
            name: spec.name,
            status: VmStatus::Stopped,
            cpu_cores: spec.cpu_cores,
            memory_mib: spec.memory_mib,
            disk_gb,
            networks: spec.networks,
            console: Some(vnc_endpoint(
                self.config.vnc_bind_host.clone(),
                5900 + self.vnc_display(&paths_to_id(&paths)),
            )),
            pid: None,
            qmp_socket: Some(paths.qmp_socket),
            serial_log: Some(paths.serial_log),
            created_at: Some(Utc::now()),
            observed_at: Utc::now(),
            metadata,
        };
        self.states.save(&state)?;
        Ok(state)
    }

    async fn start(&self, id: &VmId) -> Result<VmCommandResult, VmProviderError> {
        let mut state = self.states.load(id)?;
        if state.status == VmStatus::Running {
            return Ok(self.command_result(
                Some(id.clone()),
                VmCommandStatus::Succeeded,
                "qemu vm is already running",
            ));
        }
        let (binary, args) = qemu_command_from_metadata(&state)?;
        let child = Command::new(binary)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        state.pid = Some(child.id());
        state.status = VmStatus::Running;
        state.observed_at = Utc::now();
        self.states.save(&state)?;
        Ok(self.command_result(
            Some(id.clone()),
            VmCommandStatus::Succeeded,
            "qemu process started",
        ))
    }

    async fn stop(&self, id: &VmId, _mode: VmStopMode) -> Result<VmCommandResult, VmProviderError> {
        let mut state = self.states.load(id)?;
        if let Some(pid) = state.pid {
            let _ = Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        state.status = VmStatus::Stopped;
        state.pid = None;
        state.observed_at = Utc::now();
        self.states.save(&state)?;
        Ok(self.command_result(
            Some(id.clone()),
            VmCommandStatus::Succeeded,
            "qemu stop requested",
        ))
    }

    async fn restart(&self, id: &VmId) -> Result<VmCommandResult, VmProviderError> {
        let _ = self.stop(id, VmStopMode::Graceful).await?;
        self.start(id).await
    }

    async fn delete(
        &self,
        id: &VmId,
        _mode: VmDeleteMode,
    ) -> Result<VmCommandResult, VmProviderError> {
        self.states.delete(id)?;
        Ok(self.command_result(
            Some(id.clone()),
            VmCommandStatus::Succeeded,
            "qemu vm deleted",
        ))
    }
}

#[async_trait]
impl VirtualMachineImageStore for QemuProvider {
    async fn images(&self) -> Result<Vec<VmImageRef>, VmProviderError> {
        self.images.images()
    }
}

#[async_trait]
impl VirtualMachineSnapshotStore for QemuProvider {
    async fn snapshots(&self, _id: &VmId) -> Result<Vec<VmSnapshot>, VmProviderError> {
        Ok(Vec::new())
    }

    async fn snapshot(
        &self,
        id: &VmId,
        request: VmSnapshotRequest,
    ) -> Result<VmSnapshot, VmProviderError> {
        let _ = self.states.load(id)?;
        Ok(VmSnapshot {
            id: Uuid::new_v4(),
            vm_id: id.clone(),
            name: request.name,
            description: request.description,
            created_at: Utc::now(),
        })
    }
}

#[async_trait]
impl VirtualMachineConsoleProvider for QemuProvider {
    async fn console(&self, id: &VmId) -> Result<VmConsoleEndpoint, VmProviderError> {
        let state = self.states.load(id)?;
        state.console.ok_or_else(|| {
            VmProviderError::InvalidRequest(format!("virtual machine {id} has no console"))
        })
    }
}

fn paths_to_id(paths: &QemuPaths) -> VmId {
    paths
        .qmp_socket
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|value| value.to_str())
        .map(VmId::new)
        .unwrap_or_else(|| VmId::new("vm"))
}

fn qemu_command_from_metadata(
    state: &VmRuntimeState,
) -> Result<(String, Vec<String>), VmProviderError> {
    let qemu = state.metadata.get("qemu").ok_or_else(|| {
        VmProviderError::InvalidRequest(format!(
            "virtual machine {} has no qemu metadata",
            state.id
        ))
    })?;
    let binary = qemu
        .get("binary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            VmProviderError::InvalidRequest(format!(
                "virtual machine {} has no qemu binary",
                state.id
            ))
        })?
        .to_string();
    let args = qemu
        .get("args")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(|| {
            VmProviderError::InvalidRequest(format!(
                "virtual machine {} has no qemu arguments",
                state.id
            ))
        })?;
    Ok((binary, args))
}
