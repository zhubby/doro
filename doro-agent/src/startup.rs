use crate::runtime::Agent;
use doro_protocol::{CapabilityName, CapabilityRisk};
use std::path::Path;
use std::time::Duration;

pub(crate) fn print_agent_startup_summary(
    config_path: &Path,
    config_created: bool,
    file_config: &doro_config::AgentFileConfig,
    agent: &Agent,
) {
    print!(
        "{}",
        render_agent_startup_summary(config_path, config_created, file_config, agent)
    );
}

fn render_agent_startup_summary(
    config_path: &Path,
    config_created: bool,
    file_config: &doro_config::AgentFileConfig,
    agent: &Agent,
) -> String {
    let mut output = String::new();
    output.push('\n');
    push_line(
        &mut output,
        "╭────────────────────────────────────────────╮",
    );
    push_line(
        &mut output,
        "│ 🚀 Doro Agent 正在启动                     │",
    );
    push_line(
        &mut output,
        "╰────────────────────────────────────────────╯",
    );
    push_line(
        &mut output,
        &format!(
            "📄 配置文件: {} ({})",
            config_path.display(),
            if config_created {
                "新建"
            } else {
                "已读取"
            }
        ),
    );
    push_line(
        &mut output,
        &format!("🏷️  主机名称: {}", agent.config.hostname),
    );
    push_line(
        &mut output,
        &format!("🔗 控制平面: {}", agent.config.control_plane_url),
    );
    push_line(
        &mut output,
        &format!("🆔 Agent ID: {}", optional_uuid(agent.config.agent_id)),
    );
    push_line(
        &mut output,
        &format!("🧭 Host ID: {}", optional_uuid(file_config.agent.host_id)),
    );
    push_line(
        &mut output,
        &format!(
            "🔐 Enrollment token: {}",
            redacted_option(file_config.agent.enrollment_token.as_deref())
        ),
    );
    push_line(&mut output, "");
    push_line(&mut output, "🧩 子模块状态");
    push_module_line(&mut output, "🔌", "控制平面会话", true, "gRPC stream");
    push_module_line(
        &mut output,
        "💓",
        "心跳上报",
        true,
        &format!("每 {}s", seconds(agent.config.heartbeat_interval)),
    );
    push_module_line(
        &mut output,
        "📊",
        "主机指标采集",
        true,
        &format!("每 {}s", seconds(agent.config.metrics_interval)),
    );
    push_module_line(
        &mut output,
        "🔎",
        "进程追踪",
        !agent.config.process_names.is_empty(),
        &process_tracking_detail(&agent.config.process_names),
    );
    push_module_line(
        &mut output,
        "📦",
        "容器指标采集",
        agent.container_runtime.is_some(),
        runtime_detail(
            &agent.discovery.docker,
            socket_detail(agent.config.docker_socket_path.as_deref()),
        ),
    );
    push_module_line(
        &mut output,
        "🐳",
        "Docker 管理",
        agent.container_runtime.is_some(),
        runtime_detail(
            &agent.discovery.docker,
            socket_detail(agent.config.docker_socket_path.as_deref()),
        ),
    );
    push_module_line(
        &mut output,
        "🧱",
        "Docker Compose",
        agent.discovery.docker_compose.is_available(),
        agent.discovery.docker_compose.detail(),
    );
    push_module_line(
        &mut output,
        "🎮",
        "GPU 指标采集",
        agent.discovery.gpu.is_available(),
        agent.discovery.gpu.detail(),
    );
    push_module_line(
        &mut output,
        "🖥️",
        "虚拟机管理",
        agent.vm_runtime.is_some(),
        runtime_detail(&agent.discovery.vm, &vm_detail(&agent.config)),
    );
    push_module_line(
        &mut output,
        "🌐",
        "网站反向代理",
        agent.website_runtime.is_some(),
        runtime_detail(
            &agent.discovery.website,
            &format!("HTTP {}", agent.config.websites.http_bind),
        ),
    );
    push_module_line(
        &mut output,
        "🧠",
        "AI 本地工具",
        agent.config.ai.provider != "disabled",
        &format!(
            "provider={}, max_turns={}, max_tool_calls={}",
            agent.config.ai.provider,
            agent.config.ai.agent.max_turns.max(1),
            agent.config.ai.agent.max_tool_calls.max(1)
        ),
    );
    push_module_line(
        &mut output,
        "🪵",
        "运行日志转发",
        true,
        "上线后同步到控制平面",
    );
    push_module_line(
        &mut output,
        "📁",
        "文件操作",
        true,
        "按控制平面审批策略执行",
    );
    push_module_line(&mut output, "⌨️", "终端命令", true, "高风险操作需要审批");
    push_line(&mut output, "");
    push_line(&mut output, "🛡️  声明能力");
    for capability in agent.capabilities() {
        push_line(
            &mut output,
            &format!(
                "  • {} [{}] {}",
                capability_name_label(capability.name),
                risk_label(capability.risk),
                capability.description
            ),
        );
    }
    push_line(&mut output, "");
    push_line(&mut output, "⚙️  启动配置内容 (敏感字段已隐藏)");
    for line in redacted_config_toml(file_config).lines() {
        push_line(&mut output, &format!("  {line}"));
    }
    push_line(&mut output, "");
    output
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn push_module_line(output: &mut String, emoji: &str, name: &str, enabled: bool, detail: &str) {
    push_line(
        output,
        &format!(
            "  {} {} {:<18} {}",
            if enabled { "✅" } else { "⏸️" },
            emoji,
            name,
            detail
        ),
    );
}

fn redacted_config_toml(file_config: &doro_config::AgentFileConfig) -> String {
    let mut redacted = file_config.clone();
    if redacted.agent.enrollment_token.is_some() {
        redacted.agent.enrollment_token = Some("<redacted>".to_string());
    }

    toml::to_string_pretty(&redacted)
        .map(|body| body.trim().to_string())
        .unwrap_or_else(|error| format!("# failed to render agent config: {error}"))
}

fn optional_uuid(value: Option<uuid::Uuid>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "未注册".to_string())
}

fn redacted_option(value: Option<&str>) -> &'static str {
    if value.is_some() {
        "已设置 (<redacted>)"
    } else {
        "未设置"
    }
}

fn seconds(duration: Duration) -> u64 {
    duration.as_secs().max(1)
}

fn socket_detail(socket_path: Option<&str>) -> &str {
    socket_path.unwrap_or("默认 Docker socket")
}

fn runtime_detail<'a>(
    availability: &'a crate::runtime::RuntimeAvailability,
    fallback: &'a str,
) -> &'a str {
    if availability.detail().is_empty() {
        fallback
    } else {
        availability.detail()
    }
}

fn process_tracking_detail(process_names: &[String]) -> String {
    if process_names.is_empty() {
        return "未配置进程名单".to_string();
    }
    process_names.join(", ")
}

fn vm_detail(config: &crate::config::AgentConfig) -> String {
    let state_dir = config.vm_state_dir.as_deref().unwrap_or(".doro/vms");
    let image_dir = config.vm_image_dir.as_deref().unwrap_or(".doro/vm-images");
    let network = if config.vm_user_network_enabled {
        "user NAT"
    } else if config.vm_bridge_names.is_empty() {
        "未配置网络"
    } else {
        "bridge"
    };
    format!("state={state_dir}, images={image_dir}, network={network}")
}

fn capability_name_label(name: CapabilityName) -> &'static str {
    match name {
        CapabilityName::MetricsRead => "metrics.read",
        CapabilityName::LogsRead => "logs.read",
        CapabilityName::AgentRun => "agent.run",
        CapabilityName::ServicesManage => "services.manage",
        CapabilityName::ContainersManage => "containers.manage",
        CapabilityName::VirtualMachinesManage => "virtual_machines.manage",
        CapabilityName::FilesRead => "files.read",
        CapabilityName::FilesWrite => "files.write",
        CapabilityName::ShellExecute => "shell.execute",
        CapabilityName::NetworkExpose => "network.expose",
        CapabilityName::DatabaseRestore => "database.restore",
    }
}

fn risk_label(risk: CapabilityRisk) -> &'static str {
    match risk {
        CapabilityRisk::Low => "低风险",
        CapabilityRisk::Medium => "中风险",
        CapabilityRisk::High => "高风险",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfig;

    #[test]
    fn startup_summary_redacts_enrollment_token() {
        let file_config = doro_config::AgentFileConfig {
            agent: doro_config::AgentConfig {
                enrollment_token: Some("super-secret-token".to_string()),
                ..doro_config::AgentConfig::default()
            },
            ..doro_config::AgentFileConfig::default()
        };
        let agent = Agent::new(AgentConfig::from_file_config(&file_config));

        let output =
            render_agent_startup_summary(Path::new("/tmp/agent.toml"), false, &file_config, &agent);

        assert!(output.contains("<redacted>"));
        assert!(!output.contains("super-secret-token"));
    }

    #[test]
    fn startup_summary_lists_module_states() {
        let file_config = doro_config::AgentFileConfig {
            agent: doro_config::AgentConfig {
                process_names: vec!["doro-control-plane".to_string()],
                ..doro_config::AgentConfig::default()
            },
            ai: doro_config::AiConfig {
                provider: "openai".to_string(),
                ..doro_config::AiConfig::default()
            },
            ..doro_config::AgentFileConfig::default()
        };
        let mut agent = Agent::new(AgentConfig::from_file_config(&file_config));
        agent.discovery.docker_compose = crate::runtime::RuntimeAvailability::Unavailable {
            reason: "docker compose missing".to_string(),
        };
        agent.discovery.vm = crate::runtime::RuntimeAvailability::Available {
            detail: "QEMU".to_string(),
        };
        agent.vm_runtime = Some(crate::runtime::VmRuntime {
            provider: std::sync::Arc::new(doro_vm::QemuProvider::new(
                doro_vm::QemuProviderConfig::default(),
            )),
        });

        let output =
            render_agent_startup_summary(Path::new("/tmp/agent.toml"), true, &file_config, &agent);

        assert!(output.contains("配置文件: /tmp/agent.toml (新建)"));
        assert!(output.contains("进程追踪"));
        assert!(output.contains("doro-control-plane"));
        assert!(output.contains("Docker 管理"));
        assert!(output.contains("Docker Compose"));
        assert!(output.contains("docker compose missing"));
        assert!(output.contains("虚拟机管理"));
        assert!(output.contains("AI 本地工具"));
        assert!(output.contains("provider=openai"));
        assert!(output.contains("virtual_machines.manage"));
    }
}
