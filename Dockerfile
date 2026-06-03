# syntax=docker/dockerfile:1.7

ARG DEBIAN_VERSION=bookworm
ARG RUST_VERSION=1.95
ARG BUN_VERSION=1.0.4
ARG NODE_VERSION=22

FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS rust-builder

WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        pkg-config \
        protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY doro-agent ./doro-agent
COPY doro-ai ./doro-ai
COPY doro-cli ./doro-cli
COPY doro-config ./doro-config
COPY doro-container ./doro-container
COPY doro-control-plane ./doro-control-plane
COPY doro-protocol ./doro-protocol
COPY doro-store ./doro-store
COPY doro-vm ./doro-vm
COPY doro-website ./doro-website

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/workspace/target \
    cargo build --release --locked -p doro-cli --bin doro \
    && cp /workspace/target/release/doro /workspace/doro

FROM oven/bun:${BUN_VERSION} AS ui-deps

WORKDIR /workspace/doro-ui

COPY doro-ui/package.json doro-ui/bun.lockb ./

RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile

FROM ui-deps AS ui-builder

ENV NEXT_TELEMETRY_DISABLED=1

COPY doro-ui ./

RUN bun run build

FROM node:${NODE_VERSION}-${DEBIAN_VERSION}-slim AS doro-ui

ENV NEXT_TELEMETRY_DISABLED=1
ENV NODE_ENV=production
ENV HOSTNAME=0.0.0.0
ENV PORT=3000

WORKDIR /app

RUN chown node:node /app

COPY --from=ui-builder --chown=node:node /workspace/doro-ui ./

USER node

EXPOSE 3000

CMD ["node", "node_modules/next/dist/bin/next", "start", "-H", "0.0.0.0", "-p", "3000"]

FROM debian:${DEBIAN_VERSION}-slim AS doro-runtime

ENV RUST_LOG=doro_cli=info,doro_agent=info,doro_control_plane=info,tower_http=info

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 doro \
    && useradd --system --uid 10001 --gid doro --home-dir /var/lib/doro --shell /usr/sbin/nologin doro \
    && mkdir -p /etc/doro /var/lib/doro \
    && chown -R doro:doro /var/lib/doro

COPY --from=rust-builder /workspace/doro /usr/local/bin/doro

FROM doro-runtime AS doro-agent

RUN { \
        echo '[agent]'; \
        echo 'control_plane_url = "http://doro-control-plane:8788"'; \
        echo 'hostname = "doro-container-agent"'; \
        echo 'heartbeat_interval_seconds = 30'; \
        echo 'metrics_enabled = true'; \
        echo 'metrics_interval_seconds = 10'; \
        echo 'container_metrics_enabled = true'; \
        echo 'docker_manage_enabled = false'; \
        echo 'vm_manage_enabled = false'; \
        echo ''; \
        echo '[ai]'; \
        echo 'provider = "disabled"'; \
    } > /etc/doro/agent.toml \
    && chown doro:doro /etc/doro/agent.toml

USER doro

VOLUME ["/var/lib/doro"]

ENTRYPOINT ["doro"]
CMD ["--config", "/etc/doro/agent.toml", "agent"]

FROM doro-runtime AS doro-control-plane

RUN { \
        echo '[server]'; \
        echo 'console_bind = "0.0.0.0:8787"'; \
        echo 'agent_bind = "0.0.0.0:8788"'; \
        echo ''; \
        echo '[store]'; \
        echo 'backend = "postgres"'; \
        echo 'database_url = "postgres://doro:doro@postgres:5432/doro"'; \
        echo 'max_connections = 10'; \
        echo 'min_connections = 1'; \
        echo 'connect_timeout_seconds = 8'; \
        echo 'idle_timeout_seconds = 300'; \
        echo ''; \
        echo '[security]'; \
        echo 'approval_policy = "policy_and_human_approval"'; \
        echo 'require_tls = false'; \
        echo ''; \
        echo '[websites]'; \
        echo 'enabled = true'; \
        echo 'http_bind = "0.0.0.0:8080"'; \
        echo ''; \
        echo '[ai]'; \
        echo 'provider = "disabled"'; \
    } > /etc/doro/control-plane.toml \
    && chown doro:doro /etc/doro/control-plane.toml

USER doro

VOLUME ["/var/lib/doro"]

EXPOSE 8787 8788 8080

ENTRYPOINT ["doro"]
CMD ["--config", "/etc/doro/control-plane.toml", "control-plane"]
