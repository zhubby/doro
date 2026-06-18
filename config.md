# Doro Configuration

Doro configuration is intentionally small while the control plane and agent protocol stabilize.

The default Agent path is:

```text
~/.doro/agent.toml
```

The control plane can run without a config file. It starts from defaults, applies any existing `~/.doro/control-plane.toml`, and then applies `DORO_CONTROL_PLANE_*` environment variable overrides. CLI service commands still accept a global TOML override when a file is desired:

```bash
doro --config /path/to/control-plane.toml control-plane
doro --config /path/to/agent.toml agent
```

## Control Plane

The control plane defaults to:

```toml
[server]
console_bind = "0.0.0.0:8787"
agent_bind = "0.0.0.0:8788"

[store]
backend = "postgres"
database_url = "postgres://doro:doro@127.0.0.1:5432/doro"
max_connections = 10
min_connections = 1
connect_timeout_seconds = 8
idle_timeout_seconds = 300

[security]
approval_policy = "policy_and_human_approval"
require_tls = false
```

Production deployments should enable TLS and store secrets outside the repository.

Every control-plane field can be overridden with environment variables. Common examples:

```bash
DORO_CONTROL_PLANE_CONSOLE_BIND=0.0.0.0:8787
DORO_CONTROL_PLANE_AGENT_BIND=0.0.0.0:8788
DORO_CONTROL_PLANE_DATABASE_URL=postgres://doro:doro@postgres.lan:5432/doro
DORO_CONTROL_PLANE_REQUIRE_TLS=false
DORO_CONTROL_PLANE_AI_PROVIDER=disabled
```

Full control-plane environment variable mapping:

| TOML field | Environment variable |
| --- | --- |
| `server.console_bind` | `DORO_CONTROL_PLANE_CONSOLE_BIND` |
| `server.agent_bind` | `DORO_CONTROL_PLANE_AGENT_BIND` |
| `store.backend` | `DORO_CONTROL_PLANE_STORE_BACKEND` |
| `store.database_url` | `DORO_CONTROL_PLANE_DATABASE_URL` |
| `store.max_connections` | `DORO_CONTROL_PLANE_STORE_MAX_CONNECTIONS` |
| `store.min_connections` | `DORO_CONTROL_PLANE_STORE_MIN_CONNECTIONS` |
| `store.connect_timeout_seconds` | `DORO_CONTROL_PLANE_STORE_CONNECT_TIMEOUT_SECONDS` |
| `store.idle_timeout_seconds` | `DORO_CONTROL_PLANE_STORE_IDLE_TIMEOUT_SECONDS` |
| `security.approval_policy` | `DORO_CONTROL_PLANE_APPROVAL_POLICY` |
| `security.require_tls` | `DORO_CONTROL_PLANE_REQUIRE_TLS` |
| `security.jwt_secret` | `DORO_CONTROL_PLANE_JWT_SECRET` |
| `ai.provider` | `DORO_CONTROL_PLANE_AI_PROVIDER` |
| `ai.openai.api_key_env` | `DORO_CONTROL_PLANE_OPENAI_API_KEY_ENV` |
| `ai.openai.base_url` | `DORO_CONTROL_PLANE_OPENAI_BASE_URL` |
| `ai.openai.default_chat_model` | `DORO_CONTROL_PLANE_OPENAI_DEFAULT_CHAT_MODEL` |
| `ai.openai.default_response_model` | `DORO_CONTROL_PLANE_OPENAI_DEFAULT_RESPONSE_MODEL` |
| `ai.openai.timeout_seconds` | `DORO_CONTROL_PLANE_OPENAI_TIMEOUT_SECONDS` |
| `ai.agent.max_turns` | `DORO_CONTROL_PLANE_AI_AGENT_MAX_TURNS` |
| `ai.agent.max_tool_calls` | `DORO_CONTROL_PLANE_AI_AGENT_MAX_TOOL_CALLS` |
| `ai.agent.tool_timeout_seconds` | `DORO_CONTROL_PLANE_AI_AGENT_TOOL_TIMEOUT_SECONDS` |
| `ai.agent.shell_timeout_seconds` | `DORO_CONTROL_PLANE_AI_AGENT_SHELL_TIMEOUT_SECONDS` |
| `ai.agent.approval_timeout_seconds` | `DORO_CONTROL_PLANE_AI_AGENT_APPROVAL_TIMEOUT_SECONDS` |

An existing TOML file is still supported for local overrides. Environment variables always win over TOML values.

## Service managers

Doro provides Makefile targets for Linux systemd and macOS launchd. Use the platform-neutral `service` targets to pick the default manager from `uname`:

```bash
make control-plane-service-install
make control-plane-service-start

make agent-service-install
sudoedit /etc/doro/agent.toml
make agent-service-start
```

On Linux, explicit targets use systemd:

```bash
make control-plane-systemd-install
make agent-systemd-install
```

On macOS, explicit targets use launchd and `launchctl`:

```bash
make control-plane-launchd-install
make agent-launchd-install
```

Install targets build the release `doro` binary, install it to `/usr/local/bin/doro`, write Agent TOML when missing, install the service definition, and enable the service manager entry. The control plane service stores its configuration in service environment variables. Install targets do not start services automatically so database, security, and enrollment settings can be reviewed first.

The common platform-neutral service targets are:

```bash
make control-plane-service-status
make control-plane-service-logs
make control-plane-service-restart
make control-plane-service-stop
make control-plane-service-uninstall

make agent-service-status
make agent-service-logs
make agent-service-restart
make agent-service-stop
make agent-service-uninstall
```

Override Makefile variables for host-specific installs:

```bash
make control-plane-systemd-install \
  DORO_CONTROL_PLANE_CONSOLE_BIND=0.0.0.0:8787 \
  DORO_CONTROL_PLANE_AGENT_BIND=0.0.0.0:8788 \
  DORO_CONTROL_PLANE_DATABASE_URL=postgres://doro:doro@postgres.lan:5432/doro
```

## Agent

Agents connect outbound to the control-plane Agent endpoint:

```toml
[agent]
control_plane_url = "http://127.0.0.1:8788"
hostname = "doro-local-agent"
metrics_interval_seconds = 10
process_names = []
docker_socket_path = "/var/run/docker.sock"
docker_compose_root = "/home/doro/.doro/compose"
qemu_binary_dir = "/usr/local/bin"
vm_state_dir = "/var/lib/doro/vms"
vm_image_dir = "/var/lib/doro/vm-images"
vm_bridge_names = []
vm_user_network_enabled = true
vm_console_enabled = true
vm_vnc_bind = "127.0.0.1"

[websites]
http_bind = "127.0.0.1:8080"
```

Enrollment uses a one-time token generated by the control plane or CLI. After enrollment, agents should use a durable identity token over the gRPC transport.

The Agent probes Docker, Docker Compose, QEMU, GPU collector support, and the website HTTP bind address during startup. Missing runtimes are reported in logs and omitted from capability declarations; they no longer require `*_enabled` config flags.

Override Agent variables for host-specific installs:

```bash
make agent-systemd-install \
  DORO_AGENT_CONTROL_PLANE_URL=http://control-plane.lan:8788 \
  DORO_AGENT_HOSTNAME=nas-1 \
  DORO_AGENT_SUPPLEMENTARY_GROUPS=docker
```

## AI

AI providers are advisory. They can produce task plans, summaries, and explanations, but the control plane still enforces policy and approval.

```toml
[ai]
provider = "disabled"
```

Provider-specific settings will be added after the MVP control-plane workflow is complete.
