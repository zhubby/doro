use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub agent_id: Option<Uuid>,
    pub host_id: Uuid,
    pub hostname: String,
    pub control_plane_url: String,
    pub enrollment_token: Option<String>,
    pub heartbeat_interval: Duration,
    pub metrics_interval: Duration,
    pub process_names: Vec<String>,
    pub docker_socket_path: Option<String>,
    pub docker_compose_root: Option<String>,
    pub qemu_binary_dir: Option<String>,
    pub vm_state_dir: Option<String>,
    pub vm_image_dir: Option<String>,
    pub vm_bridge_names: Vec<String>,
    pub vm_user_network_enabled: bool,
    pub vm_console_enabled: bool,
    pub vm_vnc_bind: String,
    pub websites: doro_config::WebsiteConfig,
    pub reliability: doro_config::AgentReliabilityConfig,
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
            metrics_interval: Duration::from_secs(10),
            process_names: Vec::new(),
            docker_socket_path: None,
            docker_compose_root: None,
            qemu_binary_dir: None,
            vm_state_dir: None,
            vm_image_dir: None,
            vm_bridge_names: Vec::new(),
            vm_user_network_enabled: true,
            vm_console_enabled: true,
            vm_vnc_bind: "127.0.0.1".to_string(),
            websites: doro_config::WebsiteConfig::default(),
            reliability: doro_config::AgentReliabilityConfig::default(),
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
            metrics_interval: Duration::from_secs(config.metrics_interval_seconds.max(1)),
            process_names: config.process_names.clone(),
            docker_socket_path: config.docker_socket_path.clone(),
            docker_compose_root: config.docker_compose_root.clone(),
            qemu_binary_dir: config.qemu_binary_dir.clone(),
            vm_state_dir: config.vm_state_dir.clone(),
            vm_image_dir: config.vm_image_dir.clone(),
            vm_bridge_names: config.vm_bridge_names.clone(),
            vm_user_network_enabled: config.vm_user_network_enabled,
            vm_console_enabled: config.vm_console_enabled,
            vm_vnc_bind: config.vm_vnc_bind.clone(),
            websites: doro_config::WebsiteConfig::default(),
            reliability: doro_config::AgentReliabilityConfig::default(),
            ai,
        }
    }

    pub fn from_file_config(config: &doro_config::AgentFileConfig) -> Self {
        let mut agent = Self::from_config_with_ai(&config.agent, config.ai.clone());
        agent.websites = config.websites.clone();
        agent.reliability = config.reliability.clone();
        agent
    }
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
}
