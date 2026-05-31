# Websites

Doro websites are HTTP reverse proxy routes managed by the control plane and served by the embedded `doro-website` Pingora runtime.

The first version intentionally focuses on the smallest useful website control loop:

- Persist website desired state in Postgres.
- Create stopped reverse proxy sites from the operations console.
- Require approval before exposing or restarting a route.
- Hot-reload Pingora routes after approved start or restart actions.
- Stop and delete routes directly because those actions reduce network exposure.

## Runtime

`doro-control-plane` starts `doro-website` when `[websites].enabled` is true. The default listener is:

```toml
[websites]
enabled = true
http_bind = "127.0.0.1:8080"
```

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

New sites are created as `stopped`. Configuration changes are only accepted while a site is stopped, so running proxy state is not partially mutated.

## Scope

The v1 website module does not implement HTTPS certificates, static site file serving, PHP runtimes, rewrite rules, anti-hotlinking, password access, real IP processing, TCP/UDP proxying, or multiple upstream load balancing. Those features should extend the same control-plane, approval, and route-table model instead of introducing OpenResty or Nginx-style configuration ownership.
