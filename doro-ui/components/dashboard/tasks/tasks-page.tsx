import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import type { Task } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";
import { useTranslations } from "next-intl";

type TasksPageProps = {
  tasks: Task[];
  apiError?: string | null;
};

function TaskStatusBadge({ status }: { status: Task["status"] }) {
  const t = useTranslations("common.status");

  if (status === "running" || status === "queued") {
    return <Badge>{status === "running" ? t("running") : t("queued")}</Badge>;
  }

  if (status === "waiting_approval") {
    return <Badge variant="secondary">{t("waitingApproval")}</Badge>;
  }

  if (status === "succeeded") {
    return <Badge variant="secondary">{t("completed")}</Badge>;
  }

  if (status === "failed" || status === "cancelled") {
    return <Badge variant="outline">{t("stopped")}</Badge>;
  }

  return <Badge variant="outline">{t("draft")}</Badge>;
}

export function TasksPage({ tasks, apiError }: TasksPageProps) {
  const t = useTranslations("resources");
  const tCommon = useTranslations("common");
  const taskColumns: ResourceColumn<Task>[] = [
    {
      key: "title",
      label: t("columns.task"),
      render: (task) => (
        <div>
          <p className="font-medium">{task.title}</p>
          <p className="text-xs text-muted-foreground">{task.id}</p>
        </div>
      ),
    },
    {
      key: "status",
      label: t("columns.status"),
      render: (task) => <TaskStatusBadge status={task.status} />,
    },
    {
      key: "steps",
      label: t("columns.steps"),
      render: (task) => t("tasks.steps", { count: task.steps.length }),
    },
    {
      key: "host_id",
      label: t("columns.host"),
      render: (task) => task.host_id ?? t("tasks.unassigned"),
    },
    {
      key: "created_at",
      label: t("columns.createdAt"),
    },
  ];

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          {tCommon("errors.controlPlaneUnavailable", { error: apiError })}
        </div>
      ) : null}
      <PageSection>
        <DataTable
          columns={taskColumns}
          rows={tasks}
          actions={[]}
          emptyText={t("tasks.empty")}
        />
      </PageSection>
    </PageContainer>
  );
}
