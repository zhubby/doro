# Doro Agent Instructions

## Project Identity

Doro is an AI-native home server control plane. It is not a Codex CLI fork.

Do not reintroduce removed Codex crates, Codex command names, Codex README/config language, or Codex protocol terminology unless a task explicitly asks for migration research. Active work should use Doro terms: control plane, agent, host, task, approval, event, capability, and policy.

## Architecture

- `doro-control-plane` owns API, orchestration, approvals, event streaming, and AI entrypoints.
- `doro-agent` runs on hosts and executes host capabilities only after policy checks.
- `doro-protocol` owns shared public types used across control plane, UI, CLI, store, and agent.
- `doro-store` owns persistence and schema migrations.
- `doro-ai` owns model/provider abstraction and planning, but it must not bypass policy or approval.
- `doro-ui` is an operations console that talks to the control-plane API.

## Rust Rules

- Keep shared protocol structs in `doro-protocol`; do not duplicate wire shapes in other crates.
- Use Tokio for async Rust and Axum for the control-plane HTTP surface.
- Use SQLite through `sqlx` for persistence unless a task explicitly changes storage direction.
- Do not use `unwrap` or `expect` in production code. Return `anyhow::Result` at binary boundaries and domain errors in libraries when useful.
- High-risk host operations must be represented as capabilities and must support approval before execution.

## Agent Safety

Every host action must be auditable. Agent code must preserve these invariants:

- The agent declares capabilities before executing tasks.
- The control plane validates capability and risk before task dispatch.
- Shell execution, file writes, service stop/restart, container deletion, network exposure, and database restore are high risk.
- High-risk steps require `ApprovalRequest` support.
- Agents should prefer least-privilege local execution and explicit error reporting over implicit fallback.

## UI Rules

- Build a dense operations console, not a marketing site.
- Chinese UI text is the default.
- Keep navigation aligned to the control-plane model: overview, hosts, tasks, approvals, apps, resources, logs, AI, settings.
- Prefer typed API data from `doro-control-plane`; keep mock data temporary and centralized.

## Docs Rules

Docs are managed with mdBook under `docs/`.

- Update `docs/src/SUMMARY.md` when adding or moving documentation.
- Architecture and product decisions belong in `docs/src/`.
- New Rust crates or major module changes must be reflected in `docs/src/modules.md`.
