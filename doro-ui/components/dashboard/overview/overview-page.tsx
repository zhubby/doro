"use client";

import { CircleGauge, HardDrive, Network, NotebookPen } from "lucide-react";

import { MetricGrid } from "@/components/dashboard/overview/metric-grid";
import { TrendPreview } from "@/components/dashboard/overview/trend-preview";
import { ApplicationList } from "@/components/dashboard/apps/application-list";
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
  applications,
  diskMetrics,
  notes,
  overviewStats,
  systemStats,
  trafficMetrics,
} from "@/lib/mock-data";

export function OverviewPage() {
  return (
    <PageContainer
      aside={
        <>
          <Card>
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

          <ApplicationList
            title="应用"
            description="常用服务与安装状态"
            applications={applications}
            compact
          />
        </>
      }
    >
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

      <Card>
        <CardHeader>
          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div>
              <CardTitle>监控</CardTitle>
              <CardDescription>使用 mock 数据展示流量和磁盘 IO 趋势</CardDescription>
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
    </PageContainer>
  );
}
