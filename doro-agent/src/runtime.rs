use crate::collectors::{collect_gpu, system_profile};
use crate::command_registry::CommandRegistry;
use crate::compose::{ComposeCommandError, ComposeManager};
use crate::config::AgentConfig;
use crate::event_spool::EventSpool;
use crate::registry_config::DockerRegistryConfigManager;
use chrono::Utc;
use doro_ai::{
    AgentError, AgentRunner, AgentRunnerConfig, DisabledAgentProvider, OpenAiAgentProvider,
};
use doro_container::{
    ContainerProvider, ContainerRuntimeCommandEnvelope, ContainerRuntimeExecutor,
    ContainerRuntimeSnapshot, DockerProvider, DockerProviderConfig,
};
use doro_protocol::{
    AgentCapability, AgentEvent, CapabilityName, CapabilityRisk, Host, HostStatus, MetricSnapshot,
    grpc, protobuf_timestamp_now,
};
use doro_vm::network::NetworkPolicy;
use doro_vm::{QemuProvider, QemuProviderConfig, VirtualMachineInventory, VirtualMachineProvider};
use doro_website::{WebsiteRuntime, WebsiteRuntimeConfig, WebsiteRuntimeHandle};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ContainerRuntime {
    provider: Arc<dyn ContainerProvider>,
    executor: ContainerRuntimeExecutor,
    compose: Option<ComposeManager>,
    registry_config: Option<DockerRegistryConfigManager>,
}

impl std::fmt::Debug for ContainerRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ContainerRuntime")
    }
}

impl ContainerRuntime {
    async fn discover(
        config: &AgentConfig,
    ) -> (Option<Self>, RuntimeAvailability, RuntimeAvailability) {
        let registry_config = match DockerRegistryConfigManager::from_default_config_dir() {
            Ok(manager) => Some(manager),
            Err(error) => {
                tracing::warn!(%error, "Docker registry config manager unavailable");
                None
            }
        };
        let docker_config_dir = registry_config
            .as_ref()
            .map(|manager| manager.config_dir().to_path_buf());
        let docker = match DockerProvider::connect(
            &DockerProviderConfig::new(config.docker_socket_path.clone())
                .with_config_dir(docker_config_dir.clone()),
        ) {
            Ok(provider) => provider,
            Err(error) => {
                let message = error.to_string();
                tracing::warn!(%message, "Docker runtime unavailable");
                return (
                    None,
                    RuntimeAvailability::unavailable(message),
                    docker_compose_requires_docker(),
                );
            }
        };
        let info = match docker.probe().await {
            Ok(info) => info,
            Err(error) => {
                let message = error.to_string();
                tracing::warn!(%message, "Docker runtime unavailable");
                return (
                    None,
                    RuntimeAvailability::unavailable(message),
                    docker_compose_requires_docker(),
                );
            }
        };

        let detail = info
            .server_version
            .as_deref()
            .map(|version| format!("Docker {version}"))
            .unwrap_or_else(|| "Docker daemon is available".to_string());
        tracing::info!(detail, "Docker runtime available");
        let executor = ContainerRuntimeExecutor::new(docker.clone());
        let provider = Arc::new(docker) as Arc<dyn ContainerProvider>;
        let (compose, docker_compose) = discover_compose(config, docker_config_dir);
        (
            Some(Self {
                provider,
                executor,
                compose,
                registry_config,
            }),
            RuntimeAvailability::available(detail),
            docker_compose,
        )
    }

    #[cfg(test)]
    fn from_docker_provider(provider: DockerProvider) -> Self {
        Self {
            provider: Arc::new(provider.clone()),
            executor: ContainerRuntimeExecutor::new(provider),
            compose: None,
            registry_config: None,
        }
    }

    pub(crate) async fn snapshot(
        &self,
    ) -> Result<ContainerRuntimeSnapshot, doro_container::ContainerProviderError> {
        self.provider.snapshot().await
    }

    pub(crate) async fn execute(
        &self,
        envelope: ContainerRuntimeCommandEnvelope,
    ) -> doro_container::ContainerCommandResult {
        match envelope.command {
            doro_container::ContainerRuntimeCommand::Registry(command) => {
                let Some(registry_config) = &self.registry_config else {
                    return doro_container::ContainerCommandResult {
                        command_id: envelope.command_id,
                        status: doro_container::ContainerCommandStatus::Failed,
                        message: "docker registry config is not available".to_string(),
                        details: serde_json::json!({}),
                    };
                };
                match registry_config.execute(command) {
                    Ok(details) => doro_container::ContainerCommandResult {
                        command_id: envelope.command_id,
                        status: doro_container::ContainerCommandStatus::Succeeded,
                        message: "docker registry config command succeeded".to_string(),
                        details,
                    },
                    Err(error) => doro_container::ContainerCommandResult {
                        command_id: envelope.command_id,
                        status: doro_container::ContainerCommandStatus::Failed,
                        message: error.to_string(),
                        details: serde_json::json!({}),
                    },
                }
            }
            doro_container::ContainerRuntimeCommand::Compose(command) => {
                let Some(compose) = &self.compose else {
                    return doro_container::ContainerCommandResult {
                        command_id: envelope.command_id,
                        status: doro_container::ContainerCommandStatus::Failed,
                        message: "docker compose is not available".to_string(),
                        details: serde_json::json!({}),
                    };
                };
                match compose.execute(envelope.command_id, command) {
                    Ok(result) => result,
                    Err(error) => {
                        let details = error
                            .downcast_ref::<ComposeCommandError>()
                            .map(|error| serde_json::json!(error.output))
                            .unwrap_or_else(|| serde_json::json!({}));
                        doro_container::ContainerCommandResult {
                            command_id: envelope.command_id,
                            status: doro_container::ContainerCommandStatus::Failed,
                            message: error.to_string(),
                            details,
                        }
                    }
                }
            }
            command => {
                self.executor
                    .execute(ContainerRuntimeCommandEnvelope {
                        command_id: envelope.command_id,
                        task_id: envelope.task_id,
                        step_id: envelope.step_id,
                        command,
                    })
                    .await
            }
        }
    }
}

fn discover_compose(
    config: &AgentConfig,
    docker_config_dir: Option<PathBuf>,
) -> (Option<ComposeManager>, RuntimeAvailability) {
    match ComposeManager::probe_cli() {
        Ok(version) => match ComposeManager::from_config(config.docker_compose_root.as_deref()) {
            Ok(manager) => {
                tracing::info!(version, "Docker Compose available");
                (
                    Some(manager.with_docker_config_dir(docker_config_dir)),
                    RuntimeAvailability::available(version),
                )
            }
            Err(error) => {
                let reason = error.to_string();
                tracing::warn!(%reason, "Docker Compose manager unavailable");
                (None, RuntimeAvailability::unavailable(reason))
            }
        },
        Err(error) => {
            let reason = error.to_string();
            tracing::warn!(%reason, "Docker Compose unavailable");
            (None, RuntimeAvailability::unavailable(reason))
        }
    }
}

fn docker_compose_requires_docker() -> RuntimeAvailability {
    let reason = "Docker Compose requires an available Docker runtime".to_string();
    tracing::warn!(%reason, "Docker Compose unavailable");
    RuntimeAvailability::unavailable(reason)
}

#[derive(Clone)]
pub(crate) struct VmRuntime {
    pub(crate) provider: Arc<dyn VirtualMachineProvider>,
}

impl std::fmt::Debug for VmRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("VmRuntime")
    }
}

impl VmRuntime {
    async fn discover(config: &AgentConfig) -> (Option<Self>, RuntimeAvailability) {
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
        match provider.probe().await {
            Ok(status) if status.available => {
                tracing::info!(message = %status.message, "QEMU runtime available");
                let detail = status
                    .version
                    .unwrap_or_else(|| "QEMU is available".to_string());
                (
                    Some(Self {
                        provider: Arc::new(provider),
                    }),
                    RuntimeAvailability::available(detail),
                )
            }
            Ok(status) => {
                tracing::warn!(message = %status.message, "QEMU runtime unavailable");
                (None, RuntimeAvailability::unavailable(status.message))
            }
            Err(error) => {
                let message = error.to_string();
                tracing::warn!(%message, "QEMU runtime unavailable");
                (None, RuntimeAvailability::unavailable(message))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeDiscovery {
    pub(crate) docker: RuntimeAvailability,
    pub(crate) docker_compose: RuntimeAvailability,
    pub(crate) vm: RuntimeAvailability,
    pub(crate) gpu: RuntimeAvailability,
    pub(crate) website: RuntimeAvailability,
}

impl Default for RuntimeDiscovery {
    fn default() -> Self {
        Self {
            docker: RuntimeAvailability::not_checked(),
            docker_compose: RuntimeAvailability::not_checked(),
            vm: RuntimeAvailability::not_checked(),
            gpu: RuntimeAvailability::not_checked(),
            website: RuntimeAvailability::not_checked(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeAvailability {
    Available { detail: String },
    Unavailable { reason: String },
    NotChecked,
}

impl RuntimeAvailability {
    fn available(detail: impl Into<String>) -> Self {
        Self::Available {
            detail: detail.into(),
        }
    }

    fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable {
            reason: reason.into(),
        }
    }

    fn not_checked() -> Self {
        Self::NotChecked
    }

    pub(crate) fn is_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }

    pub(crate) fn detail(&self) -> &str {
        match self {
            Self::Available { detail } => detail,
            Self::Unavailable { reason } => reason,
            Self::NotChecked => "未探测",
        }
    }
}

fn discover_gpu() -> RuntimeAvailability {
    match collect_gpu() {
        Ok(gpus) => {
            let count = gpus.as_array().map_or(0, Vec::len);
            if count == 0 {
                let reason = "no NVIDIA GPUs detected".to_string();
                tracing::warn!(%reason, "GPU collector unavailable");
                return RuntimeAvailability::unavailable(reason);
            }
            let detail = if count == 1 {
                "1 GPU detected".to_string()
            } else {
                format!("{count} GPUs detected")
            };
            tracing::info!(detail, "GPU collector available");
            RuntimeAvailability::available(detail)
        }
        Err(error) => {
            let reason = error.to_string();
            tracing::warn!(%reason, "GPU collector unavailable");
            RuntimeAvailability::unavailable(reason)
        }
    }
}

fn discover_website(config: &AgentConfig) -> (Option<WebsiteRuntimeHandle>, RuntimeAvailability) {
    let runtime_config = WebsiteRuntimeConfig {
        http_bind: config.websites.http_bind.clone(),
    };
    match WebsiteRuntime::check_http_bind(&runtime_config) {
        Ok(()) => {
            let detail = format!("HTTP {}", config.websites.http_bind);
            tracing::info!(detail, "website runtime available");
            (
                Some(WebsiteRuntimeHandle::default()),
                RuntimeAvailability::available(detail),
            )
        }
        Err(error) => {
            let reason = error.to_string();
            tracing::warn!(%reason, "website runtime unavailable");
            (None, RuntimeAvailability::unavailable(reason))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub(crate) config: AgentConfig,
    pub(crate) container_runtime: Option<ContainerRuntime>,
    pub(crate) vm_runtime: Option<VmRuntime>,
    pub(crate) website_runtime: Option<WebsiteRuntimeHandle>,
    pub(crate) discovery: RuntimeDiscovery,
    pub(crate) command_registry: CommandRegistry,
    pub(crate) event_spool: Arc<Mutex<EventSpool>>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            event_spool: Arc::new(Mutex::new(EventSpool::from_config(&config.reliability))),
            config,
            container_runtime: None,
            vm_runtime: None,
            website_runtime: None,
            discovery: RuntimeDiscovery::default(),
            command_registry: CommandRegistry::default(),
        }
    }

    pub async fn discover(config: AgentConfig) -> Self {
        let (container_runtime, docker, docker_compose) = ContainerRuntime::discover(&config).await;
        let (vm_runtime, vm) = VmRuntime::discover(&config).await;
        let gpu = discover_gpu();
        let (website_runtime, website) = discover_website(&config);
        Self {
            event_spool: Arc::new(Mutex::new(EventSpool::from_config(&config.reliability))),
            config,
            container_runtime,
            vm_runtime,
            website_runtime,
            discovery: RuntimeDiscovery {
                docker,
                docker_compose,
                vm,
                gpu,
                website,
            },
            command_registry: CommandRegistry::default(),
        }
    }

    pub fn start_website_runtime(&self) -> anyhow::Result<Option<std::thread::JoinHandle<()>>> {
        let Some(handle) = self.website_runtime.clone() else {
            return Ok(None);
        };
        let runtime = WebsiteRuntime::with_handle(
            WebsiteRuntimeConfig {
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
        if self.container_runtime.is_some() {
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
            system_profile_json: system_profile().to_string(),
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

    pub(crate) fn ai_runner_for_command(
        &self,
        provider_config: Option<&grpc::AgentAiProviderConfig>,
    ) -> Result<AgentRunner, AgentError> {
        if let Some(provider_config) = provider_config {
            return self.ai_runner_from_command_provider(provider_config);
        }

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

    fn ai_runner_from_command_provider(
        &self,
        provider_config: &grpc::AgentAiProviderConfig,
    ) -> Result<AgentRunner, AgentError> {
        if provider_config.provider_type != "openai_responses" {
            return Err(AgentError::Model(format!(
                "unsupported AI provider for agent task: {}",
                provider_config.provider_type
            )));
        }
        let model = provider_config.model.trim();
        if model.is_empty() {
            return Err(AgentError::Model(
                "AI provider model is required for agent task".to_string(),
            ));
        }
        let api_key = provider_config.api_key.trim();
        if api_key.is_empty() {
            return Err(AgentError::Model(
                "AI provider API key is required for agent task".to_string(),
            ));
        }
        let base_url = provider_config.base_url.trim();
        if base_url.is_empty() {
            return Err(AgentError::Model(
                "AI provider base URL is required for agent task".to_string(),
            ));
        }

        let openai_config = doro_ai::openai::OpenAiConfig {
            api_key_env: String::new(),
            base_url: base_url.to_string(),
            timeout_seconds: u64::from(provider_config.timeout_seconds.max(1)),
        };
        let client = doro_ai::openai::OpenAiClient::with_api_key(openai_config, api_key)
            .map_err(|error| AgentError::Model(error.to_string()))?;
        let provider: Arc<dyn doro_ai::AgentModelProvider> =
            Arc::new(OpenAiAgentProvider::new(client, model.to_string()));

        Ok(AgentRunner::new(
            provider,
            self.ai_tool_definitions(),
            AgentRunnerConfig {
                max_turns: self.config.ai.agent.max_turns.max(1),
                max_tool_calls: self.config.ai.agent.max_tool_calls.max(1),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_agent_config;

    #[test]
    fn docker_manage_capability_is_declared_when_runtime_is_available() {
        let mut agent = Agent::new(AgentConfig::new("doro-test", "http://127.0.0.1:8788"));
        let docker = DockerProvider::connect(&DockerProviderConfig::new(None))
            .unwrap_or_else(|error| panic!("Docker provider should initialize: {error}"));
        agent.container_runtime = Some(ContainerRuntime::from_docker_provider(docker));

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
    fn command_ai_provider_takes_precedence_over_local_disabled_config() {
        let agent = Agent::new(AgentConfig::new("doro-test", "http://127.0.0.1:8788"));

        let runner = agent.ai_runner_for_command(Some(&grpc::AgentAiProviderConfig {
            provider_type: "openai_responses".to_string(),
            name: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4.1-mini".to_string(),
            api_key: "sk-test".to_string(),
            timeout_seconds: 60,
        }));

        assert!(runner.is_ok());
    }

    #[test]
    fn local_ai_config_is_used_when_command_provider_is_missing() {
        let agent = Agent::new(AgentConfig::new("doro-test", "http://127.0.0.1:8788"));

        let runner = agent.ai_runner_for_command(None);

        assert!(runner.is_ok());
    }

    #[test]
    fn network_expose_capability_is_omitted_without_website_runtime() {
        let agent = Agent::new(test_agent_config(Uuid::new_v4()));

        assert!(
            !agent
                .capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::NetworkExpose)
        );
    }

    #[test]
    fn network_expose_capability_is_declared_when_website_runtime_is_available() {
        let mut agent = Agent::new(test_agent_config(Uuid::new_v4()));
        agent.website_runtime = Some(WebsiteRuntimeHandle::default());

        assert!(
            agent
                .capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::NetworkExpose)
        );
    }

    #[test]
    fn docker_manage_capability_is_omitted_without_container_runtime() {
        let agent = Agent::new(test_agent_config(Uuid::new_v4()));

        assert!(
            !agent
                .capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::ContainersManage)
        );
    }
}
