import {
  Activity,
  Cpu,
  Gauge,
  HardDrive,
  MemoryStick,
  Network,
  Server,
  Thermometer,
} from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { MetricGrid } from "@/components/dashboard/overview/metric-grid";
import {
  TrendPreview,
  type TrendPoint,
} from "@/components/dashboard/overview/trend-preview";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { Select } from "@/components/ui/select";
import { ToastMessage } from "@/components/ui/use-toast-message";
import { formatRelativeTime } from "@/lib/datetime";
import type { AgentCapability, Host, MetricSnapshot } from "@/types/api";
import type { Metric, SystemMetric } from "@/types/dashboard";

const icons = [Cpu, MemoryStick, HardDrive, Gauge];

type JsonObject = Record<string, unknown>;

type NetworkTotals = {
  receivedBytesPerSecond: number;
  transmittedBytesPerSecond: number;
  totalReceivedBytes: number;
  totalTransmittedBytes: number;
};

type DiskIoTotals = {
  readBytesPerSecond: number;
  writeBytesPerSecond: number;
  totalReadBytes: number;
  totalWrittenBytes: number;
};

type SystemPageProps = {
  hosts?: Host[];
  selectedHostId?: string;
  metricHistory?: MetricSnapshot[];
  apiError?: string | null;
  onSelectedHostChange?: (hostId: string) => void;
};

function formatPercent(value?: number | null) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return "-";
  }
  return `${value.toFixed(1)}%`;
}

function formatLoad(value?: number | null) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return "-";
  }
  return value.toFixed(2);
}

function objectValue(value: unknown) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as JsonObject;
}

function numberValue(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function stringValue(value: unknown) {
  return typeof value === "string" && value.trim() ? value : null;
}

function arrayObjects(value: unknown) {
  return Array.isArray(value)
    ? value
        .map((item) => objectValue(item))
        .filter((item): item is JsonObject => Boolean(item))
    : [];
}

function formatBytes(bytes?: number | null) {
  if (typeof bytes !== "number" || !Number.isFinite(bytes)) {
    return "-";
  }
  if (bytes < 1024) {
    return `${bytes.toFixed(0)} B`;
  }
  const units = ["KB", "MB", "GB", "TB", "PB"];
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

function formatDuration(seconds?: number | null) {
  if (typeof seconds !== "number" || !Number.isFinite(seconds) || seconds < 0) {
    return null;
  }
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  if (days > 0) {
    return `${days} 天 ${hours} 小时`;
  }
  if (hours > 0) {
    return `${hours} 小时 ${minutes} 分钟`;
  }
  return `${Math.max(1, minutes)} 分钟`;
}

function metricProgress(value?: number | null) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return 0;
  }
  return Math.min(100, Math.max(0, value));
}

function hostLabel(host: Host) {
  return host.display_name || host.hostname;
}

function hostStatusLabel(status?: Host["status"]) {
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

function capabilityLabel(capability: AgentCapability) {
  return capability.name.replaceAll("_", " ");
}

function latestMetric(history: MetricSnapshot[]) {
  return history.at(-1) ?? null;
}

function metricExtra(metric?: MetricSnapshot | null) {
  return objectValue(metric?.extra);
}

function hostProfile(host?: Host | null) {
  return objectValue(host?.system_profile);
}

function systemMetrics(
  metric: MetricSnapshot | null,
  selectedHost: Host | null,
): SystemMetric[] {
  const profile = hostProfile(selectedHost);
  const memoryTotal = numberValue(objectValue(profile?.memory)?.total_bytes);
  const disks = arrayObjects(metricExtra(metric)?.disks);
  const diskTotal = disks.reduce(
    (total, disk) => total + (numberValue(disk.total_bytes) ?? 0),
    0,
  );

  return [
    {
      label: "CPU",
      value: formatPercent(metric?.cpu_percent),
      progress: metricProgress(metric?.cpu_percent),
      detail: selectedHost
        ? `${numberValue(profile?.physical_core_count) ?? "-"}C / ${
            numberValue(profile?.logical_core_count) ?? "-"
          }T`
        : "未选择 Agent",
    },
    {
      label: "内存",
      value: formatPercent(metric?.memory_percent),
      progress: metricProgress(metric?.memory_percent),
      detail: memoryTotal ? `总量 ${formatBytes(memoryTotal)}` : "等待系统 profile",
    },
    {
      label: "磁盘",
      value: formatPercent(metric?.disk_percent),
      progress: metricProgress(metric?.disk_percent),
      detail: diskTotal ? `总量 ${formatBytes(diskTotal)}` : "等待磁盘采集",
    },
    {
      label: "负载",
      value: formatLoad(metric?.load_average),
      progress: metricProgress((metric?.load_average ?? 0) * 20),
      detail: metric ? "1 分钟 load average" : "等待 Agent 上报",
    },
  ];
}

function collectionMetrics(
  metric: MetricSnapshot | null,
  selectedHost: Host | null,
): Metric[] {
  const profile = hostProfile(selectedHost);
  return [
    {
      label: "采集主机",
      value: selectedHost ? hostLabel(selectedHost) : "-",
    },
    {
      label: "最新采集",
      value: formatRelativeTime(metric?.captured_at),
    },
    {
      label: "最后心跳",
      value: formatRelativeTime(selectedHost?.last_seen_at),
    },
    {
      label: "运行时间",
      value:
        formatDuration(numberValue(profile?.uptime_seconds)) ??
        formatRelativeTime(stringValue(profile?.booted_at), { emptyText: "-" }),
    },
  ];
}

function networkTotals(metric: MetricSnapshot | null) {
  const networks = arrayObjects(metricExtra(metric)?.networks);
  const totals = networks.reduce<NetworkTotals>(
    (current, network) => ({
      receivedBytesPerSecond:
        current.receivedBytesPerSecond +
        (numberValue(network.received_bytes_per_second) ?? 0),
      transmittedBytesPerSecond:
        current.transmittedBytesPerSecond +
        (numberValue(network.transmitted_bytes_per_second) ?? 0),
      totalReceivedBytes:
        current.totalReceivedBytes +
        (numberValue(network.total_received_bytes) ?? 0),
      totalTransmittedBytes:
        current.totalTransmittedBytes +
        (numberValue(network.total_transmitted_bytes) ?? 0),
    }),
    {
      receivedBytesPerSecond: 0,
      transmittedBytesPerSecond: 0,
      totalReceivedBytes: 0,
      totalTransmittedBytes: 0,
    },
  );
  const activeInterface = [...networks].sort(
    (left, right) =>
      (numberValue(right.received_bytes_per_second) ?? 0) +
      (numberValue(right.transmitted_bytes_per_second) ?? 0) -
      ((numberValue(left.received_bytes_per_second) ?? 0) +
        (numberValue(left.transmitted_bytes_per_second) ?? 0)),
  )[0];

  return { networks, totals, activeInterface };
}

function networkMetrics(metric: MetricSnapshot | null): Metric[] {
  const { networks, totals, activeInterface } = networkTotals(metric);
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

function diskIoTotals(metric: MetricSnapshot | null) {
  const disks = arrayObjects(metricExtra(metric)?.disk_io);
  const totals = disks.reduce<DiskIoTotals>(
    (current, disk) => ({
      readBytesPerSecond:
        current.readBytesPerSecond +
        (numberValue(disk.read_bytes_per_second) ?? 0),
      writeBytesPerSecond:
        current.writeBytesPerSecond +
        (numberValue(disk.write_bytes_per_second) ?? 0),
      totalReadBytes:
        current.totalReadBytes + (numberValue(disk.total_read_bytes) ?? 0),
      totalWrittenBytes:
        current.totalWrittenBytes +
        (numberValue(disk.total_written_bytes) ?? 0),
    }),
    {
      readBytesPerSecond: 0,
      writeBytesPerSecond: 0,
      totalReadBytes: 0,
      totalWrittenBytes: 0,
    },
  );
  const busiestDisk = [...disks].sort(
    (left, right) =>
      (numberValue(right.read_bytes_per_second) ?? 0) +
      (numberValue(right.write_bytes_per_second) ?? 0) -
      ((numberValue(left.read_bytes_per_second) ?? 0) +
        (numberValue(left.write_bytes_per_second) ?? 0)),
  )[0];

  return { disks, totals, busiestDisk };
}

function diskIoMetrics(metric: MetricSnapshot | null): Metric[] {
  const { disks, totals, busiestDisk } = diskIoTotals(metric);
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

function resourceTrendPoints(history: MetricSnapshot[]): TrendPoint[] {
  return history.map((snapshot) => ({
    primary: snapshot.cpu_percent,
    secondary: snapshot.memory_percent,
  }));
}

function networkTrendPoints(history: MetricSnapshot[]): TrendPoint[] {
  return history.map((snapshot) => {
    const { totals } = networkTotals(snapshot);
    return {
      primary: totals.transmittedBytesPerSecond,
      secondary: totals.receivedBytesPerSecond,
    };
  });
}

function diskTrendPoints(history: MetricSnapshot[]): TrendPoint[] {
  return history.map((snapshot) => {
    const { totals } = diskIoTotals(snapshot);
    return {
      primary: totals.readBytesPerSecond,
      secondary: totals.writeBytesPerSecond,
    };
  });
}

function systemInfoMetrics(selectedHost: Host | null): Metric[] {
  const profile = hostProfile(selectedHost);
  const os =
    stringValue(profile?.long_os_version) ??
    stringValue(profile?.os_name) ??
    stringValue(profile?.kernel_version);
  const arch = stringValue(profile?.cpu_arch);
  const memoryTotal = numberValue(objectValue(profile?.memory)?.total_bytes);
  return [
    { label: "主机名", value: selectedHost?.hostname ?? "-" },
    { label: "系统", value: os ?? "等待系统 profile" },
    { label: "内核", value: stringValue(profile?.kernel_version) ?? "-" },
    { label: "架构", value: arch ?? "-" },
    {
      label: "CPU",
      value: `${numberValue(profile?.physical_core_count) ?? "-"}C / ${
        numberValue(profile?.logical_core_count) ?? "-"
      }T`,
    },
    { label: "内存", value: memoryTotal ? formatBytes(memoryTotal) : "-" },
  ];
}

function CpuDetails({ metric }: { metric: MetricSnapshot | null }) {
  const cpus = arrayObjects(metricExtra(metric)?.cpus);
  if (cpus.length === 0) {
    return <EmptyState text="等待 CPU 核心采集" />;
  }

  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {cpus.slice(0, 12).map((cpu, index) => {
        const usage = numberValue(cpu.usage_percent);
        return (
          <div key={`${stringValue(cpu.name) ?? "cpu"}-${index}`} className="rounded-lg border p-3">
            <div className="flex items-center justify-between gap-3">
              <p className="truncate text-sm font-medium">
                {stringValue(cpu.name) ?? `CPU ${index + 1}`}
              </p>
              <span className="text-xs tabular-nums text-muted-foreground">
                {formatPercent(usage)}
              </span>
            </div>
            <Progress value={metricProgress(usage)} className="mt-3" />
            <p className="mt-2 text-xs text-muted-foreground">
              {numberValue(cpu.frequency_mhz)
                ? `${numberValue(cpu.frequency_mhz)} MHz`
                : "频率未知"}
            </p>
          </div>
        );
      })}
    </div>
  );
}

function DiskDetails({ metric }: { metric: MetricSnapshot | null }) {
  const disks = arrayObjects(metricExtra(metric)?.disks);
  if (disks.length === 0) {
    return <EmptyState text="等待磁盘容量采集" />;
  }

  return (
    <div className="grid gap-3 md:grid-cols-2">
      {disks.map((disk, index) => {
        const total = numberValue(disk.total_bytes);
        const used = numberValue(disk.used_bytes);
        const progress = total && used ? (used / total) * 100 : 0;
        return (
          <div
            key={`${stringValue(disk.mount_point) ?? stringValue(disk.name) ?? "disk"}-${index}`}
            className="rounded-lg border p-3"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">
                  {stringValue(disk.mount_point) ?? stringValue(disk.name) ?? "磁盘"}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {[stringValue(disk.name), stringValue(disk.kind)]
                    .filter(Boolean)
                    .join(" · ") || "磁盘详情"}
                </p>
              </div>
              <span className="text-xs tabular-nums text-muted-foreground">
                {formatPercent(progress)}
              </span>
            </div>
            <Progress value={metricProgress(progress)} className="mt-3" />
            <p className="mt-2 text-xs text-muted-foreground">
              {formatBytes(used)} / {formatBytes(total)}
            </p>
          </div>
        );
      })}
    </div>
  );
}

function ComponentDetails({ metric }: { metric: MetricSnapshot | null }) {
  const components = arrayObjects(metricExtra(metric)?.components).filter(
    (component) => numberValue(component.temperature_celsius) !== null,
  );
  if (components.length === 0) {
    return <EmptyState text="暂无温度组件数据" />;
  }

  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {components.slice(0, 12).map((component, index) => (
        <DetailTile
          key={`${stringValue(component.label) ?? "component"}-${index}`}
          label={stringValue(component.label) ?? "组件"}
          value={`${numberValue(component.temperature_celsius)?.toFixed(1)} °C`}
          helper="温度"
        />
      ))}
    </div>
  );
}

function GpuDetails({ metric }: { metric: MetricSnapshot | null }) {
  const gpus = arrayObjects(metricExtra(metric)?.gpus);
  if (gpus.length === 0) {
    return <EmptyState text="GPU 采集未启用或当前 Agent 未上报" />;
  }

  return (
    <div className="grid gap-3 md:grid-cols-2">
      {gpus.map((gpu, index) => {
        const utilization = numberValue(gpu.utilization_gpu);
        const used = numberValue(gpu.memory_used);
        const total = numberValue(gpu.memory_total);
        return (
          <div key={`${stringValue(gpu.name) ?? "gpu"}-${index}`} className="rounded-lg border p-3">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">
                  {stringValue(gpu.name) ?? `GPU ${index + 1}`}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {stringValue(gpu.pci_bus_id) ?? "NVIDIA / NVML"}
                </p>
              </div>
              <span className="text-xs tabular-nums text-muted-foreground">
                {formatPercent(utilization)}
              </span>
            </div>
            <Progress value={metricProgress(utilization)} className="mt-3" />
            <p className="mt-2 text-xs text-muted-foreground">
              显存 {formatBytes(used)} / {formatBytes(total)}
              {numberValue(gpu.temperature)
                ? ` · ${numberValue(gpu.temperature)?.toFixed(0)} °C`
                : ""}
            </p>
          </div>
        );
      })}
    </div>
  );
}

function ProcessDetails({ metric }: { metric: MetricSnapshot | null }) {
  const processes = arrayObjects(metricExtra(metric)?.processes);
  if (processes.length === 0) {
    return <EmptyState text="未配置进程监控或暂无匹配进程" />;
  }

  return (
    <div className="overflow-hidden rounded-lg border">
      <div className="grid grid-cols-[1fr_5rem_6rem_6rem] gap-3 border-b bg-muted/40 px-3 py-2 text-xs font-medium text-muted-foreground">
        <span>进程</span>
        <span className="text-right">CPU</span>
        <span className="text-right">内存</span>
        <span className="text-right">PID</span>
      </div>
      {processes.slice(0, 8).map((process, index) => (
        <div
          key={`${numberValue(process.pid) ?? stringValue(process.name) ?? "process"}-${index}`}
          className="grid grid-cols-[1fr_5rem_6rem_6rem] gap-3 border-b px-3 py-2 text-sm last:border-b-0"
        >
          <span className="min-w-0 truncate">{stringValue(process.name) ?? "-"}</span>
          <span className="text-right tabular-nums">
            {formatPercent(numberValue(process.cpu_percent))}
          </span>
          <span className="text-right tabular-nums">
            {formatBytes(numberValue(process.memory_bytes))}
          </span>
          <span className="text-right tabular-nums text-muted-foreground">
            {numberValue(process.pid) ?? "-"}
          </span>
        </div>
      ))}
    </div>
  );
}

function DetailTile({
  label,
  value,
  helper,
}: {
  label: string;
  value: string;
  helper?: string;
}) {
  return (
    <div className="rounded-lg border p-3">
      <p className="truncate text-xs text-muted-foreground">{label}</p>
      <p className="mt-1 truncate text-sm font-semibold">{value}</p>
      {helper ? <p className="mt-1 truncate text-xs text-muted-foreground">{helper}</p> : null}
    </div>
  );
}

function CompactMetricGrid({ metrics }: { metrics: Metric[] }) {
  return (
    <div className="grid gap-3 sm:grid-cols-2">
      {metrics.map((metric) => (
        <DetailTile key={metric.label} label={metric.label} value={metric.value} />
      ))}
    </div>
  );
}

function EmptyState({ text }: { text: string }) {
  return (
    <div className="flex min-h-24 items-center justify-center rounded-lg border border-dashed bg-muted/30 px-4 text-center text-sm text-muted-foreground">
      {text}
    </div>
  );
}

export function SystemPage({
  hosts = [],
  selectedHostId = "",
  metricHistory = [],
  apiError,
  onSelectedHostChange,
}: SystemPageProps) {
  const selectedHost =
    hosts.find((host) => host.id === selectedHostId) ??
    hosts.find((host) => host.status === "online") ??
    hosts[0] ??
    null;
  const selectedMetric = latestMetric(metricHistory);
  const liveMetrics = systemMetrics(selectedMetric, selectedHost);
  const detailMetrics = collectionMetrics(selectedMetric, selectedHost);
  const trafficMetrics = networkMetrics(selectedMetric);
  const diskMetrics = diskIoMetrics(selectedMetric);
  const selectedValue = selectedHost?.id ?? "";
  const profileMetrics = systemInfoMetrics(selectedHost);
  const profile = hostProfile(selectedHost);

  return (
    <PageContainer
      aside={
        <>
          <PageSection title="选择 Agent" description="切换系统指标采集来源">
            <div className="space-y-3">
              <Select
                value={selectedValue}
                onValueChange={(value) => onSelectedHostChange?.(value)}
                disabled={hosts.length === 0}
                placeholder={hosts.length === 0 ? "暂无 Agent" : "选择 Agent"}
                options={hosts.map((host) => ({
                  value: host.id,
                  label: (
                    <span className="flex min-w-0 items-center justify-between gap-3">
                      <span className="min-w-0 truncate">{hostLabel(host)}</span>
                      <span className="shrink-0 text-xs text-muted-foreground">
                        {host.status}
                      </span>
                    </span>
                  ),
                }))}
              />
              {selectedHost ? (
                <div className="rounded-lg border p-3">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <p className="truncate text-sm font-semibold">
                        {hostLabel(selectedHost)}
                      </p>
                      <p className="mt-1 truncate text-xs text-muted-foreground">
                        {selectedHost.hostname}
                      </p>
                    </div>
                    {hostStatusLabel(selectedHost.status)}
                  </div>
                  <div className="mt-3 flex flex-wrap gap-1.5">
                    {selectedHost.capabilities.slice(0, 6).map((capability) => (
                      <Badge
                        key={capability.name}
                        variant="outline"
                        className="max-w-32 truncate font-normal"
                      >
                        {capabilityLabel(capability)}
                      </Badge>
                    ))}
                  </div>
                </div>
              ) : (
                <EmptyState text="暂无可选 Agent" />
              )}
            </div>
          </PageSection>

          <PageSection title="主机信息" description="当前节点基础信息">
            <CompactMetricGrid metrics={profileMetrics} />
            <div className="mt-3 rounded-lg border p-3">
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <Server className="size-4" aria-hidden="true" />
                <span>Profile 来源</span>
              </div>
              <p className="mt-2 text-sm font-semibold">
                {profile ? "Agent enrollment / heartbeat" : "等待 Agent 上报"}
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                {stringValue(profile?.booted_at)
                  ? `启动于 ${formatRelativeTime(stringValue(profile?.booted_at))}`
                  : "新版 Agent 心跳会自动刷新系统 profile"}
              </p>
            </div>
          </PageSection>
        </>
      }
    >
      <ToastMessage
        id="system-api-error"
        kind="error"
        message={apiError}
        prefix="控制平面暂不可用："
      />
      <PageSection
        title="资源概览"
        description="来自 Agent 单向上报的主机核心资源指标。"
        toolbar={
          <Badge variant="outline">
            {selectedMetric ? "已采集" : "等待数据"}
          </Badge>
        }
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {liveMetrics.map((metric, index) => {
            const Icon = icons[index % icons.length];

            return (
              <div key={metric.label} className="rounded-lg border p-4">
                <div className="mb-4 flex items-center justify-between gap-3">
                  <div className="flex min-w-0 items-center gap-2">
                    <Icon
                      className="size-4 shrink-0 text-muted-foreground"
                      aria-hidden="true"
                    />
                    <span className="truncate text-sm font-medium">{metric.label}</span>
                  </div>
                  <span className="shrink-0 text-sm font-semibold tabular-nums">
                    {metric.value}
                  </span>
                </div>
                <Progress value={metric.progress} />
                <p className="mt-3 truncate text-xs text-muted-foreground">
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
          <TrendPreview
            label="CPU / 内存趋势"
            points={resourceTrendPoints(metricHistory)}
            seriesLabels={["CPU", "内存"]}
          />
        </div>
      </PageSection>

      <PageSection title="网络 IO" description="按 Agent 最新快照汇总的接口吞吐。">
        <MetricGrid metrics={trafficMetrics} />
        <div className="mt-6">
          <TrendPreview
            label="网络吞吐趋势"
            points={networkTrendPoints(metricHistory)}
            seriesLabels={["上行", "下行"]}
          />
        </div>
      </PageSection>

      <PageSection title="磁盘 IO" description="按 Agent 最新快照汇总的磁盘读写。">
        <MetricGrid metrics={diskMetrics} />
        <div className="mt-6">
          <TrendPreview
            label="磁盘读写趋势"
            points={diskTrendPoints(metricHistory)}
            seriesLabels={["读取", "写入"]}
          />
        </div>
      </PageSection>

      <PageSection title="CPU 核心" description="每个逻辑核心的使用率和频率。">
        <CpuDetails metric={selectedMetric} />
      </PageSection>

      <PageSection title="磁盘容量" description="各挂载点容量和使用率。">
        <DiskDetails metric={selectedMetric} />
      </PageSection>

      <div className="grid gap-6 xl:grid-cols-2">
        <PageSection
          title="温度"
          description="Agent 上报的系统组件温度。"
          toolbar={<Thermometer className="size-4 text-muted-foreground" aria-hidden="true" />}
        >
          <ComponentDetails metric={selectedMetric} />
        </PageSection>
        <PageSection
          title="GPU"
          description="可选 Linux / NVIDIA GPU 指标。"
          toolbar={<Activity className="size-4 text-muted-foreground" aria-hidden="true" />}
        >
          <GpuDetails metric={selectedMetric} />
        </PageSection>
      </div>

      <PageSection
        title="进程监控"
        description="按 Agent 配置的 process_names 采集。"
        toolbar={<Network className="size-4 text-muted-foreground" aria-hidden="true" />}
      >
        <ProcessDetails metric={selectedMetric} />
      </PageSection>
    </PageContainer>
  );
}
