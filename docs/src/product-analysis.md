# Product Analysis

Home-server operators often combine dashboards, SSH sessions, container tools, cron jobs, reverse proxies, backup scripts, and ad hoc notes. That works for a single host, but it becomes fragile when services spread across multiple machines.

Doro focuses on three product needs:

- A unified operational map of hosts, services, applications, tasks, approvals, and recent events.
- Safe delegation, where an AI system can draft plans and propose actions without bypassing policy or human approval.
- Host-local execution through agents, so the control plane does not need direct shell access to every machine.

Adjacent products include server panels, container managers, NAS dashboards, and automation systems. Doro differs by treating AI planning, capability declarations, approvals, and audit events as first-class product surfaces instead of adding a chat box on top of a traditional panel.

The MVP should prioritize operational confidence: hosts are visible, tasks are traceable, risky actions wait for approval, and every agent event can be inspected.
