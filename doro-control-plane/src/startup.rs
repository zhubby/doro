use std::path::Path;

pub(crate) fn print_control_plane_startup_summary(
    config_path: Option<&Path>,
    config_created: bool,
    config: &doro_config::ControlPlaneConfig,
) {
    print!(
        "{}",
        render_control_plane_startup_summary(config_path, config_created, config)
    );
}

fn render_control_plane_startup_summary(
    config_path: Option<&Path>,
    config_created: bool,
    config: &doro_config::ControlPlaneConfig,
) -> String {
    let mut output = String::new();
    output.push('\n');
    push_line(
        &mut output,
        "╭────────────────────────────────────────────╮",
    );
    push_line(
        &mut output,
        "│ 🛠️  Doro Control Plane 正在启动            │",
    );
    push_line(
        &mut output,
        "╰────────────────────────────────────────────╯",
    );
    push_line(
        &mut output,
        &format!(
            "📄 配置来源: {}",
            config_source_label(config_path, config_created)
        ),
    );
    push_line(
        &mut output,
        &format!("🖥️  控制台 API: http://{}", config.server.console_bind),
    );
    push_line(
        &mut output,
        &format!("🔌 Agent gRPC: http://{}", config.server.agent_bind),
    );
    push_line(
        &mut output,
        &format!(
            "🗄️  数据库: {}",
            redacted_database_url(&config.store.database_url)
        ),
    );
    push_line(
        &mut output,
        &format!(
            "🔐 安全策略: approval_policy={}, tls={}, jwt_secret={}",
            config.security.approval_policy,
            enabled_label(config.security.require_tls),
            if config.security.jwt_secret.is_some() {
                "已配置"
            } else {
                "数据库/自动生成"
            }
        ),
    );
    push_line(&mut output, "");
    push_line(&mut output, "🧩 子模块状态");
    push_module_line(&mut output, "🌐", "Console REST API", true, "/api/v1");
    push_module_line(
        &mut output,
        "📡",
        "Agent 协议入口",
        true,
        "doro.agent.v1.AgentControlPlane",
    );
    push_module_line(
        &mut output,
        "🧭",
        "Agent 流注册表",
        true,
        "在线状态与命令分发",
    );
    push_module_line(
        &mut output,
        "🗃️",
        "Postgres Store",
        true,
        &store_pool_detail(&config.store),
    );
    push_module_line(&mut output, "🧬", "数据库迁移", true, "启动时自动校验/执行");
    push_module_line(
        &mut output,
        "🔑",
        "认证服务",
        true,
        "JWT access + refresh token",
    );
    push_module_line(
        &mut output,
        "🛡️",
        "审批策略",
        true,
        &config.security.approval_policy,
    );
    push_module_line(
        &mut output,
        "⏱️",
        "计划任务调度器",
        true,
        "cron tick + dispatch",
    );
    push_module_line(
        &mut output,
        "🪵",
        "运行日志中心",
        true,
        "control-plane 与 agent 日志流",
    );
    push_module_line(
        &mut output,
        "🧠",
        "AI 入口",
        config.ai.provider != "disabled",
        &ai_detail(&config.ai),
    );
    push_line(&mut output, "");
    push_line(&mut output, "🧭 API 面");
    for surface in api_surfaces() {
        push_line(&mut output, &format!("  • {surface}"));
    }
    push_line(&mut output, "");
    push_line(&mut output, "⚙️  启动配置内容 (敏感字段已隐藏)");
    for line in redacted_config_toml(config).lines() {
        push_line(&mut output, &format!("  {line}"));
    }
    push_line(&mut output, "");
    output
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn config_source_label(config_path: Option<&Path>, config_created: bool) -> String {
    match config_path {
        Some(path) if config_created => format!("{} (新建)", path.display()),
        Some(path) => format!("{} (已读取)", path.display()),
        None => "环境变量/默认值（未使用配置文件）".to_string(),
    }
}

fn push_module_line(output: &mut String, emoji: &str, name: &str, enabled: bool, detail: &str) {
    push_line(
        output,
        &format!(
            "  {} {} {:<20} {}",
            if enabled { "✅" } else { "⏸️" },
            emoji,
            name,
            detail
        ),
    );
}

fn redacted_config_toml(config: &doro_config::ControlPlaneConfig) -> String {
    let mut redacted = config.clone();
    redacted.store.database_url = redacted_database_url(&redacted.store.database_url);
    if redacted.security.jwt_secret.is_some() {
        redacted.security.jwt_secret = Some("<redacted>".to_string());
    }

    toml::to_string_pretty(&redacted)
        .map(|body| body.trim().to_string())
        .unwrap_or_else(|error| format!("# failed to render control-plane config: {error}"))
}

fn redacted_database_url(database_url: &str) -> String {
    let Some((scheme, rest)) = database_url.split_once("://") else {
        return "<configured>".to_string();
    };
    let Some((authority, address)) = rest.split_once('@') else {
        return database_url.to_string();
    };
    if !authority.contains(':') {
        return database_url.to_string();
    }

    format!("{scheme}://<redacted>@{address}")
}

fn enabled_label(enabled: bool) -> &'static str {
    if enabled { "enabled" } else { "disabled" }
}

fn store_pool_detail(config: &doro_config::StoreConfig) -> String {
    format!(
        "backend={}, min={}, max={}, connect_timeout={}s",
        config.backend,
        config.min_connections,
        config.max_connections,
        config.connect_timeout_seconds
    )
}

fn ai_detail(config: &doro_config::AiConfig) -> String {
    format!(
        "provider={}, model={}, timeout={}s",
        config.provider, config.openai.default_response_model, config.openai.timeout_seconds
    )
}

fn api_surfaces() -> [&'static str; 13] {
    [
        "/health",
        "/api/v1/auth/*",
        "/api/v1/hosts",
        "/api/v1/alerts/*",
        "/api/v1/notifications/*",
        "/api/v1/tasks",
        "/api/v1/approvals",
        "/api/v1/scheduled-tasks",
        "/api/v1/websites",
        "/api/v1/virtual-machines",
        "/api/v1/files/:host_id/*",
        "/api/v1/logs/*",
        "/api/v1/control-plane/environment",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_summary_redacts_secrets() {
        let config = doro_config::ControlPlaneConfig {
            store: doro_config::StoreConfig {
                database_url: "postgres://doro:secret@127.0.0.1:5432/doro".to_string(),
                ..doro_config::StoreConfig::default()
            },
            security: doro_config::SecurityConfig {
                jwt_secret: Some("jwt-secret-value".to_string()),
                ..doro_config::SecurityConfig::default()
            },
            ..doro_config::ControlPlaneConfig::default()
        };

        let output = render_control_plane_startup_summary(
            Some(Path::new("/tmp/control-plane.toml")),
            false,
            &config,
        );

        assert!(output.contains("postgres://<redacted>@127.0.0.1:5432/doro"));
        assert!(output.contains("jwt_secret = \"<redacted>\""));
        assert!(!output.contains("secret@"));
        assert!(!output.contains("jwt-secret-value"));
    }

    #[test]
    fn startup_summary_lists_runtime_modules() {
        let config = doro_config::ControlPlaneConfig {
            ai: doro_config::AiConfig {
                provider: "openai".to_string(),
                ..doro_config::AiConfig::default()
            },
            ..doro_config::ControlPlaneConfig::default()
        };

        let output = render_control_plane_startup_summary(
            Some(Path::new("/tmp/control-plane.toml")),
            true,
            &config,
        );

        assert!(output.contains("配置来源: /tmp/control-plane.toml (新建)"));
        assert!(output.contains("Console REST API"));
        assert!(output.contains("Agent 协议入口"));
        assert!(output.contains("计划任务调度器"));
        assert!(output.contains("AI 入口"));
        assert!(output.contains("provider=openai"));
        assert!(output.contains("/api/v1/approvals"));
    }

    #[test]
    fn startup_summary_reports_configless_source() {
        let output = render_control_plane_startup_summary(
            None,
            false,
            &doro_config::ControlPlaneConfig::default(),
        );

        assert!(output.contains("配置来源: 环境变量/默认值（未使用配置文件）"));
    }
}
