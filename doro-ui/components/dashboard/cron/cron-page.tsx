"use client";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import { cronJobs } from "@/lib/mock-data";
import type { CronJob, ResourceColumn } from "@/types/dashboard";
import { useTranslations } from "next-intl";

export function CronPage() {
  const t = useTranslations("resources");
  const tCommon = useTranslations("common");
  const columns: ResourceColumn<CronJob>[] = [
    {
      key: "name",
      label: t("columns.name"),
      render: (row) => (
        <div>
          <p className="font-medium">{row.name}</p>
          <p className="text-xs text-muted-foreground">{row.type}</p>
        </div>
      ),
    },
    { key: "schedule", label: t("columns.schedule") },
    {
      key: "status",
      label: t("columns.status"),
      render: (row) => <ResourceStatusBadge status={row.status} />,
    },
    { key: "lastRun", label: t("columns.lastRun") },
    { key: "retention", label: t("columns.retention") },
  ];

  return (
    <ResourceListPage
      title={t("cron.title")}
      description={t("cron.description")}
      rows={cronJobs}
      columns={columns}
      createLabel={t("cron.create")}
      batchActions={[
        t("cron.run"),
        t("cron.enable"),
        t("cron.disable"),
        tCommon("actions.delete"),
      ]}
    />
  );
}
