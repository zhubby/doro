use super::ContainerProviderError;
use super::CreateVolumeRequest;
use super::DockerProvider;
use super::RemoveVolumeRequest;
use super::VolumeDetail;
use super::VolumeOperationResult;
use super::VolumeSummary;
use bollard::volume::CreateVolumeOptions;
use bollard::volume::RemoveVolumeOptions;
use serde_json::json;

impl DockerProvider {
    pub async fn volumes(&self) -> Result<Vec<VolumeSummary>, ContainerProviderError> {
        let volumes = self.docker().list_volumes::<String>(None).await?;
        Ok(volumes
            .volumes
            .unwrap_or_default()
            .into_iter()
            .map(|volume| VolumeSummary {
                name: volume.name,
                driver: Some(volume.driver),
                mountpoint: Some(volume.mountpoint),
                labels: json!(volume.labels),
                usage_size: volume.usage_data.as_ref().map(|usage| usage.size),
                usage_ref_count: volume.usage_data.as_ref().map(|usage| usage.ref_count),
            })
            .collect())
    }

    pub async fn inspect_volume(&self, name: &str) -> Result<VolumeDetail, ContainerProviderError> {
        require_identifier(name, "volume name")?;
        let volume = self.docker().inspect_volume(name).await?;
        Ok(VolumeDetail {
            name: volume.name,
            driver: Some(volume.driver),
            mountpoint: Some(volume.mountpoint),
            labels: json!(volume.labels),
            options: json!(volume.options),
            scope: volume
                .scope
                .map(|scope| format!("{scope:?}").to_lowercase()),
        })
    }

    pub async fn create_volume(
        &self,
        request: CreateVolumeRequest,
    ) -> Result<VolumeOperationResult, ContainerProviderError> {
        require_identifier(&request.name, "volume name")?;
        let driver = if request.driver.trim().is_empty() {
            "local".to_string()
        } else {
            request.driver.clone()
        };
        let volume = self
            .docker()
            .create_volume(CreateVolumeOptions {
                name: request.name.clone(),
                driver,
                driver_opts: request.driver_opts,
                labels: request.labels,
            })
            .await?;
        Ok(VolumeOperationResult {
            name: volume.name,
            action: "create".to_string(),
            details: json!({
                "driver": volume.driver,
                "mountpoint": volume.mountpoint,
            }),
        })
    }

    pub async fn remove_volume(
        &self,
        request: RemoveVolumeRequest,
    ) -> Result<VolumeOperationResult, ContainerProviderError> {
        require_identifier(&request.name, "volume name")?;
        self.docker()
            .remove_volume(
                &request.name,
                Some(RemoveVolumeOptions {
                    force: request.force,
                }),
            )
            .await?;
        Ok(VolumeOperationResult {
            name: request.name,
            action: "remove".to_string(),
            details: json!({}),
        })
    }
}

fn require_identifier(value: &str, field: &'static str) -> Result<(), ContainerProviderError> {
    if value.trim().is_empty() {
        return Err(ContainerProviderError::InvalidRequest(format!(
            "{field} is required"
        )));
    }
    Ok(())
}
