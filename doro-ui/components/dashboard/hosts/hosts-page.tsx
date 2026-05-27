import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import type { Host } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";

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

const hostColumns: ResourceColumn<Host>[] = [
  {
    key: "hostname",
    label: "主机",
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
    render: (host) => `${host.capabilities.length} 项`,
  },
  {
    key: "labels",
    label: "标签",
    render: (host) => host.labels.join(" / ") || "-",
  },
  {
    key: "last_seen_at",
    label: "最后心跳",
    render: (host) => host.last_seen_at ?? "-",
  },
];

export function HostsPage({ hosts, apiError }: HostsPageProps) {
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
      >
        <DataTable
          columns={hostColumns}
          rows={hosts}
          actions={[]}
          emptyText="暂无主机"
        />
      </PageSection>
    </PageContainer>
  );
}

