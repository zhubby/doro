"use client";

import { RefreshCw, Search, Settings2 } from "lucide-react";
import type { ReactNode } from "react";
import { useMemo, useState } from "react";

import { DataTable } from "@/components/admin/data-table";
import { FilterChips, type FilterChip } from "@/components/admin/filter-chips";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Button } from "@/components/ui/button";
import type { ResourceColumn, ResourceStatus } from "@/types/dashboard";
import { useTranslations } from "next-intl";

type ResourceListPageProps<T extends { id: string; status: ResourceStatus }> = {
  title: string;
  description: string;
  rows: T[];
  columns: ResourceColumn<T>[];
  createLabel?: string;
  importLabel?: string;
  batchActions?: string[];
  rowActions?: string[];
  notice?: ReactNode;
  filteredRows?: T[];
  toolbarRight?: ReactNode;
  showStatusChips?: boolean;
};

export function ResourceListPage<T extends { id: string; status: ResourceStatus }>({
  title: _title,
  description: _description,
  rows,
  columns,
  createLabel,
  importLabel,
  batchActions = [],
  rowActions,
  notice,
  filteredRows: controlledRows,
  toolbarRight,
  showStatusChips = true,
}: ResourceListPageProps<T>) {
  const [activeStatus, setActiveStatus] = useState<ResourceStatus | "all">("all");
  const t = useTranslations("common.status");
  const tResources = useTranslations("resources.list");
  const tActions = useTranslations("common.actions");
  const filters = useMemo<FilterChip[]>(() => {
    const statuses: Array<ResourceStatus | "all"> = [
      "all",
      "running",
      "stopped",
      "warning",
    ];

    return statuses.map((status) => ({
      value: status,
      label: status === "all" ? t("all") : t(status),
      count:
        status === "all"
          ? rows.length
          : rows.filter((row) => row.status === status).length,
    }));
  }, [rows, t]);
  const visibleRowActions = rowActions ?? [tActions("manage"), tActions("logs")];
  const localFilteredRows = useMemo(() => {
    if (activeStatus === "all") {
      return rows;
    }

    return rows.filter((row) => row.status === activeStatus);
  }, [activeStatus, rows]);
  const tableRows = controlledRows ?? localFilteredRows;

  return (
    <PageContainer>
      {notice}
      {showStatusChips ? (
        <PageSection contentClassName="space-y-4">
          <FilterChips
            filters={filters}
            value={activeStatus}
            onValueChange={(value) => setActiveStatus(value as ResourceStatus | "all")}
          />
        </PageSection>
      ) : null}

      <PageSection title={_title} description={_description} contentClassName="space-y-4">
        <Toolbar
          left={
            <>
              {createLabel ? <Button>{createLabel}</Button> : null}
              {importLabel ? <Button variant="outline">{importLabel}</Button> : null}
              {batchActions.map((action) => (
                <Button key={action} variant="outline">
                  {action}
                </Button>
              ))}
            </>
          }
          right={toolbarRight ?? (
            <>
              <Button variant="outline">
                <Search className="size-4" aria-hidden="true" />
                {tResources("search")}
              </Button>
              <Button variant="outline" size="icon" aria-label={tResources("refresh")}>
                <RefreshCw className="size-4" aria-hidden="true" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                aria-label={tResources("columnSettings")}
              >
                <Settings2 className="size-4" aria-hidden="true" />
              </Button>
            </>
          )}
        />
        <DataTable columns={columns} rows={tableRows} actions={visibleRowActions} />
      </PageSection>
    </PageContainer>
  );
}
