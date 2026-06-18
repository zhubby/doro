"use client";

import { useEffect, useState } from "react";
import {
  Bell,
  Check,
  CircleGauge,
  Download,
  HardDrive,
  HardDriveDownload,
  HardDriveUpload,
  Network,
  Upload,
} from "lucide-react";

import { MetricGrid } from "@/components/dashboard/overview/metric-grid";
import type { TrendPoint } from "@/components/dashboard/overview/trend-preview";
import { TrendPreview } from "@/components/dashboard/overview/trend-preview";
import { ControlPlaneEnvironmentPanel } from "@/components/dashboard/overview/control-plane-environment";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Select } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useToastMessage } from "@/components/ui/use-toast-message";
import { markSystemNotificationRead } from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import type {
  ApprovalRequest,
  ControlPlaneEnvironment,
  Host,
  HostContainer,
  MetricSnapshot,
  SystemNotification,
  VirtualMachine,
} from "@/types/api";
import type { Metric } from "@/types/dashboard";

type OverviewPageProps = {
  hosts?: Host[];
  approvals?: ApprovalRequest[];
  virtualMachines?: VirtualMachine[];
  containers?: HostContainer[];
  controlPlaneEnvironment?: ControlPlaneEnvironment | null;
  metricHistoryByHost?: Record<string, MetricSnapshot[]>;
  systemNotifications?: SystemNotification[];
  onSystemNotificationRead?: (notificationId: string) => void;
  apiError?: string | null;
};

type ResourceStat = {
  label: string;
  value: string;
  detail: string;
  progress: number;
};

type DiskTotals = {
  usedBytes: number;
  totalBytes: number;
};

function objectValue(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function numberValue(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function formatPercent(value: number) {
  return `${value.toFixed(2)}%`;
}

function metricProgress(value?: number | null) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return 0;
  }
  return Math.min(100, Math.max(0, value));
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes)) {
    return "-";
  }
  if (bytes < 1024) {
    return `${bytes.toFixed(0)} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value >= 10 ? value.toFixed(1) : value.toFixed(2)} ${units[unitIndex]}`;
}

function formatBytesPerSecond(bytes: number) {
  return `${formatBytes(bytes)}/s`;
}

function coreCount(host: Host) {
  const profile = objectValue(host.system_profile);
  return numberValue(profile?.logical_core_count) ?? 0;
}

function totalMemoryBytes(host: Host) {
  const profile = objectValue(host.system_profile);
  const memory = objectValue(profile?.memory);
  return numberValue(memory?.total_bytes) ?? 0;
}

function physicalCoreCount(host: Host) {
  const profile = objectValue(host.system_profile);
  return numberValue(profile?.physical_core_count) ?? 0;
}

function diskTotals(snapshot: MetricSnapshot): DiskTotals | null {
  const extra = objectValue(snapshot.extra);
  const disks = extra?.disks;
  if (!Array.isArray(disks)) {
    return null;
  }

  const totals = disks.reduce<DiskTotals>(
    (current, disk) => {
      const diskObject = objectValue(disk);
      if (!diskObject) {
        return current;
      }

      return {
        usedBytes: current.usedBytes + (numberValue(diskObject.used_bytes) ?? 0),
        totalBytes: current.totalBytes + (numberValue(diskObject.total_bytes) ?? 0),
      };
    },
    { usedBytes: 0, totalBytes: 0 },
  );

  return totals.totalBytes > 0 ? totals : null;
}

function hostLabel(host: Host) {
  return host.display_name || host.hostname;
}

function hostStatusText(status?: Host["status"]) {
  if (status === "online") {
    return "在线";
  }
  if (status === "degraded") {
    return "需关注";
  }
  if (status === "pending") {
    return "待接入";
  }
  return "离线";
}

function preferredHost(hosts: Host[]) {
  return hosts.find((host) => host.status === "online") ?? hosts[0] ?? null;
}

function latestMetric(history: MetricSnapshot[]) {
  return history.at(-1) ?? null;
}

function selectedMetricHistory(
  selectedHost: Host | null,
  metricHistoryByHost: Record<string, MetricSnapshot[]>,
) {
  return selectedHost ? (metricHistoryByHost[selectedHost.id] ?? []) : [];
}

function sortedMetricHistory(history: MetricSnapshot[]) {
  return [...history].sort(
    (left, right) =>
      new Date(left.captured_at).getTime() -
      new Date(right.captured_at).getTime(),
  );
}

function sumIoFields(
  snapshots: MetricSnapshot[],
  extraKey: "networks" | "disk_io",
  fields: [string, string, string, string],
) {
  return snapshots.reduce(
    (totals, snapshot) => {
      const extra = objectValue(snapshot.extra);
      const items = extra?.[extraKey];
      if (!Array.isArray(items)) {
        return totals;
      }

      for (const item of items) {
        const itemObject = objectValue(item);
        totals.primaryRate += numberValue(itemObject?.[fields[0]]) ?? 0;
        totals.secondaryRate += numberValue(itemObject?.[fields[1]]) ?? 0;
        totals.primaryTotal += numberValue(itemObject?.[fields[2]]) ?? 0;
        totals.secondaryTotal += numberValue(itemObject?.[fields[3]]) ?? 0;
      }
      return totals;
    },
    {
      primaryRate: 0,
      secondaryRate: 0,
      primaryTotal: 0,
      secondaryTotal: 0,
    },
  );
}

function trafficMetricCards(metric: MetricSnapshot | null): Metric[] {
  const totals = metric
    ? sumIoFields([metric], "networks", [
        "transmitted_bytes_per_second",
        "received_bytes_per_second",
        "total_transmitted_bytes",
        "total_received_bytes",
      ])
    : null;

  return [
    {
      label: "上行",
      value: totals ? formatBytesPerSecond(totals.primaryRate) : "等待采集",
    },
    {
      label: "下行",
      value: totals ? formatBytesPerSecond(totals.secondaryRate) : "等待采集",
    },
    { label: "总发送", value: totals ? formatBytes(totals.primaryTotal) : "等待采集" },
    { label: "总接收", value: totals ? formatBytes(totals.secondaryTotal) : "等待采集" },
  ];
}

function diskIoMetricCards(metric: MetricSnapshot | null): Metric[] {
  const totals = metric
    ? sumIoFields([metric], "disk_io", [
        "read_bytes_per_second",
        "write_bytes_per_second",
        "total_read_bytes",
        "total_written_bytes",
      ])
    : null;

  return [
    {
      label: "读取",
      value: totals ? formatBytesPerSecond(totals.primaryRate) : "等待采集",
    },
    {
      label: "写入",
      value: totals ? formatBytesPerSecond(totals.secondaryRate) : "等待采集",
    },
    { label: "总读取", value: totals ? formatBytes(totals.primaryTotal) : "等待采集" },
    { label: "总写入", value: totals ? formatBytes(totals.secondaryTotal) : "等待采集" },
  ];
}

function trendPoints(
  history: MetricSnapshot[],
  extraKey: "networks" | "disk_io",
  fields: [string, string],
): TrendPoint[] {
  return sortedMetricHistory(history)
    .map((snapshot) => {
      const totals = sumIoFields([snapshot], extraKey, [fields[0], fields[1], "", ""]);
      return {
        capturedAt: snapshot.captured_at,
        primary: totals.primaryRate,
        secondary: totals.secondaryRate,
      };
    })
    .slice(-24);
}

function unavailableResourceStats(selectedHost: Host | null): ResourceStat[] {
  return ["负载", "CPU", "内存", "磁盘"].map((label) => ({
    label,
    value: "n/a",
    detail: selectedHost ? "等待 Agent 上报" : "未选择 Agent",
    progress: 0,
  }));
}

function hostResourceStats(
  selectedHost: Host | null,
  metric: MetricSnapshot | null,
): ResourceStat[] {
  if (!selectedHost || !metric) {
    return unavailableResourceStats(selectedHost);
  }

  const totalCores = coreCount(selectedHost);
  const physicalCores = physicalCoreCount(selectedHost);
  const cpuPercent = metric.cpu_percent;
  const loadPercent =
    totalCores > 0 ? (metric.load_average / totalCores) * 100 : metric.load_average * 100;

  const memoryTotalBytes = totalMemoryBytes(selectedHost);
  const memoryUsedBytes = (memoryTotalBytes * metric.memory_percent) / 100;
  const memoryPercent =
    memoryTotalBytes > 0 ? (memoryUsedBytes / memoryTotalBytes) * 100 : metric.memory_percent;

  const disk = diskTotals(metric);
  const diskPercent =
    disk && disk.totalBytes > 0 ? (disk.usedBytes / disk.totalBytes) * 100 : metric.disk_percent;

  return [
    {
      label: "负载",
      value: formatPercent(Math.max(0, loadPercent)),
      detail:
        totalCores > 0
          ? `${metric.load_average.toFixed(2)} / ${totalCores} 核`
          : `${metric.load_average.toFixed(2)} load average`,
      progress: Math.min(100, Math.max(0, loadPercent)),
    },
    {
      label: "CPU",
      value: formatPercent(cpuPercent),
      detail:
        physicalCores || totalCores
          ? `${physicalCores || "-"}C / ${totalCores || "-"}T`
          : "等待系统 profile",
      progress: metricProgress(cpuPercent),
    },
    {
      label: "内存",
      value: formatPercent(memoryPercent),
      detail:
        memoryTotalBytes > 0
          ? `${formatBytes(memoryUsedBytes)} / ${formatBytes(memoryTotalBytes)}`
          : "等待容量数据",
      progress: metricProgress(memoryPercent),
    },
    {
      label: "磁盘",
      value: formatPercent(diskPercent),
      detail: disk
        ? `${formatBytes(disk.usedBytes)} / ${formatBytes(disk.totalBytes)}`
        : "等待容量数据",
      progress: metricProgress(diskPercent),
    },
  ];
}

function metricStatusLabel(selectedHost: Host | null, metric: MetricSnapshot | null) {
  if (!selectedHost) {
    return "暂无 Agent";
  }
  if (metric) {
    return "已采集";
  }
  return selectedHost.status === "online" ? "等待数据" : "Agent 离线";
}

function severityLabel(severity: SystemNotification["severity"]) {
  if (severity === "critical") {
    return "严重";
  }
  if (severity === "warning") {
    return "警告";
  }
  return "信息";
}

function severityVariant(severity: SystemNotification["severity"]) {
  if (severity === "critical") {
    return "destructive" as const;
  }
  if (severity === "warning") {
    return "secondary" as const;
  }
  return "outline" as const;
}

function AgentSelect({
  hosts,
  selectedHostId,
  onSelectedHostChange,
  ariaLabel,
}: {
  hosts: Host[];
  selectedHostId: string;
  onSelectedHostChange: (hostId: string) => void;
  ariaLabel: string;
}) {
  return (
    <Select
      value={selectedHostId}
      onValueChange={onSelectedHostChange}
      disabled={hosts.length === 0}
      placeholder={hosts.length === 0 ? "暂无 Agent" : "选择 Agent"}
      align="end"
      aria-label={ariaLabel}
      className="w-full md:w-56"
      options={hosts.map((host) => ({
        value: host.id,
        label: (
          <span className="flex min-w-0 items-center justify-between gap-3">
            <span className="min-w-0 truncate">{hostLabel(host)}</span>
            <span className="shrink-0 text-xs text-muted-foreground">
              {hostStatusText(host.status)}
            </span>
          </span>
        ),
      }))}
    />
  );
}

function SystemNotificationList({
  notifications,
  pendingNotificationId,
  onMarkRead,
}: {
  notifications: SystemNotification[];
  pendingNotificationId: string | null;
  onMarkRead: (notificationId: string) => void;
}) {
  if (notifications.length === 0) {
    return (
      <div className="flex min-h-32 items-center justify-center rounded-lg border border-dashed bg-muted/30 px-4 text-center text-sm text-muted-foreground">
        暂无未读站内通知
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {notifications.map((notification) => (
        <div key={notification.id} className="rounded-lg border p-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 space-y-1">
              <div className="flex min-w-0 items-center gap-2">
                <Badge variant={severityVariant(notification.severity)}>
                  {severityLabel(notification.severity)}
                </Badge>
                <p className="truncate text-sm font-semibold">{notification.title}</p>
              </div>
              <p className="line-clamp-2 text-xs text-muted-foreground">
                {notification.body}
              </p>
              <p className="text-xs text-muted-foreground">
                {formatRelativeTime(notification.created_at)}
              </p>
            </div>
            <Button
              size="icon"
              variant="ghost"
              aria-label="标记已读"
              disabled={pendingNotificationId === notification.id}
              onClick={() => onMarkRead(notification.id)}
            >
              <Check className="size-4" aria-hidden="true" />
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}

export function OverviewPage({
  hosts = [],
  approvals = [],
  virtualMachines = [],
  containers = [],
  controlPlaneEnvironment = null,
  metricHistoryByHost = {},
  systemNotifications = [],
  onSystemNotificationRead,
  apiError,
}: OverviewPageProps) {
  const [selectedHostId, setSelectedHostId] = useState("");
  const [notificationActionError, setNotificationActionError] = useState<string | null>(null);
  const [pendingNotificationId, setPendingNotificationId] = useState<string | null>(null);

  useEffect(() => {
    setSelectedHostId((current) => {
      if (current && hosts.some((host) => host.id === current)) {
        return current;
      }
      return preferredHost(hosts)?.id ?? "";
    });
  }, [hosts]);

  const selectedHost =
    hosts.find((host) => host.id === selectedHostId) ?? preferredHost(hosts);
  const selectedHistory = selectedMetricHistory(selectedHost, metricHistoryByHost);
  const selectedMetric = latestMetric(selectedHistory);
  const waitingApprovals = approvals.filter(
    (approval) => approval.status === "pending",
  ).length;
  const onlineHosts = hosts.filter((host) => host.status === "online").length;
  const runningVirtualMachines = virtualMachines.filter(
    (machine) => machine.status === "running",
  ).length;
  const runningContainers = containers.filter(
    (container) => container.status === "running",
  ).length;
  const selectedHostValue = selectedHost?.id ?? "";
  const systemStats = hostResourceStats(selectedHost, selectedMetric);
  const trafficMetrics = trafficMetricCards(selectedMetric);
  const diskMetrics = diskIoMetricCards(selectedMetric);
  const trafficTrend = trendPoints(selectedHistory, "networks", [
    "transmitted_bytes_per_second",
    "received_bytes_per_second",
  ]);
  const diskTrend = trendPoints(selectedHistory, "disk_io", [
    "read_bytes_per_second",
    "write_bytes_per_second",
  ]);
  const overviewStats = [
    {
      label: "智能体",
      value: String(hosts.length),
      helper: `${onlineHosts} 个在线`,
    },
    {
      label: "审批",
      value: String(approvals.length),
      helper: waitingApprovals > 0 ? `${waitingApprovals} 个待处理` : "当前无需处理",
    },
    {
      label: "虚拟机总数",
      value: String(virtualMachines.length),
      helper:
        virtualMachines.length > 0
          ? `${runningVirtualMachines} 台运行中`
          : "等待虚拟机接入",
    },
    {
      label: "容器总数",
      value: String(containers.length),
      helper: containers.length > 0 ? `${runningContainers} 个运行中` : "等待容器接入",
    },
  ];

  useToastMessage(apiError, {
    id: "overview-api-error",
    kind: "error",
    prefix: "控制平面暂不可用：",
  });
  useToastMessage(notificationActionError, {
    id: "overview-system-notification-error",
    kind: "error",
    prefix: "站内通知更新失败：",
  });

  async function handleMarkSystemNotificationRead(notificationId: string) {
    setPendingNotificationId(notificationId);
    setNotificationActionError(null);
    const result = await markSystemNotificationRead(notificationId);
    setPendingNotificationId(null);
    if (!result.data) {
      setNotificationActionError(result.error ?? "无法标记已读");
      return;
    }
    onSystemNotificationRead?.(notificationId);
  }

  return (
    <PageContainer>
      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <div className="space-y-6 xl:col-start-1 xl:row-start-1">
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
            {overviewStats.map((stat) => (
              <Card key={stat.label}>
                <CardHeader className="pb-2">
                  <CardDescription>{stat.label}</CardDescription>
                  <CardTitle className="text-3xl">{stat.value}</CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-sm text-muted-foreground">{stat.helper}</p>
                </CardContent>
              </Card>
            ))}
          </div>

          <Card>
            <CardHeader>
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div>
                  <CardTitle>系统状态</CardTitle>
                  <CardDescription>展示所选 Agent 的关键资源使用率</CardDescription>
                </div>
                <div className="flex w-full flex-col gap-2 md:w-auto md:flex-row md:items-center md:justify-end">
                  <Badge variant="outline">
                    {metricStatusLabel(selectedHost, selectedMetric)}
                  </Badge>
                  <AgentSelect
                    hosts={hosts}
                    selectedHostId={selectedHostValue}
                    onSelectedHostChange={setSelectedHostId}
                    ariaLabel="选择系统状态 Agent"
                  />
                </div>
              </div>
            </CardHeader>
            <CardContent className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
              {systemStats.map((stat) => (
                <div key={stat.label} className="rounded-lg border p-4">
                  <div className="mb-4 flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <CircleGauge
                        className="size-4 text-muted-foreground"
                        aria-hidden="true"
                      />
                      <span className="text-sm font-medium">{stat.label}</span>
                    </div>
                    <span className="text-sm font-semibold">{stat.value}</span>
                  </div>
                  <Progress value={stat.progress} />
                  <p className="mt-3 text-xs text-muted-foreground">
                    {stat.detail}
                  </p>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>

        <Card className="h-full xl:col-start-2 xl:row-start-1">
          <CardHeader>
            <div className="flex items-center justify-between">
              <div>
                <CardTitle>站内通知</CardTitle>
                <CardDescription>未读系统通知</CardDescription>
              </div>
              <Badge variant="outline" className="gap-1.5">
                <Bell className="size-3.5" aria-hidden="true" />
                {systemNotifications.length}
              </Badge>
            </div>
          </CardHeader>
          <CardContent>
            <SystemNotificationList
              notifications={systemNotifications}
              pendingNotificationId={pendingNotificationId}
              onMarkRead={handleMarkSystemNotificationRead}
            />
          </CardContent>
        </Card>

        <Card className="h-full xl:col-start-1 xl:row-start-2">
          <CardHeader>
            <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
              <div>
                <CardTitle>监控</CardTitle>
                <CardDescription>展示所选 Agent 的流量和磁盘 IO 趋势</CardDescription>
              </div>
              <div className="flex w-full flex-col gap-2 md:w-auto md:flex-row md:items-center md:justify-end">
                <Badge variant="secondary">最近 240 条</Badge>
                <AgentSelect
                  hosts={hosts}
                  selectedHostId={selectedHostValue}
                  onSelectedHostChange={setSelectedHostId}
                  ariaLabel="选择监控 Agent"
                />
              </div>
            </div>
          </CardHeader>
          <CardContent>
            <Tabs defaultValue="traffic">
              <TabsList>
                <TabsTrigger value="traffic">
                  <Network className="mr-2 size-4" aria-hidden="true" />
                  流量
                </TabsTrigger>
                <TabsTrigger value="disk">
                  <HardDrive className="mr-2 size-4" aria-hidden="true" />
                  磁盘 IO
                </TabsTrigger>
              </TabsList>
              <TabsContent value="traffic" className="space-y-6">
                <MetricGrid metrics={trafficMetrics} />
                <TrendPreview
                  label="网络吞吐趋势"
                  points={trafficTrend}
                  seriesLabels={["上行", "下行"]}
                  seriesIcons={[Upload, Download]}
                  emptyText="暂无网络趋势数据，等待 Agent 指标采集"
                  valueFormatter={formatBytesPerSecond}
                />
              </TabsContent>
              <TabsContent value="disk" className="space-y-6">
                <MetricGrid metrics={diskMetrics} />
                <TrendPreview
                  label="磁盘读写趋势"
                  points={diskTrend}
                  seriesLabels={["读取", "写入"]}
                  seriesIcons={[HardDriveDownload, HardDriveUpload]}
                  emptyText="暂无磁盘 IO 趋势数据，等待 Agent 指标采集"
                  valueFormatter={formatBytesPerSecond}
                />
              </TabsContent>
            </Tabs>
          </CardContent>
        </Card>

        <ControlPlaneEnvironmentPanel
          environment={controlPlaneEnvironment}
          className="h-full xl:col-start-2 xl:row-start-2"
        />
      </div>
    </PageContainer>
  );
}
