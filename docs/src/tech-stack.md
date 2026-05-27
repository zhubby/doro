# Technology Stack

Rust is the primary backend and agent language because Doro needs reliable long-running daemons, clear type boundaries, low resource usage, and safe host-level integrations.

Tokio provides async runtime support for HTTP, gRPC, timers, and future background workers.

Axum is the control-plane HTTP framework. It keeps routing and typed extraction simple while staying close to the Tokio ecosystem.

Postgres with SeaORM is the default persistence layer. Doro stores operational history, approvals, event streams, task state, and future catalog data in a database that can handle long-lived home-server state without forcing ad hoc local files. SeaORM keeps database access behind an ORM boundary while still allowing explicit schema ownership in `doro-store`.

Next.js, React, Tailwind, and shadcn/ui remain the UI stack. They are already present in `doro-ui` and fit a dense operations console.

`ts-rs` exports UI-facing REST request and response types from `doro-protocol` into `doro-ui/types/generated/`. The frontend should import those contracts through `doro-ui/types/api.ts` instead of hand-writing duplicate API types.

SSE is used for browser-facing realtime events. gRPC is used for agent transport because enrollment, heartbeat, event streaming, and command dispatch need a typed cross-process contract.

Protobuf is the first agent protocol encoding. `tonic` provides the Rust gRPC service and client types, while `prost` generates the message types from `doro-protocol/proto/doro/agent/v1/agent.proto`.

JSON remains useful for UI-facing REST payloads and internal task metadata, but it is not the Agent-to-control-plane transport.
