# Websites

Doro websites are host-bound routes managed by the control plane and served by the target Agent. The control plane owns desired state, approvals, task lifecycle, and audit events. The Agent owns the local Pingora runtime and applies the approved route table for its host.

The first executable website capability is HTTP reverse proxy routing:

- Persist website desired state in Postgres.
- Require every website to choose a target Host.
- Create stopped HTTP reverse proxy sites from the operations console.
- Require approval before exposing or restarting a route.
- Send a full host-scoped route table to the target Agent after approval.
- Stop and delete routes by applying a new full route table to the target Agent.

## Runtime

`doro-agent` probes the configured website HTTP bind address on startup. When the address is available, the Agent starts `doro-website` and declares `NetworkExpose`; when the address is invalid or already in use, the Agent logs the reason, omits `NetworkExpose`, and continues starting other capabilities.

```toml
[websites]
http_bind = "127.0.0.1:8080"
https_bind = ""
tcp_bind = ""
udp_bind = ""
static_root = ""
certificate_store = ""
```

Agents that run the website runtime declare the high-risk `NetworkExpose` capability. The control plane validates that capability and an active Agent stream before creating, starting, restarting, stopping, or deleting route state that must affect the runtime.

Pingora matches incoming `Host` headers against active website domains and aliases. A request with no matching route returns `404`; there is no fallback upstream.

## API

The UI uses typed REST contracts from `doro-protocol`:

```text
GET    /api/v1/websites
POST   /api/v1/websites
GET    /api/v1/websites/:id
PATCH  /api/v1/websites/:id
DELETE /api/v1/websites/:id
POST   /api/v1/websites/:id/start
POST   /api/v1/websites/:id/stop
POST   /api/v1/websites/:id/restart
```

`CreateWebsiteRequest` and `UpdateWebsiteRequest` require `host_id`. New sites are created as `stopped`. Configuration changes are only accepted while a site is stopped, so running proxy state is not partially mutated.

Start and restart create a `NetworkExpose` approval task. Approval applies the target Host's complete running website route table through `ApplyWebsiteRoutesCommand`. Stop and delete do not require approval because they reduce exposure, but they still apply a new route table before reporting success.

## Planned Website Capabilities

The website model reserves protocol vocabulary and configuration space for richer site management. These items are planned and must remain visibly tracked until implemented:

- HTTPS listener support, certificate import, certificate issuance, and renewal.
- Static site roots, upload/deploy flow, filesystem permissions, and rollback.
- Multiple upstreams with health checks and load balancing policy.
- Rewrite, redirect, and request/response header rules.
- TCP and UDP proxy listeners with host-scoped port conflict validation.
- Real IP handling, access control, and password protection.

Future work should extend the same control-plane desired-state, approval, Agent command, and audit-event model instead of introducing OpenResty or Nginx-style configuration ownership.
