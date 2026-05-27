import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ResourceColumn, ResourceStatus } from "@/types/dashboard";

type DataTableProps<T extends { id: string }> = {
  columns: ResourceColumn<T>[];
  rows: T[];
  actions?: string[];
  emptyText?: string;
};

export function DataTable<T extends { id: string }>({
  columns,
  rows,
  actions = ["管理"],
  emptyText = "暂无数据",
}: DataTableProps<T>) {
  return (
    <div className="overflow-hidden rounded-lg border">
      <div className="overflow-x-auto">
        <table className="w-full min-w-[760px] text-sm">
          <thead className="bg-muted/50 text-left text-xs text-muted-foreground">
            <tr>
              <th className="w-10 px-4 py-3">
                <span className="sr-only">选择</span>
              </th>
              {columns.map((column) => (
                <th key={String(column.key)} className={cn("px-4 py-3", column.className)}>
                  {column.label}
                </th>
              ))}
              <th className="px-4 py-3 text-right">操作</th>
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td
                  colSpan={columns.length + 2}
                  className="px-4 py-10 text-center text-muted-foreground"
                >
                  {emptyText}
                </td>
              </tr>
            ) : (
              rows.map((row) => (
                <tr key={row.id} className="border-t">
                  <td className="px-4 py-4">
                    <div className="size-4 rounded border bg-background" />
                  </td>
                  {columns.map((column) => (
                    <td key={String(column.key)} className={cn("px-4 py-4", column.className)}>
                      {column.render
                        ? column.render(row)
                        : String(row[column.key as keyof T] ?? "")}
                    </td>
                  ))}
                  <td className="px-4 py-4">
                    <div className="flex justify-end gap-2">
                      {actions.map((action) => (
                        <Button key={action} variant="outline" size="sm">
                          {action}
                        </Button>
                      ))}
                    </div>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function ResourceStatusBadge({ status }: { status: ResourceStatus }) {
  if (status === "running") {
    return <Badge>运行中</Badge>;
  }

  if (status === "warning") {
    return <Badge variant="secondary">需关注</Badge>;
  }

  return <Badge variant="outline">已停止</Badge>;
}
