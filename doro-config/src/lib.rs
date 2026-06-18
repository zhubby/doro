use serde::Deserialize;
use serde::Serialize;
use std::env;
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
    #[error("invalid environment variable {name}: expected {expected}, got {value:?}")]
    Env {
        name: &'static str,
        value: String,
        expected: &'static str,
    },
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
    pub websites: WebsiteConfig,
    pub reliability: AgentReliabilityConfig,
    pub ai: AiConfig,
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
            console_bind: "0.0.0.0:8787".to_string(),
            agent_bind: "0.0.0.0:8788".to_string(),
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
pub struct WebsiteConfig {
    pub http_bind: String,
    pub https_bind: Option<String>,
    pub tcp_bind: Option<String>,
    pub udp_bind: Option<String>,
    pub static_root: Option<String>,
    pub certificate_store: Option<String>,
}

impl Default for WebsiteConfig {
    fn default() -> Self {
        Self {
            http_bind: "127.0.0.1:8080".to_string(),
            https_bind: None,
            tcp_bind: None,
            udp_bind: None,
            static_root: None,
            certificate_store: None,
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
    pub metrics_interval_seconds: u64,
    pub process_names: Vec<String>,
    pub docker_socket_path: Option<String>,
    pub docker_compose_root: Option<String>,
    pub qemu_binary_dir: Option<String>,
    pub vm_state_dir: Option<String>,
    pub vm_image_dir: Option<String>,
    pub vm_bridge_names: Vec<String>,
    pub vm_user_network_enabled: bool,
    pub vm_console_enabled: bool,
    pub vm_vnc_bind: String,
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
            metrics_interval_seconds: 10,
            process_names: Vec::new(),
            docker_socket_path: None,
            docker_compose_root: None,
            qemu_binary_dir: None,
            vm_state_dir: None,
            vm_image_dir: None,
            vm_bridge_names: Vec::new(),
            vm_user_network_enabled: true,
            vm_console_enabled: true,
            vm_vnc_bind: "127.0.0.1".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AgentReliabilityConfig {
    pub event_spool_enabled: bool,
    pub event_spool_path: Option<String>,
    pub event_spool_max_files: u32,
    pub event_spool_max_bytes: u64,
    pub command_cancel_grace_seconds: u64,
    pub preflight_enabled: bool,
}

impl Default for AgentReliabilityConfig {
    fn default() -> Self {
        Self {
            event_spool_enabled: true,
            event_spool_path: None,
            event_spool_max_files: 256,
            event_spool_max_bytes: 64 * 1024 * 1024,
            command_cancel_grace_seconds: 5,
            preflight_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AiConfig {
    pub provider: String,
    pub openai: OpenAiConfig,
    pub agent: AgentAiConfig,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "disabled".to_string(),
            openai: OpenAiConfig::default(),
            agent: AgentAiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AgentAiConfig {
    pub max_turns: u32,
    pub max_tool_calls: u32,
    pub tool_timeout_seconds: u64,
    pub shell_timeout_seconds: u64,
    pub approval_timeout_seconds: u64,
}

impl Default for AgentAiConfig {
    fn default() -> Self {
        Self {
            max_turns: 12,
            max_tool_calls: 32,
            tool_timeout_seconds: 30,
            shell_timeout_seconds: 120,
            approval_timeout_seconds: 24 * 60 * 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct OpenAiConfig {
    pub api_key_env: String,
    pub base_url: String,
    pub default_chat_model: String,
    pub default_response_model: String,
    pub timeout_seconds: u64,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key_env: "OPENAI_API_KEY".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            default_chat_model: "gpt-4.1-mini".to_string(),
            default_response_model: "gpt-4.1-mini".to_string(),
            timeout_seconds: 60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedControlPlaneConfig {
    pub path: Option<PathBuf>,
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
    load_control_plane_config_with_env(path, |name| env::var(name).ok())
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

fn load_control_plane_config_with_env<F>(
    path: Option<&Path>,
    env_value: F,
) -> Result<LoadedControlPlaneConfig, ConfigError>
where
    F: FnMut(&'static str) -> Option<String>,
{
    let explicit_path = path.is_some();
    let candidate_path = path
        .map(Path::to_path_buf)
        .or_else(|| default_control_plane_config_path().ok());

    load_control_plane_config_from_candidate(candidate_path, explicit_path, env_value)
}

fn load_control_plane_config_from_candidate<F>(
    candidate_path: Option<PathBuf>,
    explicit_path: bool,
    env_value: F,
) -> Result<LoadedControlPlaneConfig, ConfigError>
where
    F: FnMut(&'static str) -> Option<String>,
{
    let (path, mut config) = match candidate_path {
        Some(path) if path.exists() => (Some(path.clone()), read_control_plane_config(&path)?),
        Some(path) if explicit_path => {
            return Err(ConfigError::Read {
                path,
                source: io::Error::new(io::ErrorKind::NotFound, "config file does not exist"),
            });
        }
        _ => (None, ControlPlaneConfig::default()),
    };
    apply_control_plane_env_overrides(&mut config, env_value)?;

    Ok(LoadedControlPlaneConfig {
        path,
        config,
        created: false,
    })
}

fn read_control_plane_config(path: &Path) -> Result<ControlPlaneConfig, ConfigError> {
    let body = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    toml::from_str(&body).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn apply_control_plane_env_overrides<F>(
    config: &mut ControlPlaneConfig,
    mut env_value: F,
) -> Result<(), ConfigError>
where
    F: FnMut(&'static str) -> Option<String>,
{
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_CONSOLE_BIND",
            "DORO_CONTROL_PLANE_SERVER_CONSOLE_BIND",
        ],
    ) {
        config.server.console_bind = value;
    }
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_AGENT_BIND",
            "DORO_CONTROL_PLANE_SERVER_AGENT_BIND",
        ],
    ) {
        config.server.agent_bind = value;
    }
    if let Some((name, value)) =
        first_env_value(&mut env_value, &["DORO_CONTROL_PLANE_STORE_BACKEND"])
    {
        config.store.backend = parse_store_backend(name, &value)?;
    }
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_DATABASE_URL",
            "DORO_CONTROL_PLANE_STORE_DATABASE_URL",
        ],
    ) {
        config.store.database_url = value;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &["DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS"],
    ) {
        config.store.max_connections = parse_u32_env(name, &value)?;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &["DORO_CONTROL_PLANE_STORE_MIN_CONNECTIONS"],
    ) {
        config.store.min_connections = parse_u32_env(name, &value)?;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &["DORO_CONTROL_PLANE_STORE_CONNECT_TIMEOUT_SECONDS"],
    ) {
        config.store.connect_timeout_seconds = parse_u64_env(name, &value)?;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &["DORO_CONTROL_PLANE_STORE_IDLE_TIMEOUT_SECONDS"],
    ) {
        config.store.idle_timeout_seconds = parse_u64_env(name, &value)?;
    }
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_APPROVAL_POLICY",
            "DORO_CONTROL_PLANE_SECURITY_APPROVAL_POLICY",
        ],
    ) {
        config.security.approval_policy = value;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_REQUIRE_TLS",
            "DORO_CONTROL_PLANE_SECURITY_REQUIRE_TLS",
        ],
    ) {
        config.security.require_tls = parse_bool_env(name, &value)?;
    }
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_JWT_SECRET",
            "DORO_CONTROL_PLANE_SECURITY_JWT_SECRET",
        ],
    ) {
        config.security.jwt_secret = Some(value);
    }
    if let Some((_, value)) = first_env_value(&mut env_value, &["DORO_CONTROL_PLANE_AI_PROVIDER"]) {
        config.ai.provider = value;
    }
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_OPENAI_API_KEY_ENV",
            "DORO_CONTROL_PLANE_AI_OPENAI_API_KEY_ENV",
        ],
    ) {
        config.ai.openai.api_key_env = value;
    }
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_OPENAI_BASE_URL",
            "DORO_CONTROL_PLANE_AI_OPENAI_BASE_URL",
        ],
    ) {
        config.ai.openai.base_url = value;
    }
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_OPENAI_DEFAULT_CHAT_MODEL",
            "DORO_CONTROL_PLANE_AI_OPENAI_DEFAULT_CHAT_MODEL",
        ],
    ) {
        config.ai.openai.default_chat_model = value;
    }
    if let Some((_, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_OPENAI_DEFAULT_RESPONSE_MODEL",
            "DORO_CONTROL_PLANE_AI_OPENAI_DEFAULT_RESPONSE_MODEL",
        ],
    ) {
        config.ai.openai.default_response_model = value;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &[
            "DORO_CONTROL_PLANE_OPENAI_TIMEOUT_SECONDS",
            "DORO_CONTROL_PLANE_AI_OPENAI_TIMEOUT_SECONDS",
        ],
    ) {
        config.ai.openai.timeout_seconds = parse_u64_env(name, &value)?;
    }
    if let Some((name, value)) =
        first_env_value(&mut env_value, &["DORO_CONTROL_PLANE_AI_AGENT_MAX_TURNS"])
    {
        config.ai.agent.max_turns = parse_u32_env(name, &value)?;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &["DORO_CONTROL_PLANE_AI_AGENT_MAX_TOOL_CALLS"],
    ) {
        config.ai.agent.max_tool_calls = parse_u32_env(name, &value)?;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &["DORO_CONTROL_PLANE_AI_AGENT_TOOL_TIMEOUT_SECONDS"],
    ) {
        config.ai.agent.tool_timeout_seconds = parse_u64_env(name, &value)?;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &["DORO_CONTROL_PLANE_AI_AGENT_SHELL_TIMEOUT_SECONDS"],
    ) {
        config.ai.agent.shell_timeout_seconds = parse_u64_env(name, &value)?;
    }
    if let Some((name, value)) = first_env_value(
        &mut env_value,
        &["DORO_CONTROL_PLANE_AI_AGENT_APPROVAL_TIMEOUT_SECONDS"],
    ) {
        config.ai.agent.approval_timeout_seconds = parse_u64_env(name, &value)?;
    }

    Ok(())
}

fn first_env_value<F>(
    env_value: &mut F,
    names: &'static [&'static str],
) -> Option<(&'static str, String)>
where
    F: FnMut(&'static str) -> Option<String>,
{
    names.iter().find_map(|name| {
        env_value(name).and_then(|value| {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some((*name, value.to_string()))
            }
        })
    })
}

fn parse_store_backend(name: &'static str, value: &str) -> Result<StoreBackend, ConfigError> {
    match value.to_ascii_lowercase().as_str() {
        "postgres" => Ok(StoreBackend::Postgres),
        _ => Err(ConfigError::Env {
            name,
            value: value.to_string(),
            expected: "postgres",
        }),
    }
}

fn parse_u32_env(name: &'static str, value: &str) -> Result<u32, ConfigError> {
    value.parse().map_err(|_| ConfigError::Env {
        name,
        value: value.to_string(),
        expected: "an unsigned 32-bit integer",
    })
}

fn parse_u64_env(name: &'static str, value: &str) -> Result<u64, ConfigError> {
    value.parse().map_err(|_| ConfigError::Env {
        name,
        value: value.to_string(),
        expected: "an unsigned 64-bit integer",
    })
}

fn parse_bool_env(name: &'static str, value: &str) -> Result<bool, ConfigError> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::Env {
            name,
            value: value.to_string(),
            expected: "true, false, 1, 0, yes, no, on, or off",
        }),
    }
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
    fn load_or_create_uses_default_control_plane_config_when_missing_without_writing_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(".doro").join("control-plane.toml");

        let loaded =
            load_control_plane_config_from_candidate(Some(path.clone()), false, empty_env())?;

        assert!(!loaded.created);
        assert!(loaded.path.is_none());
        assert!(!path.exists());
        assert_eq!(loaded.config.server.console_bind, "0.0.0.0:8787");
        assert_eq!(loaded.config.server.agent_bind, "0.0.0.0:8788");
        assert_eq!(loaded.config.store.backend, StoreBackend::Postgres);
        assert_eq!(
            loaded.config.store.database_url,
            "postgres://doro:doro@127.0.0.1:5432/doro"
        );

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
        assert_eq!(loaded.config.agent.metrics_interval_seconds, 10);
        assert!(loaded.config.agent.process_names.is_empty());
        assert!(loaded.config.agent.docker_socket_path.is_none());
        assert!(loaded.config.agent.enrollment_token.is_none());
        assert!(loaded.config.agent.agent_id.is_none());
        assert!(loaded.config.agent.host_id.is_none());
        assert_eq!(loaded.config.ai.provider, "disabled");
        assert_eq!(loaded.config.ai.agent.max_turns, 12);
        assert_eq!(loaded.config.ai.agent.max_tool_calls, 32);
        let body = fs::read_to_string(&path)?;
        assert!(body.contains("[agent]"));
        assert!(body.contains("[websites]"));
        assert!(body.contains("[ai]"));
        assert!(body.contains("[ai.openai]"));
        assert!(body.contains("[ai.agent]"));
        assert!(!body.contains("metrics_enabled"));
        assert!(!body.contains("container_metrics_enabled"));
        assert!(!body.contains("docker_manage_enabled"));
        assert!(!body.contains("docker_compose_enabled"));
        assert!(!body.contains("vm_manage_enabled"));
        assert!(!body.contains("gpu_metrics_enabled"));
        assert!(!body.contains("[websites]\nenabled"));
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

        let loaded = load_control_plane_config_with_env(Some(&path), empty_env())?;

        assert!(!loaded.created);
        assert_eq!(loaded.path.as_deref(), Some(path.as_path()));
        assert_eq!(loaded.config.server.console_bind, "0.0.0.0:9000");
        assert_eq!(loaded.config.server.agent_bind, "0.0.0.0:9001");
        assert_eq!(loaded.config.ai.provider, "disabled");
        assert_eq!(loaded.config.ai.openai.api_key_env, "OPENAI_API_KEY");
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
        assert_eq!(loaded.config.websites.http_bind, "127.0.0.1:8080");

        Ok(())
    }

    #[test]
    fn reads_legacy_agent_config_with_removed_enable_flags()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("legacy-agent.toml");
        fs::write(
            &path,
            r#"
                [agent]
                control_plane_url = "http://control-plane:8788"
                hostname = "edge-node"
                heartbeat_interval_seconds = 15
                metrics_enabled = false
                container_metrics_enabled = false
                docker_manage_enabled = false
                docker_compose_enabled = false
                docker_compose_root = "/srv/doro/compose"
                gpu_metrics_enabled = true
                vm_manage_enabled = true
                qemu_binary_dir = "/opt/qemu/bin"
                vm_state_dir = "/var/lib/doro/vms"
                vm_image_dir = "/var/lib/doro/images"

                [websites]
                enabled = false
                http_bind = "127.0.0.1:18080"
            "#,
        )?;

        let loaded = load_or_create_agent_config(Some(&path))?;

        assert!(!loaded.created);
        assert_eq!(loaded.config.agent.hostname, "edge-node");
        assert_eq!(
            loaded.config.agent.docker_compose_root.as_deref(),
            Some("/srv/doro/compose")
        );
        assert_eq!(
            loaded.config.agent.qemu_binary_dir.as_deref(),
            Some("/opt/qemu/bin")
        );
        assert_eq!(
            loaded.config.agent.vm_state_dir.as_deref(),
            Some("/var/lib/doro/vms")
        );
        assert_eq!(
            loaded.config.agent.vm_image_dir.as_deref(),
            Some("/var/lib/doro/images")
        );
        assert_eq!(loaded.config.websites.http_bind, "127.0.0.1:18080");

        Ok(())
    }

    #[test]
    fn reads_openai_control_plane_config() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("openai-control-plane.toml");
        fs::write(
            &path,
            r#"
                [ai]
                provider = "openai"

                [ai.openai]
                api_key_env = "DORO_OPENAI_API_KEY"
                base_url = "https://example.test/v1"
                default_chat_model = "gpt-4.1-mini"
                default_response_model = "gpt-4.1"
                timeout_seconds = 30
            "#,
        )?;

        let loaded = load_control_plane_config_with_env(Some(&path), empty_env())?;

        assert!(!loaded.created);
        assert_eq!(loaded.config.ai.provider, "openai");
        assert_eq!(loaded.config.ai.openai.api_key_env, "DORO_OPENAI_API_KEY");
        assert_eq!(loaded.config.ai.openai.base_url, "https://example.test/v1");
        assert_eq!(loaded.config.ai.openai.default_chat_model, "gpt-4.1-mini");
        assert_eq!(loaded.config.ai.openai.default_response_model, "gpt-4.1");
        assert_eq!(loaded.config.ai.openai.timeout_seconds, 30);
        assert_eq!(loaded.config.ai.agent.max_turns, 12);

        Ok(())
    }

    #[test]
    fn control_plane_environment_overrides_all_config_sections()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("missing-control-plane.toml");
        let loaded = load_control_plane_config_from_candidate(
            Some(path.clone()),
            false,
            static_env(&[
                ("DORO_CONTROL_PLANE_CONSOLE_BIND", "0.0.0.0:19087"),
                ("DORO_CONTROL_PLANE_AGENT_BIND", "0.0.0.0:19088"),
                ("DORO_CONTROL_PLANE_STORE_BACKEND", "postgres"),
                (
                    "DORO_CONTROL_PLANE_DATABASE_URL",
                    "postgres://env:secret@db:5432/env",
                ),
                ("DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS", "42"),
                ("DORO_CONTROL_PLANE_STORE_MIN_CONNECTIONS", "4"),
                ("DORO_CONTROL_PLANE_STORE_CONNECT_TIMEOUT_SECONDS", "12"),
                ("DORO_CONTROL_PLANE_STORE_IDLE_TIMEOUT_SECONDS", "600"),
                ("DORO_CONTROL_PLANE_APPROVAL_POLICY", "policy_only"),
                ("DORO_CONTROL_PLANE_REQUIRE_TLS", "yes"),
                ("DORO_CONTROL_PLANE_JWT_SECRET", "jwt-from-env"),
                ("DORO_CONTROL_PLANE_AI_PROVIDER", "openai"),
                (
                    "DORO_CONTROL_PLANE_OPENAI_API_KEY_ENV",
                    "DORO_OPENAI_SECRET",
                ),
                (
                    "DORO_CONTROL_PLANE_OPENAI_BASE_URL",
                    "https://api.example.test/v1",
                ),
                ("DORO_CONTROL_PLANE_OPENAI_DEFAULT_CHAT_MODEL", "chat-env"),
                (
                    "DORO_CONTROL_PLANE_OPENAI_DEFAULT_RESPONSE_MODEL",
                    "response-env",
                ),
                ("DORO_CONTROL_PLANE_OPENAI_TIMEOUT_SECONDS", "90"),
                ("DORO_CONTROL_PLANE_AI_AGENT_MAX_TURNS", "8"),
                ("DORO_CONTROL_PLANE_AI_AGENT_MAX_TOOL_CALLS", "16"),
                ("DORO_CONTROL_PLANE_AI_AGENT_TOOL_TIMEOUT_SECONDS", "11"),
                ("DORO_CONTROL_PLANE_AI_AGENT_SHELL_TIMEOUT_SECONDS", "22"),
                ("DORO_CONTROL_PLANE_AI_AGENT_APPROVAL_TIMEOUT_SECONDS", "33"),
            ]),
        )?;

        assert_eq!(loaded.path, None);
        assert_eq!(loaded.config.server.console_bind, "0.0.0.0:19087");
        assert_eq!(loaded.config.server.agent_bind, "0.0.0.0:19088");
        assert_eq!(loaded.config.store.backend, StoreBackend::Postgres);
        assert_eq!(
            loaded.config.store.database_url,
            "postgres://env:secret@db:5432/env"
        );
        assert_eq!(loaded.config.store.max_connections, 42);
        assert_eq!(loaded.config.store.min_connections, 4);
        assert_eq!(loaded.config.store.connect_timeout_seconds, 12);
        assert_eq!(loaded.config.store.idle_timeout_seconds, 600);
        assert_eq!(loaded.config.security.approval_policy, "policy_only");
        assert!(loaded.config.security.require_tls);
        assert_eq!(
            loaded.config.security.jwt_secret.as_deref(),
            Some("jwt-from-env")
        );
        assert_eq!(loaded.config.ai.provider, "openai");
        assert_eq!(loaded.config.ai.openai.api_key_env, "DORO_OPENAI_SECRET");
        assert_eq!(
            loaded.config.ai.openai.base_url,
            "https://api.example.test/v1"
        );
        assert_eq!(loaded.config.ai.openai.default_chat_model, "chat-env");
        assert_eq!(
            loaded.config.ai.openai.default_response_model,
            "response-env"
        );
        assert_eq!(loaded.config.ai.openai.timeout_seconds, 90);
        assert_eq!(loaded.config.ai.agent.max_turns, 8);
        assert_eq!(loaded.config.ai.agent.max_tool_calls, 16);
        assert_eq!(loaded.config.ai.agent.tool_timeout_seconds, 11);
        assert_eq!(loaded.config.ai.agent.shell_timeout_seconds, 22);
        assert_eq!(loaded.config.ai.agent.approval_timeout_seconds, 33);

        Ok(())
    }

    #[test]
    fn explicit_missing_control_plane_config_fails() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let path = dir.path().join("missing-control-plane.toml");

        let error = match load_control_plane_config_with_env(Some(&path), empty_env()) {
            Ok(_) => panic!("explicit missing config should fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("failed to read config"));
    }

    #[test]
    fn control_plane_environment_overrides_existing_file_values()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("control-plane.toml");
        fs::write(
            &path,
            r#"
                [server]
                console_bind = "127.0.0.1:9000"
            "#,
        )?;

        let loaded = load_control_plane_config_with_env(
            Some(&path),
            static_env(&[("DORO_CONTROL_PLANE_CONSOLE_BIND", "0.0.0.0:9000")]),
        )?;

        assert_eq!(loaded.path.as_deref(), Some(path.as_path()));
        assert_eq!(loaded.config.server.console_bind, "0.0.0.0:9000");

        Ok(())
    }

    #[test]
    fn invalid_control_plane_environment_value_fails() {
        let error = match load_control_plane_config_with_env(
            None,
            static_env(&[("DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS", "many")]),
        ) {
            Ok(_) => panic!("invalid env value should fail"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("invalid environment variable DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS")
        );
    }

    fn empty_env() -> impl FnMut(&'static str) -> Option<String> {
        |_| None
    }

    fn static_env<'a>(
        entries: &'a [(&'a str, &'a str)],
    ) -> impl FnMut(&'static str) -> Option<String> + 'a {
        move |name| {
            entries
                .iter()
                .find(|(entry_name, _)| *entry_name == name)
                .map(|(_, value)| (*value).to_string())
        }
    }
}
