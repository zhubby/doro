# Modules

## Rust Crates

`doro-protocol` contains shared wire types and lifecycle vocabulary. Public protocol changes should start here.

`doro-control-plane` exposes `/api/v1`, owns task orchestration, serves UI-facing state, receives agent connections, and emits events.

`doro-agent` runs on each managed host. It declares capabilities, reports heartbeat and metrics, and executes approved tasks.

`doro-store` owns SQLite persistence for hosts, agent sessions, tasks, approvals, events, app catalog state, and metric summaries.

`doro-ai` owns provider abstraction and planning. It can draft task steps, but the control plane still decides dispatch and approval.

`doro-cli` is the local operations CLI for initialization, enrollment token workflows, diagnostics, and future administrative commands.

## UI

`doro-ui` is a Next.js operations console. Its navigation should match the control-plane model: overview, hosts, tasks, approvals, apps, websites, containers, databases, logs, AI, and settings.

The UI should call `doro-control-plane`; it should not shell out, talk directly to agents, or own durable operational state.
