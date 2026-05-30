use crate::VmNetworkMode;
use crate::VmNetworkSpec;
use crate::VmPortForward;
use crate::VmProviderError;
use crate::VmSpec;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct QemuPaths {
    pub binary: PathBuf,
    pub qmp_socket: PathBuf,
    pub qga_socket: PathBuf,
    pub serial_log: PathBuf,
}

pub fn build_qemu_argv(
    spec: &VmSpec,
    paths: &QemuPaths,
    vnc_display: u16,
) -> Result<Vec<String>, VmProviderError> {
    if spec.cpu_cores == 0 {
        return Err(VmProviderError::InvalidRequest(
            "cpu_cores must be greater than zero".to_string(),
        ));
    }
    if spec.memory_mib < 128 {
        return Err(VmProviderError::InvalidRequest(
            "memory_mib must be at least 128".to_string(),
        ));
    }

    let mut args = vec![
        "-name".to_string(),
        spec.name.clone(),
        "-machine".to_string(),
        "accel=tcg".to_string(),
        "-smp".to_string(),
        spec.cpu_cores.to_string(),
        "-m".to_string(),
        spec.memory_mib.to_string(),
        "-qmp".to_string(),
        format!("unix:{},server=on,wait=off", paths.qmp_socket.display()),
        "-chardev".to_string(),
        format!(
            "socket,id=qga0,path={},server=on,wait=off",
            paths.qga_socket.display()
        ),
        "-device".to_string(),
        "virtio-serial".to_string(),
        "-device".to_string(),
        "virtserialport,chardev=qga0,name=org.qemu.guest_agent.0".to_string(),
        "-serial".to_string(),
        format!("file:{}", paths.serial_log.display()),
        "-vnc".to_string(),
        format!("127.0.0.1:{vnc_display}"),
    ];

    for disk in &spec.disks {
        args.push("-drive".to_string());
        args.push(format!(
            "file={},if=virtio,format={}",
            disk.path.display(),
            disk.format
        ));
    }

    for (index, network) in spec.networks.iter().enumerate() {
        append_network_args(&mut args, index, network)?;
    }

    Ok(args)
}

fn append_network_args(
    args: &mut Vec<String>,
    index: usize,
    network: &VmNetworkSpec,
) -> Result<(), VmProviderError> {
    match network.mode {
        VmNetworkMode::UserNat => {
            args.push("-netdev".to_string());
            args.push(format!(
                "user,id=net{index}{}",
                port_forwards_arg(&network.port_forwards)
            ));
            args.push("-device".to_string());
            args.push(format!(
                "virtio-net-pci,netdev=net{index}{}",
                network
                    .mac_address
                    .as_ref()
                    .map(|mac| format!(",mac={mac}"))
                    .unwrap_or_default()
            ));
        }
        VmNetworkMode::BridgeTap => {
            let bridge = network.bridge.as_deref().ok_or_else(|| {
                VmProviderError::InvalidRequest("bridge network requires bridge".to_string())
            })?;
            args.push("-netdev".to_string());
            args.push(format!("bridge,id=net{index},br={bridge}"));
            args.push("-device".to_string());
            args.push(format!("virtio-net-pci,netdev=net{index}"));
        }
    }
    Ok(())
}

fn port_forwards_arg(ports: &[VmPortForward]) -> String {
    ports
        .iter()
        .map(|port| {
            format!(
                ",hostfwd={}::{}-:{}",
                port.protocol, port.host_port, port.guest_port
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

pub fn qemu_binary(binary_dir: Option<&Path>) -> PathBuf {
    let binary = if cfg!(target_arch = "aarch64") {
        "qemu-system-aarch64"
    } else {
        "qemu-system-x86_64"
    };
    binary_dir
        .map(|dir| dir.join(binary))
        .unwrap_or_else(|| PathBuf::from(binary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VmDiskSpec;
    use crate::VmId;
    use crate::VmImageRef;
    use crate::VmNetworkMode;
    use crate::VmNetworkSpec;
    use serde_json::json;

    #[test]
    fn qemu_argv_includes_qmp_disk_nat_and_vnc() {
        let spec = VmSpec {
            id: VmId::new("web"),
            name: "web".to_string(),
            image: VmImageRef {
                id: "ubuntu".to_string(),
                name: "Ubuntu".to_string(),
                path: "/images/ubuntu.qcow2".into(),
                os_family: Some("linux".to_string()),
                architecture: "x86_64".to_string(),
            },
            cpu_cores: 2,
            memory_mib: 2048,
            disks: vec![VmDiskSpec {
                path: "/vms/web/disk.qcow2".into(),
                size_gb: 20,
                format: "qcow2".to_string(),
                boot: true,
            }],
            networks: vec![VmNetworkSpec {
                mode: VmNetworkMode::UserNat,
                bridge: None,
                mac_address: None,
                port_forwards: vec![crate::VmPortForward {
                    host_port: 2222,
                    guest_port: 22,
                    protocol: "tcp".to_string(),
                }],
            }],
            cloud_init: json!({}),
            metadata: json!({}),
        };
        let args = match build_qemu_argv(
            &spec,
            &QemuPaths {
                binary: "qemu-system-x86_64".into(),
                qmp_socket: "/tmp/qmp.sock".into(),
                qga_socket: "/tmp/qga.sock".into(),
                serial_log: "/tmp/serial.log".into(),
            },
            1,
        ) {
            Ok(args) => args,
            Err(error) => panic!("valid args: {error}"),
        };
        let joined = args.join(" ");

        assert!(joined.contains("unix:/tmp/qmp.sock"));
        assert!(joined.contains("file=/vms/web/disk.qcow2"));
        assert!(joined.contains("hostfwd=tcp::2222-:22"));
        assert!(joined.contains("127.0.0.1:1"));
    }
}
