"use client";

import { useEffect, useState } from "react";

import { DataTable } from "@/components/admin/data-table";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import {
  createEnrollmentToken,
  deleteHost,
  updateHost,
} from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import type { EnrollmentToken, Host, MetricSnapshot } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";
import {
  Activity,
  Clipboard,
  Cpu,
  Pencil,
  Plus,
  RefreshCw,
  Server,
  Trash2,
  X,
} from "lucide-react";

type HostsPageProps = {
  hosts: Host[];
  metricHistoryByHost?: Record<string, MetricSnapshot[]>;
  apiError?: string | null;
  onHostDeleted?: (hostId: string) => void;
  onHostUpdated?: (host: Host) => void;
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

function HostLabels({ labels }: { labels: string[] }) {
  if (labels.length === 0) {
    return <span className="text-muted-foreground">-</span>;
  }

  return (
    <div className="flex max-w-52 flex-wrap gap-1.5">
      {labels.map((label) => (
        <Badge key={label} variant="outline" className="max-w-28 truncate">
          {label}
        </Badge>
      ))}
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
          <p className="font-medium">{host.display_name || host.hostname}</p>
          <p className="text-xs text-muted-foreground">
            {[host.hostname, machineSummary(host, metricHistoryByHost[host.id] ?? [])]
              .filter(Boolean)
              .join(" · ")}
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
      render: (host) => <HostLabels labels={host.labels} />,
    },
    {
      key: "last_seen_at",
      label: "最后心跳",
      render: (host) =>
        formatRelativeTime(host.last_seen_at, { emptyText: "尚未收到" }),
    },
  ];
}

export function HostsPage({
  hosts,
  metricHistoryByHost = {},
  apiError,
  onHostDeleted,
  onHostUpdated,
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
  const [editTarget, setEditTarget] = useState<Host | null>(null);
  const [draftDisplayName, setDraftDisplayName] = useState("");
  const [draftLabels, setDraftLabels] = useState<string[]>([]);
  const [newLabel, setNewLabel] = useState("");
  const [editPending, setEditPending] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);
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

  function openEditDialog(host: Host) {
    setEditTarget(host);
    setDraftDisplayName(host.display_name || host.hostname);
    setDraftLabels(host.labels.length > 0 ? host.labels : [""]);
    setNewLabel("");
    setEditError(null);
  }

  function updateDraftLabel(index: number, value: string) {
    setDraftLabels((current) =>
      current.map((label, currentIndex) =>
        currentIndex === index ? value : label,
      ),
    );
  }

  function removeDraftLabel(index: number) {
    setDraftLabels((current) =>
      current.filter((_, currentIndex) => currentIndex !== index),
    );
  }

  function addDraftLabel() {
    const value = newLabel.trim();
    if (!value) {
      return;
    }
    setDraftLabels((current) => [...current, value]);
    setNewLabel("");
  }

  async function handleSaveHost() {
    if (!editTarget) {
      return;
    }

    setEditPending(true);
    setEditError(null);
    const labels = draftLabels.map((label) => label.trim()).filter(Boolean);
    const result = await updateHost(editTarget.id, {
      display_name: draftDisplayName,
      labels,
    });
    setEditPending(false);

    if (result.error || !result.data) {
      setEditError(result.error ?? "保存 Agent 失败");
      return;
    }

    onHostUpdated?.(result.data.item);
    setEditTarget(null);
    setDraftDisplayName("");
    setDraftLabels([]);
    setNewLabel("");
  }

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <div className="space-y-4">
        <div className="flex justify-end">
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
                      doro agent --config ~/.doro/agent.toml
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
        </div>

        <div className="grid gap-3 md:grid-cols-3">
          <div className="flex items-center gap-3 rounded-md bg-muted/35 px-4 py-3">
            <Server className="size-4 text-muted-foreground" />
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground">已注册 Agent</p>
              <p className="text-lg font-semibold leading-6">{hosts.length}</p>
            </div>
          </div>
          <div className="flex items-center gap-3 rounded-md bg-muted/35 px-4 py-3">
            <Activity className="size-4 text-muted-foreground" />
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground">当前在线</p>
              <p className="text-lg font-semibold leading-6">{onlineHosts}</p>
            </div>
          </div>
          <div className="flex items-center gap-3 rounded-md bg-muted/35 px-4 py-3">
            <Cpu className="size-4 text-muted-foreground" />
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground">声明能力</p>
              <p className="text-lg font-semibold leading-6">
                {declaredCapabilities}
              </p>
            </div>
          </div>
        </div>

        <DataTable
          columns={hostColumns(metricHistoryByHost)}
          rows={hosts}
          actions={[]}
          renderActions={(host) => (
            <>
              <Button
                aria-label={`编辑主机 ${host.hostname}`}
                title="编辑"
                variant="ghost"
                size="icon"
                className="size-8 text-muted-foreground"
                onClick={() => openEditDialog(host)}
              >
                <Pencil className="size-4" />
              </Button>
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
            </>
          )}
          emptyText="暂无已连接 Agent"
        />
      </div>
      <Dialog
        open={Boolean(editTarget)}
        onOpenChange={(open) => {
          if (!open && !editPending) {
            setEditTarget(null);
            setDraftDisplayName("");
            setDraftLabels([]);
            setNewLabel("");
            setEditError(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>编辑 Agent</DialogTitle>
            <DialogDescription>
              名称保存到 hosts.display_name，标签保存到 hosts.labels。
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-3">
            <div className="rounded-md border bg-muted/30 p-3 text-sm">
              <p className="font-medium">{editTarget?.hostname}</p>
              <p className="mt-1 text-xs text-muted-foreground">{editTarget?.id}</p>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium" htmlFor="agent-display-name">
                Agent 名称
              </label>
              <input
                id="agent-display-name"
                value={draftDisplayName}
                disabled={editPending}
                onChange={(event) => setDraftDisplayName(event.target.value)}
                className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                placeholder="Agent 名称"
              />
            </div>

            <div className="space-y-2">
              <p className="text-sm font-medium">标签</p>
              {draftLabels.length === 0 ? (
                <p className="rounded-md border border-dashed p-3 text-sm text-muted-foreground">
                  暂无标签
                </p>
              ) : (
                draftLabels.map((label, index) => (
                  <div key={`${index}-${label}`} className="flex items-center gap-2">
                    <input
                      value={label}
                      disabled={editPending}
                      onChange={(event) => updateDraftLabel(index, event.target.value)}
                      className="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                      placeholder="标签"
                    />
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      title="删除标签"
                      disabled={editPending}
                      className="size-9 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                      onClick={() => removeDraftLabel(index)}
                    >
                      <X className="size-4" />
                    </Button>
                  </div>
                ))
              )}
            </div>

            <div className="flex items-center gap-2">
              <input
                value={newLabel}
                disabled={editPending}
                onChange={(event) => setNewLabel(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    addDraftLabel();
                  }
                }}
                className="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                placeholder="新增标签"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                title="新增标签"
                disabled={editPending || !newLabel.trim()}
                onClick={addDraftLabel}
              >
                <Plus className="size-4" />
              </Button>
            </div>

            {editError ? (
              <div className="rounded-md border border-destructive/30 p-3 text-sm text-destructive">
                保存失败：{editError}
              </div>
            ) : null}
          </div>

          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline" disabled={editPending}>
                取消
              </Button>
            </DialogClose>
            <Button
              disabled={editPending || !draftDisplayName.trim()}
              onClick={handleSaveHost}
            >
              {editPending ? "保存中" : "保存"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
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
