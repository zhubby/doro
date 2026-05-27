# Roadmap

## MVP

- Compileable Rust workspace with Doro-owned crates.
- mdBook product and architecture documentation.
- Control-plane API skeleton for hosts, tasks, approvals, apps, settings, and events.
- Agent skeleton for identity, capabilities, heartbeat, and metrics.
- CLI skeleton for status and enrollment-token workflows.
- UI navigation aligned with the Doro control-plane model.

## Beta

- Durable enrollment flow.
- Authenticated UI sessions.
- Real Postgres task, host, approval, and event persistence.
- Agent task dispatch over gRPC streaming.
- Container, service, log, and metrics integrations.
- Human approval UI.

## Later

- Rich AI planning with model provider configuration.
- Policy editor and reusable automation recipes.
- Application catalog with backup and restore workflows.
- Multi-user roles.
- Database migration tooling and retention policies for metrics, logs, and audit events.
