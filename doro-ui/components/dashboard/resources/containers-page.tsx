"use client";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import type { HostContainer } from "@/types/api";
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

type ContainersPageProps = {
  containers?: HostContainer[];
  apiError?: string | null;
};

function resourceStatus(status: string): ContainerResource["status"] {
  if (status === "running") {
    return "running";
  }
  if (status === "created" || status === "restarting" || status === "paused") {
    return "warning";
  }
  return "stopped";
}

function formatPorts(ports: HostContainer["ports"]) {
  if (!Array.isArray(ports) || ports.length === 0) {
    return "-";
  }
  return ports
    .map((port) => {
      if (!port || typeof port !== "object") {
        return null;
      }
      const value = port as Record<string, unknown>;
      const privatePort = value.PrivatePort ?? value.private_port;
      const publicPort = value.PublicPort ?? value.public_port;
      return publicPort
        ? `${publicPort}:${privatePort}`
        : String(privatePort ?? "-");
    })
    .filter(Boolean)
    .join(", ");
}

function toContainerResource(container: HostContainer): ContainerResource {
  return {
    id: container.container_ref,
    name: container.name,
    image: container.image,
    status: resourceStatus(container.status),
    source: container.runtime,
    cpu: "-",
    memory: "-",
    ports: formatPorts(container.ports),
    updatedAt: new Date(container.observed_at).toLocaleString("zh-CN"),
  };
}

export function ContainersPage({
  containers = [],
  apiError,
}: ContainersPageProps) {
  const rows = containers.map(toContainerResource);

  return (
    <ResourceListPage
      title="容器"
      description="来自 Agent 单向采集的容器运行状态。"
      rows={rows}
      columns={columns}
      rowActions={[]}
      notice={
        apiError ? (
          <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
            控制平面暂不可用：{apiError}
          </div>
        ) : null
      }
    />
  );
}
