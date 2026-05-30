use super::ContainerDetail;
use super::ContainerListFilter;
use super::ContainerOperationResult;
use super::ContainerProviderError;
use super::ContainerRuntimeInfo;
use super::ContainerRuntimeSnapshot;
use super::ContainerSummary;
use super::CreateContainerRequest;
use super::CreateNetworkRequest;
use super::CreateVolumeRequest;
use super::DockerProviderConfig;
use super::ImageDetail;
use super::ImageOperationResult;
use super::ImageSummary;
use super::NetworkContainerRequest;
use super::NetworkOperationResult;
use super::NetworkSummary;
use super::PullImageRequest;
use super::RemoveContainerRequest;
use super::RemoveImageRequest;
use super::RemoveVolumeRequest;
use super::RestartContainerRequest;
use super::StopContainerRequest;
use super::VolumeDetail;
use super::VolumeOperationResult;
use super::VolumeSummary;
use crate::ContainerImageStore;
use crate::ContainerInventory;
use crate::ContainerLifecycle;
use crate::ContainerNetworkStore;
use crate::ContainerVolumeStore;
use async_trait::async_trait;
use bollard::Docker;

#[derive(Debug, Clone)]
pub struct DockerProvider {
    docker: Docker,
}

impl DockerProvider {
    pub fn connect(config: &DockerProviderConfig) -> Result<Self, ContainerProviderError> {
        let docker = match config.socket_path.as_deref() {
            Some(path) => Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION),
            None => Docker::connect_with_unix_defaults(),
        }?;
        Ok(Self { docker })
    }

    pub(crate) fn docker(&self) -> &Docker {
        &self.docker
    }

    pub async fn probe(&self) -> Result<ContainerRuntimeInfo, ContainerProviderError> {
        let info = self.docker.info().await?;
        Ok(ContainerRuntimeInfo {
            id: info.id,
            server_version: info.server_version,
            docker_root_dir: info.docker_root_dir,
            containers: info.containers,
            images: info.images,
        })
    }

    pub async fn snapshot(&self) -> Result<ContainerRuntimeSnapshot, ContainerProviderError> {
        let daemon = self.probe().await.ok();
        let containers = self.containers(ContainerListFilter { all: true }).await?;
        let networks = self.networks().await.unwrap_or_default();
        let volumes = self.volumes().await.unwrap_or_default();

        Ok(ContainerRuntimeSnapshot {
            runtime: "docker".to_string(),
            daemon,
            containers,
            networks,
            volumes,
        })
    }
}

#[async_trait]
impl ContainerInventory for DockerProvider {
    async fn probe(&self) -> Result<ContainerRuntimeInfo, ContainerProviderError> {
        DockerProvider::probe(self).await
    }

    async fn list(
        &self,
        filter: ContainerListFilter,
    ) -> Result<Vec<ContainerSummary>, ContainerProviderError> {
        self.containers(filter).await
    }

    async fn snapshot(&self) -> Result<ContainerRuntimeSnapshot, ContainerProviderError> {
        DockerProvider::snapshot(self).await
    }
}

#[async_trait]
impl ContainerLifecycle for DockerProvider {
    async fn inspect(&self, id_or_name: &str) -> Result<ContainerDetail, ContainerProviderError> {
        self.inspect_container(id_or_name).await
    }

    async fn create(
        &self,
        request: CreateContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        self.create_container(request).await
    }

    async fn start(
        &self,
        id_or_name: &str,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        self.start_container(id_or_name).await
    }

    async fn stop(
        &self,
        request: StopContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        self.stop_container(request).await
    }

    async fn restart(
        &self,
        request: RestartContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        self.restart_container(request).await
    }

    async fn remove(
        &self,
        request: RemoveContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError> {
        self.remove_container(request).await
    }
}

#[async_trait]
impl ContainerImageStore for DockerProvider {
    async fn images(&self) -> Result<Vec<ImageSummary>, ContainerProviderError> {
        DockerProvider::images(self).await
    }

    async fn inspect_image(&self, reference: &str) -> Result<ImageDetail, ContainerProviderError> {
        DockerProvider::inspect_image(self, reference).await
    }

    async fn pull_image(
        &self,
        request: PullImageRequest,
    ) -> Result<ImageOperationResult, ContainerProviderError> {
        DockerProvider::pull_image(self, request).await
    }

    async fn remove_image(
        &self,
        request: RemoveImageRequest,
    ) -> Result<ImageOperationResult, ContainerProviderError> {
        DockerProvider::remove_image(self, request).await
    }
}

#[async_trait]
impl ContainerNetworkStore for DockerProvider {
    async fn networks(&self) -> Result<Vec<NetworkSummary>, ContainerProviderError> {
        DockerProvider::networks(self).await
    }

    async fn create_network(
        &self,
        request: CreateNetworkRequest,
    ) -> Result<NetworkOperationResult, ContainerProviderError> {
        DockerProvider::create_network(self, request).await
    }

    async fn remove_network(
        &self,
        name_or_id: &str,
    ) -> Result<NetworkOperationResult, ContainerProviderError> {
        DockerProvider::remove_network(self, name_or_id).await
    }

    async fn connect_network(
        &self,
        request: NetworkContainerRequest,
    ) -> Result<NetworkOperationResult, ContainerProviderError> {
        DockerProvider::connect_network(self, request).await
    }

    async fn disconnect_network(
        &self,
        request: NetworkContainerRequest,
    ) -> Result<NetworkOperationResult, ContainerProviderError> {
        DockerProvider::disconnect_network(self, request).await
    }
}

#[async_trait]
impl ContainerVolumeStore for DockerProvider {
    async fn volumes(&self) -> Result<Vec<VolumeSummary>, ContainerProviderError> {
        DockerProvider::volumes(self).await
    }

    async fn inspect_volume(&self, name: &str) -> Result<VolumeDetail, ContainerProviderError> {
        DockerProvider::inspect_volume(self, name).await
    }

    async fn create_volume(
        &self,
        request: CreateVolumeRequest,
    ) -> Result<VolumeOperationResult, ContainerProviderError> {
        DockerProvider::create_volume(self, request).await
    }

    async fn remove_volume(
        &self,
        request: RemoveVolumeRequest,
    ) -> Result<VolumeOperationResult, ContainerProviderError> {
        DockerProvider::remove_volume(self, request).await
    }
}
