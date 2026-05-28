mod client;
mod containers;
mod executor;
mod images;
mod networks;
mod types;
mod volumes;

use bollard::errors::Error as BollardError;
use thiserror::Error;

pub use client::DockerClient;
pub use executor::DockerExecutor;
pub use types::*;

#[derive(Debug, Error)]
pub enum DockerError {
    #[error("failed to connect to Docker socket")]
    Connect(#[from] BollardError),
    #[error("invalid Docker request: {0}")]
    InvalidRequest(String),
}

#[derive(Debug, Clone)]
pub struct DockerManager {
    client: DockerClient,
}

impl DockerManager {
    pub fn connect(config: DockerConfig) -> Result<Self, DockerError> {
        Ok(Self {
            client: DockerClient::connect(&config)?,
        })
    }

    pub fn client(&self) -> &DockerClient {
        &self.client
    }

    pub fn executor(&self) -> DockerExecutor {
        DockerExecutor::new(self.client.clone())
    }
}

pub async fn collect_snapshot(socket_path: Option<&str>) -> Result<serde_json::Value, DockerError> {
    let manager = DockerManager::connect(DockerConfig::new(socket_path.map(str::to_string)))?;
    let client = manager.client();
    let daemon = client.health().await.ok();
    let containers = client.containers(ContainerListFilter { all: true }).await?;
    let networks = client.networks().await.unwrap_or_default();
    let volumes = client.volumes().await.unwrap_or_default();

    Ok(serde_json::json!({
        "runtime": "docker",
        "daemon": daemon,
        "containers": containers,
        "networks": networks,
        "volumes": volumes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn docker_config_preserves_explicit_socket_path() {
        let config = DockerConfig::new(Some("/tmp/docker.sock".to_string()));

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

        let envelope: DockerCommandEnvelope = match serde_json::from_value(payload) {
            Ok(envelope) => envelope,
            Err(error) => panic!("valid payload: {error}"),
        };

        assert_eq!(envelope.command_id, command_id);
        assert!(matches!(
            envelope.command,
            DockerCommand::Container(DockerContainerCommand::Start { id_or_name }) if id_or_name == "web"
        ));
    }
}
