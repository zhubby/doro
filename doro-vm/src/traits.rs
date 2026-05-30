use crate::VmCommandResult;
use crate::VmConsoleEndpoint;
use crate::VmDeleteMode;
use crate::VmId;
use crate::VmImageRef;
use crate::VmProviderError;
use crate::VmProviderStatus;
use crate::VmRuntimeState;
use crate::VmSnapshot;
use crate::VmSnapshotRequest;
use crate::VmSpec;
use crate::VmStopMode;
use async_trait::async_trait;

#[async_trait]
pub trait VirtualMachineInventory: Send + Sync {
    async fn probe(&self) -> Result<VmProviderStatus, VmProviderError>;
    async fn list(&self) -> Result<Vec<VmRuntimeState>, VmProviderError>;
}

#[async_trait]
pub trait VirtualMachineLifecycle: Send + Sync {
    async fn create(&self, spec: VmSpec) -> Result<VmRuntimeState, VmProviderError>;
    async fn start(&self, id: &VmId) -> Result<VmCommandResult, VmProviderError>;
    async fn stop(&self, id: &VmId, mode: VmStopMode) -> Result<VmCommandResult, VmProviderError>;
    async fn restart(&self, id: &VmId) -> Result<VmCommandResult, VmProviderError>;
    async fn delete(
        &self,
        id: &VmId,
        mode: VmDeleteMode,
    ) -> Result<VmCommandResult, VmProviderError>;
}

#[async_trait]
pub trait VirtualMachineImageStore: Send + Sync {
    async fn images(&self) -> Result<Vec<VmImageRef>, VmProviderError>;
}

#[async_trait]
pub trait VirtualMachineSnapshotStore: Send + Sync {
    async fn snapshots(&self, id: &VmId) -> Result<Vec<VmSnapshot>, VmProviderError>;
    async fn snapshot(
        &self,
        id: &VmId,
        request: VmSnapshotRequest,
    ) -> Result<VmSnapshot, VmProviderError>;
}

#[async_trait]
pub trait VirtualMachineConsoleProvider: Send + Sync {
    async fn console(&self, id: &VmId) -> Result<VmConsoleEndpoint, VmProviderError>;
}

pub trait VirtualMachineProvider:
    VirtualMachineInventory
    + VirtualMachineLifecycle
    + VirtualMachineImageStore
    + VirtualMachineSnapshotStore
    + VirtualMachineConsoleProvider
{
}

impl<T> VirtualMachineProvider for T where
    T: VirtualMachineInventory
        + VirtualMachineLifecycle
        + VirtualMachineImageStore
        + VirtualMachineSnapshotStore
        + VirtualMachineConsoleProvider
{
}
