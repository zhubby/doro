import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import type { ApprovalRequest } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";

type ApprovalsPageProps = {
  approvals: ApprovalRequest[];
  apiError?: string | null;
};

function approvalStatusLabel(status: ApprovalRequest["status"]) {
  if (status === "pending") {
    return <Badge>待审批</Badge>;
  }

  if (status === "approved") {
    return <Badge variant="secondary">已批准</Badge>;
  }

  if (status === "denied") {
    return <Badge variant="outline">已拒绝</Badge>;
  }

  return <Badge variant="outline">已过期</Badge>;
}

const approvalColumns: ResourceColumn<ApprovalRequest>[] = [
  {
    key: "reason",
    label: "原因",
    render: (approval) => (
      <div>
        <p className="font-medium">{approval.reason}</p>
        <p className="text-xs text-muted-foreground">{approval.id}</p>
      </div>
    ),
  },
  {
    key: "status",
    label: "状态",
    render: (approval) => approvalStatusLabel(approval.status),
  },
  {
    key: "task_id",
    label: "任务",
  },
  {
    key: "step_id",
    label: "步骤",
  },
  {
    key: "requested_at",
    label: "请求时间",
  },
];

export function ApprovalsPage({ approvals, apiError }: ApprovalsPageProps) {
  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <PageSection>
        <DataTable
          columns={approvalColumns}
          rows={approvals}
          actions={[]}
          emptyText="暂无审批请求"
        />
      </PageSection>
    </PageContainer>
  );
}
