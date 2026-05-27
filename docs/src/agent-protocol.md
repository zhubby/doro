# Agent Protocol

The first protocol version is `v1`.

## Enrollment

The control plane creates a one-time enrollment token. An agent uses that token to obtain a durable agent identity. After enrollment, the token should be invalidated.

## Connection

Agents connect outbound to:

```text
GET /api/v1/agent/connect
```

The transport is WebSocket with JSON messages. Production deployments require TLS.

## Lifecycle

1. Agent connects and authenticates.
2. Agent declares host identity and capabilities.
3. Agent sends heartbeat and metrics events.
4. Control plane dispatches tasks that match declared capabilities.
5. Agent executes allowed steps and reports events.
6. High-risk steps stop at approval before execution.

## Core Types

- `Host`: a managed machine.
- `AgentCapability`: an action class the agent can perform.
- `Task`: a control-plane unit of work.
- `TaskStep`: a capability-bound operation inside a task.
- `ApprovalRequest`: an explicit approval gate for risky steps.
- `AgentEvent`: event stream from agent to control plane.
- `MetricSnapshot`: point-in-time host metrics.
