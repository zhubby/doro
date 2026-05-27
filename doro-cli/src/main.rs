use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(author, version, about = "Doro home-server control-plane CLI")]
struct Cli {
    /// Path to the config file. Defaults to ~/.doro/config.toml.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Log level for service commands. Defaults to info unless RUST_LOG is set.
    #[arg(long, global = true, value_enum, value_name = "LEVEL")]
    log_level: Option<LogLevel>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the local host agent.
    Agent {
        /// Control-plane gRPC URL, for example http://127.0.0.1:8788.
        #[arg(long, value_name = "URL")]
        control_plane_url: Option<String>,
        /// Hostname to declare during enrollment and heartbeats.
        #[arg(long, value_name = "NAME")]
        hostname: Option<String>,
        /// One-time enrollment token printed by `doro enrollment-token`.
        #[arg(long, value_name = "TOKEN")]
        enrollment_token: Option<String>,
        /// Previously enrolled agent id.
        #[arg(long, value_name = "UUID")]
        agent_id: Option<uuid::Uuid>,
        /// Previously enrolled host id.
        #[arg(long, value_name = "UUID")]
        host_id: Option<uuid::Uuid>,
        /// Heartbeat interval in seconds.
        #[arg(long, value_name = "SECONDS")]
        heartbeat_interval_seconds: Option<u64>,
    },
    /// Run the control-plane HTTP API and Agent gRPC service.
    ControlPlane,
    /// Print local development status and configured project surfaces.
    Status,
    /// Generate a one-time token label for enrolling a new host agent.
    EnrollmentToken {
        /// Human readable token label.
        #[arg(default_value = "local-agent")]
        label: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let loaded_config = doro_config::load_or_create(cli.config.as_deref())?;

    match cli.command {
        Command::Agent {
            control_plane_url,
            hostname,
            enrollment_token,
            agent_id,
            host_id,
            heartbeat_interval_seconds,
        } => {
            init_logging(cli.log_level)?;
            doro_agent::run(apply_agent_overrides(
                loaded_config,
                AgentOverrides {
                    control_plane_url,
                    hostname,
                    enrollment_token,
                    agent_id,
                    host_id,
                    heartbeat_interval_seconds,
                },
            ))
            .await?;
        }
        Command::ControlPlane => {
            init_logging(cli.log_level)?;
            doro_control_plane::run(loaded_config.config).await?;
        }
        Command::Status => {
            println!("Doro control plane workspace");
            println!("config: {}", loaded_config.path.display());
            if loaded_config.created {
                println!("config_created: true");
            }
            println!("api: /api/v1");
            println!("http: {}", loaded_config.config.server.http_bind);
            println!("grpc: {}", loaded_config.config.server.grpc_bind);
            println!("store backend: {}", loaded_config.config.store.backend);
            println!(
                "store database_url: {}",
                redact_database_url(&loaded_config.config.store.database_url)
            );
            println!(
                "store pool: min={} max={}",
                loaded_config.config.store.min_connections,
                loaded_config.config.store.max_connections
            );
            println!(
                "agent transport: grpc doro.agent.v1.AgentControlPlane on {}",
                loaded_config.config.server.grpc_bind
            );
            println!(
                "agent default: {}",
                loaded_config.config.agent.control_plane_url
            );
            println!("docs: docs/");
        }
        Command::EnrollmentToken { label } => {
            let token = doro_cli::generate_enrollment_token(label);
            let store = doro_store::Store::connect_with_config(&loaded_config.config.store).await?;
            store.migrate().await?;
            store
                .enrollment_tokens()
                .create(doro_store::NewEnrollmentToken {
                    id: token.id,
                    label: token.label.clone(),
                    token: token.token.clone(),
                    expires_at: None,
                    created_at: chrono::Utc::now(),
                })
                .await?;
            println!("{}", serde_json::to_string_pretty(&token)?);
        }
    }

    Ok(())
}

#[derive(Debug, Default)]
struct AgentOverrides {
    control_plane_url: Option<String>,
    hostname: Option<String>,
    enrollment_token: Option<String>,
    agent_id: Option<uuid::Uuid>,
    host_id: Option<uuid::Uuid>,
    heartbeat_interval_seconds: Option<u64>,
}

fn apply_agent_overrides(
    mut loaded_config: doro_config::LoadedConfig,
    overrides: AgentOverrides,
) -> doro_config::LoadedConfig {
    if let Some(control_plane_url) = overrides.control_plane_url {
        loaded_config.config.agent.control_plane_url = control_plane_url;
    }
    if let Some(hostname) = overrides.hostname {
        loaded_config.config.agent.hostname = hostname;
    }
    if let Some(enrollment_token) = overrides.enrollment_token {
        loaded_config.config.agent.enrollment_token = Some(enrollment_token);
    }
    if let Some(agent_id) = overrides.agent_id {
        loaded_config.config.agent.agent_id = Some(agent_id);
    }
    if let Some(host_id) = overrides.host_id {
        loaded_config.config.agent.host_id = Some(host_id);
    }
    if let Some(heartbeat_interval_seconds) = overrides.heartbeat_interval_seconds {
        loaded_config.config.agent.heartbeat_interval_seconds = heartbeat_interval_seconds;
    }

    loaded_config
}

fn init_logging(log_level: Option<LogLevel>) -> anyhow::Result<()> {
    let env_filter = match log_level {
        Some(log_level) => tracing_subscriber::EnvFilter::try_new(default_log_filter(log_level))?,
        None => tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| default_log_filter(LogLevel::Info).into()),
    };

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize logging: {error}"))?;

    Ok(())
}

fn default_log_filter(log_level: LogLevel) -> String {
    let level = log_level.as_str();
    format!("doro_cli={level},doro_agent={level},doro_control_plane={level},tower_http={level}")
}

fn redact_database_url(database_url: &str) -> String {
    let Some((scheme, rest)) = database_url.split_once("://") else {
        return "<configured>".to_string();
    };
    let Some((authority, address)) = rest.split_once('@') else {
        return database_url.to_string();
    };
    if !authority.contains(':') {
        return database_url.to_string();
    }

    format!("{scheme}://<redacted>@{address}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agent_subcommand() {
        let cli = match Cli::try_parse_from(["doro", "agent"]) {
            Ok(cli) => cli,
            Err(error) => panic!("agent command should parse: {error}"),
        };

        assert!(matches!(cli.command, Command::Agent { .. }));
        assert!(cli.log_level.is_none());
    }

    #[test]
    fn parses_agent_cli_overrides() {
        let agent_id = uuid::Uuid::new_v4();
        let host_id = uuid::Uuid::new_v4();
        let cli = match Cli::try_parse_from([
            "doro",
            "agent",
            "--control-plane-url",
            "http://127.0.0.1:9001",
            "--hostname",
            "edge-node",
            "--enrollment-token",
            "token-value",
            "--agent-id",
            &agent_id.to_string(),
            "--host-id",
            &host_id.to_string(),
            "--heartbeat-interval-seconds",
            "15",
        ]) {
            Ok(cli) => cli,
            Err(error) => panic!("agent command with overrides should parse: {error}"),
        };

        let Command::Agent {
            control_plane_url,
            hostname,
            enrollment_token,
            agent_id: parsed_agent_id,
            host_id: parsed_host_id,
            heartbeat_interval_seconds,
        } = cli.command
        else {
            panic!("expected agent command");
        };

        assert_eq!(control_plane_url.as_deref(), Some("http://127.0.0.1:9001"));
        assert_eq!(hostname.as_deref(), Some("edge-node"));
        assert_eq!(enrollment_token.as_deref(), Some("token-value"));
        assert_eq!(parsed_agent_id, Some(agent_id));
        assert_eq!(parsed_host_id, Some(host_id));
        assert_eq!(heartbeat_interval_seconds, Some(15));
    }

    #[test]
    fn applies_agent_cli_overrides_to_loaded_config() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.toml");
        let agent_id = uuid::Uuid::new_v4();
        let host_id = uuid::Uuid::new_v4();
        let loaded_config = doro_config::LoadedConfig {
            path,
            config: doro_config::DoroConfig::default(),
            created: false,
        };

        let loaded_config = apply_agent_overrides(
            loaded_config,
            AgentOverrides {
                control_plane_url: Some("http://control-plane:8788".to_string()),
                hostname: Some("edge-node".to_string()),
                enrollment_token: Some("token-value".to_string()),
                agent_id: Some(agent_id),
                host_id: Some(host_id),
                heartbeat_interval_seconds: Some(15),
            },
        );

        assert_eq!(
            loaded_config.config.agent.control_plane_url,
            "http://control-plane:8788"
        );
        assert_eq!(loaded_config.config.agent.hostname, "edge-node");
        assert_eq!(
            loaded_config.config.agent.enrollment_token.as_deref(),
            Some("token-value")
        );
        assert_eq!(loaded_config.config.agent.agent_id, Some(agent_id));
        assert_eq!(loaded_config.config.agent.host_id, Some(host_id));
        assert_eq!(loaded_config.config.agent.heartbeat_interval_seconds, 15);

        Ok(())
    }

    #[test]
    fn parses_control_plane_subcommand_with_log_level() {
        let cli = match Cli::try_parse_from(["doro", "--log-level", "debug", "control-plane"]) {
            Ok(cli) => cli,
            Err(error) => panic!("control-plane command should parse: {error}"),
        };

        assert!(matches!(cli.command, Command::ControlPlane));
        assert!(matches!(cli.log_level, Some(LogLevel::Debug)));
    }

    #[test]
    fn rejects_unknown_log_level() {
        assert!(Cli::try_parse_from(["doro", "--log-level", "verbose", "status"]).is_err());
    }

    #[test]
    fn default_log_filter_targets_doro_services() {
        assert_eq!(
            default_log_filter(LogLevel::Info),
            "doro_cli=info,doro_agent=info,doro_control_plane=info,tower_http=info"
        );
    }

    #[test]
    fn redact_database_url_hides_password() {
        assert_eq!(
            redact_database_url("postgres://doro:secret@127.0.0.1:5432/doro"),
            "postgres://<redacted>@127.0.0.1:5432/doro"
        );
    }

    #[test]
    fn redact_database_url_keeps_passwordless_url() {
        assert_eq!(
            redact_database_url("postgres://127.0.0.1:5432/doro"),
            "postgres://127.0.0.1:5432/doro"
        );
    }
}
