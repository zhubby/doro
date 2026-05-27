# Architecture

Doro uses a hub-and-spoke architecture.

```mermaid
flowchart LR
    UI["doro-ui"] --> CP["doro-control-plane"]
    CLI["doro-cli"] --> CP
    CP --> Store["doro-store / Postgres"]
    CP --> AI["doro-ai"]
    A1["doro-agent / host-a"] --> CP
    A2["doro-agent / host-b"] --> CP
    A3["doro-agent / host-c"] --> CP
```

The control plane is authoritative for desired state, task lifecycle, approvals, and audit history. Agents are authoritative for local host observations and local execution results.

Agents connect outbound to the control plane over gRPC using the `doro.agent.v1.AgentControlPlane` service. This keeps the model compatible with NAT and home networks where inbound access to every host is undesirable.

The UI uses REST APIs for query and mutation, plus SSE at `/api/v1/events` for realtime browser updates. Agent traffic uses a separate gRPC/Protobuf contract because agents need typed enrollment, heartbeat, event streaming, and command dispatch.

Trust boundaries:

- Browser to control plane: authenticated user/API session.
- Control plane to store: trusted persistence boundary configured through `doro-config`.
- Agent to control plane: enrolled agent identity and transport security.
- AI to control plane: advisory planning only; policy and approval remain control-plane responsibilities.
