import { SquareTerminal } from "lucide-react";

import { DataTable, ResourceStatusBadge } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Button } from "@/components/ui/button";
import { terminalSessions } from "@/lib/mock-data";
import type { ResourceColumn, TerminalSession } from "@/types/dashboard";

const columns: ResourceColumn<TerminalSession>[] = [
  {
    key: "name",
    label: "会话",
    render: (row) => (
      <div>
        <p className="font-medium">{row.name}</p>
        <p className="text-xs text-muted-foreground">{row.id}</p>
      </div>
    ),
  },
  { key: "target", label: "目标地址" },
  {
    key: "status",
    label: "状态",
    render: (row) => <ResourceStatusBadge status={row.status} />,
  },
  { key: "user", label: "用户" },
  { key: "lastActive", label: "最近活跃" },
];

export function TerminalPage() {
  return (
    <PageContainer
      aside={
        <PageSection title="连接预览" description="真实终端能力后续接入。">
          <div className="rounded-lg border bg-muted/30 p-4 font-mono text-xs">
            <p className="text-muted-foreground">$ ssh deploy@10.0.0.12</p>
            <p className="mt-3">Last login: Wed May 27 09:32</p>
            <p className="text-muted-foreground">doro-builder % _</p>
          </div>
        </PageSection>
      }
    >
      <PageSection
        title="终端"
        description="展示本地和远程终端入口、连接状态与最近活跃时间。"
        contentClassName="space-y-4"
      >
        <Toolbar
          left={
            <>
              <Button>
                <SquareTerminal className="size-4" aria-hidden="true" />
                新建终端
              </Button>
              <Button variant="outline">导入连接</Button>
            </>
          }
          right={<Button variant="outline">会话设置</Button>}
        />
        <DataTable
          columns={columns}
          rows={terminalSessions}
          actions={["连接", "编辑"]}
        />
      </PageSection>
    </PageContainer>
  );
}
