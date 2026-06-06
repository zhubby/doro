use super::ContainerProviderError;
use super::DockerProvider;
use super::ImageDetail;
use super::ImageOperationResult;
use super::ImageSummary;
use super::PullImageRequest;
use super::RemoveImageRequest;
use crate::docker_credentials_for_reference;
use bollard::image::CreateImageOptions;
use bollard::image::ListImagesOptions;
use bollard::image::RemoveImageOptions;
use futures_util::stream::StreamExt;
use serde_json::json;
use std::collections::HashMap;

impl DockerProvider {
    pub async fn images(&self) -> Result<Vec<ImageSummary>, ContainerProviderError> {
        let images = self
            .docker()
            .list_images::<String>(Some(ListImagesOptions {
                all: true,
                ..Default::default()
            }))
            .await?;
        let mut summaries = Vec::with_capacity(images.len());
        for image in images {
            let architecture = self
                .docker()
                .inspect_image(&image.id)
                .await
                .ok()
                .and_then(|detail| platform_label(detail.os, detail.architecture, detail.variant));
            summaries.push(ImageSummary {
                id: Some(image.id),
                repo_tags: image.repo_tags,
                repo_digests: image.repo_digests,
                architecture,
                created: Some(image.created),
                size: Some(image.size),
                labels: json!(image.labels),
            });
        }
        Ok(summaries)
    }

    pub async fn inspect_image(
        &self,
        reference: &str,
    ) -> Result<ImageDetail, ContainerProviderError> {
        require_identifier(reference, "image reference")?;
        let image = self.docker().inspect_image(reference).await?;
        Ok(ImageDetail {
            id: image.id,
            repo_tags: image.repo_tags.unwrap_or_default(),
            repo_digests: image.repo_digests.unwrap_or_default(),
            created: image.created.map(|created| created.to_string()),
            size: image.size,
            config: json!(image.config),
        })
    }

    pub async fn pull_image(
        &self,
        request: PullImageRequest,
    ) -> Result<ImageOperationResult, ContainerProviderError> {
        require_identifier(&request.reference, "image reference")?;
        let credentials = docker_credentials_for_reference(self.config_dir(), &request.reference);
        let mut stream = self.docker().create_image(
            Some(CreateImageOptions {
                from_image: request.reference.clone(),
                tag: request.tag.clone().unwrap_or_default(),
                platform: request.platform.clone().unwrap_or_default(),
                ..Default::default()
            }),
            None,
            credentials,
        );
        let mut updates = Vec::new();
        while let Some(update) = stream.next().await {
            updates.push(update?);
        }
        Ok(ImageOperationResult {
            reference: request.reference,
            action: "pull".to_string(),
            details: json!({ "updates": updates }),
        })
    }

    pub async fn remove_image(
        &self,
        request: RemoveImageRequest,
    ) -> Result<ImageOperationResult, ContainerProviderError> {
        require_identifier(&request.reference, "image reference")?;
        let deleted = self
            .docker()
            .remove_image(
                &request.reference,
                Some(RemoveImageOptions {
                    force: request.force,
                    noprune: request.noprune,
                }),
                None,
            )
            .await?;
        Ok(ImageOperationResult {
            reference: request.reference,
            action: "remove".to_string(),
            details: json!({ "deleted": deleted }),
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

fn platform_label(
    os: Option<String>,
    architecture: Option<String>,
    variant: Option<String>,
) -> Option<String> {
    let architecture = architecture?;
    let platform = match variant.filter(|value| !value.trim().is_empty()) {
        Some(variant) => format!("{architecture}/{variant}"),
        None => architecture,
    };
    os.filter(|value| !value.trim().is_empty())
        .map(|os| format!("{os}/{platform}"))
        .or(Some(platform))
}

#[allow(dead_code)]
fn empty_filters() -> HashMap<String, Vec<String>> {
    HashMap::new()
}
