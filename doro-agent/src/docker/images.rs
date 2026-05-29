use super::DockerClient;
use super::DockerError;
use super::ImageDetail;
use super::ImageOperationResult;
use super::ImageSummary;
use super::PullImageRequest;
use super::RemoveImageRequest;
use bollard::image::CreateImageOptions;
use bollard::image::ListImagesOptions;
use bollard::image::RemoveImageOptions;
use futures_util::stream::StreamExt;
use serde_json::json;
use std::collections::HashMap;

impl DockerClient {
    pub async fn images(&self) -> Result<Vec<ImageSummary>, DockerError> {
        let images = self
            .docker()
            .list_images::<String>(Some(ListImagesOptions {
                all: true,
                ..Default::default()
            }))
            .await?;
        Ok(images
            .into_iter()
            .map(|image| ImageSummary {
                id: Some(image.id),
                repo_tags: image.repo_tags,
                repo_digests: image.repo_digests,
                created: Some(image.created),
                size: Some(image.size),
                labels: json!(image.labels),
            })
            .collect())
    }

    pub async fn inspect_image(&self, reference: &str) -> Result<ImageDetail, DockerError> {
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
    ) -> Result<ImageOperationResult, DockerError> {
        require_identifier(&request.reference, "image reference")?;
        let mut stream = self.docker().create_image(
            Some(CreateImageOptions {
                from_image: request.reference.clone(),
                tag: request.tag.clone().unwrap_or_default(),
                platform: request.platform.clone().unwrap_or_default(),
                ..Default::default()
            }),
            None,
            None,
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
    ) -> Result<ImageOperationResult, DockerError> {
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

fn require_identifier(value: &str, field: &'static str) -> Result<(), DockerError> {
    if value.trim().is_empty() {
        return Err(DockerError::InvalidRequest(format!("{field} is required")));
    }
    Ok(())
}

#[allow(dead_code)]
fn empty_filters() -> HashMap<String, Vec<String>> {
    HashMap::new()
}
