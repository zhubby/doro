use super::QemuPaths;
use super::build_qemu_argv;
use super::qemu_binary;
use super::qemu_img_binary;
use crate::VirtualMachineConsoleProvider;
use crate::VirtualMachineImageStore;
use crate::VirtualMachineInventory;
use crate::VirtualMachineLifecycle;
use crate::VirtualMachineSnapshotStore;
use crate::VmCommandResult;
use crate::VmCommandStatus;
use crate::VmConsoleEndpoint;
use crate::VmDeleteMode;
use crate::VmDiskSpec;
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
use std::fs;
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
        build_qemu_argv(
            spec,
            &paths,
            &self.config.vnc_bind_host,
            self.vnc_display(&spec.id),
        )
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
        details: serde_json::Value,
    ) -> VmCommandResult {
        VmCommandResult {
            command_id: Uuid::new_v4(),
            vm_id,
            status,
            message: message.into(),
            details,
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
        let mut states = self.states.list()?;
        for state in &mut states {
            if let Some(pid) = state.pid
                && !process_is_running(pid)
            {
                state.pid = None;
                state.status = VmStatus::Stopped;
                state.observed_at = Utc::now();
                self.states.save(state)?;
            }
        }
        Ok(states)
    }
}

#[async_trait]
impl VirtualMachineLifecycle for QemuProvider {
    async fn create(&self, mut spec: VmSpec) -> Result<VmRuntimeState, VmProviderError> {
        for network in &spec.networks {
            self.config.network_policy.validate(network)?;
        }
        let paths = self.paths(&spec.id)?;
        let vm_dir = self.states.vm_dir(&spec.id)?;
        let disk_path = self.states.disk_path(&spec.id)?;
        if disk_path.exists() || self.states.state_path(&spec.id)?.exists() {
            return Err(VmProviderError::InvalidRequest(format!(
                "virtual machine {} already exists",
                spec.id
            )));
        }
        if !spec.image.path.is_file() {
            return Err(VmProviderError::InvalidRequest(format!(
                "base image {} is not a file",
                spec.image.path.display()
            )));
        }
        fs::create_dir_all(&vm_dir)?;
        fs::copy(&spec.image.path, &disk_path)?;
        let disk_size_gb = spec
            .disks
            .iter()
            .find(|disk| disk.boot)
            .or_else(|| spec.disks.first())
            .map(|disk| disk.size_gb)
            .unwrap_or_default();
        spec.disks = vec![VmDiskSpec {
            path: disk_path.clone(),
            size_gb: disk_size_gb,
            format: "qcow2".to_string(),
            boot: true,
        }];
        let vnc_display = self.vnc_display(&spec.id);
        let args = build_qemu_argv(&spec, &paths, &self.config.vnc_bind_host, vnc_display)?;
        let mut metadata = spec.metadata;
        metadata["image"] = json!(spec.image.name);
        metadata["base_image"] = json!({
            "id": spec.image.id,
            "name": spec.image.name,
            "path": spec.image.path,
        });
        metadata["managed_disk"] = json!(disk_path);
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
                5900 + vnc_display,
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
                json!({ "id": id }),
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
            serde_json::to_value(state)?,
        ))
    }

    async fn stop(&self, id: &VmId, mode: VmStopMode) -> Result<VmCommandResult, VmProviderError> {
        let mut state = self.states.load(id)?;
        if let Some(pid) = state.pid {
            let signal = match mode {
                VmStopMode::Graceful => "-TERM",
                VmStopMode::Force => "-KILL",
            };
            let _ = Command::new("kill")
                .arg(signal)
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
            serde_json::to_value(state)?,
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
            json!({ "id": id }),
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
    async fn snapshots(&self, id: &VmId) -> Result<Vec<VmSnapshot>, VmProviderError> {
        let _ = self.states.load(id)?;
        self.states.snapshots(id)
    }

    async fn snapshot(
        &self,
        id: &VmId,
        request: VmSnapshotRequest,
    ) -> Result<VmSnapshot, VmProviderError> {
        let state = self.states.load(id)?;
        if state.status == VmStatus::Running {
            return Err(VmProviderError::InvalidRequest(
                "running virtual machine snapshots are not supported in MVP".to_string(),
            ));
        }
        let snapshot_ref = snapshot_ref(&request.name);
        let disk_path = self.states.disk_path(id)?;
        if !disk_path.is_file() {
            return Err(VmProviderError::InvalidRequest(format!(
                "managed disk {} is not a file",
                disk_path.display()
            )));
        }
        let output = Command::new(qemu_img_binary(self.config.binary_dir.as_deref()))
            .args(["snapshot", "-c", &snapshot_ref])
            .arg(&disk_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(VmProviderError::CommandFailed(if message.is_empty() {
                "qemu-img snapshot failed".to_string()
            } else {
                message
            }));
        }
        let snapshot = VmSnapshot {
            id: Uuid::new_v4(),
            vm_id: id.clone(),
            snapshot_ref,
            name: request.name,
            description: request.description,
            created_at: Utc::now(),
        };
        self.states.save_snapshot(&snapshot)?;
        Ok(snapshot)
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

fn process_is_running(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn snapshot_ref(name: &str) -> String {
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let prefix = if slug.is_empty() { "snapshot" } else { &slug };
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::VmNetworkMode;
    use crate::VmNetworkSpec;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn create_copies_base_image_to_managed_disk_and_uses_vnc_config() {
        let temp = tempdir().unwrap_or_else(|error| panic!("tempdir should create: {error}"));
        let image_dir = temp.path().join("images");
        let state_dir = temp.path().join("vms");
        fs::create_dir_all(&image_dir)
            .unwrap_or_else(|error| panic!("image dir should create: {error}"));
        let base_image = image_dir.join("ubuntu.qcow2");
        fs::write(&base_image, b"base-image")
            .unwrap_or_else(|error| panic!("base image should write: {error}"));
        let provider = QemuProvider::new(QemuProviderConfig {
            state_dir: state_dir.clone(),
            image_dir,
            vnc_bind_host: "0.0.0.0".to_string(),
            vnc_display_base: 20,
            ..QemuProviderConfig::default()
        });
        let spec = test_spec(base_image.clone());

        let state = tokio::runtime::Runtime::new()
            .unwrap_or_else(|error| panic!("runtime should start: {error}"))
            .block_on(provider.create(spec.clone()))
            .unwrap_or_else(|error| panic!("create should succeed: {error}"));

        let managed_disk = state_dir.join("vm-test").join("disk.qcow2");
        let copied =
            fs::read(&managed_disk).unwrap_or_else(|error| panic!("disk should exist: {error}"));
        assert_eq!(copied, b"base-image");
        assert_eq!(
            state
                .metadata
                .get("image")
                .and_then(serde_json::Value::as_str),
            Some("Ubuntu")
        );
        let Some(console) = state.console else {
            panic!("console endpoint should exist");
        };
        assert_eq!(console.host, "0.0.0.0");
        assert_eq!(console.port, 5900 + provider.vnc_display(&spec.id));
        assert!(base_image.exists());
    }

    #[test]
    fn snapshot_rejects_running_state_before_qemu_img() {
        let temp = tempdir().unwrap_or_else(|error| panic!("tempdir should create: {error}"));
        let image_dir = temp.path().join("images");
        let state_dir = temp.path().join("vms");
        fs::create_dir_all(&image_dir)
            .unwrap_or_else(|error| panic!("image dir should create: {error}"));
        let base_image = image_dir.join("ubuntu.qcow2");
        fs::write(&base_image, b"base-image")
            .unwrap_or_else(|error| panic!("base image should write: {error}"));
        let provider = QemuProvider::new(QemuProviderConfig {
            state_dir,
            image_dir,
            ..QemuProviderConfig::default()
        });
        let runtime = tokio::runtime::Runtime::new()
            .unwrap_or_else(|error| panic!("runtime should start: {error}"));
        let spec = test_spec(base_image);
        let mut state = runtime
            .block_on(provider.create(spec.clone()))
            .unwrap_or_else(|error| panic!("create should succeed: {error}"));
        state.status = VmStatus::Running;
        provider
            .states
            .save(&state)
            .unwrap_or_else(|error| panic!("state should save: {error}"));

        let result = runtime.block_on(provider.snapshot(
            &spec.id,
            VmSnapshotRequest {
                name: "snap".to_string(),
                description: None,
            },
        ));

        assert!(matches!(result, Err(VmProviderError::InvalidRequest(_))));
    }

    #[test]
    fn snapshot_invokes_qemu_img_and_records_metadata() {
        let temp = tempdir().unwrap_or_else(|error| panic!("tempdir should create: {error}"));
        let image_dir = temp.path().join("images");
        let state_dir = temp.path().join("vms");
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&image_dir)
            .unwrap_or_else(|error| panic!("image dir should create: {error}"));
        fs::create_dir_all(&bin_dir)
            .unwrap_or_else(|error| panic!("bin dir should create: {error}"));
        let qemu_img = bin_dir.join("qemu-img");
        let log_path = temp.path().join("qemu-img.log");
        let mut script =
            fs::File::create(&qemu_img).unwrap_or_else(|error| panic!("script create: {error}"));
        writeln!(
            script,
            "#!/bin/sh\necho \"$@\" > \"{}\"\nexit 0",
            log_path.display()
        )
        .unwrap_or_else(|error| panic!("script should write: {error}"));
        fs::set_permissions(&qemu_img, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("script chmod: {error}"));
        let base_image = image_dir.join("ubuntu.qcow2");
        fs::write(&base_image, b"base-image")
            .unwrap_or_else(|error| panic!("base image should write: {error}"));
        let provider = QemuProvider::new(QemuProviderConfig {
            binary_dir: Some(bin_dir),
            state_dir,
            image_dir,
            ..QemuProviderConfig::default()
        });
        let runtime = tokio::runtime::Runtime::new()
            .unwrap_or_else(|error| panic!("runtime should start: {error}"));
        let spec = test_spec(base_image);
        runtime
            .block_on(provider.create(spec.clone()))
            .unwrap_or_else(|error| panic!("create should succeed: {error}"));

        let snapshot = runtime
            .block_on(provider.snapshot(
                &spec.id,
                VmSnapshotRequest {
                    name: "Release 1".to_string(),
                    description: Some("before update".to_string()),
                },
            ))
            .unwrap_or_else(|error| panic!("snapshot should succeed: {error}"));

        assert!(snapshot.snapshot_ref.starts_with("release-1-"));
        let args = fs::read_to_string(&log_path)
            .unwrap_or_else(|error| panic!("qemu-img log should exist: {error}"));
        assert!(args.contains("snapshot -c release-1-"));
        assert!(args.contains("disk.qcow2"));
        let snapshots = provider
            .states
            .snapshots(&spec.id)
            .unwrap_or_else(|error| panic!("snapshots should load: {error}"));
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].name, "Release 1");
    }

    fn test_spec(image: PathBuf) -> VmSpec {
        VmSpec {
            id: VmId::new("vm-test"),
            name: "vm-test".to_string(),
            image: VmImageRef {
                id: "ubuntu".to_string(),
                name: "Ubuntu".to_string(),
                path: image,
                os_family: Some("linux".to_string()),
                architecture: "x86_64".to_string(),
            },
            cpu_cores: 2,
            memory_mib: 2048,
            disks: vec![VmDiskSpec {
                path: "disk.qcow2".into(),
                size_gb: 20,
                format: "qcow2".to_string(),
                boot: true,
            }],
            networks: vec![VmNetworkSpec {
                mode: VmNetworkMode::UserNat,
                bridge: None,
                mac_address: None,
                port_forwards: Vec::new(),
            }],
            cloud_init: json!({}),
            metadata: json!({}),
        }
    }
}
