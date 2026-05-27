"use client";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import { cronJobs } from "@/lib/mock-data";
import type { CronJob, ResourceColumn } from "@/types/dashboard";

const columns: ResourceColumn<CronJob>[] = [
  {
    key: "name",
    label: "任务名称",
    render: (row) => (
      <div>
        <p className="font-medium">{row.name}</p>
        <p className="text-xs text-muted-foreground">{row.type}</p>
      </div>
    ),
  },
  { key: "schedule", label: "执行周期" },
  {
    key: "status",
    label: "状态",
    render: (row) => <ResourceStatusBadge status={row.status} />,
  },
  { key: "lastRun", label: "最近执行" },
  { key: "retention", label: "保留策略" },
];

export function CronPage() {
  return (
    <ResourceListPage
      title="计划任务"
      description="展示备份、巡检、清理等自动化任务，沿用 1Panel 的任务列表模式。"
      rows={cronJobs}
      columns={columns}
      createLabel="创建任务"
      batchActions={["执行", "启用", "停用", "删除"]}
    />
  );
}
