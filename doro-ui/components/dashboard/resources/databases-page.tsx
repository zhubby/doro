"use client";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import { databases } from "@/lib/mock-data";
import type { DatabaseResource, ResourceColumn } from "@/types/dashboard";
import { useTranslations } from "next-intl";

export function DatabasesPage() {
  const t = useTranslations("resources");
  const tCommon = useTranslations("common");
  const columns: ResourceColumn<DatabaseResource>[] = [
    {
      key: "name",
      label: t("columns.name"),
      render: (row) => (
        <div>
          <p className="font-medium">{row.name}</p>
          <p className="text-xs text-muted-foreground">{row.engine}</p>
        </div>
      ),
    },
    { key: "version", label: t("columns.version") },
    {
      key: "status",
      label: t("columns.status"),
      render: (row) => <ResourceStatusBadge status={row.status} />,
    },
    { key: "size", label: t("columns.size") },
    { key: "backup", label: t("columns.backup") },
    { key: "updatedAt", label: t("columns.updatedAt") },
  ];

  return (
    <ResourceListPage
      title={t("databases.title")}
      description={t("databases.description")}
      rows={databases}
      columns={columns}
      createLabel={t("databases.create")}
      batchActions={[
        tCommon("actions.backup"),
        tCommon("actions.restart"),
        tCommon("actions.delete"),
      ]}
    />
  );
}
