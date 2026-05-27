use clap::Parser;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(author, version, about = "Doro home-server control-plane CLI")]
struct Cli {
    /// Path to the config file. Defaults to ~/.doro/config.toml.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print local development status and configured project surfaces.
    Status,
    /// Generate a one-time token label for enrolling a new host agent.
    EnrollmentToken {
        /// Human readable token label.
        #[arg(default_value = "local-agent")]
        label: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let loaded_config = doro_config::load_or_create(cli.config.as_deref())?;

    match cli.command {
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
