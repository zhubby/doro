use super::DockerClient;
use super::DockerCommand;
use super::DockerCommandEnvelope;
use super::DockerCommandResult;
use super::DockerCommandStatus;
use super::DockerContainerCommand;
use super::DockerError;
use super::DockerImageCommand;
use super::DockerNetworkCommand;
use super::DockerVolumeCommand;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct DockerExecutor {
    client: DockerClient,
}

impl DockerExecutor {
    pub fn new(client: DockerClient) -> Self {
        Self { client }
    }

    pub async fn execute(&self, envelope: DockerCommandEnvelope) -> DockerCommandResult {
        let command_id = envelope.command_id;
        match self.execute_inner(envelope.command).await {
            Ok(details) => DockerCommandResult {
                command_id,
                status: DockerCommandStatus::Succeeded,
                message: "docker command succeeded".to_string(),
                details,
            },
            Err(error) => DockerCommandResult {
                command_id,
                status: DockerCommandStatus::Failed,
                message: error.to_string(),
                details: json!({}),
            },
        }
    }

    async fn execute_inner(
        &self,
        command: DockerCommand,
    ) -> Result<serde_json::Value, DockerError> {
        match command {
            DockerCommand::Image(command) => self.execute_image(command).await,
            DockerCommand::Container(command) => self.execute_container(command).await,
            DockerCommand::Network(command) => self.execute_network(command).await,
            DockerCommand::Volume(command) => self.execute_volume(command).await,
        }
    }

    async fn execute_image(
        &self,
        command: DockerImageCommand,
    ) -> Result<serde_json::Value, DockerError> {
        match command {
            DockerImageCommand::List => Ok(json!(self.client.images().await?)),
            DockerImageCommand::Inspect { reference } => {
                Ok(json!(self.client.inspect_image(&reference).await?))
            }
            DockerImageCommand::Pull(request) => Ok(json!(self.client.pull_image(request).await?)),
            DockerImageCommand::Remove(request) => {
                Ok(json!(self.client.remove_image(request).await?))
            }
        }
    }

    async fn execute_container(
        &self,
        command: DockerContainerCommand,
    ) -> Result<serde_json::Value, DockerError> {
        match command {
            DockerContainerCommand::List { filter } => {
                Ok(json!(self.client.containers(filter).await?))
            }
            DockerContainerCommand::Inspect { id_or_name } => {
                Ok(json!(self.client.inspect_container(&id_or_name).await?))
            }
            DockerContainerCommand::Create(request) => {
                Ok(json!(self.client.create_container(request).await?))
            }
            DockerContainerCommand::Start { id_or_name } => {
                Ok(json!(self.client.start_container(&id_or_name).await?))
            }
            DockerContainerCommand::Stop(request) => {
                Ok(json!(self.client.stop_container(request).await?))
            }
            DockerContainerCommand::Restart(request) => {
                Ok(json!(self.client.restart_container(request).await?))
            }
            DockerContainerCommand::Remove(request) => {
                Ok(json!(self.client.remove_container(request).await?))
            }
        }
    }

    async fn execute_network(
        &self,
        command: DockerNetworkCommand,
    ) -> Result<serde_json::Value, DockerError> {
        match command {
            DockerNetworkCommand::List => Ok(json!(self.client.networks().await?)),
            DockerNetworkCommand::Create(request) => {
                Ok(json!(self.client.create_network(request).await?))
            }
            DockerNetworkCommand::Remove { name_or_id } => {
                Ok(json!(self.client.remove_network(&name_or_id).await?))
            }
            DockerNetworkCommand::Connect(request) => {
                Ok(json!(self.client.connect_network(request).await?))
            }
            DockerNetworkCommand::Disconnect(request) => {
                Ok(json!(self.client.disconnect_network(request).await?))
            }
        }
    }

    async fn execute_volume(
        &self,
        command: DockerVolumeCommand,
    ) -> Result<serde_json::Value, DockerError> {
        match command {
            DockerVolumeCommand::List => Ok(json!(self.client.volumes().await?)),
            DockerVolumeCommand::Inspect { name } => {
                Ok(json!(self.client.inspect_volume(&name).await?))
            }
            DockerVolumeCommand::Create(request) => {
                Ok(json!(self.client.create_volume(request).await?))
            }
            DockerVolumeCommand::Remove(request) => {
                Ok(json!(self.client.remove_volume(request).await?))
            }
        }
    }
}
