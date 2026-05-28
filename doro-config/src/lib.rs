use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

pub const CONFIG_DIR_NAME: &str = ".doro";
pub const CONTROL_PLANE_CONFIG_FILE_NAME: &str = "control-plane.toml";
pub const AGENT_CONFIG_FILE_NAME: &str = "agent.toml";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not determine the current user's home directory")]
    HomeDirectoryUnavailable,
    #[error("failed to read config at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write config at {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create config directory at {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse config at {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to serialize config")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ControlPlaneConfig {
    pub server: ServerConfig,
    pub store: StoreConfig,
    pub security: SecurityConfig,
    pub ai: AiConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AgentFileConfig {
    pub agent: AgentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServerConfig {
    pub console_bind: String,
    pub agent_bind: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            console_bind: "127.0.0.1:8787".to_string(),
            agent_bind: "127.0.0.1:8788".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct StoreConfig {
    pub backend: StoreBackend,
    pub database_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            backend: StoreBackend::Postgres,
            database_url: "postgres://doro:doro@127.0.0.1:5432/doro".to_string(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout_seconds: 8,
            idle_timeout_seconds: 300,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoreBackend {
    #[default]
    Postgres,
}

impl fmt::Display for StoreBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres => formatter.write_str("postgres"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SecurityConfig {
    pub approval_policy: String,
    pub require_tls: bool,
    pub jwt_secret: Option<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            approval_policy: "policy_and_human_approval".to_string(),
            require_tls: false,
            jwt_secret: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AgentConfig {
    pub control_plane_url: String,
    pub hostname: String,
    pub enrollment_token: Option<String>,
    pub agent_id: Option<Uuid>,
    pub host_id: Option<Uuid>,
    pub heartbeat_interval_seconds: u64,
    pub metrics_enabled: bool,
    pub metrics_interval_seconds: u64,
    pub process_names: Vec<String>,
    pub container_metrics_enabled: bool,
    pub docker_socket_path: Option<String>,
    pub gpu_metrics_enabled: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            control_plane_url: "http://127.0.0.1:8788".to_string(),
            hostname: "doro-local-agent".to_string(),
            enrollment_token: None,
            agent_id: None,
            host_id: None,
            heartbeat_interval_seconds: 30,
            metrics_enabled: true,
            metrics_interval_seconds: 10,
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            gpu_metrics_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AiConfig {
    pub provider: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "disabled".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedControlPlaneConfig {
    pub path: PathBuf,
    pub config: ControlPlaneConfig,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct LoadedAgentConfig {
    pub path: PathBuf,
    pub config: AgentFileConfig,
    pub created: bool,
}

pub fn default_control_plane_config_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir().ok_or(ConfigError::HomeDirectoryUnavailable)?;
    Ok(home
        .join(CONFIG_DIR_NAME)
        .join(CONTROL_PLANE_CONFIG_FILE_NAME))
}

pub fn default_agent_config_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir().ok_or(ConfigError::HomeDirectoryUnavailable)?;
    Ok(home.join(CONFIG_DIR_NAME).join(AGENT_CONFIG_FILE_NAME))
}

pub fn load_or_create_control_plane_config(
    path: Option<&Path>,
) -> Result<LoadedControlPlaneConfig, ConfigError> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => default_control_plane_config_path()?,
    };

    if path.exists() {
        return load_existing_control_plane_config(path);
    }

    let config = ControlPlaneConfig::default();
    write_toml_config(&path, &config)?;
    Ok(LoadedControlPlaneConfig {
        path,
        config,
        created: true,
    })
}

pub fn load_or_create_agent_config(path: Option<&Path>) -> Result<LoadedAgentConfig, ConfigError> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => default_agent_config_path()?,
    };

    if path.exists() {
        return load_existing_agent_config(path);
    }

    let config = AgentFileConfig::default();
    write_agent_config(&path, &config)?;
    Ok(LoadedAgentConfig {
        path,
        config,
        created: true,
    })
}

pub fn write_agent_config(path: &Path, config: &AgentFileConfig) -> Result<(), ConfigError> {
    write_toml_config(path, config)
}

fn write_toml_config<T>(path: &Path, config: &T) -> Result<(), ConfigError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let body = toml::to_string_pretty(config)?;
    fs::write(path, body).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn load_existing_control_plane_config(
    path: PathBuf,
) -> Result<LoadedControlPlaneConfig, ConfigError> {
    let body = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
        path: path.clone(),
        source,
    })?;
    let config = toml::from_str(&body).map_err(|source| ConfigError::Parse {
        path: path.clone(),
        source,
    })?;
    Ok(LoadedControlPlaneConfig {
        path,
        config,
        created: false,
    })
}

fn load_existing_agent_config(path: PathBuf) -> Result<LoadedAgentConfig, ConfigError> {
    let body = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
        path: path.clone(),
        source,
    })?;
    let config = toml::from_str(&body).map_err(|source| ConfigError::Parse {
        path: path.clone(),
        source,
    })?;
    Ok(LoadedAgentConfig {
        path,
        config,
        created: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_or_create_writes_default_control_plane_config_when_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(".doro").join("control-plane.toml");

        let loaded = load_or_create_control_plane_config(Some(&path))?;

        assert!(loaded.created);
        assert!(path.exists());
        assert_eq!(loaded.config.server.console_bind, "127.0.0.1:8787");
        assert_eq!(loaded.config.store.backend, StoreBackend::Postgres);
        assert_eq!(
            loaded.config.store.database_url,
            "postgres://doro:doro@127.0.0.1:5432/doro"
        );
        let body = fs::read_to_string(&path)?;
        assert!(body.contains("[server]"));
        assert!(body.contains("[store]"));
        assert!(body.contains("[security]"));
        assert!(body.contains("[ai]"));
        assert!(!body.contains("[agent]"));

        Ok(())
    }

    #[test]
    fn load_or_create_writes_default_agent_config_when_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(".doro").join("agent.toml");

        let loaded = load_or_create_agent_config(Some(&path))?;

        assert!(loaded.created);
        assert!(path.exists());
        assert_eq!(
            loaded.config.agent.control_plane_url,
            "http://127.0.0.1:8788"
        );
        assert_eq!(loaded.config.agent.heartbeat_interval_seconds, 30);
        assert!(loaded.config.agent.metrics_enabled);
        assert_eq!(loaded.config.agent.metrics_interval_seconds, 10);
        assert!(loaded.config.agent.process_names.is_empty());
        assert!(!loaded.config.agent.container_metrics_enabled);
        assert!(loaded.config.agent.docker_socket_path.is_none());
        assert!(!loaded.config.agent.gpu_metrics_enabled);
        assert!(loaded.config.agent.enrollment_token.is_none());
        assert!(loaded.config.agent.agent_id.is_none());
        assert!(loaded.config.agent.host_id.is_none());
        let body = fs::read_to_string(&path)?;
        assert!(body.contains("[agent]"));
        assert!(!body.contains("[server]"));
        assert!(!body.contains("[store]"));
        assert!(!body.contains("[security]"));

        Ok(())
    }

    #[test]
    fn load_or_create_reads_existing_control_plane_config() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("custom-control-plane.toml");
        fs::write(
            &path,
            r#"
                [server]
                console_bind = "0.0.0.0:9000"
                agent_bind = "0.0.0.0:9001"
            "#,
        )?;

        let loaded = load_or_create_control_plane_config(Some(&path))?;

        assert!(!loaded.created);
        assert_eq!(loaded.config.server.console_bind, "0.0.0.0:9000");
        assert_eq!(loaded.config.server.agent_bind, "0.0.0.0:9001");
        assert_eq!(loaded.config.store.backend, StoreBackend::Postgres);
        assert_eq!(
            loaded.config.store.database_url,
            "postgres://doro:doro@127.0.0.1:5432/doro"
        );

        Ok(())
    }

    #[test]
    fn load_or_create_reads_existing_agent_config() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("custom-agent.toml");
        fs::write(
            &path,
            r#"
                [agent]
                control_plane_url = "http://control-plane:8788"
                hostname = "edge-node"
                heartbeat_interval_seconds = 15
            "#,
        )?;

        let loaded = load_or_create_agent_config(Some(&path))?;

        assert!(!loaded.created);
        assert_eq!(
            loaded.config.agent.control_plane_url,
            "http://control-plane:8788"
        );
        assert_eq!(loaded.config.agent.hostname, "edge-node");
        assert_eq!(loaded.config.agent.heartbeat_interval_seconds, 15);
        assert_eq!(loaded.config.agent.metrics_interval_seconds, 10);

        Ok(())
    }
}
