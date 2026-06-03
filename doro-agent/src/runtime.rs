use crate::collectors::system_profile;
use crate::compose::{ComposeCommandError, ComposeManager};
use crate::config::AgentConfig;
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
use doro_vm::{QemuProvider, QemuProviderConfig, VirtualMachineProvider};
use doro_website::{WebsiteRuntime, WebsiteRuntimeConfig, WebsiteRuntimeHandle};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ContainerRuntime {
    provider: Result<Arc<dyn ContainerProvider>, String>,
    executor: Option<ContainerRuntimeExecutor>,
    compose: Option<ComposeManager>,
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
        let docker = DockerProvider::connect(&DockerProviderConfig::new(
            config.docker_socket_path.clone(),
        ));
        let executor = docker
            .as_ref()
            .ok()
            .filter(|_| config.docker_manage_enabled)
            .cloned()
            .map(ContainerRuntimeExecutor::new);
        let provider = docker
            .map(|provider| Arc::new(provider) as Arc<dyn ContainerProvider>)
            .map_err(|error| error.to_string());
        let compose = if config.docker_manage_enabled && config.docker_compose_enabled {
            match ComposeManager::from_config(config.docker_compose_root.as_deref()) {
                Ok(manager) => Some(manager),
                Err(error) => {
                    tracing::warn!(%error, "failed to initialize Docker Compose manager");
                    None
                }
            }
        } else {
            None
        };
        Some(Self {
            provider,
            executor,
            compose,
        })
    }

    pub(crate) async fn snapshot(
        &self,
    ) -> Result<ContainerRuntimeSnapshot, doro_container::ContainerProviderError> {
        match &self.provider {
            Ok(provider) => provider.snapshot().await,
            Err(error) => Err(doro_container::ContainerProviderError::InvalidRequest(
                format!("failed to initialize container provider: {error}"),
            )),
        }
    }

    pub(crate) async fn execute(
        &self,
        envelope: ContainerRuntimeCommandEnvelope,
    ) -> doro_container::ContainerCommandResult {
        match envelope.command {
            doro_container::ContainerRuntimeCommand::Compose(command) => {
                let Some(compose) = &self.compose else {
                    return doro_container::ContainerCommandResult {
                        command_id: envelope.command_id,
                        status: doro_container::ContainerCommandStatus::Failed,
                        message: "docker compose is not enabled".to_string(),
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
                let Some(executor) = &self.executor else {
                    return doro_container::ContainerCommandResult {
                        command_id: envelope.command_id,
                        status: doro_container::ContainerCommandStatus::Failed,
                        message: "docker management is not enabled".to_string(),
                        details: serde_json::json!({}),
                    };
                };
                executor
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
    pub(crate) config: AgentConfig,
    pub(crate) container_runtime: Option<ContainerRuntime>,
    pub(crate) vm_runtime: Option<VmRuntime>,
    pub(crate) website_runtime: Option<WebsiteRuntimeHandle>,
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
}
