use crate::ContainerDetail;
use crate::ContainerListFilter;
use crate::ContainerOperationResult;
use crate::ContainerProviderError;
use crate::ContainerRuntimeInfo;
use crate::ContainerRuntimeSnapshot;
use crate::ContainerSummary;
use crate::CreateContainerRequest;
use crate::CreateNetworkRequest;
use crate::CreateVolumeRequest;
use crate::ImageDetail;
use crate::ImageOperationResult;
use crate::ImageSummary;
use crate::NetworkContainerRequest;
use crate::NetworkOperationResult;
use crate::NetworkSummary;
use crate::PullImageRequest;
use crate::RemoveContainerRequest;
use crate::RemoveImageRequest;
use crate::RemoveVolumeRequest;
use crate::RestartContainerRequest;
use crate::StopContainerRequest;
use crate::VolumeDetail;
use crate::VolumeOperationResult;
use crate::VolumeSummary;
use async_trait::async_trait;

#[async_trait]
pub trait ContainerInventory: Send + Sync {
    async fn probe(&self) -> Result<ContainerRuntimeInfo, ContainerProviderError>;
    async fn list(
        &self,
        filter: ContainerListFilter,
    ) -> Result<Vec<ContainerSummary>, ContainerProviderError>;
    async fn snapshot(&self) -> Result<ContainerRuntimeSnapshot, ContainerProviderError>;
}

#[async_trait]
pub trait ContainerLifecycle: Send + Sync {
    async fn inspect(&self, id_or_name: &str) -> Result<ContainerDetail, ContainerProviderError>;
    async fn create(
        &self,
        request: CreateContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError>;
    async fn start(
        &self,
        id_or_name: &str,
    ) -> Result<ContainerOperationResult, ContainerProviderError>;
    async fn stop(
        &self,
        request: StopContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError>;
    async fn restart(
        &self,
        request: RestartContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError>;
    async fn remove(
        &self,
        request: RemoveContainerRequest,
    ) -> Result<ContainerOperationResult, ContainerProviderError>;
}

#[async_trait]
pub trait ContainerImageStore: Send + Sync {
    async fn images(&self) -> Result<Vec<ImageSummary>, ContainerProviderError>;
    async fn inspect_image(&self, reference: &str) -> Result<ImageDetail, ContainerProviderError>;
    async fn pull_image(
        &self,
        request: PullImageRequest,
    ) -> Result<ImageOperationResult, ContainerProviderError>;
    async fn remove_image(
        &self,
        request: RemoveImageRequest,
    ) -> Result<ImageOperationResult, ContainerProviderError>;
}

#[async_trait]
pub trait ContainerNetworkStore: Send + Sync {
    async fn networks(&self) -> Result<Vec<NetworkSummary>, ContainerProviderError>;
    async fn create_network(
        &self,
        request: CreateNetworkRequest,
    ) -> Result<NetworkOperationResult, ContainerProviderError>;
    async fn remove_network(
        &self,
        name_or_id: &str,
    ) -> Result<NetworkOperationResult, ContainerProviderError>;
    async fn connect_network(
        &self,
        request: NetworkContainerRequest,
    ) -> Result<NetworkOperationResult, ContainerProviderError>;
    async fn disconnect_network(
        &self,
        request: NetworkContainerRequest,
    ) -> Result<NetworkOperationResult, ContainerProviderError>;
}

#[async_trait]
pub trait ContainerVolumeStore: Send + Sync {
    async fn volumes(&self) -> Result<Vec<VolumeSummary>, ContainerProviderError>;
    async fn inspect_volume(&self, name: &str) -> Result<VolumeDetail, ContainerProviderError>;
    async fn create_volume(
        &self,
        request: CreateVolumeRequest,
    ) -> Result<VolumeOperationResult, ContainerProviderError>;
    async fn remove_volume(
        &self,
        request: RemoveVolumeRequest,
    ) -> Result<VolumeOperationResult, ContainerProviderError>;
}

pub trait ContainerProvider:
    ContainerInventory
    + ContainerLifecycle
    + ContainerImageStore
    + ContainerNetworkStore
    + ContainerVolumeStore
{
}

impl<T> ContainerProvider for T where
    T: ContainerInventory
        + ContainerLifecycle
        + ContainerImageStore
        + ContainerNetworkStore
        + ContainerVolumeStore
{
}
