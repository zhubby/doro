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
pub const CONFIG_FILE_NAME: &str = "config.toml";

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
pub struct DoroConfig {
    pub server: ServerConfig,
    pub store: StoreConfig,
    pub security: SecurityConfig,
    pub agent: AgentConfig,
    pub ai: AiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServerConfig {
    pub http_bind: String,
    pub grpc_bind: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            http_bind: "127.0.0.1:8787".to_string(),
            grpc_bind: "127.0.0.1:8788".to_string(),
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
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: DoroConfig,
    pub created: bool,
}

pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir().ok_or(ConfigError::HomeDirectoryUnavailable)?;
    Ok(home.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME))
}

pub fn load_or_create(path: Option<&Path>) -> Result<LoadedConfig, ConfigError> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => default_config_path()?,
    };

    if path.exists() {
        return load_existing(path);
    }

    let config = DoroConfig::default();
    write_config(&path, &config)?;
    Ok(LoadedConfig {
        path,
        config,
        created: true,
    })
}

pub fn write_config(path: &Path, config: &DoroConfig) -> Result<(), ConfigError> {
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

fn load_existing(path: PathBuf) -> Result<LoadedConfig, ConfigError> {
    let body = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
        path: path.clone(),
        source,
    })?;
    let config = toml::from_str(&body).map_err(|source| ConfigError::Parse {
        path: path.clone(),
        source,
    })?;
    Ok(LoadedConfig {
        path,
        config,
        created: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_or_create_writes_default_config_when_missing() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(".doro").join("config.toml");

        let loaded = load_or_create(Some(&path))?;

        assert!(loaded.created);
        assert!(path.exists());
        assert_eq!(loaded.config.server.http_bind, "127.0.0.1:8787");
        assert_eq!(loaded.config.store.backend, StoreBackend::Postgres);
        assert_eq!(
            loaded.config.store.database_url,
            "postgres://doro:doro@127.0.0.1:5432/doro"
        );
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

        Ok(())
    }

    #[test]
    fn load_or_create_reads_existing_config() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("custom.toml");
        fs::write(
            &path,
            r#"
                [server]
                http_bind = "0.0.0.0:9000"
                grpc_bind = "0.0.0.0:9001"
            "#,
        )?;

        let loaded = load_or_create(Some(&path))?;

        assert!(!loaded.created);
        assert_eq!(loaded.config.server.http_bind, "0.0.0.0:9000");
        assert_eq!(loaded.config.server.grpc_bind, "0.0.0.0:9001");
        assert_eq!(loaded.config.store.backend, StoreBackend::Postgres);
        assert_eq!(
            loaded.config.store.database_url,
            "postgres://doro:doro@127.0.0.1:5432/doro"
        );

        Ok(())
    }
}
