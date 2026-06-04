# Roadmap

## MVP

- Compileable Rust workspace with Doro-owned crates.
- mdBook product and architecture documentation.
- Control-plane API skeleton for hosts, tasks, approvals, apps, settings, and events.
- Agent skeleton for identity, capabilities, heartbeat, and metrics.
- CLI skeleton for status, diagnostics, and service entrypoints.
- UI navigation aligned with the Doro control-plane model.

## Beta

- Durable enrollment flow.
- Authenticated UI sessions.
- Real Postgres task, host, approval, and event persistence.
- Agent task dispatch over gRPC streaming.
- Cancellable long-running Agent commands with standard cancelled results.
- Agent event spool and idempotent event ingestion for reconnect recovery.
- Agent preflight checks for path scope, disk space, runtime readiness, and provider configuration.
- Runtime metrics for pending commands, cancellation count, and event spool health.
- Container, service, log, and metrics integrations.
- Human approval UI.
- Agent-side Pingora website runtime with approved HTTP reverse proxy routes.
- Host-bound website management with visible placeholders for HTTPS, certificates, static sites, upstream pools, rewrite rules, and TCP/UDP proxying.

## Later

- Rich AI planning with model provider configuration.
- Policy editor and reusable automation recipes.
- Application catalog with backup and restore workflows.
- HTTPS certificate issuance/renewal, static site deployment, multi-upstream health checks, rewrite/redirect rules, TCP/UDP proxying, real IP handling, access control, and password gates for websites.
- Multi-user roles.
- Database migration tooling and retention policies for metrics, logs, and audit events.
