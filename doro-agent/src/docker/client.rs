use super::DockerConfig;
use super::DockerError;
use super::DockerHealth;
use bollard::Docker;

#[derive(Debug, Clone)]
pub struct DockerClient {
    docker: Docker,
}

impl DockerClient {
    pub fn connect(config: &DockerConfig) -> Result<Self, DockerError> {
        let docker = match config.socket_path.as_deref() {
            Some(path) => Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION),
            None => Docker::connect_with_unix_defaults(),
        }?;
        Ok(Self { docker })
    }

    pub(crate) fn docker(&self) -> &Docker {
        &self.docker
    }

    pub async fn health(&self) -> Result<DockerHealth, DockerError> {
        let info = self.docker.info().await?;
        Ok(DockerHealth {
            id: info.id,
            server_version: info.server_version,
            docker_root_dir: info.docker_root_dir,
            containers: info.containers,
            images: info.images,
        })
    }
}
