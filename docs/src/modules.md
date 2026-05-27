# Modules

## Rust Crates

`doro-protocol` contains shared wire types, lifecycle vocabulary, generated tonic/prost gRPC types, and ts-rs TypeScript bindings for UI REST contracts. Public protocol changes should start here.

`doro-control-plane` exposes `/api/v1`, owns task orchestration, serves UI-facing state, receives agent connections, and emits events.

`doro-agent` runs on each managed host. It declares capabilities, reports heartbeat and metrics, and executes approved tasks.

`doro-store` owns Postgres persistence for hosts, agent sessions, tasks, approvals, events, app catalog state, and metric summaries. It uses SeaORM for database access and reads backend URL and pool settings through `doro-config`.

`doro-ai` owns provider abstraction and planning. It can draft task steps, but the control plane still decides dispatch and approval.

`doro-cli` is the local operations CLI for initialization, enrollment token workflows, diagnostics, and future administrative commands.

`doro-config` owns TOML configuration loading, default config creation, and the `~/.doro/config.toml` schema shared by CLI and future runtime crates.

## UI

`doro-ui` is a Next.js operations console. Its navigation should match the control-plane model: overview, hosts, tasks, approvals, apps, websites, containers, databases, logs, AI, and settings.

The UI should call `doro-control-plane`; it should not shell out, talk directly to agents, or own durable operational state. UI API types should come from `doro-ui/types/api.ts`, which re-exports ts-rs bindings generated from `doro-protocol`.
