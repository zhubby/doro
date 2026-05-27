# Technology Stack

Rust is the primary backend and agent language because Doro needs reliable long-running daemons, clear type boundaries, low resource usage, and safe host-level integrations.

Tokio provides async runtime support for HTTP, WebSocket, timers, and future background workers.

Axum is the control-plane HTTP framework. It keeps routing and typed extraction simple while staying close to the Tokio ecosystem.

SQLite with `sqlx` is the MVP persistence layer. It is easy to deploy on a home server, supports durable local state, and can later be abstracted if a larger deployment needs Postgres.

Next.js, React, Tailwind, and shadcn/ui remain the UI stack. They are already present in `doro-ui` and fit a dense operations console.

SSE is used for browser-facing realtime events. WebSocket is used for agent transport because task dispatch and agent event reporting are bidirectional.

JSON is the first protocol encoding. Shared Rust types live in `doro-protocol` so a future Protobuf or gRPC transport can be introduced without redefining product concepts.
