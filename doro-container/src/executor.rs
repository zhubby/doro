use super::ContainerCommand;
use super::ContainerCommandResult;
use super::ContainerCommandStatus;
use super::ContainerImageCommand;
use super::ContainerNetworkCommand;
use super::ContainerProviderError;
use super::ContainerRuntimeCommand;
use super::ContainerRuntimeCommandEnvelope;
use super::ContainerVolumeCommand;
use super::DockerProvider;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ContainerRuntimeExecutor {
    client: DockerProvider,
}

impl ContainerRuntimeExecutor {
    pub fn new(client: DockerProvider) -> Self {
        Self { client }
    }

    pub async fn execute(
        &self,
        envelope: ContainerRuntimeCommandEnvelope,
    ) -> ContainerCommandResult {
        let command_id = envelope.command_id;
        match self.execute_inner(envelope.command).await {
            Ok(details) => ContainerCommandResult {
                command_id,
                status: ContainerCommandStatus::Succeeded,
                message: "docker command succeeded".to_string(),
                details,
            },
            Err(error) => ContainerCommandResult {
                command_id,
                status: ContainerCommandStatus::Failed,
                message: error.to_string(),
                details: json!({}),
            },
        }
    }

    async fn execute_inner(
        &self,
        command: ContainerRuntimeCommand,
    ) -> Result<serde_json::Value, ContainerProviderError> {
        match command {
            ContainerRuntimeCommand::Image(command) => self.execute_image(command).await,
            ContainerRuntimeCommand::Container(command) => self.execute_container(command).await,
            ContainerRuntimeCommand::Network(command) => self.execute_network(command).await,
            ContainerRuntimeCommand::Volume(command) => self.execute_volume(command).await,
        }
    }

    async fn execute_image(
        &self,
        command: ContainerImageCommand,
    ) -> Result<serde_json::Value, ContainerProviderError> {
        match command {
            ContainerImageCommand::List => Ok(json!(self.client.images().await?)),
            ContainerImageCommand::Inspect { reference } => {
                Ok(json!(self.client.inspect_image(&reference).await?))
            }
            ContainerImageCommand::Pull(request) => {
                Ok(json!(self.client.pull_image(request).await?))
            }
            ContainerImageCommand::Remove(request) => {
                Ok(json!(self.client.remove_image(request).await?))
            }
        }
    }

    async fn execute_container(
        &self,
        command: ContainerCommand,
    ) -> Result<serde_json::Value, ContainerProviderError> {
        match command {
            ContainerCommand::List { filter } => Ok(json!(self.client.containers(filter).await?)),
            ContainerCommand::Inspect { id_or_name } => {
                Ok(json!(self.client.inspect_container(&id_or_name).await?))
            }
            ContainerCommand::Create(request) => {
                Ok(json!(self.client.create_container(request).await?))
            }
            ContainerCommand::Start { id_or_name } => {
                Ok(json!(self.client.start_container(&id_or_name).await?))
            }
            ContainerCommand::Stop(request) => {
                Ok(json!(self.client.stop_container(request).await?))
            }
            ContainerCommand::Restart(request) => {
                Ok(json!(self.client.restart_container(request).await?))
            }
            ContainerCommand::Remove(request) => {
                Ok(json!(self.client.remove_container(request).await?))
            }
        }
    }

    async fn execute_network(
        &self,
        command: ContainerNetworkCommand,
    ) -> Result<serde_json::Value, ContainerProviderError> {
        match command {
            ContainerNetworkCommand::List => Ok(json!(self.client.networks().await?)),
            ContainerNetworkCommand::Create(request) => {
                Ok(json!(self.client.create_network(request).await?))
            }
            ContainerNetworkCommand::Remove { name_or_id } => {
                Ok(json!(self.client.remove_network(&name_or_id).await?))
            }
            ContainerNetworkCommand::Connect(request) => {
                Ok(json!(self.client.connect_network(request).await?))
            }
            ContainerNetworkCommand::Disconnect(request) => {
                Ok(json!(self.client.disconnect_network(request).await?))
            }
        }
    }

    async fn execute_volume(
        &self,
        command: ContainerVolumeCommand,
    ) -> Result<serde_json::Value, ContainerProviderError> {
        match command {
            ContainerVolumeCommand::List => Ok(json!(self.client.volumes().await?)),
            ContainerVolumeCommand::Inspect { name } => {
                Ok(json!(self.client.inspect_volume(&name).await?))
            }
            ContainerVolumeCommand::Create(request) => {
                Ok(json!(self.client.create_volume(request).await?))
            }
            ContainerVolumeCommand::Remove(request) => {
                Ok(json!(self.client.remove_volume(request).await?))
            }
        }
    }
}
