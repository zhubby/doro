"use client";

import { Filter, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";

import { ResourceStatusBadge, TruncatedText } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Host, HostContainer } from "@/types/api";
import type { ContainerResource, ResourceColumn, ResourceStatus } from "@/types/dashboard";

const columns: ResourceColumn<ContainerResource>[] = [
  {
    key: "name",
    label: "名称",
    width: "34%",
    render: (row) => (
      <div className="min-w-0">
        <p className="truncate font-medium" title={row.name}>
          {row.name}
        </p>
        <p className="truncate text-xs text-muted-foreground" title={row.id}>
          {row.id}
        </p>
      </div>
    ),
  },
  {
    key: "image",
    label: "镜像",
    width: "30%",
    render: (row) => <TruncatedText value={row.image} />,
  },
  {
    key: "agentName",
    label: "Agent",
    width: "18%",
    render: (row) => <TruncatedText value={row.agentName} />,
  },
  {
    key: "status",
    label: "状态",
    width: "6rem",
    render: (row) => <ResourceStatusBadge status={row.status} />,
  },
  {
    key: "updatedAt",
    label: "更新时间",
    width: "9rem",
    render: (row) => <TruncatedText value={row.updatedAt} />,
  },
];

type ContainersPageProps = {
  hosts?: Host[];
  containers?: HostContainer[];
  apiError?: string | null;
};

const statusLabels: Record<ResourceStatus | "all", string> = {
  all: "全部状态",
  running: "运行中",
  warning: "需关注",
  stopped: "已停止",
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

function toContainerResource(
  container: HostContainer,
  hostNames: Map<string, string>,
): ContainerResource {
  return {
    id: container.container_ref,
    hostId: container.host_id,
    agentName: hostNames.get(container.host_id) ?? container.host_id,
    name: container.name,
    image: container.image,
    status: resourceStatus(container.status),
    source: container.runtime,
    cpu: "-",
    memory: "-",
    ports: formatPorts(container.ports),
    updatedAt: new Date(container.created_at ?? container.observed_at).toLocaleString("zh-CN"),
  };
}

export function ContainersPage({
  hosts = [],
  containers = [],
  apiError,
}: ContainersPageProps) {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<ResourceStatus | "all">("all");
  const [agentName, setAgentName] = useState("all");
  const hostNames = useMemo(
    () => new Map(hosts.map((host) => [host.id, host.hostname])),
    [hosts],
  );
  const agentOptions = useMemo(
    () =>
      Array.from(new Set(rowsWithHostNames(containers, hostNames).map((row) => row.agentName)))
        .filter(Boolean)
        .sort((left, right) => left.localeCompare(right, "zh-CN")),
    [containers, hostNames],
  );
  const rows = useMemo(
    () => rowsWithHostNames(containers, hostNames),
    [containers, hostNames],
  );
  const filteredRows = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();

    return rows.filter((row) => {
      const matchesStatus = status === "all" || row.status === status;
      const matchesAgent = agentName === "all" || row.agentName === agentName;
      const matchesQuery =
        normalizedQuery.length === 0 ||
        row.name.toLowerCase().includes(normalizedQuery) ||
        row.id.toLowerCase().includes(normalizedQuery);

      return matchesStatus && matchesAgent && matchesQuery;
    });
  }, [agentName, query, rows, status]);
  const refresh = () => {
    window.location.reload();
  };

  return (
    <ResourceListPage
      title="容器"
      description="来自 Agent 单向采集的容器运行状态。"
      rows={rows}
      filteredRows={filteredRows}
      columns={columns}
      rowActions={[]}
      showStatusChips={false}
      toolbarRight={
        <div className="flex w-full flex-col gap-2 sm:w-auto sm:flex-row">
          <label className="relative min-w-0 sm:w-64">
            <Search
              className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <span className="sr-only">搜索容器</span>
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索名称或 Hash"
              className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
            />
          </label>
          <Button variant="outline" size="icon" aria-label="刷新" onClick={refresh}>
            <RefreshCw className="size-4" aria-hidden="true" />
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="justify-start">
                <Filter className="size-4" aria-hidden="true" />
                {statusLabels[status]}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuLabel>状态筛选</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuRadioGroup
                value={status}
                onValueChange={(value) => setStatus(value as ResourceStatus | "all")}
              >
                {(["all", "running", "warning", "stopped"] as const).map((value) => (
                  <DropdownMenuRadioItem key={value} value={value}>
                    {statusLabels[value]}
                  </DropdownMenuRadioItem>
                ))}
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="justify-start">
                <Filter className="size-4" aria-hidden="true" />
                {agentName === "all" ? "全部 Agent" : agentName}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuLabel>Agent 筛选</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuRadioGroup value={agentName} onValueChange={setAgentName}>
                <DropdownMenuRadioItem value="all">全部 Agent</DropdownMenuRadioItem>
                {agentOptions.map((name) => (
                  <DropdownMenuRadioItem key={name} value={name}>
                    {name}
                  </DropdownMenuRadioItem>
                ))}
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      }
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

function rowsWithHostNames(
  containers: HostContainer[],
  hostNames: Map<string, string>,
) {
  return containers.map((container) => toContainerResource(container, hostNames));
}
