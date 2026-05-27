import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import type { Host } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";
import { Activity, Cpu, Plus, Server } from "lucide-react";

type HostsPageProps = {
  hosts: Host[];
  apiError?: string | null;
};

function hostStatusLabel(status: Host["status"]) {
  if (status === "online") {
    return <Badge>在线</Badge>;
  }

  if (status === "degraded") {
    return <Badge variant="secondary">需关注</Badge>;
  }

  if (status === "pending") {
    return <Badge variant="outline">待接入</Badge>;
  }

  return <Badge variant="outline">离线</Badge>;
}

function formatLastSeen(value: string | null) {
  if (!value) {
    return "尚未收到";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}

const hostColumns: ResourceColumn<Host>[] = [
  {
    key: "hostname",
    label: "Agent",
    render: (host) => (
      <div>
        <p className="font-medium">{host.hostname}</p>
        <p className="text-xs text-muted-foreground">{host.id}</p>
      </div>
    ),
  },
  {
    key: "status",
    label: "状态",
    render: (host) => hostStatusLabel(host.status),
  },
  {
    key: "capabilities",
    label: "能力",
    render: (host) => (
      <div className="flex flex-wrap gap-1.5">
        {host.capabilities.length === 0 ? (
          <span className="text-muted-foreground">未声明</span>
        ) : (
          host.capabilities.map((capability) => (
            <Badge
              key={capability.name}
              variant={capability.risk === "high" ? "secondary" : "outline"}
            >
              {capability.name}
            </Badge>
          ))
        )}
      </div>
    ),
  },
  {
    key: "labels",
    label: "标签",
    render: (host) => host.labels.join(" / ") || "-",
  },
  {
    key: "last_seen_at",
    label: "最后心跳",
    render: (host) => formatLastSeen(host.last_seen_at),
  },
];

export function HostsPage({ hosts, apiError }: HostsPageProps) {
  const onlineHosts = hosts.filter((host) => host.status === "online").length;
  const declaredCapabilities = hosts.reduce(
    (total, host) => total + host.capabilities.length,
    0,
  );
  const enrollmentCommands = [
    "doro enrollment-token homelab-node",
    [
      "doro agent",
      "--control-plane-url http://CONTROL_PLANE_HOST:8788",
      "--hostname homelab-node",
      "--enrollment-token PASTE_TOKEN_HERE",
    ].join(" \\\n  "),
  ];

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <PageSection
        title="主机"
        description="来自控制平面的 Agent 注册状态、能力声明和心跳。"
        toolbar={
          <Dialog>
            <DialogTrigger asChild>
              <Button size="sm">
                <Plus className="size-4" />
                New Host
              </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-xl">
              <DialogHeader>
                <DialogTitle>接入新主机</DialogTitle>
                <DialogDescription>
                  在目标主机上安装并运行 Doro Agent，通过命令行参数把一次性
                  enrollment token 传给当前控制平面。
                </DialogDescription>
              </DialogHeader>

              <div className="space-y-4 text-sm">
                <div className="rounded-md border bg-muted/30 p-4">
                  <p className="mb-3 font-medium">1. 在控制平面生成接入令牌</p>
                  <pre className="overflow-x-auto rounded-md bg-background p-3 text-xs text-foreground">
                    <code>{enrollmentCommands[0]}</code>
                  </pre>
                </div>

                <div className="rounded-md border bg-muted/30 p-4">
                  <p className="mb-3 font-medium">
                    2. 在目标主机启动 Agent
                  </p>
                  <pre className="overflow-x-auto rounded-md bg-background p-3 text-xs text-foreground">
                    <code>{enrollmentCommands[1]}</code>
                  </pre>
                </div>

                <div className="rounded-md border bg-muted/30 p-4">
                  <p className="mb-3 font-medium">3. 后续重启可直接使用已写回的配置</p>
                  <pre className="overflow-x-auto rounded-md bg-background p-3 text-xs text-foreground">
                    <code>doro agent --config ~/.doro/config.toml</code>
                  </pre>
                </div>

                <p className="text-xs leading-5 text-muted-foreground">
                  首次连接成功后，Agent 会把 agent_id 和 host_id 写回本机配置；
                  令牌会在控制平面标记为已使用。
                </p>
              </div>
            </DialogContent>
          </Dialog>
        }
      >
        <div className="mb-4 grid gap-3 md:grid-cols-3">
          <Card>
            <CardContent className="flex items-center gap-3 p-4">
              <Server className="size-4 text-muted-foreground" />
              <div>
                <p className="text-xs text-muted-foreground">已注册 Agent</p>
                <p className="text-xl font-semibold">{hosts.length}</p>
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="flex items-center gap-3 p-4">
              <Activity className="size-4 text-muted-foreground" />
              <div>
                <p className="text-xs text-muted-foreground">当前在线</p>
                <p className="text-xl font-semibold">{onlineHosts}</p>
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="flex items-center gap-3 p-4">
              <Cpu className="size-4 text-muted-foreground" />
              <div>
                <p className="text-xs text-muted-foreground">声明能力</p>
                <p className="text-xl font-semibold">{declaredCapabilities}</p>
              </div>
            </CardContent>
          </Card>
        </div>
        <DataTable
          columns={hostColumns}
          rows={hosts}
          actions={[]}
          emptyText="暂无已连接 Agent"
        />
      </PageSection>
    </PageContainer>
  );
}
