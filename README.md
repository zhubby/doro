# Doro

Doro is an AI-native home server control plane.

The project is built around one central control panel and many host agents:

- `doro-control-plane` exposes the API, stores state, orchestrates tasks, handles approvals, and provides the AI entrypoint.
- `doro-agent` runs on each host and exposes host capabilities over gRPC such as metrics, logs, services, containers, files, and command execution.
- `doro-ui` is the Next.js operations console for hosts, tasks, approvals, applications, resources, logs, and settings.

Doro is not a Codex CLI fork. The previous Codex-derived files were removed from the active workspace and the remaining project surface is being rebuilt around the home-server control-plane model.

## Workspace

- `doro-protocol` - shared versioned protocol types for the UI, control plane, and agents.
- `doro-control-plane` - Axum HTTP API, event stream, and agent connection surface.
- `doro-agent` - host daemon skeleton for registration, heartbeat, metrics, and task execution.
- `doro-store` - SQLite persistence boundary using `sqlx`.
- `doro-ai` - AI planning/provider abstraction that never bypasses policy or approval.
- `doro-cli` - Doro operations CLI.
- `doro-ui` - Next.js frontend.
- `docs` - mdBook product and architecture documentation.

## Development

```bash
cargo check --workspace
cargo test --workspace
cd doro-ui && bun run build
mdbook build docs
```

Run the control-plane API:

```bash
cargo run -p doro-control-plane
```

The control plane listens on `127.0.0.1:8787` for HTTP and `127.0.0.1:8788` for Agent gRPC.

Run the agent skeleton:

```bash
cargo run -p doro-agent
```

Run the CLI:

```bash
cargo run -p doro-cli -- status
```
