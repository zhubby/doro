mod client;
mod containers;
mod executor;
mod images;
mod networks;
mod traits;
mod types;
mod volumes;

use bollard::errors::Error as BollardError;
use thiserror::Error;

pub use client::DockerProvider;
pub use executor::ContainerRuntimeExecutor;
pub use traits::*;
pub use types::*;

#[derive(Debug, Error)]
pub enum ContainerProviderError {
    #[error("failed to connect to Docker socket")]
    Connect(#[from] BollardError),
    #[error("invalid container runtime request: {0}")]
    InvalidRequest(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn docker_config_preserves_explicit_socket_path() {
        let config = DockerProviderConfig::new(Some("/tmp/docker.sock".to_string()));

        assert_eq!(config.socket_path.as_deref(), Some("/tmp/docker.sock"));
    }

    #[test]
    fn remove_requests_default_to_non_force() {
        let container = RemoveContainerRequest {
            id_or_name: "web".to_string(),
            force: false,
            remove_volumes: false,
        };
        let image = RemoveImageRequest {
            reference: "nginx:latest".to_string(),
            force: false,
            noprune: false,
        };
        let volume = RemoveVolumeRequest {
            name: "data".to_string(),
            force: false,
        };

        assert!(!container.force);
        assert!(!image.force);
        assert!(!volume.force);
    }

    #[tokio::test]
    async fn empty_identifiers_fail_validation_before_provider_calls() {
        let provider = match DockerProvider::connect(&DockerProviderConfig::new(None)) {
            Ok(provider) => provider,
            Err(error) => panic!("provider should initialize without socket I/O: {error}"),
        };

        let container = provider.inspect_container("").await;
        let image = provider.inspect_image("").await;
        let network = provider.remove_network("").await;
        let volume = provider.inspect_volume("").await;

        assert!(matches!(
            container,
            Err(ContainerProviderError::InvalidRequest(_))
        ));
        assert!(matches!(
            image,
            Err(ContainerProviderError::InvalidRequest(_))
        ));
        assert!(matches!(
            network,
            Err(ContainerProviderError::InvalidRequest(_))
        ));
        assert!(matches!(
            volume,
            Err(ContainerProviderError::InvalidRequest(_))
        ));
    }

    #[test]
    fn command_envelope_deserializes_container_start_payload() {
        let command_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "command_id": command_id,
            "task_id": null,
            "step_id": null,
            "command": {
                "resource": "container",
                "command": {
                    "action": "start",
                    "id_or_name": "web"
                }
            }
        });

        let envelope: ContainerRuntimeCommandEnvelope = match serde_json::from_value(payload) {
            Ok(envelope) => envelope,
            Err(error) => panic!("valid payload: {error}"),
        };

        assert_eq!(envelope.command_id, command_id);
        assert!(matches!(
            envelope.command,
            ContainerRuntimeCommand::Container(ContainerCommand::Start { id_or_name }) if id_or_name == "web"
        ));
    }

    #[test]
    fn runtime_snapshot_serializes_current_payload_shape() {
        let snapshot = ContainerRuntimeSnapshot {
            runtime: "docker".to_string(),
            daemon: Some(ContainerRuntimeInfo {
                id: Some("daemon-id".to_string()),
                server_version: Some("27.0.0".to_string()),
                docker_root_dir: Some("/var/lib/docker".to_string()),
                containers: Some(1),
                images: Some(2),
            }),
            containers: vec![ContainerSummary {
                id: Some("abc123".to_string()),
                names: vec!["/web".to_string()],
                image: Some("nginx:latest".to_string()),
                image_id: Some("sha256:image".to_string()),
                command: Some("nginx".to_string()),
                created: Some(1_700_000_000),
                ports: json!([]),
                labels: json!({}),
                state: Some("running".to_string()),
                status: Some("Up 1 minute".to_string()),
            }],
            networks: Vec::new(),
            volumes: Vec::new(),
        };

        let payload = match serde_json::to_value(snapshot) {
            Ok(payload) => payload,
            Err(error) => panic!("snapshot should serialize: {error}"),
        };

        assert_eq!(payload["runtime"], "docker");
        assert_eq!(payload["daemon"]["docker_root_dir"], "/var/lib/docker");
        assert_eq!(payload["containers"][0]["id"], "abc123");
        assert!(payload["networks"].is_array());
        assert!(payload["volumes"].is_array());
    }
}
