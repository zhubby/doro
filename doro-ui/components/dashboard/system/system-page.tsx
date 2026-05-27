import { Activity, HardDrive, Network, ShieldCheck } from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { MetricGrid } from "@/components/dashboard/overview/metric-grid";
import { TrendPreview } from "@/components/dashboard/overview/trend-preview";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import {
  diskMetrics,
  systemInfo,
  systemMetrics,
  trafficMetrics,
} from "@/lib/mock-data";

const icons = [Activity, HardDrive, Network, ShieldCheck];

export function SystemPage() {
  return (
    <PageContainer
      aside={
        <PageSection title="主机信息" description="当前节点基础信息">
          <div className="space-y-3">
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
      <PageSection
        title="资源概览"
        description="将首页系统状态扩展为更完整的主机资源页面。"
        toolbar={<Badge variant="outline">运行正常</Badge>}
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {systemMetrics.map((metric, index) => {
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

      <PageSection title="网络指标" description="实时吞吐、累计发送和接收数据。">
        <MetricGrid metrics={trafficMetrics} />
        <div className="mt-6">
          <TrendPreview label="网络吞吐趋势" />
        </div>
      </PageSection>

      <PageSection title="磁盘 IO" description="读取、写入、等待时间和使用率。">
        <MetricGrid metrics={diskMetrics} />
        <div className="mt-6">
          <TrendPreview label="磁盘读写趋势" />
        </div>
      </PageSection>
    </PageContainer>
  );
}
