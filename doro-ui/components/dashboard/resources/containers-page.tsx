"use client";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import { containers } from "@/lib/mock-data";
import type { ContainerResource, ResourceColumn } from "@/types/dashboard";

const columns: ResourceColumn<ContainerResource>[] = [
  {
    key: "name",
    label: "名称",
    render: (row) => (
      <div>
        <p className="font-medium">{row.name}</p>
        <p className="text-xs text-muted-foreground">{row.id}</p>
      </div>
    ),
  },
  { key: "image", label: "镜像" },
  {
    key: "status",
    label: "状态",
    render: (row) => <ResourceStatusBadge status={row.status} />,
  },
  { key: "source", label: "来源" },
  {
    key: "resource",
    label: "资源",
    render: (row) => (
      <div className="space-y-1 text-xs">
        <p>CPU: {row.cpu}</p>
        <p>内存: {row.memory}</p>
      </div>
    ),
  },
  { key: "ports", label: "端口" },
  { key: "updatedAt", label: "更新时间" },
];

export function ContainersPage() {
  return (
    <ResourceListPage
      title="容器"
      description="复刻 1Panel 容器列表的筛选、批量操作、搜索与列设置入口。"
      rows={containers}
      columns={columns}
      createLabel="创建容器"
      importLabel="导入"
      batchActions={["启动", "停止", "重启", "删除"]}
    />
  );
}
