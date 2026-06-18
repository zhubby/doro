# Modules

## Rust Crates

`doro-protocol` contains shared wire types, lifecycle vocabulary, generated tonic/prost gRPC types, and ts-rs TypeScript bindings for UI REST contracts. Public protocol changes should start here.

`doro-control-plane` exposes `/api/v1`, owns task orchestration, AI model provider management, persistent AI chat conversations, command-scoped AI provider dispatch, runs the scheduled-task dispatcher, serves UI-facing state, receives agent connections, ingests one-way agent observations, evaluates metrics alert rules, sends configured notification emails, emits events, and dispatches cancellable Agent stream commands.

`doro-agent` runs on macOS and Linux managed hosts. It enrolls with a one-time token, persists its durable agent and host identifiers in config, probes local runtimes at startup, declares only detected capabilities, reports heartbeat and local metrics, and executes approved tasks. Its local command registry tracks long-running terminal, AI, Docker, virtual machine, file, website, and refresh work by `command_id` so the control plane can cancel running commands and receive standard final results. Its event spool buffers non-log operational events when the Agent stream is unavailable and drains them after reconnect. Its local collectors read cross-platform system metrics, discovered container runtime state through `doro-container`, and discovered Linux/NVIDIA GPU state, then send observations only through the agent protocol stream. It also owns direct filesystem operations for the file manager and performs them as the current agent OS user after preflight checks. The Agent owns Docker Compose file execution under the configured managed root; it writes only `compose.yaml` and optional `.env` inside that root for audited direct create/update tasks and runs the local Docker CLI plugin for approved or explicitly direct Compose actions when Compose is detected. It also owns Agent-local Docker registry credential management by updating the current Agent user's default `~/.docker/config.json`; Compose commands use that same config directory through `DOCKER_CONFIG`. The `AgentRun` capability runs the local AI runner for natural-language host operations while pausing high-risk tools for control-plane approval. When website routing is detected as bindable, the Agent owns the local Pingora runtime and declares `NetworkExpose`.

`doro-container` owns the provider-neutral container runtime boundary. It defines container inventory, lifecycle, image, network, volume, registry, snapshot, and command abstractions, and exports the direct Docker provider backed by Bollard. Docker socket handling, Docker config registry normalization, Bollard model conversion, and runtime-specific container/image/network/volume operations belong in this crate rather than in the control plane. Compose and registry config commands are represented in the command envelope for protocol consistency, but CLI execution, Docker config file mutation, and managed-root path safety stay in `doro-agent`.

`doro-vm` owns the provider-neutral virtual machine boundary. It defines the virtual machine provider traits, lifecycle/image/snapshot/console abstractions, command envelopes, and the direct QEMU provider. QEMU process arguments, QMP/QGA socket paths, VM state files, image scanning, NAT/bridge validation, and console endpoint construction belong in this crate rather than in the agent or control plane.

`doro-website` owns the embedded Pingora website reverse proxy runtime used by Agents. It builds a hot-swappable route table from control-plane website records sent over the Agent protocol, matches HTTP Host headers to reverse proxy upstreams, and exposes runtime handles for Agents to reload routes after approved website changes. It does not own persisted website state or UI contracts.

`doro-store` owns Postgres persistence for control-plane facts, agent observations, task lifecycle, scheduled task definitions and runs, approvals, alert rules and incidents, events, app catalog state, container observations, virtual machine observations, and metric summaries. It uses SeaORM for database access and reads backend URL and pool settings through `doro-config`. Agent event writes are idempotent by external event ID so replayed Agent spool events do not duplicate audit rows. Docker image, network, volume, Compose, and registry credential inventories are live Agent queries; only tasks, approvals, runs, and agent events are persisted for Docker management actions, and registry secrets are not persisted by the control plane.

The first durable schema is organized into table families:

- Identity: `hosts`, `agents`, `enrollment_tokens`, and `agent_capabilities`.
- Observability: `metric_snapshots`, `agent_events`, `operation_logs`, `alert_rules`, `alert_rule_states`, `alert_incidents`, and `alert_notifications`.
- Workflows: `tasks`, `task_steps`, `task_runs`, `approvals`, `ai_conversations`,
  `ai_chat_messages`, and `ai_chat_events`. Approvals are
  durable control-plane records with explicit expiration and decision metadata.
- Configuration and resource directory: `settings`, `ai_model_providers`, `resource_groups`, `apps`, `app_installs`, `websites`, `databases`, `containers`, `virtual_machines`, `virtual_machine_images`, `virtual_machine_templates`, `virtual_machine_snapshots`, `backup_accounts`, `backup_records`, `cron_jobs`, and `cron_job_runs`. Email notification settings live in `settings` as redacted control-plane configuration.

The control plane should access these tables through typed `doro-store` repositories rather than constructing SeaORM entity queries directly. Agent enrollment token validation and consumption belongs in `doro-store` so identity writes and token state stay transactional. Agents remain authoritative for local observations; the store records those observations as metric snapshots, current container rows, current virtual machine rows, and audit events.

`doro-ai` owns provider abstraction, planning, and the provider-neutral Agent runner. It can draft task steps and drive model tool calls, but local tool execution stays in `doro-agent` and the control plane still decides dispatch and approval.

`doro-cli` is the local operations CLI for diagnostics and service entrypoints. Enrollment tokens are generated by the control plane and consumed by `doro agent`. Run the control plane with `doro control-plane` and the host agent with `doro agent`.

`doro-config` owns control-plane configuration loading from `DORO_CONTROL_PLANE_*` environment variables with optional TOML fallback, and Agent TOML config loading for `~/.doro/agent.toml`, including Agent reliability settings for event spooling, cancellation grace, and preflight behavior.

## UI

`doro-ui` is a Next.js operations console. Its navigation should match the control-plane model: overview, hosts, tasks, approvals, virtual machines, AI chat, model providers, files, websites, Docker, databases, logs, alerts, and notifications.

The UI should call `doro-control-plane`; it should not shell out, talk directly to agents, or own durable operational state. UI API types should come from `doro-ui/types/api.ts`, which re-exports ts-rs bindings generated from `doro-protocol`.
