# Security Model

Doro defaults to policy plus approval.

The control plane must never treat AI output as authorization. AI may propose a plan, but policy and approval decide whether a task can run.

High-risk capabilities include:

- Shell command execution.
- File writes.
- Service stop or restart.
- Container deletion or destructive mutation.
- Virtual machine creation, deletion, snapshot mutation, lifecycle changes, bridge networking, and console access.
- Network exposure and port publishing.
- Database restore.

Security requirements:

- Agents declare capabilities before receiving tasks.
- The control plane validates capability and risk before dispatch.
- Approval requests are recorded before high-risk execution unless an explicit audited direct-execution path is selected by an operator.
- Agent preflight checks reject invalid paths, oversized file transfers, insufficient local readiness, and unavailable runtimes before local execution starts.
- Long-running Agent commands can be cancelled by `command_id`; cancelled work records `COMMAND_STATUS_CANCELLED` rather than being hidden as a generic failure.
- Agent events are recorded for auditability.
- Replayed Agent events use the external `AgentEvent.event_id` for idempotent audit insertion, so reconnect drains do not create duplicate records.
- Enrollment tokens are one-time credentials.
- Production deployments require TLS and durable secret storage.

The terminal UI is an explicit administrative direct-execution path for agents that declare `ShellExecute`. Terminal commands and interactive sessions are still validated by the control plane, routed only over the established agent stream, and recorded in `agent_events` at session open/close and command completion boundaries. Deployments that require stricter change control should gate this route behind per-command or per-session approval before enabling it for operators.

Docker container creation is also an explicit operator-selected direct-execution path. Direct creation still creates a task, task step, task run, and Docker command event before dispatching over the enrolled Agent stream, and it still requires an online Agent that declares `ContainersManage`. Operators can switch the create request to the approval path when local change-control policy requires a pending approval before dispatch. Other Docker lifecycle, image, network, volume, and Compose write operations continue to use the approval workflow by default.

AI AgentRun tasks are not an authorization path. The Agent may use AI to choose tools and arguments, but shell execution and file mutation pause at a control-plane approval request before the local operation starts. Approval decisions are sent back over the enrolled Agent stream and task progress remains auditable through `task_steps`, `task_runs`, and `agent_events`.

Local Agent reliability features preserve this model. The event spool only replays Agent-originated operational events after reconnect; it does not accept commands from disk and cannot bypass capability or approval checks. Cancellation is an operator control over already-dispatched work and is itself audited as a command result.

The MVP can keep policy simple, but it must preserve the data model and lifecycle needed for stricter policy later.
