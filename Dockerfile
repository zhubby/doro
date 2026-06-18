# syntax=docker/dockerfile:1.7

ARG DEBIAN_VERSION=bookworm
ARG RUST_VERSION=1.95

FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS rust-builder

WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        cmake \
        libprotobuf-dev \
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

FROM doro-runtime AS doro

RUN { \
        echo '[agent]'; \
        echo 'control_plane_url = "http://doro-control-plane:8788"'; \
        echo 'hostname = "doro-container-agent"'; \
        echo 'heartbeat_interval_seconds = 30'; \
        echo 'metrics_interval_seconds = 10'; \
        echo ''; \
        echo '[ai]'; \
        echo 'provider = "disabled"'; \
    } > /etc/doro/agent.toml \
    && chown doro:doro /etc/doro/agent.toml

USER doro

VOLUME ["/var/lib/doro"]

EXPOSE 8787 8788 8080

ENTRYPOINT ["doro"]
CMD ["--help"]
