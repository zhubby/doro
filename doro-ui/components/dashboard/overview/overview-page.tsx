"use client";

import { AlertTriangle, CircleGauge, HardDrive, Network, NotebookPen } from "lucide-react";

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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { notes } from "@/lib/mock-data";
import type { AppSummary, ApprovalRequest, ControlPlaneEnvironment, Host, MetricSnapshot, Task } from "@/types/api";
import type { Metric } from "@/types/dashboard";

type OverviewPageProps = {
  hosts?: Host[];
  tasks?: Task[];
  approvals?: ApprovalRequest[];
  apps?: AppSummary[];
  controlPlaneEnvironment?: ControlPlaneEnvironment | null;
  metricHistoryByHost?: Record<string, MetricSnapshot[]>;
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

function metricHistories(metricHistoryByHost: Record<string, MetricSnapshot[]>) {
  return Object.values(metricHistoryByHost)
    .flat()
    .sort(
      (left, right) =>
        new Date(left.captured_at).getTime() - new Date(right.captured_at).getTime(),
    );
}

function latestMetricSnapshots(metricHistoryByHost: Record<string, MetricSnapshot[]>) {
  return Object.values(metricHistoryByHost)
    .map((history) => history.at(-1))
    .filter((snapshot): snapshot is MetricSnapshot => Boolean(snapshot));
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

function aggregateTrafficMetrics(
  metricHistoryByHost: Record<string, MetricSnapshot[]>,
): Metric[] {
  const snapshots = latestMetricSnapshots(metricHistoryByHost);
  const totals = sumIoFields(snapshots, "networks", [
    "transmitted_bytes_per_second",
    "received_bytes_per_second",
    "total_transmitted_bytes",
    "total_received_bytes",
  ]);
  const hasData = snapshots.length > 0;

  return [
    { label: "上行", value: hasData ? formatBytesPerSecond(totals.primaryRate) : "等待采集" },
    { label: "下行", value: hasData ? formatBytesPerSecond(totals.secondaryRate) : "等待采集" },
    { label: "总发送", value: hasData ? formatBytes(totals.primaryTotal) : "等待采集" },
    { label: "总接收", value: hasData ? formatBytes(totals.secondaryTotal) : "等待采集" },
  ];
}

function aggregateDiskIoMetrics(
  metricHistoryByHost: Record<string, MetricSnapshot[]>,
): Metric[] {
  const snapshots = latestMetricSnapshots(metricHistoryByHost);
  const totals = sumIoFields(snapshots, "disk_io", [
    "read_bytes_per_second",
    "write_bytes_per_second",
    "total_read_bytes",
    "total_written_bytes",
  ]);
  const hasData = snapshots.length > 0;

  return [
    { label: "读取", value: hasData ? formatBytesPerSecond(totals.primaryRate) : "等待采集" },
    { label: "写入", value: hasData ? formatBytesPerSecond(totals.secondaryRate) : "等待采集" },
    { label: "总读取", value: hasData ? formatBytes(totals.primaryTotal) : "等待采集" },
    { label: "总写入", value: hasData ? formatBytes(totals.secondaryTotal) : "等待采集" },
  ];
}

function trendPoints(
  metricHistoryByHost: Record<string, MetricSnapshot[]>,
  extraKey: "networks" | "disk_io",
  fields: [string, string],
): TrendPoint[] {
  return metricHistories(metricHistoryByHost)
    .map((snapshot) => {
      const totals = sumIoFields([snapshot], extraKey, [fields[0], fields[1], "", ""]);
      return {
        primary: totals.primaryRate,
        secondary: totals.secondaryRate,
      };
    })
    .filter((point) => point.primary > 0 || point.secondary > 0)
    .slice(-24);
}

function latestMetrics(
  hosts: Host[],
  metricHistoryByHost: Record<string, MetricSnapshot[]>,
) {
  return hosts
    .filter((host) => host.status === "online")
    .map((host) => ({
      host,
      metric: metricHistoryByHost[host.id]?.at(-1) ?? null,
    }))
    .filter((item): item is { host: Host; metric: MetricSnapshot } => Boolean(item.metric));
}

function unavailableResourceStats(hasOnlineAgents: boolean): ResourceStat[] {
  return ["负载", "CPU", "内存", "磁盘"].map((label) => ({
    label,
    value: "n/a",
    detail: hasOnlineAgents ? "等待 Agent 上报" : "等待 Agent 连接",
    progress: 0,
  }));
}

function aggregateResourceStats(
  hosts: Host[],
  metricHistoryByHost: Record<string, MetricSnapshot[]>,
): ResourceStat[] {
  const samples = latestMetrics(hosts, metricHistoryByHost);
  if (samples.length === 0) {
    return unavailableResourceStats(hosts.some((host) => host.status === "online"));
  }

  const totalCores = samples.reduce((sum, sample) => sum + coreCount(sample.host), 0);
  const cpuWeightedUnits = samples.reduce((sum, sample) => {
    const cores = coreCount(sample.host) || 1;
    return sum + sample.metric.cpu_percent * cores;
  }, 0);
  const cpuWeight = samples.reduce((sum, sample) => sum + (coreCount(sample.host) || 1), 0);
  const cpuPercent = cpuWeight > 0 ? cpuWeightedUnits / cpuWeight : 0;

  const totalLoad = samples.reduce((sum, sample) => sum + sample.metric.load_average, 0);
  const loadPercent = totalCores > 0 ? (totalLoad / totalCores) * 100 : totalLoad * 100;

  const memoryTotals = samples.reduce(
    (current, sample) => {
      const totalBytes = totalMemoryBytes(sample.host);
      if (totalBytes === 0) {
        return current;
      }
      return {
        usedBytes: current.usedBytes + (totalBytes * sample.metric.memory_percent) / 100,
        totalBytes: current.totalBytes + totalBytes,
      };
    },
    { usedBytes: 0, totalBytes: 0 },
  );
  const memoryPercent =
    memoryTotals.totalBytes > 0 ? (memoryTotals.usedBytes / memoryTotals.totalBytes) * 100 : null;

  const disk = samples.reduce(
    (current, sample) => {
      const totals = diskTotals(sample.metric);
      if (!totals) {
        return current;
      }
      return {
        usedBytes: current.usedBytes + totals.usedBytes,
        totalBytes: current.totalBytes + totals.totalBytes,
      };
    },
    { usedBytes: 0, totalBytes: 0 },
  );
  const diskPercent = disk.totalBytes > 0 ? (disk.usedBytes / disk.totalBytes) * 100 : null;

  return [
    {
      label: "负载",
      value: formatPercent(Math.max(0, loadPercent)),
      detail: totalCores > 0 ? `${totalLoad.toFixed(2)} / ${totalCores} 核` : "按在线 Agent 汇总",
      progress: Math.min(100, Math.max(0, loadPercent)),
    },
    {
      label: "CPU",
      value: formatPercent(cpuPercent),
      detail: totalCores > 0 ? `${totalLoad.toFixed(2)} / ${totalCores} 核` : "按在线 Agent 汇总",
      progress: Math.min(100, Math.max(0, cpuPercent)),
    },
    {
      label: "内存",
      value: memoryPercent === null ? "n/a" : formatPercent(memoryPercent),
      detail:
        memoryPercent === null
          ? "等待容量数据"
          : `${formatBytes(memoryTotals.usedBytes)} / ${formatBytes(memoryTotals.totalBytes)}`,
      progress: memoryPercent === null ? 0 : Math.min(100, Math.max(0, memoryPercent)),
    },
    {
      label: "磁盘",
      value: diskPercent === null ? "n/a" : formatPercent(diskPercent),
      detail:
        diskPercent === null
          ? "等待容量数据"
          : `${formatBytes(disk.usedBytes)} / ${formatBytes(disk.totalBytes)}`,
      progress: diskPercent === null ? 0 : Math.min(100, Math.max(0, diskPercent)),
    },
  ];
}

export function OverviewPage({
  hosts = [],
  tasks = [],
  approvals = [],
  apps = [],
  controlPlaneEnvironment = null,
  metricHistoryByHost = {},
  apiError,
}: OverviewPageProps) {
  const waitingApprovals = approvals.filter(
    (approval) => approval.status === "pending",
  ).length;
  const onlineHosts = hosts.filter((host) => host.status === "online").length;
  const systemStats = aggregateResourceStats(hosts, metricHistoryByHost);
  const trafficMetrics = aggregateTrafficMetrics(metricHistoryByHost);
  const diskMetrics = aggregateDiskIoMetrics(metricHistoryByHost);
  const trafficTrend = trendPoints(metricHistoryByHost, "networks", [
    "transmitted_bytes_per_second",
    "received_bytes_per_second",
  ]);
  const diskTrend = trendPoints(metricHistoryByHost, "disk_io", [
    "read_bytes_per_second",
    "write_bytes_per_second",
  ]);
  const hasOnlineAgents = hosts.some((host) => host.status === "online");
  const hasMetricSamples = latestMetrics(hosts, metricHistoryByHost).length > 0;
  const overviewStats = [
    {
      label: "智能体",
      value: String(hosts.length),
      helper: `${onlineHosts} 个在线`,
    },
    {
      label: "任务",
      value: String(tasks.length),
      helper: waitingApprovals > 0 ? `${waitingApprovals} 个等待审批` : "无阻塞任务",
    },
    {
      label: "审批",
      value: String(approvals.length),
      helper: waitingApprovals > 0 ? `${waitingApprovals} 个待处理` : "当前无需处理",
    },
    {
      label: "应用",
      value: String(apps.length),
      helper: apps.length > 0 ? "已接入应用目录" : "等待应用接入",
    },
  ];

  return (
    <PageContainer>
      {apiError ? (
        <Card className="border-destructive/30">
          <CardContent className="flex items-center gap-3 pt-6 text-sm text-muted-foreground">
            <AlertTriangle className="size-4 text-destructive" aria-hidden="true" />
            控制平面暂不可用：{apiError}
          </CardContent>
        </Card>
      ) : null}

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
                  <CardDescription>关键资源使用率与容量概览</CardDescription>
                </div>
                <Badge variant="outline">
                  {hasMetricSamples
                    ? "运行正常"
                    : hasOnlineAgents
                      ? "等待数据"
                      : "等待 Agent"}
                </Badge>
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
                <CardTitle>备忘录</CardTitle>
                <CardDescription>运维提醒与待办记录</CardDescription>
              </div>
              <Button size="icon" variant="outline" aria-label="添加备忘">
                <NotebookPen className="size-4" aria-hidden="true" />
              </Button>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {notes.map((note) => (
              <div key={note} className="rounded-lg border p-3 text-sm">
                {note}
              </div>
            ))}
          </CardContent>
        </Card>

        <Card className="h-full xl:col-start-1 xl:row-start-2">
          <CardHeader>
            <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
              <div>
                <CardTitle>监控</CardTitle>
                <CardDescription>展示流量和磁盘 IO 趋势</CardDescription>
              </div>
              <Badge variant="secondary">最近 240 条</Badge>
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
                  emptyText="暂无网络趋势数据，等待 Agent 指标采集"
                />
              </TabsContent>
              <TabsContent value="disk" className="space-y-6">
                <MetricGrid metrics={diskMetrics} />
                <TrendPreview
                  label="磁盘读写趋势"
                  points={diskTrend}
                  seriesLabels={["读取", "写入"]}
                  emptyText="暂无磁盘 IO 趋势数据，等待 Agent 指标采集"
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
