"use client";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import { websites } from "@/lib/mock-data";
import type { ResourceColumn, WebsiteResource } from "@/types/dashboard";

const columns: ResourceColumn<WebsiteResource>[] = [
  {
    key: "primaryDomain",
    label: "主域名",
    render: (row) => (
      <div>
        <p className="font-medium">{row.primaryDomain}</p>
        <p className="text-xs text-muted-foreground">{row.id}</p>
      </div>
    ),
  },
  {
    key: "status",
    label: "状态",
    render: (row) => <ResourceStatusBadge status={row.status} />,
  },
  { key: "runtime", label: "运行环境" },
  { key: "ssl", label: "SSL" },
  { key: "rootPath", label: "站点目录 / 代理目标" },
  { key: "traffic", label: "流量" },
  { key: "updatedAt", label: "更新时间" },
];

export function WebsitesPage() {
  return (
    <ResourceListPage
      title="网站"
      description="复刻 1Panel 网站列表的域名、SSL、运行环境和操作入口。"
      rows={websites}
      columns={columns}
      createLabel="创建网站"
      importLabel="导入配置"
      batchActions={["启动", "停止", "续签证书"]}
    />
  );
}
