# Modules

## Rust Crates

`doro-protocol` contains shared wire types, lifecycle vocabulary, generated tonic/prost gRPC types, and ts-rs TypeScript bindings for UI REST contracts. Public protocol changes should start here.

`doro-control-plane` exposes `/api/v1`, owns task orchestration, serves UI-facing state, receives agent connections, and emits events.

`doro-agent` runs on each managed host. It declares capabilities, reports heartbeat and metrics, and executes approved tasks.

`doro-store` owns Postgres persistence for control-plane facts, agent observations, task lifecycle, approvals, events, app catalog state, and metric summaries. It uses SeaORM for database access and reads backend URL and pool settings through `doro-config`.

The first durable schema is organized into table families:

- Identity: `hosts`, `agents`, `enrollment_tokens`, and `agent_capabilities`.
- Observability: `metric_snapshots`, `agent_events`, and `operation_logs`.
- Workflows: `tasks`, `task_steps`, `task_runs`, and `approvals`.
- Configuration and resource directory: `settings`, `resource_groups`, `apps`, `app_installs`, `websites`, `databases`, `containers`, `backup_accounts`, `backup_records`, `cron_jobs`, and `cron_job_runs`.

The control plane should access these tables through typed `doro-store` repositories rather than constructing SeaORM entity queries directly. Agents remain authoritative for local observations; the store records those observations as snapshots and audit events.

`doro-ai` owns provider abstraction and planning. It can draft task steps, but the control plane still decides dispatch and approval.

`doro-cli` is the local operations CLI for initialization, enrollment token workflows, diagnostics, and service entrypoints. Run the control plane with `doro control-plane` and the host agent with `doro agent`.

`doro-config` owns TOML configuration loading, default config creation, and the `~/.doro/config.toml` schema shared by CLI and future runtime crates.

## UI

`doro-ui` is a Next.js operations console. Its navigation should match the control-plane model: overview, hosts, tasks, approvals, apps, websites, containers, databases, logs, AI, and settings.

The UI should call `doro-control-plane`; it should not shell out, talk directly to agents, or own durable operational state. UI API types should come from `doro-ui/types/api.ts`, which re-exports ts-rs bindings generated from `doro-protocol`.
