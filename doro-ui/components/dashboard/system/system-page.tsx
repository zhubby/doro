import { Activity, HardDrive, Network, ShieldCheck } from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { MetricGrid } from "@/components/dashboard/overview/metric-grid";
import { TrendPreview } from "@/components/dashboard/overview/trend-preview";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
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
      value: metric ? new Date(metric.captured_at).toLocaleString("zh-CN") : "-",
    },
  ];
}

export function SystemPage({ hosts = [], metric, apiError }: SystemPageProps) {
  const selectedHost =
    hosts.find((host) => host.id === metric?.host_id) ?? hosts[0];
  const liveMetrics = systemMetrics(metric);
  const detailMetrics = liveTrafficMetrics(metric);

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

      <PageSection title="磁盘 IO" description="细粒度磁盘、网络、进程和 GPU 明细已保存在事件 payload。">
        <MetricGrid metrics={[{ label: "明细来源", value: "agent_events / metric extra" }]} />
        <div className="mt-6">
          <TrendPreview label="磁盘读写趋势" />
        </div>
      </PageSection>
    </PageContainer>
  );
}
