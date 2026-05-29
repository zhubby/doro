use chrono::Utc;
use collectors::CollectorConfig;
use collectors::CollectorEvent;
use collectors::LocalCollectors;
use collectors::MetricsCapture;
use collectors::system_profile;
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
use doro_protocol::protobuf_timestamp_from_utc;
use doro_protocol::protobuf_timestamp_now;
use std::path::Path;
use std::time::Duration;
use terminal::TerminalCommand;
use terminal::TerminalManager;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use uuid::Uuid;

mod collectors;
pub mod docker;
mod terminal;

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
    pub docker_manage_enabled: bool,
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
            docker_manage_enabled: false,
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
            docker_manage_enabled: config.docker_manage_enabled,
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
            display_name: self.config.hostname.clone(),
            labels: vec!["agent".to_string()],
            status: HostStatus::Online,
            last_seen_at: Some(Utc::now()),
            capabilities: self.capabilities(),
            system_profile: serde_json::json!({}),
        }
    }

    pub fn capabilities(&self) -> Vec<AgentCapability> {
        let mut capabilities = vec![
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
        ];
        if self.config.docker_manage_enabled {
            capabilities.push(AgentCapability {
                name: CapabilityName::ContainersManage,
                risk: CapabilityRisk::High,
                description:
                    "Manage Docker images, containers, networks, and volumes after approval"
                        .to_string(),
            });
        }
        capabilities
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
            system_profile_json: system_profile().to_string(),
        }
    }

    fn grpc_event(&self, agent_id: Uuid, event: grpc::agent_event::Event) -> grpc::AgentEvent {
        grpc::AgentEvent {
            event_id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            host_id: self.config.host_id.to_string(),
            recorded_at: Some(protobuf_timestamp_now()),
            event: Some(event),
        }
    }

    pub fn connected_event(&self, agent_id: Uuid) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::Connected(grpc::ConnectedEvent {
                protocol_version: PROTOCOL_VERSION.to_string(),
                hostname: self.config.hostname.clone(),
            }),
        )
    }

    pub fn heartbeat_event(&self, agent_id: Uuid) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::Heartbeat(grpc::HeartbeatEvent {
                protocol_version: PROTOCOL_VERSION.to_string(),
            }),
        )
    }

    pub fn metrics_snapshot_event(
        &self,
        agent_id: Uuid,
        metrics: MetricsCapture,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::MetricsSnapshot(grpc::MetricsSnapshotEvent {
                host_id: metrics.snapshot.host_id.to_string(),
                captured_at: Some(protobuf_timestamp_from_utc(metrics.snapshot.captured_at)),
                cpu_percent: metrics.snapshot.cpu_percent,
                memory_percent: metrics.snapshot.memory_percent,
                disk_percent: metrics.snapshot.disk_percent,
                load_average: metrics.snapshot.load_average,
                extra_json: metrics.extra.to_string(),
            }),
        )
    }

    pub fn container_snapshot_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        payload: serde_json::Value,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::ContainerSnapshot(container_snapshot_from_payload(
                command_id, payload,
            )),
        )
    }

    pub fn collector_error_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        collector: impl Into<String>,
        message: impl Into<String>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::CollectorError(grpc::CollectorErrorEvent {
                command_id,
                collector: collector.into(),
                message: message.into(),
            }),
        )
    }

    pub fn command_result_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        status: grpc::CommandStatus,
        message: impl Into<String>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::CommandResult(grpc::CommandResultEvent {
                command_id,
                status: status as i32,
                message: message.into(),
            }),
        )
    }

    pub fn terminal_command_result_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        output: terminal::TerminalCommandOutput,
    ) -> grpc::AgentEvent {
        let status = if output.exit_code == Some(0) && !output.timed_out {
            grpc::CommandStatus::Succeeded
        } else {
            grpc::CommandStatus::Failed
        };
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::TerminalCommandResult(grpc::TerminalCommandResultEvent {
                command_id,
                status: status as i32,
                output: output.output,
                exit_code: output.exit_code.unwrap_or(-1),
                started_at: Some(protobuf_timestamp_from_utc(output.started_at)),
                finished_at: Some(protobuf_timestamp_from_utc(output.finished_at)),
            }),
        )
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

pub async fn run(loaded_config: doro_config::LoadedAgentConfig) -> anyhow::Result<()> {
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
                        CollectorEvent::Containers(payload) => {
                            metrics_agent.container_snapshot_event(agent_id, String::new(), payload)
                        }
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
    tracing::debug!(agent_id = %agent_id, "agent stream opened");
    loop {
        tokio::select! {
            command = commands.message() => {
                let Some(command) = command? else {
                    anyhow::bail!("agent stream closed");
                };
                if handle_command(command, &agent, agent_id, &sender, &terminal).await
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

fn container_snapshot_from_payload(
    command_id: String,
    payload: serde_json::Value,
) -> grpc::ContainerSnapshotEvent {
    let runtime = payload
        .get("runtime")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("docker")
        .to_string();
    let containers = payload
        .get("containers")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(container_observation_from_json)
        .collect();
    let extra_json = serde_json::json!({
        "daemon": payload.get("daemon").cloned().unwrap_or(serde_json::Value::Null),
        "networks": payload.get("networks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "volumes": payload.get("volumes").cloned().unwrap_or_else(|| serde_json::json!([])),
    })
    .to_string();

    grpc::ContainerSnapshotEvent {
        command_id,
        runtime,
        containers,
        extra_json,
    }
}

fn container_observation_from_json(container: &serde_json::Value) -> grpc::ContainerObservation {
    grpc::ContainerObservation {
        id: container
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        names: container
            .get("names")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(ToString::to_string)
            .collect(),
        image: container
            .get("image")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        image_id: container
            .get("image_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        command: container
            .get("command")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        created: container
            .get("created")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or_default(),
        ports_json: container
            .get("ports")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]))
            .to_string(),
        labels_json: container
            .get("labels")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}))
            .to_string(),
        state: container
            .get("state")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        status: container
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

fn next_reconnect_delay(current: Duration) -> Duration {
    (current * 2).min(MAX_RECONNECT_DELAY)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentCommandAction {
    Continue,
    Reconnect,
}

async fn handle_command(
    command: grpc::ControlPlaneCommand,
    agent: &Agent,
    agent_id: Uuid,
    sender: &mpsc::Sender<grpc::AgentEvent>,
    terminal: &TerminalManager,
) -> AgentCommandAction {
    let command_id = command.command_id.clone();
    match command.command {
        Some(grpc::control_plane_command::Command::Ack(_)) => {
            tracing::info!(command_id = %command_id, "control-plane acknowledged stream")
        }
        Some(grpc::control_plane_command::Command::CollectContainers(_)) => {
            tracing::info!(command_id = %command_id, "collecting containers by control-plane request");
            let event =
                match docker::collect_snapshot(agent.config.docker_socket_path.as_deref()).await {
                    Ok(payload) => agent.container_snapshot_event(agent_id, command_id, payload),
                    Err(error) => agent.command_result_event(
                        agent_id,
                        command_id,
                        grpc::CommandStatus::Failed,
                        error.to_string(),
                    ),
                };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue command result event");
            }
        }
        Some(grpc::control_plane_command::Command::RunTerminalCommand(terminal_command)) => {
            tracing::info!(command_id = %command_id, "executing terminal command by control-plane request");
            let event = match terminal
                .execute(TerminalCommand {
                    command_id: command_id.clone(),
                    input: terminal_command.input,
                    cols: terminal_command.cols.clamp(20, 300) as u16,
                    rows: terminal_command.rows.clamp(5, 120) as u16,
                    timeout: Duration::from_secs(
                        terminal_command.timeout_seconds.clamp(1, 120) as u64
                    ),
                })
                .await
            {
                Ok(output) => agent.terminal_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue terminal command result event");
            }
        }
        Some(grpc::control_plane_command::Command::Shutdown(shutdown)) => {
            tracing::info!(
                command_id = %command_id,
                reason = shutdown.reason,
                "control-plane requested agent stream reconnect"
            );
            return AgentCommandAction::Reconnect;
        }
        None => {
            tracing::warn!(command_id = %command_id, "control-plane command missing typed payload")
        }
    }
    AgentCommandAction::Continue
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
            docker_manage_enabled: false,
            gpu_metrics_enabled: false,
        });

        let event = agent.connected_event(agent_id);

        assert_eq!(event.agent_id, agent_id.to_string());
        assert_eq!(event.host_id, host_id.to_string());
        let Some(grpc::agent_event::Event::Connected(connected)) = event.event else {
            panic!("connected event should use typed payload");
        };
        assert_eq!(connected.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn container_snapshot_event_preserves_command_id() {
        let agent_id = Uuid::new_v4();
        let command_id = Uuid::new_v4().to_string();
        let agent = Agent::new(AgentConfig {
            agent_id: Some(agent_id),
            host_id: Uuid::new_v4(),
            hostname: "doro-test".to_string(),
            control_plane_url: "http://127.0.0.1:8788".to_string(),
            enrollment_token: None,
            heartbeat_interval: Duration::from_secs(30),
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(10),
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            docker_manage_enabled: false,
            gpu_metrics_enabled: false,
        });

        let event = agent.container_snapshot_event(
            agent_id,
            command_id.clone(),
            serde_json::json!({
                "runtime": "docker",
                "containers": [{"id": "abc", "names": ["/db"], "image": "postgres"}]
            }),
        );

        let Some(grpc::agent_event::Event::ContainerSnapshot(snapshot)) = event.event else {
            panic!("container event should use typed payload");
        };
        assert_eq!(snapshot.command_id, command_id);
        assert_eq!(snapshot.containers.len(), 1);
        assert_eq!(snapshot.containers[0].id, "abc");
    }

    #[tokio::test]
    async fn handle_command_continues_for_ack() {
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(AgentConfig {
            agent_id: Some(agent_id),
            host_id: Uuid::new_v4(),
            hostname: "doro-test".to_string(),
            control_plane_url: "http://127.0.0.1:8788".to_string(),
            enrollment_token: None,
            heartbeat_interval: Duration::from_secs(30),
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(10),
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            docker_manage_enabled: false,
            gpu_metrics_enabled: false,
        });
        let (sender, _receiver) = mpsc::channel(1);
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: None,
            command: Some(grpc::control_plane_command::Command::Ack(
                grpc::AckCommand {
                    message: "connected".to_string(),
                },
            )),
        };

        let terminal = TerminalManager::new().expect("terminal should start");
        let action = handle_command(command, &agent, agent_id, &sender, &terminal).await;

        assert_eq!(action, AgentCommandAction::Continue);
    }

    #[tokio::test]
    async fn handle_command_returns_reconnect_for_shutdown_command() {
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(AgentConfig {
            agent_id: Some(agent_id),
            host_id: Uuid::new_v4(),
            hostname: "doro-test".to_string(),
            control_plane_url: "http://127.0.0.1:8788".to_string(),
            enrollment_token: None,
            heartbeat_interval: Duration::from_secs(30),
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(10),
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            docker_manage_enabled: false,
            gpu_metrics_enabled: false,
        });
        let (sender, _receiver) = mpsc::channel(1);
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: None,
            command: Some(grpc::control_plane_command::Command::Shutdown(
                grpc::ShutdownCommand {
                    reason: "control-plane shutting down".to_string(),
                },
            )),
        };

        let terminal = TerminalManager::new().expect("terminal should start");
        let action = handle_command(command, &agent, agent_id, &sender, &terminal).await;

        assert_eq!(action, AgentCommandAction::Reconnect);
    }

    #[test]
    fn docker_manage_capability_is_declared_only_when_enabled() {
        let base_config = AgentConfig {
            agent_id: Some(Uuid::new_v4()),
            host_id: Uuid::new_v4(),
            hostname: "doro-test".to_string(),
            control_plane_url: "http://127.0.0.1:8788".to_string(),
            enrollment_token: None,
            heartbeat_interval: Duration::from_secs(30),
            metrics_enabled: true,
            metrics_interval: Duration::from_secs(10),
            process_names: Vec::new(),
            container_metrics_enabled: false,
            docker_socket_path: None,
            docker_manage_enabled: false,
            gpu_metrics_enabled: false,
        };
        let agent = Agent::new(base_config.clone());

        assert!(
            !agent
                .capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::ContainersManage)
        );

        let agent = Agent::new(AgentConfig {
            docker_manage_enabled: true,
            ..base_config
        });

        assert!(agent.capabilities().iter().any(|capability| {
            capability.name == CapabilityName::ContainersManage
                && capability.risk == CapabilityRisk::High
        }));
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
