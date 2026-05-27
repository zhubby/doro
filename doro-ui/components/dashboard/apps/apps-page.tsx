"use client";

import { Settings } from "lucide-react";

import { ApplicationList } from "@/components/dashboard/apps/application-list";
import { PageContainer } from "@/components/layout/page-container";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { applications } from "@/lib/mock-data";
import type { Application } from "@/types/dashboard";

type AppsPageProps = {
  initialApplications?: Application[];
  apiError?: string | null;
};

export function AppsPage({
  initialApplications = applications,
  apiError,
}: AppsPageProps) {
  const applications = initialApplications;
  const upgradeCount = applications.filter((app) => app.updateAvailable).length;

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <Tabs defaultValue="all" className="space-y-4">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <TabsList className="w-fit">
            <TabsTrigger value="all">全部</TabsTrigger>
            <TabsTrigger value="installed">已安装</TabsTrigger>
            <TabsTrigger value="upgrade">可升级 · {upgradeCount}</TabsTrigger>
            <TabsTrigger value="setting">设置</TabsTrigger>
          </TabsList>
          <Button variant="outline">
            <Settings className="size-4" aria-hidden="true" />
            应用源设置
          </Button>
        </div>
        <TabsContent value="all">
          <ApplicationList
            title="全部应用"
            description="来自控制平面的应用目录。"
            applications={applications}
          />
        </TabsContent>
        <TabsContent value="installed">
          <ApplicationList
            title="已安装"
            description="已安装或正在运行的服务应用。"
            applications={applications}
            filter="installed"
          />
        </TabsContent>
        <TabsContent value="upgrade">
          <ApplicationList
            title="可升级"
            description="需要关注版本更新的应用。"
            applications={applications}
            filter="upgrade"
          />
        </TabsContent>
        <TabsContent value="setting">
          <ApplicationList
            title="应用商店设置"
            description="应用源和同步状态入口。"
            applications={applications.slice(0, 2)}
          />
        </TabsContent>
      </Tabs>
    </PageContainer>
  );
}
