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

const labels: Record<ResourceStatus | "all", string> = {
  all: "全部",
  running: "运行中",
  stopped: "已停止",
  warning: "需关注",
};

export function ResourceListPage<T extends { id: string; status: ResourceStatus }>({
  title: _title,
  description: _description,
  rows,
  columns,
  createLabel,
  importLabel,
  batchActions = [],
  rowActions = ["管理", "日志"],
  notice,
  filteredRows: controlledRows,
  toolbarRight,
  showStatusChips = true,
}: ResourceListPageProps<T>) {
  const [activeStatus, setActiveStatus] = useState<ResourceStatus | "all">("all");
  const filters = useMemo<FilterChip[]>(() => {
    const statuses: Array<ResourceStatus | "all"> = [
      "all",
      "running",
      "stopped",
      "warning",
    ];

    return statuses.map((status) => ({
      value: status,
      label: labels[status],
      count:
        status === "all"
          ? rows.length
          : rows.filter((row) => row.status === status).length,
    }));
  }, [rows]);
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

      <PageSection contentClassName="space-y-4">
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
                搜索
              </Button>
              <Button variant="outline" size="icon" aria-label="刷新">
                <RefreshCw className="size-4" aria-hidden="true" />
              </Button>
              <Button variant="outline" size="icon" aria-label="列设置">
                <Settings2 className="size-4" aria-hidden="true" />
              </Button>
            </>
          )}
        />
        <DataTable columns={columns} rows={tableRows} actions={rowActions} />
      </PageSection>
    </PageContainer>
  );
}
