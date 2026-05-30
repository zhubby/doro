"use client";

import { Check, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";

import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  approveApproval,
  createApproval,
  deleteApproval,
  denyApproval,
} from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import type { ApprovalRequest } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";

type ApprovalsPageProps = {
  approvals: ApprovalRequest[];
  apiError?: string | null;
  onApprovalCreated?: (approval: ApprovalRequest) => void;
  onApprovalDeleted?: (approvalId: string) => void;
  onApprovalUpdated?: (approval: ApprovalRequest) => void;
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
    width: "28%",
    render: (approval) => (
      <div className="min-w-0">
        <p className="truncate font-medium" title={approval.reason}>
          {approval.reason}
        </p>
        <p className="truncate text-xs text-muted-foreground" title={approval.id}>
          {approval.id}
        </p>
      </div>
    ),
  },
  {
    key: "status",
    label: "状态",
    width: "90px",
    render: (approval) => approvalStatusLabel(approval.status),
  },
  {
    key: "task_id",
    label: "任务",
    width: "18%",
    render: (approval) => (
      <span className="block truncate font-mono text-xs" title={approval.task_id}>
        {approval.task_id}
      </span>
    ),
  },
  {
    key: "step_id",
    label: "步骤",
    width: "18%",
    render: (approval) => (
      <span className="block truncate font-mono text-xs" title={approval.step_id}>
        {approval.step_id}
      </span>
    ),
  },
  {
    key: "requested_at",
    label: "请求时间",
    width: "110px",
    render: (approval) => (
      <span title={approval.requested_at}>
        {formatRelativeTime(approval.requested_at)}
      </span>
    ),
  },
  {
    key: "expires_at",
    label: "有效期",
    width: "110px",
    render: (approval) => (
      <span title={approval.expires_at}>{formatRelativeTime(approval.expires_at)}</span>
    ),
  },
  {
    key: "resolved_by",
    label: "处理",
    width: "130px",
    render: (approval) => (
      <div className="min-w-0 text-xs">
        <p className="truncate" title={approval.resolved_by ?? ""}>
          {approval.resolved_by ?? "-"}
        </p>
        <p className="truncate text-muted-foreground" title={approval.decision_note ?? ""}>
          {approval.decision_note ?? (approval.resolved_at ? formatRelativeTime(approval.resolved_at) : "")}
        </p>
      </div>
    ),
  },
];

export function ApprovalsPage({
  approvals,
  apiError,
  onApprovalCreated,
  onApprovalDeleted,
  onApprovalUpdated,
}: ApprovalsPageProps) {
  const [createOpen, setCreateOpen] = useState(false);
  const [taskId, setTaskId] = useState("");
  const [stepId, setStepId] = useState("");
  const [reason, setReason] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [createPending, setCreatePending] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [actionPending, setActionPending] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  async function handleCreateApproval() {
    setCreatePending(true);
    setCreateError(null);
    const result = await createApproval({
      task_id: taskId.trim(),
      step_id: stepId.trim(),
      reason: reason.trim(),
      expires_at: expiresAt ? new Date(expiresAt).toISOString() : null,
    });
    setCreatePending(false);
    if (!result.data) {
      setCreateError(result.error ?? "创建失败");
      return;
    }

    onApprovalCreated?.(result.data.item);
    setTaskId("");
    setStepId("");
    setReason("");
    setExpiresAt("");
    setCreateOpen(false);
  }

  async function handleApprove(approval: ApprovalRequest) {
    setActionPending(`${approval.id}:approve`);
    setActionError(null);
    const result = await approveApproval(approval.id);
    setActionPending(null);
    if (!result.data) {
      setActionError(result.error ?? "通过失败");
      return;
    }
    onApprovalUpdated?.(result.data.item);
  }

  async function handleDeny(approval: ApprovalRequest) {
    setActionPending(`${approval.id}:deny`);
    setActionError(null);
    const result = await denyApproval(approval.id);
    setActionPending(null);
    if (!result.data) {
      setActionError(result.error ?? "拒绝失败");
      return;
    }
    onApprovalUpdated?.(result.data.item);
  }

  async function handleDelete(approval: ApprovalRequest) {
    setActionPending(`${approval.id}:delete`);
    setActionError(null);
    const result = await deleteApproval(approval.id);
    setActionPending(null);
    if (result.error) {
      setActionError(result.error);
      return;
    }
    onApprovalDeleted?.(approval.id);
  }

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <PageSection>
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="text-base font-semibold">审批请求</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              管理高风险任务步骤的人工审批和有效期。
            </p>
          </div>
          <Dialog open={createOpen} onOpenChange={setCreateOpen}>
            <DialogTrigger asChild>
              <Button size="sm">
                <Plus className="size-4" />
                新建审批
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>新建审批</DialogTitle>
                <DialogDescription>
                  为已有任务步骤创建一个待处理审批，未设置有效期时后端默认 24 小时。
                </DialogDescription>
              </DialogHeader>

              <div className="space-y-3">
                <div className="space-y-2">
                  <label className="text-sm font-medium" htmlFor="approval-task-id">
                    任务 ID
                  </label>
                  <input
                    id="approval-task-id"
                    value={taskId}
                    disabled={createPending}
                    onChange={(event) => setTaskId(event.target.value)}
                    className="h-9 w-full rounded-md border bg-background px-3 font-mono text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    placeholder="00000000-0000-0000-0000-000000000000"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium" htmlFor="approval-step-id">
                    步骤 ID
                  </label>
                  <input
                    id="approval-step-id"
                    value={stepId}
                    disabled={createPending}
                    onChange={(event) => setStepId(event.target.value)}
                    className="h-9 w-full rounded-md border bg-background px-3 font-mono text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    placeholder="00000000-0000-0000-0000-000000000000"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium" htmlFor="approval-reason">
                    审批原因
                  </label>
                  <textarea
                    id="approval-reason"
                    value={reason}
                    disabled={createPending}
                    onChange={(event) => setReason(event.target.value)}
                    className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                    placeholder="说明为什么该步骤需要人工审批"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium" htmlFor="approval-expires-at">
                    过期时间
                  </label>
                  <input
                    id="approval-expires-at"
                    type="datetime-local"
                    value={expiresAt}
                    disabled={createPending}
                    onChange={(event) => setExpiresAt(event.target.value)}
                    className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                  />
                </div>
                {createError ? (
                  <div className="rounded-md border border-destructive/30 p-3 text-sm text-destructive">
                    创建失败：{createError}
                  </div>
                ) : null}
              </div>

              <DialogFooter>
                <DialogClose asChild>
                  <Button variant="outline" disabled={createPending}>
                    取消
                  </Button>
                </DialogClose>
                <Button
                  disabled={
                    createPending ||
                    !taskId.trim() ||
                    !stepId.trim() ||
                    !reason.trim()
                  }
                  onClick={handleCreateApproval}
                >
                  {createPending ? "创建中" : "创建"}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>
        {actionError ? (
          <div className="mb-4 rounded-md border border-destructive/30 p-3 text-sm text-destructive">
            操作失败：{actionError}
          </div>
        ) : null}
        <DataTable
          columns={approvalColumns}
          rows={approvals}
          actions={[]}
          renderActions={(approval) => (
            <>
              {approval.status === "pending" ? (
                <>
                  <Button
                    aria-label={`通过审批 ${approval.id}`}
                    title="通过"
                    variant="ghost"
                    size="icon"
                    className="size-8 text-muted-foreground hover:bg-primary/10 hover:text-primary"
                    disabled={actionPending !== null}
                    onClick={() => void handleApprove(approval)}
                  >
                    <Check className="size-4" />
                  </Button>
                  <Button
                    aria-label={`拒绝审批 ${approval.id}`}
                    title="拒绝"
                    variant="ghost"
                    size="icon"
                    className="size-8 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                    disabled={actionPending !== null}
                    onClick={() => void handleDeny(approval)}
                  >
                    <X className="size-4" />
                  </Button>
                </>
              ) : null}
              <Button
                aria-label={`删除审批 ${approval.id}`}
                title="删除"
                variant="ghost"
                size="icon"
                className="size-8 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                disabled={actionPending !== null}
                onClick={() => void handleDelete(approval)}
              >
                <Trash2 className="size-4" />
              </Button>
            </>
          )}
          emptyText="暂无审批请求"
        />
      </PageSection>
    </PageContainer>
  );
}
