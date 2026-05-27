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
    Agent,
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
        Command::Agent => {
            init_logging(cli.log_level)?;
            doro_agent::run(loaded_config.config.agent).await?;
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
            println!("{}", serde_json::to_string_pretty(&token)?);
        }
    }

    Ok(())
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

        assert!(matches!(cli.command, Command::Agent));
        assert!(cli.log_level.is_none());
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
