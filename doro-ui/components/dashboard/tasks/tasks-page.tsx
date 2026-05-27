import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import type { Task } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";

type TasksPageProps = {
  tasks: Task[];
  apiError?: string | null;
};

function taskStatusLabel(status: Task["status"]) {
  if (status === "running" || status === "queued") {
    return <Badge>{status === "running" ? "运行中" : "排队中"}</Badge>;
  }

  if (status === "waiting_approval") {
    return <Badge variant="secondary">等待审批</Badge>;
  }

  if (status === "succeeded") {
    return <Badge variant="secondary">已完成</Badge>;
  }

  if (status === "failed" || status === "cancelled") {
    return <Badge variant="outline">已停止</Badge>;
  }

  return <Badge variant="outline">草稿</Badge>;
}

const taskColumns: ResourceColumn<Task>[] = [
  {
    key: "title",
    label: "任务",
    render: (task) => (
      <div>
        <p className="font-medium">{task.title}</p>
        <p className="text-xs text-muted-foreground">{task.id}</p>
      </div>
    ),
  },
  {
    key: "status",
    label: "状态",
    render: (task) => taskStatusLabel(task.status),
  },
  {
    key: "steps",
    label: "步骤",
    render: (task) => `${task.steps.length} 步`,
  },
  {
    key: "host_id",
    label: "目标主机",
    render: (task) => task.host_id ?? "未指定",
  },
  {
    key: "created_at",
    label: "创建时间",
  },
];

export function TasksPage({ tasks, apiError }: TasksPageProps) {
  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <PageSection
        title="任务"
        description="控制面下发的任务、步骤和执行状态。"
      >
        <DataTable
          columns={taskColumns}
          rows={tasks}
          actions={[]}
          emptyText="暂无任务"
        />
      </PageSection>
    </PageContainer>
  );
}

