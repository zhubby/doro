"use client";

import { useEffect, useState } from "react";

import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { createEnrollmentToken, deleteHost } from "@/lib/control-plane-api";
import type { EnrollmentToken, Host, MetricSnapshot } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";
import {
  Activity,
  Clipboard,
  Cpu,
  Plus,
  RefreshCw,
  Server,
  Trash2,
} from "lucide-react";

type HostsPageProps = {
  hosts: Host[];
  metricHistoryByHost?: Record<string, MetricSnapshot[]>;
  apiError?: string | null;
  onHostDeleted?: (hostId: string) => void;
};

const DEFAULT_AGENT_CONTROL_PLANE_URL = "http://127.0.0.1:8788";

function inferredAgentControlPlaneUrl() {
  const configuredUrl =
    process.env.NEXT_PUBLIC_DORO_AGENT_CONTROL_PLANE_URL?.trim();
  if (configuredUrl) {
    return configuredUrl.replace(/\/$/, "");
  }

  if (typeof window === "undefined" || !window.location.hostname) {
    return DEFAULT_AGENT_CONTROL_PLANE_URL;
  }

  return `http://${window.location.hostname}:8788`;
}

function hostStatusLabel(status: Host["status"]) {
  if (status === "online") {
    return <Badge>在线</Badge>;
  }

  if (status === "degraded") {
    return <Badge variant="secondary">需关注</Badge>;
  }

  if (status === "pending") {
    return <Badge variant="outline">待接入</Badge>;
  }

  return <Badge variant="outline">离线</Badge>;
}

function formatLastSeen(value: string | null) {
  if (!value) {
    return "尚未收到";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}

function objectValue(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function stringValue(value: unknown) {
  return typeof value === "string" && value.trim() ? value : null;
}

function numberValue(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function formatBytes(value: unknown) {
  const bytes = numberValue(value);
  if (!bytes) {
    return null;
  }
  const gib = bytes / 1024 ** 3;
  return `${gib >= 10 ? gib.toFixed(0) : gib.toFixed(1)} GB`;
}

function machineSummary(host: Host, history: MetricSnapshot[]) {
  const profile = objectValue(host.system_profile);
  if (!profile) {
    return host.id;
  }

  const os =
    stringValue(profile.long_os_version) ??
    stringValue(profile.os_name) ??
    stringValue(profile.kernel_version);
  const arch = stringValue(profile.cpu_arch);
  const physicalCores = numberValue(profile.physical_core_count);
  const logicalCores = numberValue(profile.logical_core_count);
  const memory = formatBytes(objectValue(profile.memory)?.total_bytes);
  const cores = physicalCores
    ? logicalCores && logicalCores !== physicalCores
      ? `${physicalCores}C/${logicalCores}T`
      : `${physicalCores}C`
    : logicalCores
      ? `${logicalCores}T`
      : null;
  const summary = [os, arch, cores, memory].filter(Boolean).join(" · ");
  return summary || host.id;
}

function latestPercent(
  history: MetricSnapshot[],
  key: "cpu_percent" | "memory_percent",
) {
  const latest = history.at(-1)?.[key];
  if (typeof latest !== "number" || Number.isNaN(latest)) {
    return "-";
  }
  return `${latest.toFixed(1)}%`;
}

function sparklinePath(values: number[], width: number, height: number) {
  if (values.length === 0) {
    return "";
  }
  if (values.length === 1) {
    const y = height - (Math.min(100, Math.max(0, values[0])) / 100) * height;
    return `M 0 ${y.toFixed(2)} L ${width} ${y.toFixed(2)}`;
  }
  return values
    .map((value, index) => {
      const x = (index / (values.length - 1)) * width;
      const y = height - (Math.min(100, Math.max(0, value)) / 100) * height;
      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");
}

function MetricMiniChart({
  history,
  label,
  field,
}: {
  history: MetricSnapshot[];
  label: string;
  field: "cpu_percent" | "memory_percent";
}) {
  const width = 128;
  const height = 24;
  const path = sparklinePath(
    history.map((snapshot) => snapshot[field]),
    width,
    height,
  );

  return (
    <div className="w-24 space-y-1">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[11px] font-medium text-muted-foreground">
          {label}
        </span>
        <span className="text-right text-[11px] tabular-nums">
          {latestPercent(history, field)}
        </span>
      </div>
      <svg
        viewBox={`0 0 ${width} ${height}`}
        className="h-6 w-24 overflow-visible"
        role="img"
        aria-label={`${label} 使用率趋势`}
      >
        <line
          x1="0"
          y1={height / 2}
          x2={width}
          y2={height / 2}
          className="stroke-muted"
          strokeDasharray="3 4"
        />
        <path
          d={path}
          fill="none"
          className={
            field === "cpu_percent"
              ? "stroke-primary"
              : "stroke-muted-foreground"
          }
          strokeWidth="2"
        />
      </svg>
    </div>
  );
}

function HostMetricChart({ history }: { history: MetricSnapshot[] }) {
  if (history.length === 0) {
    return <span className="text-muted-foreground">等待采样</span>;
  }

  return (
    <div className="flex w-52 items-center gap-4">
      <MetricMiniChart history={history} label="CPU" field="cpu_percent" />
      <MetricMiniChart history={history} label="内存" field="memory_percent" />
    </div>
  );
}

function hostColumns(
  metricHistoryByHost: Record<string, MetricSnapshot[]>,
): ResourceColumn<Host>[] {
  return [
    {
      key: "hostname",
      label: "Agent",
      render: (host) => (
        <div>
          <p className="font-medium">{host.hostname}</p>
          <p className="text-xs text-muted-foreground">
            {machineSummary(host, metricHistoryByHost[host.id] ?? [])}
          </p>
        </div>
      ),
    },
    {
      key: "status",
      label: "状态",
      render: (host) => hostStatusLabel(host.status),
    },
    {
      key: "capabilities",
      label: "CPU / 内存",
      render: (host) => (
        <HostMetricChart history={metricHistoryByHost[host.id] ?? []} />
      ),
    },
    {
      key: "labels",
      label: "标签",
      render: (host) => host.labels.join(" / ") || "-",
    },
    {
      key: "last_seen_at",
      label: "最后心跳",
      render: (host) => formatLastSeen(host.last_seen_at),
    },
  ];
}

export function HostsPage({
  hosts,
  metricHistoryByHost = {},
  apiError,
  onHostDeleted,
}: HostsPageProps) {
  const [hostDialogOpen, setHostDialogOpen] = useState(false);
  const [enrollmentToken, setEnrollmentToken] =
    useState<EnrollmentToken | null>(null);
  const [tokenPending, setTokenPending] = useState(false);
  const [tokenError, setTokenError] = useState<string | null>(null);
  const [copiedCommand, setCopiedCommand] = useState<string | null>(null);
  const [agentControlPlaneUrl, setAgentControlPlaneUrl] = useState(
    DEFAULT_AGENT_CONTROL_PLANE_URL,
  );
  const [deleteTarget, setDeleteTarget] = useState<Host | null>(null);
  const [deletePending, setDeletePending] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const onlineHosts = hosts.filter((host) => host.status === "online").length;
  const declaredCapabilities = hosts.reduce(
    (total, host) => total + host.capabilities.length,
    0,
  );
  const enrollmentCommand = [
    "doro agent",
    `--control-plane-url ${agentControlPlaneUrl}`,
    "--hostname homelab-node",
    `--enrollment-token ${enrollmentToken?.token ?? "TOKEN_LOADING"}`,
  ].join(" \\\n  ");

  useEffect(() => {
    setAgentControlPlaneUrl(inferredAgentControlPlaneUrl());
  }, []);

  async function generateHostToken() {
    setTokenPending(true);
    setTokenError(null);
    setEnrollmentToken(null);
    setCopiedCommand(null);
    const result = await createEnrollmentToken();
    setTokenPending(false);

    if (result.error || !result.data) {
      setTokenError(result.error ?? "创建接入令牌失败");
      return;
    }

    setEnrollmentToken(result.data.item);
  }

  function handleHostDialogOpen(open: boolean) {
    setHostDialogOpen(open);
    if (!open) {
      setEnrollmentToken(null);
      setTokenError(null);
      setCopiedCommand(null);
      return;
    }

    void generateHostToken();
  }

  async function copyText(value: string, copiedKey: string) {
    if (!navigator.clipboard) {
      return;
    }

    await navigator.clipboard.writeText(value);
    setCopiedCommand(copiedKey);
  }

  async function handleDeleteHost() {
    if (!deleteTarget) {
      return;
    }

    setDeletePending(true);
    setDeleteError(null);
    const result = await deleteHost(deleteTarget.id);
    setDeletePending(false);

    if (result.error) {
      setDeleteError(result.error);
      return;
    }

    onHostDeleted?.(deleteTarget.id);
    setDeleteTarget(null);
  }

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <PageSection
        title="主机"
        description="来自控制平面的 Agent 注册状态、能力声明和心跳。"
        toolbar={
          <Dialog open={hostDialogOpen} onOpenChange={handleHostDialogOpen}>
            <DialogTrigger asChild>
              <Button size="sm">
                <Plus className="size-4" />
                New Host
              </Button>
            </DialogTrigger>
            <DialogContent className="max-h-[calc(100vh-2rem)] max-w-[calc(100vw-2rem)] overflow-y-auto sm:max-w-xl">
              <DialogHeader>
                <DialogTitle>接入新主机</DialogTitle>
                <DialogDescription>
                  控制平面已为这次接入生成一次性 enrollment
                  token。复制命令到目标主机运行。
                </DialogDescription>
              </DialogHeader>

              <div className="min-w-0 space-y-4 text-sm">
                <div className="min-w-0 rounded-md border bg-muted/30 p-4">
                  <div className="mb-3 flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0">
                      <p className="font-medium">1. 在目标主机启动 Agent</p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        令牌仅显示这一次，首次接入成功后会自动失效。
                      </p>
                    </div>
                    <div className="flex shrink-0 items-center gap-2">
                      <Button
                        type="button"
                        variant="outline"
                        size="icon"
                        title="重新生成令牌"
                        disabled={tokenPending}
                        onClick={generateHostToken}
                      >
                        <RefreshCw className="size-4" />
                      </Button>
                      <Button
                        type="button"
                        variant="outline"
                        size="icon"
                        title="复制命令"
                        disabled={!enrollmentToken}
                        onClick={() => void copyText(enrollmentCommand, "agent")}
                      >
                        <Clipboard className="size-4" />
                      </Button>
                    </div>
                  </div>
                  {tokenError ? (
                    <div className="rounded-md border border-destructive/30 p-3 text-sm text-destructive">
                      创建失败：{tokenError}
                    </div>
                  ) : (
                    <pre className="max-w-full overflow-x-auto rounded-md bg-background p-3 text-xs text-foreground">
                      <code className="block min-w-max">
                        {tokenPending ? "正在生成接入令牌..." : enrollmentCommand}
                      </code>
                    </pre>
                  )}
                  {copiedCommand === "agent" ? (
                    <p className="mt-2 text-xs text-muted-foreground">命令已复制</p>
                  ) : null}
                </div>

                <div className="min-w-0 rounded-md border bg-muted/30 p-4">
                  <p className="mb-3 font-medium">2. 后续重启可直接使用已写回的配置</p>
                  <pre className="max-w-full overflow-x-auto rounded-md bg-background p-3 text-xs text-foreground">
                    <code className="block min-w-max">
                      doro agent --config ~/.doro/config.toml
                    </code>
                  </pre>
                </div>

                <p className="text-xs leading-5 text-muted-foreground">
                  首次连接成功后，Agent 会把 agent_id 和 host_id 写回本机配置；
                  令牌会在控制平面标记为已使用。
                </p>
              </div>
            </DialogContent>
          </Dialog>
        }
      >
        <div className="mb-4 grid gap-3 md:grid-cols-3">
          <Card>
            <CardContent className="flex items-center gap-3 p-4">
              <Server className="size-4 text-muted-foreground" />
              <div>
                <p className="text-xs text-muted-foreground">已注册 Agent</p>
                <p className="text-xl font-semibold">{hosts.length}</p>
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="flex items-center gap-3 p-4">
              <Activity className="size-4 text-muted-foreground" />
              <div>
                <p className="text-xs text-muted-foreground">当前在线</p>
                <p className="text-xl font-semibold">{onlineHosts}</p>
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="flex items-center gap-3 p-4">
              <Cpu className="size-4 text-muted-foreground" />
              <div>
                <p className="text-xs text-muted-foreground">声明能力</p>
                <p className="text-xl font-semibold">{declaredCapabilities}</p>
              </div>
            </CardContent>
          </Card>
        </div>
        <DataTable
          columns={hostColumns(metricHistoryByHost)}
          rows={hosts}
          actions={[]}
          renderActions={(host) => (
            <Button
              aria-label={`删除主机 ${host.hostname}`}
              title="删除"
              variant="ghost"
              size="icon"
              className="size-8 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              onClick={() => {
                setDeleteTarget(host);
                setDeleteError(null);
              }}
            >
              <Trash2 className="size-4" />
            </Button>
          )}
          emptyText="暂无已连接 Agent"
        />
      </PageSection>
      <Dialog
        open={Boolean(deleteTarget)}
        onOpenChange={(open) => {
          if (!open && !deletePending) {
            setDeleteTarget(null);
            setDeleteError(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>删除主机</DialogTitle>
            <DialogDescription>
              删除后会移除该主机的 Agent 记录、能力声明、指标快照和容器观测数据。仍在运行的
              Agent 需要重新接入。
            </DialogDescription>
          </DialogHeader>

          <div className="rounded-md border bg-muted/30 p-3 text-sm">
            <p className="font-medium">{deleteTarget?.hostname}</p>
            <p className="mt-1 text-xs text-muted-foreground">{deleteTarget?.id}</p>
          </div>

          {deleteError ? (
            <div className="rounded-md border border-destructive/30 p-3 text-sm text-destructive">
              删除失败：{deleteError}
            </div>
          ) : null}

          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline" disabled={deletePending}>
                取消
              </Button>
            </DialogClose>
            <Button
              variant="destructive"
              disabled={deletePending}
              onClick={handleDeleteHost}
            >
              {deletePending ? "删除中" : "确认删除"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
