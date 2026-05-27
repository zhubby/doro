"use client";

import { AlertTriangle, CircleGauge, HardDrive, Network, NotebookPen } from "lucide-react";

import { MetricGrid } from "@/components/dashboard/overview/metric-grid";
import { TrendPreview } from "@/components/dashboard/overview/trend-preview";
import { ContainerList } from "@/components/dashboard/overview/container-list";
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
import {
  containers,
  diskMetrics,
  notes,
  systemStats,
  trafficMetrics,
} from "@/lib/mock-data";
import type { AppSummary, ApprovalRequest, Host, Task } from "@/types/api";

type OverviewPageProps = {
  hosts?: Host[];
  tasks?: Task[];
  approvals?: ApprovalRequest[];
  apps?: AppSummary[];
  apiError?: string | null;
};

export function OverviewPage({
  hosts = [],
  tasks = [],
  approvals = [],
  apps = [],
  apiError,
}: OverviewPageProps) {
  const waitingApprovals = approvals.filter(
    (approval) => approval.status === "pending",
  ).length;
  const onlineHosts = hosts.filter((host) => host.status === "online").length;
  const runningContainers = containers.filter(
    (container) => container.status === "running",
  ).length;
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
      label: "容器",
      value: String(containers.length),
      helper: `${runningContainers} 个运行中`,
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
                <Badge variant="outline">运行正常</Badge>
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

        <Card className="xl:col-start-1 xl:row-start-2">
          <CardHeader>
            <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
              <div>
                <CardTitle>监控</CardTitle>
                <CardDescription>展示流量和磁盘 IO 趋势</CardDescription>
              </div>
              <Badge variant="secondary">近 1 小时</Badge>
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
                <TrendPreview label="网络吞吐趋势" />
              </TabsContent>
              <TabsContent value="disk" className="space-y-6">
                <MetricGrid metrics={diskMetrics} />
                <TrendPreview label="磁盘读写趋势" />
              </TabsContent>
            </Tabs>
          </CardContent>
        </Card>

        <ContainerList
          containers={containers}
          className="h-full xl:col-start-2 xl:row-start-2"
        />
      </div>
    </PageContainer>
  );
}
