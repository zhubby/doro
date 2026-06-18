<h1 align="center">Doro</h1>

<p align="center">
  <img src="doro-ui/public/brand/doro-logo.png" alt="Doro logo" width="160" />
</p>

<p align="center">Doro is an AI-native home server control plane.</p>

The project is built around one central control panel and many host agents:

- `doro-control-plane` exposes the API, stores state, orchestrates tasks, handles approvals, and provides the AI entrypoint.
- `doro-agent` runs on each host and exposes host capabilities over gRPC such as metrics, logs, services, containers, files, and command execution.
- `doro-ui` is the Next.js operations console for hosts, tasks, approvals, applications, resources, logs, and settings.

Doro is not a Codex CLI fork. The previous Codex-derived files were removed from the active workspace and the remaining project surface is being rebuilt around the home-server control-plane model.

## Workspace

- `doro-protocol` - shared versioned protocol types for the UI, control plane, and agents.
- `doro-control-plane` - console API, event stream, and agent connection surface.
- `doro-agent` - host daemon for enrollment, heartbeat, metrics, cancellable command execution, and host-local task work.
- `doro-store` - Postgres persistence boundary using SeaORM.
- `doro-config` - environment/TOML configuration loading for the control plane and Agent config loading for `~/.doro/agent.toml`.
- `doro-ai` - AI planning/provider abstraction that never bypasses policy or approval.
- `doro-cli` - Doro operations CLI.
- `doro-ui` - Next.js frontend.
- `docs` - mdBook product and architecture documentation.

## Agent Reliability

Doro keeps one Agent protocol: outbound gRPC from each enrolled Agent to the control plane. Long-running Agent work is keyed by `command_id`, tracked locally, and can be cancelled by the control plane with `CancelCommand`. Cancelled work reports `COMMAND_STATUS_CANCELLED` so task, audit, and UI code can distinguish operator cancellation from failure.

The Agent also includes reliability settings in `~/.doro/agent.toml` under `[reliability]`:

- `event_spool_enabled`, `event_spool_path`, `event_spool_max_files`, and `event_spool_max_bytes` bound local event buffering while the stream is unavailable.
- `command_cancel_grace_seconds` controls graceful terminal cancellation before the PTY is reset.
- `preflight_enabled` enables execution-time checks for file scope, transfer size, disk space, runtime readiness, and provider configuration.

Runtime health is exposed in `metrics.snapshot.extra_json.agent_runtime`, including pending command count, cancel count, and event spool counters.

## Development

```bash
cargo check --workspace
cargo test --workspace
cd doro-ui && bun run build
mdbook build docs
```

Run the control-plane API:

```bash
cargo run -p doro-cli -- control-plane
```

The control plane listens on `0.0.0.0:8787` for the console and `0.0.0.0:8788` for agents so the UI can reach it over the host network.

Run the agent:

```bash
cargo run -p doro-cli -- agent
```

Install the control plane as a managed service. The `service` target uses systemd on Linux and launchd on macOS:

```bash
make control-plane-service-install
make control-plane-service-start
make control-plane-service-status
```

Pass `DORO_CONTROL_PLANE_*` variables to `make control-plane-service-install` to override database, bind, security, and AI settings.

Install the agent the same way:

```bash
make agent-service-install
sudoedit /etc/doro/agent.toml
make agent-service-start
make agent-service-status
```

For hosts where the agent should manage Docker, pass a supplementary group during install:

```bash
make agent-systemd-install DORO_AGENT_SUPPLEMENTARY_GROUPS=docker
```

Explicit platform targets are also available: `*-systemd-*` for Linux and `*-launchd-*` for macOS.

Set service log verbosity with the global CLI flag:

```bash
cargo run -p doro-cli -- --log-level debug control-plane
```

Run the CLI:

```bash
cargo run -p doro-cli -- status
```
