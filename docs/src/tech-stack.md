# Technology Stack

Rust is the primary backend and agent language because Doro needs reliable long-running daemons, clear type boundaries, low resource usage, and safe host-level integrations.

Tokio provides async runtime support for HTTP, gRPC, timers, and future background workers.

Axum is the control-plane HTTP framework. It keeps routing and typed extraction simple while staying close to the Tokio ecosystem.

SQLite with `sqlx` is the MVP persistence layer. It is easy to deploy on a home server, supports durable local state, and can later be abstracted if a larger deployment needs Postgres.

Next.js, React, Tailwind, and shadcn/ui remain the UI stack. They are already present in `doro-ui` and fit a dense operations console.

SSE is used for browser-facing realtime events. gRPC is used for agent transport because enrollment, heartbeat, event streaming, and command dispatch need a typed cross-process contract.

Protobuf is the first agent protocol encoding. `tonic` provides the Rust gRPC service and client types, while `prost` generates the message types from `doro-protocol/proto/doro/agent/v1/agent.proto`.

JSON remains useful for UI-facing REST payloads and internal task metadata, but it is not the Agent-to-control-plane transport.
