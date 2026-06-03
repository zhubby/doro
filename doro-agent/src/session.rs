use crate::collectors::{CollectorConfig, CollectorEvent, LocalCollectors};
use crate::commands::{AgentCommandAction, handle_command};
use crate::config::AgentConfig;
use crate::constants::{INITIAL_RECONNECT_DELAY, MAX_RECONNECT_DELAY};
use crate::logs::runtime_log_subscription;
use crate::runtime::Agent;
use crate::terminal::TerminalManager;
use crate::tools::AgentCommandState;
use doro_protocol::grpc::agent_control_plane_client::AgentControlPlaneClient;
use std::path::Path;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use uuid::Uuid;

pub async fn run(loaded_config: doro_config::LoadedAgentConfig) -> anyhow::Result<()> {
    let mut persisted_config = loaded_config.config;
    let mut agent = Agent::new(AgentConfig::from_file_config(&persisted_config));
    let _website_runtime_thread = agent.start_website_runtime()?;
    let mut reconnect_delay = INITIAL_RECONNECT_DELAY;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!("shutdown signal received, stopping agent");
        let _ = shutdown_tx.send(true);
    });

    loop {
        let session_result = tokio::select! {
            result = run_session(
                &loaded_config.path,
                &mut persisted_config,
                &mut agent,
                shutdown_rx.clone(),
            ) => result,
            () = wait_for_shutdown(shutdown_rx.clone()) => return Ok(()),
        };

        if shutdown_requested(&shutdown_rx) {
            return session_result;
        }

        match session_result {
            Ok(()) => {
                reconnect_delay = INITIAL_RECONNECT_DELAY;
                tracing::warn!(
                    delay_seconds = reconnect_delay.as_secs(),
                    "agent session ended; reconnecting"
                );
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    delay_seconds = reconnect_delay.as_secs(),
                    "agent session failed; reconnecting"
                );
            }
        }

        tokio::select! {
            () = tokio::time::sleep(reconnect_delay) => {}
            () = wait_for_shutdown(shutdown_rx.clone()) => return Ok(()),
        }
        reconnect_delay = next_reconnect_delay(reconnect_delay);
    }
}

async fn run_session(
    config_path: &Path,
    persisted_config: &mut doro_config::AgentFileConfig,
    agent: &mut Agent,
    shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let control_plane_url = agent.config.control_plane_url.clone();
    tracing::debug!(
        control_plane_url,
        "connecting to control-plane agent endpoint"
    );
    let mut client = AgentControlPlaneClient::connect(control_plane_url.clone()).await?;
    tracing::debug!(
        control_plane_url,
        "connected to control-plane agent endpoint"
    );
    let agent_id = ensure_registered(client.clone(), persisted_config, config_path, agent).await?;
    tracing::debug!(
        agent_id = %agent_id,
        host_id = %agent.config.host_id,
        "agent identity ready for control-plane session"
    );

    report_heartbeat(&mut client, agent, agent_id).await?;
    open_agent_stream(client, agent.clone(), agent_id, shutdown_rx).await
}

async fn ensure_registered(
    mut client: AgentControlPlaneClient<Channel>,
    persisted_config: &mut doro_config::AgentFileConfig,
    config_path: &Path,
    agent: &mut Agent,
) -> anyhow::Result<Uuid> {
    if let (Some(agent_id), Some(host_id)) = (
        persisted_config.agent.agent_id,
        persisted_config.agent.host_id,
    ) {
        agent.config.agent_id = Some(agent_id);
        agent.config.host_id = host_id;
        return Ok(agent_id);
    }

    let token = persisted_config
        .agent
        .enrollment_token
        .clone()
        .ok_or_else(|| anyhow::anyhow!("agent enrollment_token is required before first run"))?;
    let response = client.enroll(agent.grpc_enroll(token)).await?.into_inner();
    let agent_id = parse_uuid(&response.agent_id, "agent_id")?;
    let host_id = parse_uuid(&response.host_id, "host_id")?;

    persisted_config.agent.agent_id = Some(agent_id);
    persisted_config.agent.host_id = Some(host_id);
    doro_config::write_agent_config(config_path, persisted_config)?;
    agent.config.agent_id = Some(agent_id);
    agent.config.host_id = host_id;

    Ok(agent_id)
}

async fn report_heartbeat(
    client: &mut AgentControlPlaneClient<Channel>,
    agent: &Agent,
    agent_id: Uuid,
) -> anyhow::Result<()> {
    let response = client
        .report_heartbeat(agent.grpc_heartbeat(agent_id))
        .await?
        .into_inner();
    if !response.accepted {
        anyhow::bail!("control plane rejected heartbeat: {}", response.message);
    }
    Ok(())
}

async fn open_agent_stream(
    mut client: AgentControlPlaneClient<Channel>,
    agent: Agent,
    agent_id: Uuid,
    shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let (sender, receiver) = mpsc::channel(8);
    tracing::debug!(
        agent_id = %agent_id,
        host_id = %agent.config.host_id,
        hostname = %agent.config.hostname,
        "opening agent stream"
    );
    sender.send(agent.connected_event(agent_id)).await?;
    tracing::debug!(agent_id = %agent_id, "queued agent connected event");

    if let Some(mut runtime_logs) = runtime_log_subscription() {
        for log in runtime_logs.snapshot {
            if sender
                .send(agent.log_line_event(agent_id, log))
                .await
                .is_err()
            {
                break;
            }
        }

        let log_agent = agent.clone();
        let log_sender = sender.clone();
        let log_shutdown = shutdown_rx.clone();
        tokio::spawn(async move {
            loop {
                let log = tokio::select! {
                    log = runtime_logs.receiver.recv() => {
                        match log {
                            Ok(log) => log,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => return,
                        }
                    }
                    () = wait_for_shutdown(log_shutdown.clone()) => return,
                };
                if log_sender
                    .send(log_agent.log_line_event(agent_id, log))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        });
    }
    let heartbeat_agent = agent.clone();
    let heartbeat_sender = sender.clone();
    let heartbeat_shutdown = shutdown_rx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(heartbeat_agent.config.heartbeat_interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                () = wait_for_shutdown(heartbeat_shutdown.clone()) => break,
            }
            let event = heartbeat_agent.heartbeat_event(agent_id);
            if heartbeat_sender.send(event).await.is_err() {
                break;
            }
            tracing::debug!(agent_id = %agent_id, "queued heartbeat event");
        }
    });

    if agent.config.metrics_enabled {
        let metrics_agent = agent.clone();
        let metrics_sender = sender.clone();
        let metrics_shutdown = shutdown_rx.clone();
        tokio::spawn(async move {
            let collector_config = CollectorConfig {
                process_names: metrics_agent.config.process_names.clone(),
                container_metrics_enabled: metrics_agent.config.container_metrics_enabled,
                docker_socket_path: metrics_agent.config.docker_socket_path.clone(),
                gpu_metrics_enabled: metrics_agent.config.gpu_metrics_enabled,
            };
            let mut collectors = LocalCollectors::new(collector_config);
            let mut interval = tokio::time::interval(metrics_agent.config.metrics_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    () = wait_for_shutdown(metrics_shutdown.clone()) => return,
                }
                for collector_event in collectors.collect(metrics_agent.config.host_id).await {
                    let event = match collector_event {
                        CollectorEvent::Metrics(metrics) => {
                            metrics_agent.metrics_snapshot_event(agent_id, metrics)
                        }
                        CollectorEvent::Containers(snapshot) => metrics_agent
                            .container_snapshot_event(agent_id, String::new(), snapshot),
                        CollectorEvent::Error { collector, message } => metrics_agent
                            .collector_error_event(agent_id, String::new(), collector, message),
                    };
                    tracing::debug!(
                        agent_id = %agent_id,
                        host_id = %metrics_agent.config.host_id,
                        "queued telemetry event"
                    );
                    if metrics_sender.send(event).await.is_err() {
                        return;
                    }
                }
            }
        });
    }

    let mut commands = client
        .open_agent_stream(ReceiverStream::new(receiver))
        .await?
        .into_inner();
    let terminal = TerminalManager::new()?;
    let command_state = AgentCommandState::default();
    tracing::debug!(agent_id = %agent_id, "agent stream opened");
    loop {
        tokio::select! {
            command = commands.message() => {
                let Some(command) = command? else {
                    anyhow::bail!("agent stream closed");
                };
                if handle_command(
                    command,
                    &agent,
                    agent_id,
                    &sender,
                    &terminal,
                    &command_state,
                )
                .await
                    == AgentCommandAction::Reconnect
                {
                    return Ok(());
                }
            }
            () = wait_for_shutdown(shutdown_rx.clone()) => return Ok(()),
        }
    }
}

fn shutdown_requested(shutdown_rx: &watch::Receiver<bool>) -> bool {
    *shutdown_rx.borrow()
}

async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    while !*shutdown_rx.borrow_and_update() {
        if shutdown_rx.changed().await.is_err() {
            break;
        }
    }
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "failed to listen for ctrl-c shutdown signal");
        }
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::SignalKind;

        let terminate = async {
            match tokio::signal::unix::signal(SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to listen for terminate shutdown signal");
                    std::future::pending::<()>().await;
                }
            }
        };

        tokio::select! {
            () = ctrl_c => {}
            () = terminate => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}

fn next_reconnect_delay(current: Duration) -> Duration {
    (current * 2).min(MAX_RECONNECT_DELAY)
}

fn parse_uuid(value: &str, field: &str) -> anyhow::Result<Uuid> {
    value
        .parse()
        .map_err(|error| anyhow::anyhow!("{field} must be a uuid: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_delay_backs_off_to_cap() {
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(2)),
            Duration::from_secs(4)
        );
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(20)),
            Duration::from_secs(30)
        );
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(30)),
            Duration::from_secs(30)
        );
    }
}
