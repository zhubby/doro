use super::CreateNetworkRequest;
use super::DockerClient;
use super::DockerError;
use super::NetworkContainerRequest;
use super::NetworkOperationResult;
use super::NetworkSummary;
use bollard::models::EndpointSettings;
use bollard::models::Ipam;
use bollard::network::ConnectNetworkOptions;
use bollard::network::CreateNetworkOptions;
use bollard::network::DisconnectNetworkOptions;
use serde_json::json;
use std::collections::HashMap;

impl DockerClient {
    pub async fn networks(&self) -> Result<Vec<NetworkSummary>, DockerError> {
        let networks = self.docker().list_networks::<String>(None).await?;
        Ok(networks
            .into_iter()
            .map(|network| NetworkSummary {
                id: network.id,
                name: network.name,
                driver: network.driver,
                scope: network.scope,
                internal: network.internal,
                attachable: network.attachable,
                ingress: network.ingress,
            })
            .collect())
    }

    pub async fn create_network(
        &self,
        request: CreateNetworkRequest,
    ) -> Result<NetworkOperationResult, DockerError> {
        require_identifier(&request.name, "network name")?;
        let driver = if request.driver.trim().is_empty() {
            "bridge".to_string()
        } else {
            request.driver.clone()
        };
        let response = self
            .docker()
            .create_network(CreateNetworkOptions {
                name: request.name.clone(),
                check_duplicate: true,
                driver,
                internal: request.internal,
                attachable: request.attachable,
                ingress: false,
                ipam: Ipam::default(),
                enable_ipv6: false,
                options: HashMap::new(),
                labels: request.labels,
            })
            .await?;
        Ok(NetworkOperationResult {
            id: Some(response.id),
            name: Some(request.name),
            action: "create".to_string(),
            details: json!({ "warning": response.warning }),
        })
    }

    pub async fn remove_network(
        &self,
        name_or_id: &str,
    ) -> Result<NetworkOperationResult, DockerError> {
        require_identifier(name_or_id, "network name or id")?;
        self.docker().remove_network(name_or_id).await?;
        Ok(NetworkOperationResult {
            id: Some(name_or_id.to_string()),
            name: None,
            action: "remove".to_string(),
            details: json!({}),
        })
    }

    pub async fn connect_network(
        &self,
        request: NetworkContainerRequest,
    ) -> Result<NetworkOperationResult, DockerError> {
        require_identifier(&request.network, "network name or id")?;
        require_identifier(&request.container, "container id or name")?;
        self.docker()
            .connect_network(
                &request.network,
                ConnectNetworkOptions {
                    container: request.container.clone(),
                    endpoint_config: EndpointSettings::default(),
                },
            )
            .await?;
        Ok(NetworkOperationResult {
            id: Some(request.network),
            name: None,
            action: "connect".to_string(),
            details: json!({ "container": request.container }),
        })
    }

    pub async fn disconnect_network(
        &self,
        request: NetworkContainerRequest,
    ) -> Result<NetworkOperationResult, DockerError> {
        require_identifier(&request.network, "network name or id")?;
        require_identifier(&request.container, "container id or name")?;
        self.docker()
            .disconnect_network(
                &request.network,
                DisconnectNetworkOptions {
                    container: request.container.clone(),
                    force: request.force,
                },
            )
            .await?;
        Ok(NetworkOperationResult {
            id: Some(request.network),
            name: None,
            action: "disconnect".to_string(),
            details: json!({ "container": request.container }),
        })
    }
}

fn require_identifier(value: &str, field: &'static str) -> Result<(), DockerError> {
    if value.trim().is_empty() {
        return Err(DockerError::InvalidRequest(format!("{field} is required")));
    }
    Ok(())
}
