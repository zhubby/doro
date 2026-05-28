use super::ContainerDetail;
use super::ContainerListFilter;
use super::ContainerOperationResult;
use super::ContainerSummary;
use super::CreateContainerRequest;
use super::DockerClient;
use super::DockerError;
use super::RemoveContainerRequest;
use super::RestartContainerRequest;
use super::StopContainerRequest;
use bollard::container::Config;
use bollard::container::CreateContainerOptions;
use bollard::container::ListContainersOptions;
use bollard::container::RemoveContainerOptions;
use bollard::container::RestartContainerOptions;
use bollard::container::StopContainerOptions;
use serde_json::json;

impl DockerClient {
    pub async fn containers(
        &self,
        filter: ContainerListFilter,
    ) -> Result<Vec<ContainerSummary>, DockerError> {
        let containers = self
            .docker()
            .list_containers::<String>(Some(ListContainersOptions {
                all: filter.all,
                ..Default::default()
            }))
            .await?;
        Ok(containers
            .into_iter()
            .map(|container| ContainerSummary {
                id: container.id,
                names: container.names.unwrap_or_default(),
                image: container.image,
                image_id: container.image_id,
                command: container.command,
                created: container.created,
                ports: json!(container.ports.unwrap_or_default()),
                labels: json!(container.labels.unwrap_or_default()),
                state: container.state,
                status: container.status,
            })
            .collect())
    }

    pub async fn inspect_container(
        &self,
        id_or_name: &str,
    ) -> Result<ContainerDetail, DockerError> {
        require_identifier(id_or_name, "container id or name")?;
        let container = self.docker().inspect_container(id_or_name, None).await?;
        Ok(ContainerDetail {
            id: container.id,
            name: container.name,
            image: container.image,
            state: json!(container.state),
            config: json!(container.config),
            host_config: json!(container.host_config),
            network_settings: json!(container.network_settings),
        })
    }

    pub async fn create_container(
        &self,
        request: CreateContainerRequest,
    ) -> Result<ContainerOperationResult, DockerError> {
        require_identifier(&request.name, "container name")?;
        require_identifier(&request.image, "container image")?;
        let response = self
            .docker()
            .create_container(
                Some(CreateContainerOptions {
                    name: request.name.clone(),
                    platform: None,
                }),
                Config {
                    image: Some(request.image),
                    cmd: optional_vec(request.command),
                    env: optional_vec(request.env),
                    labels: optional_map(request.labels),
                    ..Default::default()
                },
            )
            .await?;
        Ok(ContainerOperationResult {
            id: Some(response.id),
            name: Some(request.name),
            action: "create".to_string(),
            details: json!({ "warnings": response.warnings }),
        })
    }

    pub async fn start_container(
        &self,
        id_or_name: &str,
    ) -> Result<ContainerOperationResult, DockerError> {
        require_identifier(id_or_name, "container id or name")?;
        self.docker()
            .start_container::<String>(id_or_name, None)
            .await?;
        Ok(simple_result(id_or_name, "start"))
    }

    pub async fn stop_container(
        &self,
        request: StopContainerRequest,
    ) -> Result<ContainerOperationResult, DockerError> {
        require_identifier(&request.id_or_name, "container id or name")?;
        self.docker()
            .stop_container(
                &request.id_or_name,
                Some(StopContainerOptions {
                    t: request.timeout_seconds,
                }),
            )
            .await?;
        Ok(simple_result(&request.id_or_name, "stop"))
    }

    pub async fn restart_container(
        &self,
        request: RestartContainerRequest,
    ) -> Result<ContainerOperationResult, DockerError> {
        require_identifier(&request.id_or_name, "container id or name")?;
        self.docker()
            .restart_container(
                &request.id_or_name,
                Some(RestartContainerOptions {
                    t: request.timeout_seconds as isize,
                }),
            )
            .await?;
        Ok(simple_result(&request.id_or_name, "restart"))
    }

    pub async fn remove_container(
        &self,
        request: RemoveContainerRequest,
    ) -> Result<ContainerOperationResult, DockerError> {
        require_identifier(&request.id_or_name, "container id or name")?;
        self.docker()
            .remove_container(
                &request.id_or_name,
                Some(RemoveContainerOptions {
                    v: request.remove_volumes,
                    force: request.force,
                    link: false,
                }),
            )
            .await?;
        Ok(simple_result(&request.id_or_name, "remove"))
    }
}

fn optional_vec(values: Vec<String>) -> Option<Vec<String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn optional_map(
    values: std::collections::HashMap<String, String>,
) -> Option<std::collections::HashMap<String, String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn simple_result(id_or_name: &str, action: &str) -> ContainerOperationResult {
    ContainerOperationResult {
        id: Some(id_or_name.to_string()),
        name: None,
        action: action.to_string(),
        details: json!({}),
    }
}

fn require_identifier(value: &str, field: &'static str) -> Result<(), DockerError> {
    if value.trim().is_empty() {
        return Err(DockerError::InvalidRequest(format!("{field} is required")));
    }
    Ok(())
}
