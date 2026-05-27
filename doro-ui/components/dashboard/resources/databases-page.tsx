"use client";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import { databases } from "@/lib/mock-data";
import type { DatabaseResource, ResourceColumn } from "@/types/dashboard";

const columns: ResourceColumn<DatabaseResource>[] = [
  {
    key: "name",
    label: "名称",
    render: (row) => (
      <div>
        <p className="font-medium">{row.name}</p>
        <p className="text-xs text-muted-foreground">{row.engine}</p>
      </div>
    ),
  },
  { key: "version", label: "版本" },
  {
    key: "status",
    label: "状态",
    render: (row) => <ResourceStatusBadge status={row.status} />,
  },
  { key: "size", label: "数据量" },
  { key: "backup", label: "最近备份" },
  { key: "updatedAt", label: "更新时间" },
];

export function DatabasesPage() {
  return (
    <ResourceListPage
      title="数据库"
      description="复用标准列表模式展示数据库实例、备份和连接状态。"
      rows={databases}
      columns={columns}
      createLabel="创建数据库"
      batchActions={["备份", "重启", "删除"]}
    />
  );
}
