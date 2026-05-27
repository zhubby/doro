use clap::Parser;
use clap::Subcommand;

#[derive(Debug, Parser)]
#[command(author, version, about = "Doro home-server control-plane CLI")]
struct Cli {
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

    match cli.command {
        Command::Status => {
            println!("Doro control plane workspace");
            println!("api: /api/v1");
            println!("agent transport: grpc doro.agent.v1.AgentControlPlane on 127.0.0.1:8788");
            println!("docs: docs/");
        }
        Command::EnrollmentToken { label } => {
            let token = doro_cli::generate_enrollment_token(label);
            println!("{}", serde_json::to_string_pretty(&token)?);
        }
    }

    Ok(())
}
