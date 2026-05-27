use clap::Parser;
use doro_agent::Agent;
use doro_agent::AgentConfig;

#[derive(Debug, Parser)]
#[command(author, version, about = "Doro host agent")]
struct Cli {
    #[arg(long, default_value = "ws://127.0.0.1:8787/api/v1/agent/connect")]
    control_plane_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let agent = Agent::new(AgentConfig::local(cli.control_plane_url));

    println!("{}", serde_json::to_string_pretty(&agent.host())?);
    println!("{}", serde_json::to_string_pretty(&agent.heartbeat())?);

    Ok(())
}
