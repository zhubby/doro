"use client";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { ResourceListPage } from "@/components/dashboard/resources/resource-list-page";
import { websites } from "@/lib/mock-data";
import type { ResourceColumn, WebsiteResource } from "@/types/dashboard";
import { useTranslations } from "next-intl";

export function WebsitesPage() {
  const t = useTranslations("resources");
  const tCommon = useTranslations("common");
  const columns: ResourceColumn<WebsiteResource>[] = [
    {
      key: "primaryDomain",
      label: t("columns.primaryDomain"),
      render: (row) => (
        <div>
          <p className="font-medium">{row.primaryDomain}</p>
          <p className="text-xs text-muted-foreground">{row.id}</p>
        </div>
      ),
    },
    {
      key: "status",
      label: t("columns.status"),
      render: (row) => <ResourceStatusBadge status={row.status} />,
    },
    { key: "runtime", label: t("columns.runtime") },
    { key: "ssl", label: t("columns.ssl") },
    { key: "rootPath", label: t("columns.rootPath") },
    { key: "traffic", label: t("columns.traffic") },
    { key: "updatedAt", label: t("columns.updatedAt") },
  ];

  return (
    <ResourceListPage
      title={t("websites.title")}
      description={t("websites.description")}
      rows={websites}
      columns={columns}
      createLabel={t("websites.create")}
      importLabel={tCommon("actions.importConfig")}
      batchActions={[
        tCommon("actions.start"),
        tCommon("actions.stop"),
        t("websites.renewCertificate"),
      ]}
    />
  );
}
