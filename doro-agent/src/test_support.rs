use crate::AgentConfig;
use std::time::Duration;
use uuid::Uuid;

pub(crate) fn test_agent_config(agent_id: Uuid) -> AgentConfig {
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
        docker_compose_enabled: false,
        docker_compose_root: None,
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
        reliability: doro_config::AgentReliabilityConfig::default(),
    }
}
