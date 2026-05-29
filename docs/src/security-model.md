# Security Model

Doro defaults to policy plus approval.

The control plane must never treat AI output as authorization. AI may propose a plan, but policy and approval decide whether a task can run.

High-risk capabilities include:

- Shell command execution.
- File writes.
- Service stop or restart.
- Container deletion or destructive mutation.
- Network exposure and port publishing.
- Database restore.

Security requirements:

- Agents declare capabilities before receiving tasks.
- The control plane validates capability and risk before dispatch.
- Approval requests are recorded before high-risk execution.
- Agent events are recorded for auditability.
- Enrollment tokens are one-time credentials.
- Production deployments require TLS and durable secret storage.

The terminal UI is an explicit administrative direct-execution path for agents that declare `ShellExecute`. Each command is still validated by the control plane, routed only over the established agent stream, and recorded in `agent_events` before and after execution. Deployments that require stricter change control should gate this route behind per-command or per-session approval before enabling it for operators.

The MVP can keep policy simple, but it must preserve the data model and lifecycle needed for stricter policy later.
