import { Bot, Play, Settings2 } from "lucide-react";

import { DataTable, ResourceStatusBadge } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { aiAgents } from "@/lib/mock-data";
import type { AiAgent, ResourceColumn } from "@/types/dashboard";

const columns: ResourceColumn<AiAgent>[] = [
  {
    key: "name",
    label: "智能体",
    render: (row) => (
      <div>
        <p className="font-medium">{row.name}</p>
        <p className="text-xs text-muted-foreground">{row.role}</p>
      </div>
    ),
  },
  {
    key: "status",
    label: "状态",
    render: (row) => <ResourceStatusBadge status={row.status} />,
  },
  { key: "model", label: "模型" },
  { key: "lastRun", label: "最近运行" },
];

export function AiPage() {
  return (
    <PageContainer
      aside={
        <PageSection title="运行配置" description="当前智能体执行环境概览。">
          <div className="space-y-3">
            {["工具调用已开启", "本地任务队列空闲", "模型路由使用默认策略"].map(
              (item) => (
                <div key={item} className="flex items-center justify-between rounded-lg border p-3">
                  <span className="text-sm">{item}</span>
                  <Badge variant="secondary">正常</Badge>
                </div>
              ),
            )}
          </div>
        </PageSection>
      }
    >
      <PageSection contentClassName="space-y-4">
        <Toolbar
          left={
            <>
              <Button>
                <Bot className="size-4" aria-hidden="true" />
                创建智能体
              </Button>
              <Button variant="outline">
                <Play className="size-4" aria-hidden="true" />
                运行任务
              </Button>
            </>
          }
          right={
            <Button variant="outline">
              <Settings2 className="size-4" aria-hidden="true" />
              模型设置
            </Button>
          }
        />
        <DataTable columns={columns} rows={aiAgents} actions={["运行", "配置"]} />
      </PageSection>
    </PageContainer>
  );
}
