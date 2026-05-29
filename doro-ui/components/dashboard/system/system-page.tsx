import { Activity, HardDrive, Network, ShieldCheck } from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { MetricGrid } from "@/components/dashboard/overview/metric-grid";
import { TrendPreview } from "@/components/dashboard/overview/trend-preview";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { formatRelativeTime } from "@/lib/datetime";
import { systemInfo } from "@/lib/mock-data";
import type { Host, MetricSnapshot } from "@/types/api";
import type { Metric, SystemMetric } from "@/types/dashboard";

const icons = [Activity, HardDrive, Network, ShieldCheck];

type SystemPageProps = {
  hosts?: Host[];
  metric?: MetricSnapshot | null;
  apiError?: string | null;
};

function formatPercent(value?: number) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return "-";
  }
  return `${value.toFixed(1)}%`;
}

function formatLoad(value?: number) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return "-";
  }
  return value.toFixed(2);
}

function objectValue(value: unknown) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function numberValue(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function stringValue(value: unknown) {
  return typeof value === "string" && value.trim() ? value : null;
}

function formatBytes(bytes?: number | null) {
  if (typeof bytes !== "number" || !Number.isFinite(bytes)) {
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
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

function formatBytesPerSecond(bytes?: number | null) {
  const formatted = formatBytes(bytes);
  return formatted === "-" ? "-" : `${formatted}/s`;
}

function metricProgress(value?: number) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return 0;
  }
  return Math.min(100, Math.max(0, value));
}

function systemMetrics(metric?: MetricSnapshot | null): SystemMetric[] {
  return [
    {
      label: "CPU",
      value: formatPercent(metric?.cpu_percent),
      progress: metricProgress(metric?.cpu_percent),
      detail: metric ? "来自 Agent 本地采集" : "尚未收到采集快照",
    },
    {
      label: "内存",
      value: formatPercent(metric?.memory_percent),
      progress: metricProgress(metric?.memory_percent),
      detail: metric ? "按已用内存 / 总内存计算" : "等待 Agent 上报",
    },
    {
      label: "磁盘",
      value: formatPercent(metric?.disk_percent),
      progress: metricProgress(metric?.disk_percent),
      detail: metric ? "按已用容量 / 总容量计算" : "等待 Agent 上报",
    },
    {
      label: "负载",
      value: formatLoad(metric?.load_average),
      progress: metricProgress((metric?.load_average ?? 0) * 20),
      detail: metric ? "1 分钟 load average" : "等待 Agent 上报",
    },
  ];
}

function liveTrafficMetrics(metric?: MetricSnapshot | null): Metric[] {
  return [
    { label: "采集主机", value: metric?.host_id.slice(0, 8) ?? "-" },
    {
      label: "采集时间",
      value: formatRelativeTime(metric?.captured_at),
    },
  ];
}

function networkMetrics(metric?: MetricSnapshot | null): Metric[] {
  const extra = objectValue(metric?.extra);
  const networks = Array.isArray(extra?.networks) ? extra.networks : [];
  const totals = networks.reduce(
    (current, network) => {
      const networkObject = objectValue(network);
      return {
        receivedBytesPerSecond:
          current.receivedBytesPerSecond +
          (numberValue(networkObject?.received_bytes_per_second) ?? 0),
        transmittedBytesPerSecond:
          current.transmittedBytesPerSecond +
          (numberValue(networkObject?.transmitted_bytes_per_second) ?? 0),
        totalReceivedBytes:
          current.totalReceivedBytes +
          (numberValue(networkObject?.total_received_bytes) ?? 0),
        totalTransmittedBytes:
          current.totalTransmittedBytes +
          (numberValue(networkObject?.total_transmitted_bytes) ?? 0),
      };
    },
    {
      receivedBytesPerSecond: 0,
      transmittedBytesPerSecond: 0,
      totalReceivedBytes: 0,
      totalTransmittedBytes: 0,
    },
  );

  const activeInterface = networks
    .map((network) => objectValue(network))
    .filter((network): network is Record<string, unknown> => Boolean(network))
    .sort(
      (left, right) =>
        (numberValue(right.received_bytes_per_second) ?? 0) +
        (numberValue(right.transmitted_bytes_per_second) ?? 0) -
        ((numberValue(left.received_bytes_per_second) ?? 0) +
          (numberValue(left.transmitted_bytes_per_second) ?? 0)),
    )[0];

  return [
    {
      label: "网络下行",
      value: metric ? formatBytesPerSecond(totals.receivedBytesPerSecond) : "等待采集",
    },
    {
      label: "网络上行",
      value: metric ? formatBytesPerSecond(totals.transmittedBytesPerSecond) : "等待采集",
    },
    {
      label: "累计接收",
      value: metric ? formatBytes(totals.totalReceivedBytes) : "等待采集",
    },
    {
      label: "活跃接口",
      value:
        stringValue(activeInterface?.name) ??
        (metric && networks.length === 0 ? "暂无接口数据" : "等待采集"),
    },
  ];
}

function diskIoMetrics(metric?: MetricSnapshot | null): Metric[] {
  const extra = objectValue(metric?.extra);
  const disks = Array.isArray(extra?.disk_io) ? extra.disk_io : [];
  const totals = disks.reduce(
    (current, disk) => {
      const diskObject = objectValue(disk);
      return {
        readBytesPerSecond:
          current.readBytesPerSecond +
          (numberValue(diskObject?.read_bytes_per_second) ?? 0),
        writeBytesPerSecond:
          current.writeBytesPerSecond +
          (numberValue(diskObject?.write_bytes_per_second) ?? 0),
        totalReadBytes:
          current.totalReadBytes +
          (numberValue(diskObject?.total_read_bytes) ?? 0),
        totalWrittenBytes:
          current.totalWrittenBytes +
          (numberValue(diskObject?.total_written_bytes) ?? 0),
      };
    },
    {
      readBytesPerSecond: 0,
      writeBytesPerSecond: 0,
      totalReadBytes: 0,
      totalWrittenBytes: 0,
    },
  );

  const busiestDisk = disks
    .map((disk) => objectValue(disk))
    .filter((disk): disk is Record<string, unknown> => Boolean(disk))
    .sort(
      (left, right) =>
        (numberValue(right.read_bytes_per_second) ?? 0) +
        (numberValue(right.write_bytes_per_second) ?? 0) -
        ((numberValue(left.read_bytes_per_second) ?? 0) +
          (numberValue(left.write_bytes_per_second) ?? 0)),
    )[0];

  return [
    {
      label: "磁盘读取",
      value: metric ? formatBytesPerSecond(totals.readBytesPerSecond) : "等待采集",
    },
    {
      label: "磁盘写入",
      value: metric ? formatBytesPerSecond(totals.writeBytesPerSecond) : "等待采集",
    },
    {
      label: "累计读写",
      value: metric
        ? `${formatBytes(totals.totalReadBytes)} / ${formatBytes(totals.totalWrittenBytes)}`
        : "等待采集",
    },
    {
      label: "主要磁盘",
      value:
        stringValue(busiestDisk?.mount_point) ??
        stringValue(busiestDisk?.name) ??
        (metric && disks.length === 0 ? "暂无磁盘 IO 数据" : "等待采集"),
    },
  ];
}

export function SystemPage({ hosts = [], metric, apiError }: SystemPageProps) {
  const selectedHost =
    hosts.find((host) => host.id === metric?.host_id) ?? hosts[0];
  const liveMetrics = systemMetrics(metric);
  const detailMetrics = liveTrafficMetrics(metric);
  const trafficMetrics = networkMetrics(metric);
  const diskMetrics = diskIoMetrics(metric);

  return (
    <PageContainer
      aside={
        <PageSection title="主机信息" description="当前节点基础信息">
          <div className="space-y-3">
            {selectedHost ? (
              <div className="rounded-lg border p-3">
                <p className="text-xs text-muted-foreground">当前主机</p>
                <p className="mt-1 text-sm font-semibold">
                  {selectedHost.hostname}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">{selectedHost.status}</p>
              </div>
            ) : null}
            {systemInfo.map((item) => (
              <div key={item.label} className="rounded-lg border p-3">
                <p className="text-xs text-muted-foreground">{item.label}</p>
                <p className="mt-1 text-sm font-semibold">{item.value}</p>
                <p className="mt-1 text-xs text-muted-foreground">{item.helper}</p>
              </div>
            ))}
          </div>
        </PageSection>
      }
    >
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <PageSection
        title="资源概览"
        description="来自 Agent 单向上报的主机核心资源指标。"
        toolbar={
          <Badge variant="outline">{metric ? "已采集" : "等待数据"}</Badge>
        }
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {liveMetrics.map((metric, index) => {
            const Icon = icons[index % icons.length];

            return (
              <div key={metric.label} className="rounded-lg border p-4">
                <div className="mb-4 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Icon
                      className="size-4 text-muted-foreground"
                      aria-hidden="true"
                    />
                    <span className="text-sm font-medium">{metric.label}</span>
                  </div>
                  <span className="text-sm font-semibold">{metric.value}</span>
                </div>
                <Progress value={metric.progress} />
                <p className="mt-3 text-xs text-muted-foreground">
                  {metric.detail}
                </p>
              </div>
            );
          })}
        </div>
      </PageSection>

      <PageSection title="采集状态" description="最新快照的主机和时间信息。">
        <MetricGrid metrics={detailMetrics} />
        <div className="mt-6">
          <TrendPreview label="采集趋势" />
        </div>
      </PageSection>

      <PageSection title="网络 IO" description="按 Agent 最新快照汇总的接口吞吐。">
        <MetricGrid metrics={trafficMetrics} />
        <div className="mt-6">
          <TrendPreview label="网络吞吐趋势" />
        </div>
      </PageSection>

      <PageSection title="磁盘 IO" description="按 Agent 最新快照汇总的磁盘读写。">
        <MetricGrid metrics={diskMetrics} />
        <div className="mt-6">
          <TrendPreview label="磁盘读写趋势" />
        </div>
      </PageSection>
    </PageContainer>
  );
}
