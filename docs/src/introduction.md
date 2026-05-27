# Introduction

Doro is an AI-native control plane for home servers.

It targets users who run services across one or more local machines and want a single place to inspect hosts, deploy applications, manage resources, approve risky actions, and delegate operational work to AI-assisted workflows.

The architecture has two primary runtime roles:

- Control plane: central API, UI backend, task orchestrator, approval system, state store, event stream, and AI entrypoint.
- Agent: host-local daemon that reports state and executes approved host capabilities.

Doro is built as an operations product first. AI is part of the execution model, but the first version must still be understandable, auditable, and useful without relying on a model for every action.
