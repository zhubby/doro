use chrono::Utc;
use collectors::CollectorConfig;
use collectors::CollectorEvent;
use collectors::LocalCollectors;
use doro_protocol::AgentCapability;
use doro_protocol::AgentEvent;
use doro_protocol::CapabilityName;
use doro_protocol::CapabilityRisk;
use doro_protocol::Host;
use doro_protocol::HostStatus;
use doro_protocol::MetricSnapshot;
use doro_protocol::PROTOCOL_VERSION;
use doro_protocol::grpc;
use doro_protocol::grpc::agent_control_plane_client::AgentControlPlaneClient;
use doro_protocol::protobuf_timestamp_now;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use uuid::Uuid;

mod collectors;

const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(2);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub agent_id: Option<Uuid>,
    pub host_id: Uuid,
    pub hostname: String,
    pub control_plane_url: String,
    pub enrollment_token: Option<String>,
    pub heartbeat_interval: Duration,
    pub metrics_enabled: bool,
    pub metrics_interval: Duration,
    pub process_names: Vec<String>,
    pub container_metrics_enabled: bool,
    pub docker_socket_path: Option<String>,
    pub gpu_metrics_enabled: bool,
}

impl AgentConfig {
    pub fn local(control_plane_url: impl Into<String>) -> Self {
        Self::new("doro-local-agent", control_plane_url)
    }

    pub fn new(hostname: impl Into<String>, control_plane_url: impl Into<String>) -> Self {
        Self {
            agent_id: None,
            host_id: Uuid::new_v4(),
            hostname: hostname.into(),
            control_plane_url: control_plane_url.into(),
            enrollment_token: None,
            heartbeat_interval: Duration::from_secs(30),
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(10),
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            gpu_metrics_enabled: false,
        }
    }

    pub fn from_config(config: &doro_config::AgentConfig) -> Self {
        Self {
            agent_id: config.agent_id,
            host_id: config.host_id.unwrap_or_else(Uuid::new_v4),
            hostname: config.hostname.clone(),
            control_plane_url: config.control_plane_url.clone(),
            enrollment_token: config.enrollment_token.clone(),
            heartbeat_interval: Duration::from_secs(config.heartbeat_interval_seconds.max(1)),
            metrics_enabled: config.metrics_enabled,
            metrics_interval: Duration::from_secs(config.metrics_interval_seconds.max(1)),
            process_names: config.process_names.clone(),
            container_metrics_enabled: config.container_metrics_enabled,
            docker_socket_path: config.docker_socket_path.clone(),
            gpu_metrics_enabled: config.gpu_metrics_enabled,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Agent {
    config: AgentConfig,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    pub fn host(&self) -> Host {
        Host {
            id: self.config.host_id,
            hostname: self.config.hostname.clone(),
            labels: vec!["agent".to_string()],
            status: HostStatus::Online,
            last_seen_at: Some(Utc::now()),
            capabilities: self.capabilities(),
        }
    }

    pub fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability {
                name: CapabilityName::MetricsRead,
                risk: CapabilityRisk::Low,
                description: "Collect local host metrics".to_string(),
            },
            AgentCapability {
                name: CapabilityName::LogsRead,
                risk: CapabilityRisk::Low,
                description: "Read local service logs".to_string(),
            },
            AgentCapability {
                name: CapabilityName::ShellExecute,
                risk: CapabilityRisk::High,
                description: "Execute approved shell commands".to_string(),
            },
        ]
    }

    pub fn grpc_capabilities(&self) -> Vec<grpc::AgentCapability> {
        self.capabilities()
            .into_iter()
            .map(|capability| grpc::AgentCapability {
                name: format!("{:?}", capability.name),
                risk: format!("{:?}", capability.risk),
                description: capability.description,
            })
            .collect()
    }

    pub fn grpc_heartbeat(&self, agent_id: Uuid) -> grpc::HeartbeatRequest {
        grpc::HeartbeatRequest {
            agent_id: agent_id.to_string(),
            host_id: self.config.host_id.to_string(),
            observed_at: Some(protobuf_timestamp_now()),
            capabilities: self.grpc_capabilities(),
        }
    }

    pub fn grpc_enroll(&self, enrollment_token: String) -> grpc::EnrollRequest {
        grpc::EnrollRequest {
            enrollment_token,
            hostname: self.config.hostname.clone(),
            capabilities: self.grpc_capabilities(),
        }
    }

    pub fn grpc_event(
        &self,
        agent_id: Uuid,
        kind: &str,
        payload: serde_json::Value,
    ) -> grpc::AgentEvent {
        grpc::AgentEvent {
            event_id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            host_id: self.config.host_id.to_string(),
            kind: kind.to_string(),
            payload_json: payload.to_string(),
            recorded_at: Some(protobuf_timestamp_now()),
        }
    }

    pub fn heartbeat(&self) -> AgentEvent {
        AgentEvent::Heartbeat {
            host_id: self.config.host_id,
            at: Utc::now(),
        }
    }

    pub fn metrics(&self) -> MetricSnapshot {
        MetricSnapshot {
            host_id: self.config.host_id,
            captured_at: Utc::now(),
            cpu_percent: 0.0,
            memory_percent: 0.0,
            disk_percent: 0.0,
            load_average: 0.0,
            extra: serde_json::json!({}),
        }
    }
}

pub async fn run(loaded_config: doro_config::LoadedConfig) -> anyhow::Result<()> {
    let mut persisted_config = loaded_config.config;
    let mut agent = Agent::new(AgentConfig::from_config(&persisted_config.agent));
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
    persisted_config: &mut doro_config::DoroConfig,
    agent: &mut Agent,
    shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let mut client =
        AgentControlPlaneClient::connect(agent.config.control_plane_url.clone()).await?;
    let agent_id = ensure_registered(client.clone(), persisted_config, config_path, agent).await?;

    report_heartbeat(&mut client, agent, agent_id).await?;
    open_agent_stream(client, agent.clone(), agent_id, shutdown_rx).await
}

async fn ensure_registered(
    mut client: AgentControlPlaneClient<Channel>,
    persisted_config: &mut doro_config::DoroConfig,
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
    doro_config::write_config(config_path, persisted_config)?;
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
    sender
        .send(agent.grpc_event(
            agent_id,
            "connected",
            serde_json::json!({
                "protocol_version": PROTOCOL_VERSION,
                "hostname": agent.config.hostname
            }),
        ))
        .await?;
    tracing::debug!(agent_id = %agent_id, "queued agent connected event");

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
            let event = heartbeat_agent.grpc_event(
                agent_id,
                "heartbeat",
                serde_json::json!({
                    "protocol_version": PROTOCOL_VERSION
                }),
            );
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
                    let (kind, payload) = match collector_event {
                        CollectorEvent::Metrics(metrics) => (
                            "metrics.snapshot",
                            serde_json::json!({
                                "host_id": metrics.snapshot.host_id,
                                "captured_at": metrics.snapshot.captured_at,
                                "cpu_percent": metrics.snapshot.cpu_percent,
                                "memory_percent": metrics.snapshot.memory_percent,
                                "disk_percent": metrics.snapshot.disk_percent,
                                "load_average": metrics.snapshot.load_average,
                                "extra": metrics.extra,
                            }),
                        ),
                        CollectorEvent::Containers(payload) => ("container.snapshot", payload),
                        CollectorEvent::Error { collector, message } => (
                            "metrics.collector_error",
                            serde_json::json!({
                                "collector": collector,
                                "message": message,
                            }),
                        ),
                    };
                    tracing::debug!(
                        agent_id = %agent_id,
                        host_id = %metrics_agent.config.host_id,
                        kind,
                        "queued telemetry event"
                    );
                    if metrics_sender
                        .send(metrics_agent.grpc_event(agent_id, kind, payload))
                        .await
                        .is_err()
                    {
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
    tracing::debug!(agent_id = %agent_id, "agent stream opened");
    loop {
        tokio::select! {
            command = commands.message() => {
                let Some(command) = command? else {
                    anyhow::bail!("agent stream closed");
                };
                handle_command(command);
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

fn handle_command(command: grpc::ControlPlaneCommand) {
    match command.kind.as_str() {
        "ack" => {
            tracing::info!(command_id = %command.command_id, "control-plane acknowledged stream")
        }
        kind => {
            tracing::warn!(command_id = %command.command_id, kind, "unsupported control-plane command")
        }
    }
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
    fn agent_config_uses_persisted_identity() {
        let agent_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let config = doro_config::AgentConfig {
            agent_id: Some(agent_id),
            host_id: Some(host_id),
            heartbeat_interval_seconds: 0,
            ..Default::default()
        };

        let agent_config = AgentConfig::from_config(&config);

        assert_eq!(agent_config.agent_id, Some(agent_id));
        assert_eq!(agent_config.host_id, host_id);
        assert_eq!(agent_config.heartbeat_interval, Duration::from_secs(1));
    }

    #[test]
    fn grpc_event_includes_durable_identity_and_payload() {
        let agent_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let agent = Agent::new(AgentConfig {
            agent_id: Some(agent_id),
            host_id,
            hostname: "doro-test".to_string(),
            control_plane_url: "http://127.0.0.1:8788".to_string(),
            enrollment_token: None,
            heartbeat_interval: Duration::from_secs(30),
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(10),
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            gpu_metrics_enabled: false,
        });

        let event = agent.grpc_event(
            agent_id,
            "connected",
            serde_json::json!({"protocol_version": PROTOCOL_VERSION}),
        );

        assert_eq!(event.agent_id, agent_id.to_string());
        assert_eq!(event.host_id, host_id.to_string());
        assert_eq!(event.kind, "connected");
        assert!(event.payload_json.contains(PROTOCOL_VERSION));
    }

    #[test]
    fn handle_command_accepts_ack_and_unknown_commands() {
        handle_command(grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            kind: "ack".to_string(),
            payload_json: "{}".to_string(),
            requires_approval: false,
        });
        handle_command(grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            kind: "future_command".to_string(),
            payload_json: "{}".to_string(),
            requires_approval: false,
        });
    }

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
