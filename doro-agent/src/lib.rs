use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::Utc;
use collectors::CollectorConfig;
use collectors::CollectorEvent;
use collectors::LocalCollectors;
use collectors::MetricsCapture;
use collectors::system_profile;
use doro_ai::AgentError;
use doro_ai::AgentRunOutcome;
use doro_ai::AgentRunRequest;
use doro_ai::AgentRunStatus;
use doro_ai::AgentRunner;
use doro_ai::AgentRunnerConfig;
use doro_ai::AgentToolCall;
use doro_ai::AgentToolDefinition;
use doro_ai::AgentToolExecutor;
use doro_ai::AgentToolResult;
use doro_ai::AgentToolResultStatus;
use doro_ai::DisabledAgentProvider;
use doro_ai::OpenAiAgentProvider;
use doro_container::ContainerProvider;
use doro_container::ContainerRuntimeSnapshot;
use doro_container::ContainerSummary;
use doro_container::DockerProvider;
use doro_container::DockerProviderConfig;
use doro_protocol::AgentCapability;
use doro_protocol::AgentEvent;
use doro_protocol::CapabilityName;
use doro_protocol::CapabilityRisk;
use doro_protocol::Host;
use doro_protocol::HostStatus;
use doro_protocol::MetricSnapshot;
use doro_protocol::PROTOCOL_VERSION;
use doro_protocol::Website;
use doro_protocol::WebsiteKind;
use doro_protocol::WebsiteProtocol;
use doro_protocol::WebsiteProxyTarget;
use doro_protocol::WebsiteStatus;
use doro_protocol::grpc;
use doro_protocol::grpc::agent_control_plane_client::AgentControlPlaneClient;
use doro_protocol::protobuf_timestamp_from_utc;
use doro_protocol::protobuf_timestamp_now;
use doro_vm::QemuProvider;
use doro_vm::QemuProviderConfig;
use doro_vm::VirtualMachineProvider;
use doro_vm::VmCommand;
use doro_vm::VmCommandEnvelope;
use doro_vm::VmCommandStatus;
use doro_vm::VmProviderError;
use doro_vm::VmRuntimeState;
use doro_vm::VmStatus;
use doro_vm::network::NetworkPolicy;
use doro_website::WebsiteRuntime;
use doro_website::WebsiteRuntimeConfig;
use doro_website::WebsiteRuntimeHandle;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::time::Duration;
use terminal::TerminalCommand;
use terminal::TerminalManager;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use uuid::Uuid;

mod collectors;
mod filesystem;
mod terminal;

const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(2);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
const AGENT_RUNTIME_LOG_LIMIT: usize = 200;
const MAX_FILE_TRANSFER_BYTES: usize = 64 * 1024 * 1024;

static AGENT_RUNTIME_LOGS: OnceLock<AgentRuntimeLogHub> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AgentRuntimeLog {
    pub id: Uuid,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: Value,
}

#[derive(Debug, Clone)]
struct AgentRuntimeLogHub {
    entries: Arc<StdMutex<VecDeque<AgentRuntimeLog>>>,
    sender: broadcast::Sender<AgentRuntimeLog>,
}

impl Default for AgentRuntimeLogHub {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(512);
        Self {
            entries: Arc::new(StdMutex::new(VecDeque::new())),
            sender,
        }
    }
}

impl AgentRuntimeLogHub {
    fn push(&self, entry: AgentRuntimeLog) {
        {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            entries.push_back(entry.clone());
            while entries.len() > AGENT_RUNTIME_LOG_LIMIT {
                entries.pop_front();
            }
        }
        let _ = self.sender.send(entry);
    }

    fn snapshot(&self) -> Vec<AgentRuntimeLog> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    fn subscribe(&self) -> broadcast::Receiver<AgentRuntimeLog> {
        self.sender.subscribe()
    }
}

pub fn init_runtime_log_capture() {
    let _ = AGENT_RUNTIME_LOGS.set(AgentRuntimeLogHub::default());
}

pub fn publish_runtime_log(
    level: impl Into<String>,
    target: impl Into<String>,
    message: impl Into<String>,
    fields: Value,
) {
    let Some(hub) = AGENT_RUNTIME_LOGS.get() else {
        return;
    };
    hub.push(AgentRuntimeLog {
        id: Uuid::new_v4(),
        level: level.into(),
        target: target.into(),
        message: message.into(),
        fields,
    });
}

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
    pub vm_manage_enabled: bool,
    pub qemu_binary_dir: Option<String>,
    pub vm_state_dir: Option<String>,
    pub vm_image_dir: Option<String>,
    pub vm_bridge_names: Vec<String>,
    pub vm_user_network_enabled: bool,
    pub vm_console_enabled: bool,
    pub vm_vnc_bind: String,
    pub gpu_metrics_enabled: bool,
    pub websites: doro_config::WebsiteConfig,
    pub ai: doro_config::AiConfig,
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
            container_metrics_enabled: true,
            docker_socket_path: None,
            docker_manage_enabled: true,
            vm_manage_enabled: false,
            qemu_binary_dir: None,
            vm_state_dir: None,
            vm_image_dir: None,
            vm_bridge_names: Vec::new(),
            vm_user_network_enabled: true,
            vm_console_enabled: true,
            vm_vnc_bind: "127.0.0.1".to_string(),
            gpu_metrics_enabled: false,
            websites: doro_config::WebsiteConfig::default(),
            ai: doro_config::AiConfig::default(),
        }
    }

    pub fn from_config(config: &doro_config::AgentConfig) -> Self {
        Self::from_config_with_ai(config, doro_config::AiConfig::default())
    }

    fn from_config_with_ai(config: &doro_config::AgentConfig, ai: doro_config::AiConfig) -> Self {
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
            vm_manage_enabled: config.vm_manage_enabled,
            qemu_binary_dir: config.qemu_binary_dir.clone(),
            vm_state_dir: config.vm_state_dir.clone(),
            vm_image_dir: config.vm_image_dir.clone(),
            vm_bridge_names: config.vm_bridge_names.clone(),
            vm_user_network_enabled: config.vm_user_network_enabled,
            vm_console_enabled: config.vm_console_enabled,
            vm_vnc_bind: config.vm_vnc_bind.clone(),
            gpu_metrics_enabled: config.gpu_metrics_enabled,
            websites: doro_config::WebsiteConfig::default(),
            ai,
        }
    }

    pub fn from_file_config(config: &doro_config::AgentFileConfig) -> Self {
        let mut agent = Self::from_config_with_ai(&config.agent, config.ai.clone());
        agent.websites = config.websites.clone();
        agent
    }
}

#[derive(Clone)]
struct ContainerRuntime {
    provider: Result<Arc<dyn ContainerProvider>, String>,
}

impl std::fmt::Debug for ContainerRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ContainerRuntime")
    }
}

impl ContainerRuntime {
    fn from_config(config: &AgentConfig) -> Option<Self> {
        if !config.container_metrics_enabled && !config.docker_manage_enabled {
            return None;
        }
        let provider = DockerProvider::connect(&DockerProviderConfig::new(
            config.docker_socket_path.clone(),
        ))
        .map(|provider| Arc::new(provider) as Arc<dyn ContainerProvider>)
        .map_err(|error| error.to_string());
        Some(Self { provider })
    }

    async fn snapshot(
        &self,
    ) -> Result<ContainerRuntimeSnapshot, doro_container::ContainerProviderError> {
        match &self.provider {
            Ok(provider) => provider.snapshot().await,
            Err(error) => Err(doro_container::ContainerProviderError::InvalidRequest(
                format!("failed to initialize container provider: {error}"),
            )),
        }
    }
}

#[derive(Clone)]
struct VmRuntime {
    provider: Arc<dyn VirtualMachineProvider>,
}

impl std::fmt::Debug for VmRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("VmRuntime")
    }
}

impl VmRuntime {
    fn from_config(config: &AgentConfig) -> Option<Self> {
        if !config.vm_manage_enabled {
            return None;
        }
        let provider = QemuProvider::new(QemuProviderConfig {
            binary_dir: config.qemu_binary_dir.as_ref().map(PathBuf::from),
            state_dir: config
                .vm_state_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".doro/vms")),
            image_dir: config
                .vm_image_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".doro/vm-images")),
            network_policy: NetworkPolicy {
                user_nat_enabled: config.vm_user_network_enabled,
                allowed_bridges: config.vm_bridge_names.clone(),
            },
            vnc_bind_host: config.vm_vnc_bind.clone(),
            vnc_display_base: 10,
        });
        Some(Self {
            provider: Arc::new(provider),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Agent {
    config: AgentConfig,
    container_runtime: Option<ContainerRuntime>,
    vm_runtime: Option<VmRuntime>,
    website_runtime: Option<WebsiteRuntimeHandle>,
}

#[derive(Debug, Clone, Default)]
struct AgentCommandState {
    pending_tool_approvals:
        Arc<Mutex<HashMap<String, oneshot::Sender<grpc::AgentToolApprovalDecisionCommand>>>>,
}

impl AgentCommandState {
    async fn wait_for_tool_approval(
        &self,
        request_id: String,
        timeout: Duration,
    ) -> Result<grpc::AgentToolApprovalDecisionCommand, AgentError> {
        let (sender, receiver) = oneshot::channel();
        self.pending_tool_approvals
            .lock()
            .await
            .insert(request_id.clone(), sender);

        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(decision)) => Ok(decision),
            Ok(Err(_)) => Err(AgentError::Tool {
                name: "approval".to_string(),
                message: "approval channel closed".to_string(),
            }),
            Err(_) => {
                self.pending_tool_approvals.lock().await.remove(&request_id);
                Err(AgentError::Tool {
                    name: "approval".to_string(),
                    message: "approval timed out".to_string(),
                })
            }
        }
    }

    async fn resolve_tool_approval(&self, decision: grpc::AgentToolApprovalDecisionCommand) {
        let sender = self
            .pending_tool_approvals
            .lock()
            .await
            .remove(&decision.request_id);
        if let Some(sender) = sender {
            let _ = sender.send(decision);
        }
    }
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        let container_runtime = ContainerRuntime::from_config(&config);
        let vm_runtime = VmRuntime::from_config(&config);
        let website_runtime = config.websites.enabled.then(WebsiteRuntimeHandle::default);
        Self {
            config,
            container_runtime,
            vm_runtime,
            website_runtime,
        }
    }

    pub fn start_website_runtime(&self) -> anyhow::Result<Option<std::thread::JoinHandle<()>>> {
        let Some(handle) = self.website_runtime.clone() else {
            return Ok(None);
        };
        let runtime = WebsiteRuntime::with_handle(
            WebsiteRuntimeConfig {
                enabled: self.config.websites.enabled,
                http_bind: self.config.websites.http_bind.clone(),
            },
            handle,
        );
        runtime.start().map_err(anyhow::Error::from)
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
                name: CapabilityName::AgentRun,
                risk: CapabilityRisk::Medium,
                description: "Run AI-guided local operations with Doro approval gates".to_string(),
            },
            AgentCapability {
                name: CapabilityName::ShellExecute,
                risk: CapabilityRisk::High,
                description: "Execute approved shell commands".to_string(),
            },
            AgentCapability {
                name: CapabilityName::FilesRead,
                risk: CapabilityRisk::Low,
                description: "Browse and read the host filesystem as the agent OS user".to_string(),
            },
            AgentCapability {
                name: CapabilityName::FilesWrite,
                risk: CapabilityRisk::High,
                description: "Manage the host filesystem as the agent OS user".to_string(),
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
        if self.vm_runtime.is_some() {
            capabilities.push(AgentCapability {
                name: CapabilityName::VirtualMachinesManage,
                risk: CapabilityRisk::High,
                description: "Manage QEMU virtual machines after approval".to_string(),
            });
        }
        if self.website_runtime.is_some() {
            capabilities.push(AgentCapability {
                name: CapabilityName::NetworkExpose,
                risk: CapabilityRisk::High,
                description: "Apply approved website reverse proxy routes with Pingora".to_string(),
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
        snapshot: ContainerRuntimeSnapshot,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::ContainerSnapshot(container_snapshot_from_runtime(
                command_id, snapshot,
            )),
        )
    }

    pub fn virtual_machine_snapshot_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        states: Vec<VmRuntimeState>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::VirtualMachineSnapshot(grpc::VirtualMachineSnapshotEvent {
                command_id,
                provider: "qemu".to_string(),
                virtual_machines: states
                    .into_iter()
                    .map(virtual_machine_observation_from_state)
                    .collect(),
                extra_json: serde_json::json!({}).to_string(),
            }),
        )
    }

    pub fn virtual_machine_command_result_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        result: doro_vm::VmCommandResult,
    ) -> grpc::AgentEvent {
        let status = match result.status {
            VmCommandStatus::Succeeded => grpc::CommandStatus::Succeeded,
            VmCommandStatus::Failed => grpc::CommandStatus::Failed,
        };
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::VirtualMachineCommandResult(
                grpc::VirtualMachineCommandResultEvent {
                    command_id,
                    status: status as i32,
                    message: result.message,
                    details_json: result.details.to_string(),
                },
            ),
        )
    }

    pub fn file_command_result_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        output: filesystem::FileCommandOutput,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::FileCommandResult(grpc::FileCommandResultEvent {
                command_id,
                status: grpc::CommandStatus::Succeeded as i32,
                message: output.message,
                result_json: output.result_json,
                content: output.content,
            }),
        )
    }

    pub fn website_routes_applied_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        result: Result<usize, String>,
        website_ids: Vec<String>,
    ) -> grpc::AgentEvent {
        let (status, message, route_count) = match result {
            Ok(route_count) => (
                grpc::CommandStatus::Succeeded,
                "website routes applied".to_string(),
                route_count as u32,
            ),
            Err(message) => (grpc::CommandStatus::Failed, message, 0),
        };
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::WebsiteRoutesApplied(grpc::WebsiteRoutesAppliedEvent {
                command_id,
                status: status as i32,
                message,
                route_count,
                website_ids,
            }),
        )
    }

    pub fn agent_task_progress_event(
        &self,
        agent_id: Uuid,
        progress: grpc::AgentTaskProgressEvent,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::AgentTaskProgress(progress),
        )
    }

    pub fn agent_task_result_event(
        &self,
        agent_id: Uuid,
        command_id: impl Into<String>,
        task_id: impl Into<String>,
        outcome: &AgentRunOutcome,
    ) -> grpc::AgentEvent {
        let status = match outcome.status {
            AgentRunStatus::Succeeded => grpc::CommandStatus::Succeeded,
            AgentRunStatus::Failed => grpc::CommandStatus::Failed,
        };
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::AgentTaskResult(grpc::AgentTaskResultEvent {
                command_id: command_id.into(),
                task_id: task_id.into(),
                status: status as i32,
                summary: outcome.summary.clone(),
                result_json: serde_json::json!({
                    "transcript": outcome.transcript,
                })
                .to_string(),
            }),
        )
    }

    pub fn agent_tool_approval_request_event(
        &self,
        agent_id: Uuid,
        request: grpc::AgentToolApprovalRequestEvent,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::AgentToolApprovalRequest(request),
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

    pub fn terminal_output_event(
        &self,
        agent_id: Uuid,
        session_id: String,
        data: Vec<u8>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::TerminalOutput(grpc::TerminalOutputEvent {
                session_id,
                data: String::from_utf8_lossy(&data).into_owned(),
            }),
        )
    }

    pub fn terminal_session_closed_event(
        &self,
        agent_id: Uuid,
        session_id: String,
        reason: impl Into<String>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::TerminalSessionClosed(grpc::TerminalSessionClosedEvent {
                session_id,
                reason: reason.into(),
            }),
        )
    }

    pub fn log_line_event(&self, agent_id: Uuid, log: AgentRuntimeLog) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::LogLine(grpc::LogLineEvent {
                log_id: log.id.to_string(),
                level: log.level,
                target: log.target,
                message: log.message,
                fields_json: log.fields.to_string(),
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

    fn ai_runner(&self) -> Result<AgentRunner, AgentError> {
        let provider: Arc<dyn doro_ai::AgentModelProvider> = match self.config.ai.provider.as_str()
        {
            "openai" => {
                let openai_config = doro_ai::openai::OpenAiConfig {
                    api_key_env: self.config.ai.openai.api_key_env.clone(),
                    base_url: self.config.ai.openai.base_url.clone(),
                    timeout_seconds: self.config.ai.openai.timeout_seconds,
                };
                let client = doro_ai::openai::OpenAiClient::new(openai_config)
                    .map_err(|error| AgentError::Model(error.to_string()))?;
                Arc::new(OpenAiAgentProvider::new(
                    client,
                    self.config.ai.openai.default_response_model.clone(),
                ))
            }
            "disabled" => Arc::new(DisabledAgentProvider),
            provider => {
                return Err(AgentError::Model(format!(
                    "unsupported AI provider for agent: {provider}"
                )));
            }
        };

        Ok(AgentRunner::new(
            provider,
            self.ai_tool_definitions(),
            AgentRunnerConfig {
                max_turns: self.config.ai.agent.max_turns.max(1),
                max_tool_calls: self.config.ai.agent.max_tool_calls.max(1),
            },
        ))
    }

    fn ai_tool_definitions(&self) -> Vec<AgentToolDefinition> {
        let mut tools = vec![
            AgentToolDefinition {
                name: "host_metrics".to_string(),
                description: "Read current host metrics and basic resource status".to_string(),
                risk: CapabilityRisk::Low,
                parameters: empty_schema(),
            },
            AgentToolDefinition {
                name: "list_directory".to_string(),
                description: "List files in a directory as the agent OS user".to_string(),
                risk: CapabilityRisk::Low,
                parameters: object_schema(vec![("path", "Directory path")], &["path"]),
            },
            AgentToolDefinition {
                name: "read_file".to_string(),
                description: "Read a file as the agent OS user within the transfer limit"
                    .to_string(),
                risk: CapabilityRisk::Low,
                parameters: object_schema(vec![("path", "File path")], &["path"]),
            },
            AgentToolDefinition {
                name: "search_files".to_string(),
                description: "Search file and directory names below a path".to_string(),
                risk: CapabilityRisk::Low,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Root directory path" },
                        "query": { "type": "string", "description": "Case-insensitive name query" },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 500 }
                    },
                    "required": ["path", "query"],
                    "additionalProperties": false
                }),
            },
            AgentToolDefinition {
                name: "run_shell".to_string(),
                description:
                    "Run a shell command through the Doro terminal path after approval"
                        .to_string(),
                risk: CapabilityRisk::High,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Shell command or script" },
                        "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 120 }
                    },
                    "required": ["input"],
                    "additionalProperties": false
                }),
            },
            AgentToolDefinition {
                name: "write_file".to_string(),
                description: "Write UTF-8 text to a file after approval".to_string(),
                risk: CapabilityRisk::High,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Target file path" },
                        "content": { "type": "string", "description": "UTF-8 file content" },
                        "overwrite": { "type": "boolean" }
                    },
                    "required": ["path", "content"],
                    "additionalProperties": false
                }),
            },
            AgentToolDefinition {
                name: "file_operation".to_string(),
                description:
                    "Create directory, rename, move, copy, or delete a filesystem path after approval"
                        .to_string(),
                risk: CapabilityRisk::High,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["create_directory", "rename", "move", "copy", "delete"]
                        },
                        "path": { "type": "string" },
                        "target_path": { "type": "string" },
                        "name": { "type": "string" },
                        "overwrite": { "type": "boolean" }
                    },
                    "required": ["operation", "path"],
                    "additionalProperties": false
                }),
            },
        ];

        if self.container_runtime.is_some() {
            tools.push(AgentToolDefinition {
                name: "container_snapshot".to_string(),
                description: "Read current Docker runtime, container, network, and volume state"
                    .to_string(),
                risk: CapabilityRisk::Low,
                parameters: empty_schema(),
            });
        }
        if self.vm_runtime.is_some() {
            tools.push(AgentToolDefinition {
                name: "virtual_machine_snapshot".to_string(),
                description: "Read current QEMU virtual machine state".to_string(),
                risk: CapabilityRisk::Low,
                parameters: empty_schema(),
            });
        }

        tools
    }
}

fn empty_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

fn object_schema(properties: Vec<(&str, &str)>, required: &[&str]) -> Value {
    let properties = properties
        .into_iter()
        .map(|(name, description)| {
            (
                name.to_string(),
                serde_json::json!({
                    "type": "string",
                    "description": description,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

#[derive(Clone)]
struct LocalAgentToolExecutor {
    agent: Agent,
    agent_id: Uuid,
    command_id: String,
    task_id: String,
    sender: mpsc::Sender<grpc::AgentEvent>,
    terminal: TerminalManager,
    command_state: AgentCommandState,
    tool_timeout: Duration,
    shell_timeout: Duration,
    approval_timeout: Duration,
}

#[async_trait]
impl AgentToolExecutor for LocalAgentToolExecutor {
    async fn execute(
        &self,
        call: AgentToolCall,
        definition: &AgentToolDefinition,
    ) -> Result<AgentToolResult, AgentError> {
        let step_id = if definition.risk >= CapabilityRisk::High {
            let request_id = Uuid::new_v4().to_string();
            let request = grpc::AgentToolApprovalRequestEvent {
                request_id: request_id.clone(),
                command_id: self.command_id.clone(),
                task_id: self.task_id.clone(),
                tool_call_id: call.call_id.clone(),
                tool_name: call.name.clone(),
                risk: format!("{:?}", definition.risk),
                summary: tool_approval_summary(&call),
                arguments_json: call.arguments.to_string(),
            };
            if self
                .sender
                .send(
                    self.agent
                        .agent_tool_approval_request_event(self.agent_id, request),
                )
                .await
                .is_err()
            {
                return Err(AgentError::Tool {
                    name: call.name,
                    message: "failed to send tool approval request".to_string(),
                });
            }
            let decision = self
                .command_state
                .wait_for_tool_approval(request_id, self.approval_timeout)
                .await?;
            if !decision.approved {
                return Err(AgentError::ApprovalDenied {
                    name: call.name,
                    message: if decision.message.trim().is_empty() {
                        "approval denied".to_string()
                    } else {
                        decision.message
                    },
                });
            }
            decision.step_id
        } else {
            String::new()
        };

        if !step_id.is_empty() {
            self.send_tool_progress(&step_id, "running", "tool execution started", json!({}))
                .await;
        }

        let execution_timeout = if call.name == "run_shell" {
            self.shell_timeout + Duration::from_secs(2)
        } else {
            self.tool_timeout
        };
        let execution = tokio::time::timeout(
            execution_timeout,
            self.execute_approved_tool(call.clone(), definition),
        )
        .await;
        let result = match execution {
            Ok(result) => result,
            Err(_) => AgentToolResult {
                status: AgentToolResultStatus::Failed,
                output: json!({
                    "error": "tool execution timed out",
                    "timeout_seconds": execution_timeout.as_secs(),
                }),
            },
        };

        if !step_id.is_empty() {
            let status = match result.status {
                AgentToolResultStatus::Succeeded => "succeeded",
                AgentToolResultStatus::Failed => "failed",
            };
            self.send_tool_progress(
                &step_id,
                status,
                "tool execution finished",
                result.output.clone(),
            )
            .await;
        }

        Ok(result)
    }
}

impl LocalAgentToolExecutor {
    async fn execute_approved_tool(
        &self,
        call: AgentToolCall,
        _definition: &AgentToolDefinition,
    ) -> AgentToolResult {
        match call.name.as_str() {
            "host_metrics" => value_tool_result(
                serde_json::to_value(self.agent.metrics()).map_err(anyhow::Error::from),
            ),
            "list_directory" => {
                let path = required_argument(&call.arguments, "path");
                file_output_tool_result(path.and_then(|path| filesystem::list_directory(&path)))
            }
            "read_file" => {
                let path = required_argument(&call.arguments, "path");
                match path.and_then(|path| filesystem::read_file(&path, MAX_FILE_TRANSFER_BYTES)) {
                    Ok(output) => {
                        let content = String::from_utf8_lossy(&output.content).into_owned();
                        AgentToolResult {
                            status: AgentToolResultStatus::Succeeded,
                            output: json!({
                                "message": output.message,
                                "metadata": parse_json_value(&output.result_json),
                                "content": content,
                            }),
                        }
                    }
                    Err(error) => failed_tool_result(error),
                }
            }
            "search_files" => {
                let path = required_argument(&call.arguments, "path");
                let query = required_argument(&call.arguments, "query");
                let limit = call
                    .arguments
                    .get("limit")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .unwrap_or(500)
                    .min(500);
                file_output_tool_result(
                    path.and_then(|path| query.map(|query| (path, query)))
                        .and_then(|(path, query)| filesystem::search_files(&path, &query, limit)),
                )
            }
            "run_shell" => self.run_shell_tool(&call).await,
            "write_file" => self.write_file_tool(&call),
            "file_operation" => self.file_operation_tool(&call),
            "container_snapshot" => self.container_snapshot_tool().await,
            "virtual_machine_snapshot" => self.virtual_machine_snapshot_tool().await,
            other => AgentToolResult {
                status: AgentToolResultStatus::Failed,
                output: json!({ "error": format!("unsupported tool: {other}") }),
            },
        }
    }

    async fn run_shell_tool(&self, call: &AgentToolCall) -> AgentToolResult {
        let input = match required_argument(&call.arguments, "input") {
            Ok(input) => input,
            Err(error) => return failed_tool_result(error),
        };
        let timeout = call
            .arguments
            .get("timeout_seconds")
            .and_then(Value::as_u64)
            .map(Duration::from_secs)
            .unwrap_or(self.shell_timeout)
            .min(self.shell_timeout);
        match self
            .terminal
            .execute(TerminalCommand {
                command_id: call.call_id.clone(),
                input,
                cols: 100,
                rows: 30,
                timeout,
            })
            .await
        {
            Ok(output) => AgentToolResult {
                status: if output.exit_code == Some(0) && !output.timed_out {
                    AgentToolResultStatus::Succeeded
                } else {
                    AgentToolResultStatus::Failed
                },
                output: json!({
                    "output": output.output,
                    "exit_code": output.exit_code,
                    "timed_out": output.timed_out,
                    "started_at": output.started_at,
                    "finished_at": output.finished_at,
                }),
            },
            Err(error) => failed_tool_result(error),
        }
    }

    fn write_file_tool(&self, call: &AgentToolCall) -> AgentToolResult {
        let path = match required_argument(&call.arguments, "path") {
            Ok(path) => path,
            Err(error) => return failed_tool_result(error),
        };
        let content = match required_argument(&call.arguments, "content") {
            Ok(content) => content,
            Err(error) => return failed_tool_result(error),
        };
        let overwrite = call
            .arguments
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let command = grpc::RunFileOperationCommand {
            command_id: call.call_id.clone(),
            operation: "upload".to_string(),
            path,
            target_path: String::new(),
            name: String::new(),
            content: content.into_bytes(),
            overwrite,
        };
        file_output_tool_result(filesystem::run_operation(command, MAX_FILE_TRANSFER_BYTES))
    }

    fn file_operation_tool(&self, call: &AgentToolCall) -> AgentToolResult {
        let operation = match required_argument(&call.arguments, "operation") {
            Ok(operation) => operation,
            Err(error) => return failed_tool_result(error),
        };
        let path = match required_argument(&call.arguments, "path") {
            Ok(path) => path,
            Err(error) => return failed_tool_result(error),
        };
        let command = grpc::RunFileOperationCommand {
            command_id: call.call_id.clone(),
            operation,
            path,
            target_path: call
                .arguments
                .get("target_path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            name: call
                .arguments
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            content: call
                .arguments
                .get("content_base64")
                .and_then(Value::as_str)
                .and_then(|content| STANDARD.decode(content.as_bytes()).ok())
                .unwrap_or_default(),
            overwrite: call
                .arguments
                .get("overwrite")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        file_output_tool_result(filesystem::run_operation(command, MAX_FILE_TRANSFER_BYTES))
    }

    async fn container_snapshot_tool(&self) -> AgentToolResult {
        let Some(runtime) = &self.agent.container_runtime else {
            return AgentToolResult {
                status: AgentToolResultStatus::Failed,
                output: json!({ "error": "container runtime is not enabled" }),
            };
        };
        value_tool_result(
            runtime
                .snapshot()
                .await
                .map_err(anyhow::Error::from)
                .and_then(|snapshot| serde_json::to_value(snapshot).map_err(anyhow::Error::from)),
        )
    }

    async fn virtual_machine_snapshot_tool(&self) -> AgentToolResult {
        let Some(runtime) = &self.agent.vm_runtime else {
            return AgentToolResult {
                status: AgentToolResultStatus::Failed,
                output: json!({ "error": "virtual machine provider is not enabled" }),
            };
        };
        value_tool_result(
            runtime
                .provider
                .list()
                .await
                .map_err(anyhow::Error::from)
                .and_then(|states| serde_json::to_value(states).map_err(anyhow::Error::from)),
        )
    }

    async fn send_tool_progress(&self, step_id: &str, status: &str, message: &str, details: Value) {
        let event = self.agent.agent_task_progress_event(
            self.agent_id,
            grpc::AgentTaskProgressEvent {
                command_id: self.command_id.clone(),
                task_id: self.task_id.clone(),
                step_id: step_id.to_string(),
                status: status.to_string(),
                message: message.to_string(),
                details_json: details.to_string(),
            },
        );
        if self.sender.send(event).await.is_err() {
            tracing::warn!("failed to enqueue agent task progress event");
        }
    }
}

fn tool_approval_summary(call: &AgentToolCall) -> String {
    match call.name.as_str() {
        "run_shell" => call
            .arguments
            .get("input")
            .and_then(Value::as_str)
            .map(|input| format!("Run shell command: {input}"))
            .unwrap_or_else(|| "Run shell command".to_string()),
        "write_file" => call
            .arguments
            .get("path")
            .and_then(Value::as_str)
            .map(|path| format!("Write file {path}"))
            .unwrap_or_else(|| "Write file".to_string()),
        "file_operation" => {
            let operation = call
                .arguments
                .get("operation")
                .and_then(Value::as_str)
                .unwrap_or("file_operation");
            let path = call
                .arguments
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!("Run {operation} on {path}")
        }
        other => format!("Run high-risk AI tool {other}"),
    }
}

fn required_argument(arguments: &Value, name: &str) -> anyhow::Result<String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("tool argument {name} is required"))
}

fn file_output_tool_result(
    output: anyhow::Result<filesystem::FileCommandOutput>,
) -> AgentToolResult {
    match output {
        Ok(output) => AgentToolResult {
            status: AgentToolResultStatus::Succeeded,
            output: json!({
                "message": output.message,
                "result": parse_json_value(&output.result_json),
                "content_bytes": output.content.len(),
            }),
        },
        Err(error) => failed_tool_result(error),
    }
}

fn value_tool_result(output: anyhow::Result<Value>) -> AgentToolResult {
    match output {
        Ok(output) => AgentToolResult {
            status: AgentToolResultStatus::Succeeded,
            output,
        },
        Err(error) => failed_tool_result(error),
    }
}

fn failed_tool_result(error: impl std::fmt::Display) -> AgentToolResult {
    AgentToolResult {
        status: AgentToolResultStatus::Failed,
        output: json!({ "error": error.to_string() }),
    }
}

fn parse_json_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| json!({ "raw": value }))
}

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

    if let Some(runtime_logs) = AGENT_RUNTIME_LOGS.get() {
        for log in runtime_logs.snapshot() {
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
        let mut log_receiver = runtime_logs.subscribe();
        let log_shutdown = shutdown_rx.clone();
        tokio::spawn(async move {
            loop {
                let log = tokio::select! {
                    log = log_receiver.recv() => {
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

fn container_snapshot_from_runtime(
    command_id: String,
    snapshot: ContainerRuntimeSnapshot,
) -> grpc::ContainerSnapshotEvent {
    let extra_json = serde_json::json!({
        "daemon": snapshot.daemon,
        "networks": snapshot.networks,
        "volumes": snapshot.volumes,
    })
    .to_string();

    grpc::ContainerSnapshotEvent {
        command_id,
        runtime: snapshot.runtime,
        containers: snapshot
            .containers
            .into_iter()
            .map(container_observation_from_summary)
            .collect(),
        extra_json,
    }
}

fn container_observation_from_summary(container: ContainerSummary) -> grpc::ContainerObservation {
    grpc::ContainerObservation {
        id: container.id.unwrap_or_default(),
        names: container.names,
        image: container.image.unwrap_or_default(),
        image_id: container.image_id.unwrap_or_default(),
        command: container.command.unwrap_or_default(),
        created: container.created.unwrap_or_default(),
        ports_json: container.ports.to_string(),
        labels_json: container.labels.to_string(),
        state: container.state.unwrap_or_default(),
        status: container.status.unwrap_or_default(),
    }
}

fn apply_website_routes(
    runtime: &WebsiteRuntimeHandle,
    routes: Vec<grpc::WebsiteRoute>,
) -> Result<usize, String> {
    let websites = routes
        .into_iter()
        .map(website_from_grpc_route)
        .collect::<Result<Vec<_>, _>>()?;
    runtime.reload(&websites).map_err(|error| error.to_string())
}

fn website_from_grpc_route(route: grpc::WebsiteRoute) -> Result<Website, String> {
    let website_id = Uuid::parse_str(&route.website_id)
        .map_err(|_| "website route website_id must be a uuid".to_string())?;
    let status = parse_website_status(&route.status)?;
    let kind = parse_website_kind(&route.kind)?;
    let protocol = parse_website_protocol(&route.protocol)?;
    if kind != WebsiteKind::ReverseProxy || protocol != WebsiteProtocol::Http {
        return Err(
            "agent website runtime currently supports only HTTP reverse proxy routes".to_string(),
        );
    }
    let listen_port =
        u16::try_from(route.listen_port).map_err(|_| "website listen port is invalid")?;
    let config = serde_json::from_str(&route.config_json).unwrap_or_else(|_| {
        json!({
            "raw": route.config_json
        })
    });
    Ok(Website {
        id: website_id,
        host_id: Some(Uuid::nil()),
        name: route.primary_domain.clone(),
        primary_domain: route.primary_domain,
        aliases: route.aliases,
        status,
        kind,
        protocol,
        listen_port,
        upstream: WebsiteProxyTarget {
            url: route.upstream_url,
        },
        app_install_id: None,
        tls_certificate_id: None,
        config,
        notes: None,
        last_runtime_error: None,
        last_checked_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    })
}

fn parse_website_status(value: &str) -> Result<WebsiteStatus, String> {
    match normalize_enum_token(value).as_str() {
        "running" => Ok(WebsiteStatus::Running),
        "stopped" => Ok(WebsiteStatus::Stopped),
        "warning" => Ok(WebsiteStatus::Warning),
        _ => Err("website route status is invalid".to_string()),
    }
}

fn parse_website_kind(value: &str) -> Result<WebsiteKind, String> {
    match normalize_enum_token(value).as_str() {
        "reverse_proxy" => Ok(WebsiteKind::ReverseProxy),
        "static_site" => Ok(WebsiteKind::StaticSite),
        "tcp_proxy" => Ok(WebsiteKind::TcpProxy),
        "udp_proxy" => Ok(WebsiteKind::UdpProxy),
        _ => Err("website route kind is invalid".to_string()),
    }
}

fn parse_website_protocol(value: &str) -> Result<WebsiteProtocol, String> {
    match normalize_enum_token(value).as_str() {
        "http" => Ok(WebsiteProtocol::Http),
        "https" => Ok(WebsiteProtocol::Https),
        "tcp" => Ok(WebsiteProtocol::Tcp),
        "udp" => Ok(WebsiteProtocol::Udp),
        _ => Err("website route protocol is invalid".to_string()),
    }
}

fn normalize_enum_token(value: &str) -> String {
    let mut token = String::new();
    for (index, character) in value.chars().enumerate() {
        if character == '-' || character == ' ' {
            token.push('_');
        } else if character.is_uppercase() {
            if index > 0 {
                token.push('_');
            }
            token.extend(character.to_lowercase());
        } else {
            token.push(character);
        }
    }
    token
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
    command_state: &AgentCommandState,
) -> AgentCommandAction {
    let command_id = command.command_id.clone();
    match command.command {
        Some(grpc::control_plane_command::Command::Ack(_)) => {
            tracing::info!(command_id = %command_id, "control-plane acknowledged stream")
        }
        Some(grpc::control_plane_command::Command::CollectContainers(_)) => {
            tracing::info!(command_id = %command_id, "collecting containers by control-plane request");
            let event = match &agent.container_runtime {
                Some(runtime) => match runtime.snapshot().await {
                    Ok(snapshot) => agent.container_snapshot_event(agent_id, command_id, snapshot),
                    Err(error) => agent.command_result_event(
                        agent_id,
                        command_id,
                        grpc::CommandStatus::Failed,
                        error.to_string(),
                    ),
                },
                None => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    "container provider is not enabled",
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue command result event");
            }
        }
        Some(grpc::control_plane_command::Command::CollectVirtualMachines(_)) => {
            tracing::info!(command_id = %command_id, "collecting virtual machines by control-plane request");
            let event = match &agent.vm_runtime {
                Some(runtime) => match runtime.provider.list().await {
                    Ok(states) => {
                        agent.virtual_machine_snapshot_event(agent_id, command_id, states)
                    }
                    Err(error) => agent.command_result_event(
                        agent_id,
                        command_id,
                        grpc::CommandStatus::Failed,
                        error.to_string(),
                    ),
                },
                None => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    "virtual machine provider is not enabled",
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue virtual machine snapshot event");
            }
        }
        Some(grpc::control_plane_command::Command::RunVirtualMachineCommand(vm_command)) => {
            tracing::info!(command_id = %command_id, "executing virtual machine command by control-plane request");
            let event = match &agent.vm_runtime {
                Some(runtime) => {
                    match serde_json::from_str::<VmCommandEnvelope>(&vm_command.command_json) {
                        Ok(envelope) => match execute_vm_command(runtime, envelope).await {
                            Ok(result) => agent
                                .virtual_machine_command_result_event(agent_id, command_id, result),
                            Err(error) => agent.command_result_event(
                                agent_id,
                                command_id,
                                grpc::CommandStatus::Failed,
                                error.to_string(),
                            ),
                        },
                        Err(error) => agent.command_result_event(
                            agent_id,
                            command_id,
                            grpc::CommandStatus::Failed,
                            format!("invalid virtual machine command payload: {error}"),
                        ),
                    }
                }
                None => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    "virtual machine provider is not enabled",
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue virtual machine command result event");
            }
        }
        Some(grpc::control_plane_command::Command::ListDirectory(list_command)) => {
            tracing::info!(command_id = %command_id, path = list_command.path, "listing directory by control-plane request");
            let event = match filesystem::list_directory(&list_command.path) {
                Ok(output) => agent.file_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue file list result event");
            }
        }
        Some(grpc::control_plane_command::Command::ReadFile(read_command)) => {
            tracing::info!(command_id = %command_id, path = read_command.path, "reading file by control-plane request");
            let event = match filesystem::read_file(&read_command.path, MAX_FILE_TRANSFER_BYTES) {
                Ok(output) => agent.file_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue file read result event");
            }
        }
        Some(grpc::control_plane_command::Command::SearchFiles(search_command)) => {
            tracing::info!(command_id = %command_id, path = search_command.path, query = search_command.query, "searching files by control-plane request");
            let event = match filesystem::search_files(
                &search_command.path,
                &search_command.query,
                search_command.limit,
            ) {
                Ok(output) => agent.file_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue file search result event");
            }
        }
        Some(grpc::control_plane_command::Command::RunFileOperation(file_command)) => {
            tracing::info!(command_id = %command_id, operation = file_command.operation, path = file_command.path, "running file operation by control-plane request");
            let event = match filesystem::run_operation(file_command, MAX_FILE_TRANSFER_BYTES) {
                Ok(output) => agent.file_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue file operation result event");
            }
        }
        Some(grpc::control_plane_command::Command::ApplyWebsiteRoutes(route_command)) => {
            tracing::info!(
                command_id = %command_id,
                route_count = route_command.routes.len(),
                "applying website routes by control-plane request"
            );
            let website_ids = route_command
                .routes
                .iter()
                .map(|route| route.website_id.clone())
                .collect::<Vec<_>>();
            let result = match &agent.website_runtime {
                Some(runtime) => apply_website_routes(runtime, route_command.routes),
                None => Err("website runtime is not enabled".to_string()),
            };
            let event =
                agent.website_routes_applied_event(agent_id, command_id, result, website_ids);
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue website routes applied event");
            }
        }
        Some(grpc::control_plane_command::Command::RunAgentTask(agent_task)) => {
            tracing::info!(
                command_id = %command_id,
                task_id = agent_task.task_id,
                scheduled_task_id = agent_task.scheduled_task_id,
                "running agent AI task"
            );
            let task_agent = agent.clone();
            let task_sender = sender.clone();
            let task_terminal = terminal.clone();
            let task_state = command_state.clone();
            tokio::spawn(async move {
                run_agent_task_command(
                    task_agent,
                    agent_id,
                    command_id,
                    agent_task,
                    task_sender,
                    task_terminal,
                    task_state,
                )
                .await;
            });
        }
        Some(grpc::control_plane_command::Command::AgentToolApprovalDecision(decision)) => {
            tracing::info!(
                request_id = decision.request_id,
                task_id = decision.task_id,
                approved = decision.approved,
                "received agent tool approval decision"
            );
            command_state.resolve_tool_approval(decision).await;
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
        Some(grpc::control_plane_command::Command::OpenTerminalSession(open)) => {
            tracing::info!(
                session_id = open.session_id,
                command_id = %command_id,
                "opening interactive terminal session"
            );
            match terminal
                .open_interactive(
                    open.session_id.clone(),
                    open.cols.clamp(20, 300) as u16,
                    open.rows.clamp(5, 120) as u16,
                )
                .await
            {
                Ok(mut output_rx) => {
                    let output_agent = agent.clone();
                    let output_sender = sender.clone();
                    let session_id = open.session_id;
                    tokio::spawn(async move {
                        while let Some(output) = output_rx.recv().await {
                            let event = output_agent.terminal_output_event(
                                agent_id,
                                session_id.clone(),
                                output,
                            );
                            if output_sender.send(event).await.is_err() {
                                return;
                            }
                        }
                        let event = output_agent.terminal_session_closed_event(
                            agent_id,
                            session_id,
                            "pty output ended",
                        );
                        let _ = output_sender.send(event).await;
                    });
                }
                Err(error) => {
                    let event = agent.terminal_session_closed_event(
                        agent_id,
                        open.session_id,
                        error.to_string(),
                    );
                    if sender.send(event).await.is_err() {
                        tracing::warn!("failed to enqueue terminal session error event");
                    }
                }
            }
        }
        Some(grpc::control_plane_command::Command::TerminalInput(input)) => {
            if let Err(error) = terminal
                .write_interactive(input.session_id.clone(), input.data)
                .await
            {
                let event = agent.terminal_session_closed_event(
                    agent_id,
                    input.session_id,
                    error.to_string(),
                );
                if sender.send(event).await.is_err() {
                    tracing::warn!("failed to enqueue terminal input error event");
                }
            }
        }
        Some(grpc::control_plane_command::Command::ResizeTerminalSession(resize)) => {
            if let Err(error) = terminal
                .resize_interactive(
                    resize.session_id.clone(),
                    resize.cols.clamp(20, 300) as u16,
                    resize.rows.clamp(5, 120) as u16,
                )
                .await
            {
                tracing::warn!(%error, session_id = resize.session_id, "failed to resize terminal session");
            }
        }
        Some(grpc::control_plane_command::Command::CloseTerminalSession(close)) => {
            let _ = terminal.close_interactive(close.session_id.clone()).await;
            let event =
                agent.terminal_session_closed_event(agent_id, close.session_id, close.reason);
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue terminal close event");
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

async fn run_agent_task_command(
    agent: Agent,
    agent_id: Uuid,
    command_id: String,
    agent_task: grpc::RunAgentTaskCommand,
    sender: mpsc::Sender<grpc::AgentEvent>,
    terminal: TerminalManager,
    command_state: AgentCommandState,
) {
    let task_id = agent_task.task_id.clone();
    let started = agent.agent_task_progress_event(
        agent_id,
        grpc::AgentTaskProgressEvent {
            command_id: command_id.clone(),
            task_id: task_id.clone(),
            step_id: String::new(),
            status: "running".to_string(),
            message: "agent AI task started".to_string(),
            details_json: json!({
                "scheduled_task_id": agent_task.scheduled_task_id.clone(),
            })
            .to_string(),
        },
    );
    if sender.send(started).await.is_err() {
        tracing::warn!("failed to enqueue agent task start event");
    }

    let outcome = match agent.ai_runner() {
        Ok(runner) => {
            let executor = LocalAgentToolExecutor {
                agent: agent.clone(),
                agent_id,
                command_id: command_id.clone(),
                task_id: task_id.clone(),
                sender: sender.clone(),
                terminal,
                command_state,
                tool_timeout: Duration::from_secs(
                    agent.config.ai.agent.tool_timeout_seconds.max(1),
                ),
                shell_timeout: Duration::from_secs(
                    agent.config.ai.agent.shell_timeout_seconds.max(1),
                ),
                approval_timeout: Duration::from_secs(
                    agent.config.ai.agent.approval_timeout_seconds.max(1),
                ),
            };
            runner
                .run(
                    AgentRunRequest {
                        prompt: agent_task.prompt,
                        context: json!({
                            "agent_id": agent_id,
                            "host_id": agent.config.host_id,
                            "hostname": agent.config.hostname.clone(),
                            "scheduled_task_id": agent_task.scheduled_task_id.clone(),
                            "template": parse_json_value(&agent_task.template_json),
                        }),
                    },
                    &executor,
                )
                .await
        }
        Err(error) => Err(error),
    };

    let (status, message, result_outcome) = match outcome {
        Ok(outcome) => {
            let status = match outcome.status {
                AgentRunStatus::Succeeded => grpc::CommandStatus::Succeeded,
                AgentRunStatus::Failed => grpc::CommandStatus::Failed,
            };
            (status, outcome.summary.clone(), outcome)
        }
        Err(error) => {
            let message = error.to_string();
            (
                grpc::CommandStatus::Failed,
                message.clone(),
                AgentRunOutcome {
                    status: AgentRunStatus::Failed,
                    summary: message,
                    transcript: Vec::new(),
                },
            )
        }
    };

    let result_event =
        agent.agent_task_result_event(agent_id, command_id.clone(), task_id, &result_outcome);
    if sender.send(result_event).await.is_err() {
        tracing::warn!("failed to enqueue agent task result event");
    }

    let command_result = agent.command_result_event(agent_id, command_id, status, message);
    if sender.send(command_result).await.is_err() {
        tracing::warn!("failed to enqueue agent task command result event");
    }
}

async fn execute_vm_command(
    runtime: &VmRuntime,
    envelope: VmCommandEnvelope,
) -> Result<doro_vm::VmCommandResult, VmProviderError> {
    match envelope.command {
        VmCommand::Create { spec } => {
            let state = runtime.provider.create(*spec).await?;
            Ok(doro_vm::VmCommandResult {
                command_id: envelope.command_id,
                vm_id: Some(state.id.clone()),
                status: VmCommandStatus::Succeeded,
                message: "virtual machine created".to_string(),
                details: serde_json::to_value(state)?,
            })
        }
        VmCommand::Start { id } => runtime.provider.start(&id).await,
        VmCommand::Stop { id, mode } => runtime.provider.stop(&id, mode).await,
        VmCommand::Restart { id } => runtime.provider.restart(&id).await,
        VmCommand::Delete { id, mode } => runtime.provider.delete(&id, mode).await,
        VmCommand::Snapshot { id, request } => {
            let snapshot = runtime.provider.snapshot(&id, request).await?;
            Ok(doro_vm::VmCommandResult {
                command_id: envelope.command_id,
                vm_id: Some(id),
                status: VmCommandStatus::Succeeded,
                message: "virtual machine snapshot created".to_string(),
                details: serde_json::to_value(snapshot)?,
            })
        }
        VmCommand::Console { id } => {
            let console = runtime.provider.console(&id).await?;
            Ok(doro_vm::VmCommandResult {
                command_id: envelope.command_id,
                vm_id: Some(id),
                status: VmCommandStatus::Succeeded,
                message: "virtual machine console resolved".to_string(),
                details: serde_json::to_value(console)?,
            })
        }
    }
}

fn virtual_machine_observation_from_state(
    state: VmRuntimeState,
) -> grpc::VirtualMachineObservation {
    grpc::VirtualMachineObservation {
        vm_ref: state.id.to_string(),
        name: state.name,
        status: serialize_vm_status(state.status).to_string(),
        cpu_cores: u32::from(state.cpu_cores),
        memory_mib: state.memory_mib,
        disk_gb: state.disk_gb,
        image: state
            .metadata
            .get("image")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        networks_json: serde_json::to_string(&state.networks).unwrap_or_else(|_| "[]".to_string()),
        console_json: state
            .console
            .and_then(|console| serde_json::to_string(&console).ok())
            .unwrap_or_else(|| "null".to_string()),
        metadata_json: state.metadata.to_string(),
        created_at: state.created_at.map(protobuf_timestamp_from_utc),
        observed_at: Some(protobuf_timestamp_from_utc(state.observed_at)),
    }
}

fn serialize_vm_status(status: VmStatus) -> &'static str {
    match status {
        VmStatus::Unknown => "unknown",
        VmStatus::Stopped => "stopped",
        VmStatus::Starting => "starting",
        VmStatus::Running => "running",
        VmStatus::Paused => "paused",
        VmStatus::Stopping => "stopping",
        VmStatus::Failed => "failed",
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

    fn test_agent_config(agent_id: Uuid) -> AgentConfig {
        AgentConfig {
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
            vm_manage_enabled: false,
            qemu_binary_dir: None,
            vm_state_dir: None,
            vm_image_dir: None,
            vm_bridge_names: Vec::new(),
            vm_user_network_enabled: true,
            vm_console_enabled: true,
            vm_vnc_bind: "127.0.0.1".to_string(),
            gpu_metrics_enabled: false,
            websites: doro_config::WebsiteConfig::default(),
            ai: doro_config::AiConfig::default(),
        }
    }

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
            host_id,
            ..test_agent_config(agent_id)
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
        let agent = Agent::new(test_agent_config(agent_id));

        let event = agent.container_snapshot_event(
            agent_id,
            command_id.clone(),
            ContainerRuntimeSnapshot {
                runtime: "docker".to_string(),
                daemon: None,
                containers: vec![ContainerSummary {
                    id: Some("abc".to_string()),
                    names: vec!["/db".to_string()],
                    image: Some("postgres".to_string()),
                    image_id: None,
                    command: None,
                    created: None,
                    ports: serde_json::json!([]),
                    labels: serde_json::json!({}),
                    state: None,
                    status: None,
                }],
                networks: Vec::new(),
                volumes: Vec::new(),
            },
        );

        let Some(grpc::agent_event::Event::ContainerSnapshot(snapshot)) = event.event else {
            panic!("container event should use typed payload");
        };
        assert_eq!(snapshot.command_id, command_id);
        assert_eq!(snapshot.containers.len(), 1);
        assert_eq!(snapshot.containers[0].id, "abc");
    }

    #[test]
    fn log_line_event_preserves_runtime_log_fields() {
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(test_agent_config(agent_id));
        let log_id = Uuid::new_v4();

        let event = agent.log_line_event(
            agent_id,
            AgentRuntimeLog {
                id: log_id,
                level: "INFO".to_string(),
                target: "doro_agent".to_string(),
                message: "agent connected".to_string(),
                fields: serde_json::json!({"message": "agent connected"}),
            },
        );

        let Some(grpc::agent_event::Event::LogLine(log)) = event.event else {
            panic!("log line event should use typed payload");
        };
        assert_eq!(log.log_id, log_id.to_string());
        assert_eq!(log.level, "INFO");
        assert_eq!(log.message, "agent connected");
    }

    #[test]
    fn runtime_log_hub_keeps_bounded_tail() {
        let hub = AgentRuntimeLogHub::default();
        for index in 0..250 {
            hub.push(AgentRuntimeLog {
                id: Uuid::new_v4(),
                level: "INFO".to_string(),
                target: "doro_agent".to_string(),
                message: format!("line {index}"),
                fields: serde_json::json!({}),
            });
        }

        let snapshot = hub.snapshot();
        assert_eq!(snapshot.len(), AGENT_RUNTIME_LOG_LIMIT);
        assert_eq!(
            snapshot.first().map(|entry| entry.message.as_str()),
            Some("line 50")
        );
        assert_eq!(
            snapshot.last().map(|entry| entry.message.as_str()),
            Some("line 249")
        );
    }

    #[tokio::test]
    async fn handle_command_continues_for_ack() {
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(test_agent_config(agent_id));
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

        let terminal = match TerminalManager::new() {
            Ok(terminal) => terminal,
            Err(error) => panic!("terminal should start: {error}"),
        };
        let command_state = AgentCommandState::default();
        let action = handle_command(
            command,
            &agent,
            agent_id,
            &sender,
            &terminal,
            &command_state,
        )
        .await;

        assert_eq!(action, AgentCommandAction::Continue);
    }

    #[tokio::test]
    async fn handle_command_returns_reconnect_for_shutdown_command() {
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(test_agent_config(agent_id));
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

        let terminal = match TerminalManager::new() {
            Ok(terminal) => terminal,
            Err(error) => panic!("terminal should start: {error}"),
        };
        let command_state = AgentCommandState::default();
        let action = handle_command(
            command,
            &agent,
            agent_id,
            &sender,
            &terminal,
            &command_state,
        )
        .await;

        assert_eq!(action, AgentCommandAction::Reconnect);
    }

    #[test]
    fn docker_manage_capability_is_declared_when_enabled() {
        let agent = Agent::new(AgentConfig::new("doro-test", "http://127.0.0.1:8788"));

        assert!(agent.capabilities().iter().any(|capability| {
            capability.name == CapabilityName::ContainersManage
                && capability.risk == CapabilityRisk::High
        }));
    }

    #[test]
    fn agent_run_capability_is_declared_for_ai_operations() {
        let agent = Agent::new(AgentConfig::new("doro-test", "http://127.0.0.1:8788"));

        assert!(agent.capabilities().iter().any(|capability| {
            capability.name == CapabilityName::AgentRun && capability.risk == CapabilityRisk::Medium
        }));
    }

    #[test]
    fn network_expose_capability_is_omitted_when_website_runtime_disabled() {
        let base_config = test_agent_config(Uuid::new_v4());

        let agent = Agent::new(AgentConfig {
            websites: doro_config::WebsiteConfig {
                enabled: false,
                ..doro_config::WebsiteConfig::default()
            },
            ..base_config
        });

        assert!(
            !agent
                .capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::NetworkExpose)
        );
    }

    #[test]
    fn invalid_website_routes_do_not_replace_previous_route_table() {
        let runtime = WebsiteRuntimeHandle::default();
        let mut route = grpc::WebsiteRoute {
            website_id: Uuid::new_v4().to_string(),
            primary_domain: "example.com".to_string(),
            aliases: Vec::new(),
            status: "running".to_string(),
            kind: "reverse_proxy".to_string(),
            protocol: "http".to_string(),
            listen_port: 8080,
            upstream_url: "http://127.0.0.1:8787".to_string(),
            config_json: "{}".to_string(),
        };

        let count = apply_website_routes(&runtime, vec![route.clone()])
            .unwrap_or_else(|error| panic!("valid website route should apply: {error}"));
        assert_eq!(count, 1);
        assert_eq!(runtime.route_count(), 1);
        assert!(runtime.route_for_host("example.com:8080").is_some());

        route.website_id = "not-a-uuid".to_string();
        let error = apply_website_routes(&runtime, vec![route])
            .expect_err("invalid website route should fail");
        assert!(error.contains("website_id"));
        assert_eq!(runtime.route_count(), 1);
        assert!(runtime.route_for_host("example.com:8080").is_some());
    }

    #[test]
    fn docker_manage_capability_is_omitted_when_disabled() {
        let base_config = test_agent_config(Uuid::new_v4());

        let agent = Agent::new(AgentConfig {
            docker_manage_enabled: false,
            ..base_config
        });

        assert!(
            !agent
                .capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::ContainersManage)
        );
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
