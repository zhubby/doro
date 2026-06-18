# syntax=docker/dockerfile:1.7

ARG BUN_VERSION=1.0.4
ARG NODE_VERSION=22
ARG DEBIAN_VERSION=bookworm

FROM oven/bun:${BUN_VERSION} AS deps

WORKDIR /workspace/doro-ui

COPY package.json bun.lockb ./

RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile

FROM node:${NODE_VERSION}-${DEBIAN_VERSION}-slim AS builder

ENV NEXT_TELEMETRY_DISABLED=1
ENV NODE_ENV=production

WORKDIR /workspace/doro-ui

COPY --from=deps /workspace/doro-ui/node_modules ./node_modules
COPY . ./

RUN node node_modules/next/dist/bin/next build

FROM node:${NODE_VERSION}-${DEBIAN_VERSION}-slim AS runner

ENV NEXT_TELEMETRY_DISABLED=1
ENV NODE_ENV=production
ENV HOSTNAME=0.0.0.0
ENV PORT=3000

WORKDIR /app

RUN chown node:node /app

COPY --from=builder --chown=node:node /workspace/doro-ui ./

USER node

EXPOSE 3000

CMD ["node", "node_modules/next/dist/bin/next", "start", "-H", "0.0.0.0", "-p", "3000"]
