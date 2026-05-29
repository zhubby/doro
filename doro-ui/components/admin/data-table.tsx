import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ResourceColumn, ResourceStatus } from "@/types/dashboard";
import type { ReactNode } from "react";

type DataTableProps<T extends { id: string }> = {
  columns: ResourceColumn<T>[];
  rows: T[];
  actions?: string[];
  renderActions?: (row: T) => ReactNode;
  emptyText?: string;
};

export function DataTable<T extends { id: string }>({
  columns,
  rows,
  actions = ["管理"],
  renderActions,
  emptyText = "暂无数据",
}: DataTableProps<T>) {
  const hasActions = actions.length > 0 || Boolean(renderActions);

  return (
    <div className="overflow-hidden rounded-lg border">
      <div className="overflow-hidden">
        <table className="w-full table-fixed text-sm">
          <colgroup>
            <col className="w-10" />
            {columns.map((column) => (
              <col key={String(column.key)} style={{ width: column.width }} />
            ))}
            {hasActions ? <col className="w-28" /> : null}
          </colgroup>
          <thead className="bg-muted/50 text-left text-xs text-muted-foreground">
            <tr>
              <th className="px-3 py-3 sm:px-4">
                <span className="sr-only">选择</span>
              </th>
              {columns.map((column) => (
                <th
                  key={String(column.key)}
                  className={cn("px-3 py-3 sm:px-4", column.className)}
                  title={column.label}
                >
                  {column.label}
                </th>
              ))}
              {hasActions ? <th className="px-3 py-3 text-right sm:px-4">操作</th> : null}
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td
                  colSpan={columns.length + (hasActions ? 2 : 1)}
                  className="px-4 py-10 text-center text-muted-foreground"
                >
                  {emptyText}
                </td>
              </tr>
            ) : (
              rows.map((row) => (
                <tr key={row.id} className="border-t">
                  <td className="px-3 py-4 sm:px-4">
                    <div className="size-4 rounded border bg-background" />
                  </td>
                  {columns.map((column) => (
                    <td
                      key={String(column.key)}
                      className={cn("px-3 py-4 sm:px-4", column.className)}
                    >
                      {column.render ? (
                        column.render(row)
                      ) : (
                        <TruncatedText value={String(row[column.key as keyof T] ?? "")} />
                      )}
                    </td>
                  ))}
                  {hasActions ? (
                    <td className="px-3 py-4 sm:px-4">
                      <div className="flex justify-end gap-2">
                        {renderActions
                          ? renderActions(row)
                          : actions.map((action) => (
                              <Button key={action} variant="outline" size="sm">
                                {action}
                              </Button>
                            ))}
                      </div>
                    </td>
                  ) : null}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function TruncatedText({ value }: { value: string }) {
  return (
    <span className="block truncate" title={value}>
      {value}
    </span>
  );
}

export function ResourceStatusBadge({ status }: { status: ResourceStatus }) {
  if (status === "running") {
    return <Badge className="min-w-14 justify-center">运行中</Badge>;
  }

  if (status === "warning") {
    return (
      <Badge variant="secondary" className="min-w-14 justify-center">
        需关注
      </Badge>
    );
  }

  return (
    <Badge variant="outline" className="min-w-14 justify-center">
      已停止
    </Badge>
  );
}
