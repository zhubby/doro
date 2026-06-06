"use client";

import { Check, ChevronLeft, File, Folder, Home, RefreshCw } from "lucide-react";
import { type FormEvent, useCallback, useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { listFiles } from "@/lib/control-plane-api";
import { cn } from "@/lib/utils";
import type { FileEntry } from "@/types/api";

type HostDirectoryPickerDialogProps = {
  open: boolean;
  hostId: string;
  initialPath?: string;
  title?: string;
  description?: string;
  onOpenChange: (open: boolean) => void;
  onSelect: (path: string) => void;
};

function formatModified(value: string | null) {
  if (!value) {
    return "-";
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function canOpenAsDirectory(item: FileEntry) {
  return item.kind === "directory" || item.kind === "symlink";
}

export function HostDirectoryPickerDialog({
  open,
  hostId,
  initialPath,
  title = "选择宿主机目录",
  description = "从目标 Agent 文件接口浏览目录，选择后会填入当前挂载项。",
  onOpenChange,
  onSelect,
}: HostDirectoryPickerDialogProps) {
  const [path, setPath] = useState("");
  const [typedPath, setTypedPath] = useState("");
  const [parentPath, setParentPath] = useState<string | null>(null);
  const [items, setItems] = useState<FileEntry[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const directoryItems = useMemo(
    () => items.filter(canOpenAsDirectory),
    [items],
  );

  const loadDirectory = useCallback(
    async (nextPath?: string) => {
      if (!hostId) {
        setPath("");
        setTypedPath("");
        setParentPath(null);
        setItems([]);
        setError("请先选择目标 Agent");
        return;
      }

      setLoading(true);
      const result = await listFiles(hostId, nextPath);
      if (result.data) {
        setPath(result.data.path);
        setTypedPath(result.data.path);
        setParentPath(result.data.parent_path);
        setItems(result.data.items);
        setSelectedPath(null);
        setError(null);
      } else {
        setError(result.error ?? "无法读取目录");
      }
      setLoading(false);
    },
    [hostId],
  );

  useEffect(() => {
    if (open) {
      void loadDirectory(initialPath?.trim() || undefined);
    }
  }, [initialPath, loadDirectory, open]);

  function submitTypedPath(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void loadDirectory(typedPath.trim() || undefined);
  }

  function selectPath(value: string) {
    onSelect(value);
    onOpenChange(false);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[86vh] max-w-3xl overflow-hidden">
        <div className="flex max-h-[calc(86vh-3rem)] min-h-0 flex-col gap-4">
          <DialogHeader>
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>{description}</DialogDescription>
          </DialogHeader>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="icon"
              aria-label="回到起始目录"
              onClick={() => void loadDirectory()}
              disabled={!hostId || loading}
            >
              <Home className="size-4" aria-hidden="true" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              aria-label="返回上级目录"
              onClick={() => {
                if (parentPath) {
                  void loadDirectory(parentPath);
                }
              }}
              disabled={!parentPath || loading}
            >
              <ChevronLeft className="size-4" aria-hidden="true" />
            </Button>
            <form className="min-w-52 flex-1" onSubmit={submitTypedPath}>
              <input
                value={typedPath}
                onChange={(event) => setTypedPath(event.target.value)}
                className="h-9 w-full rounded-md border bg-background px-3 font-mono text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                placeholder="输入目录路径"
              />
            </form>
            <Button
              type="button"
              variant="outline"
              size="icon"
              aria-label="刷新目录"
              onClick={() => void loadDirectory(path || undefined)}
              disabled={!hostId || loading}
            >
              <RefreshCw
                className={cn("size-4", loading && "animate-spin")}
                aria-hidden="true"
              />
            </Button>
          </div>

          {error ? (
            <div className="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
              {error}
            </div>
          ) : null}

          <div className="min-h-0 overflow-auto rounded-md border">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-card text-left text-xs text-muted-foreground">
                <tr className="border-b">
                  <th className="h-9 px-4 font-medium">目录</th>
                  <th className="h-9 w-32 px-3 font-medium">类型</th>
                  <th className="h-9 w-36 px-3 font-medium">修改时间</th>
                </tr>
              </thead>
              <tbody>
                {loading ? (
                  <tr>
                    <td className="px-4 py-8 text-center text-muted-foreground" colSpan={3}>
                      正在加载
                    </td>
                  </tr>
                ) : null}
                {!loading && directoryItems.length === 0 ? (
                  <tr>
                    <td className="px-4 py-8 text-center text-muted-foreground" colSpan={3}>
                      没有可选择的子目录
                    </td>
                  </tr>
                ) : null}
                {directoryItems.map((item) => {
                  const selected = selectedPath === item.path;
                  const Icon = item.kind === "directory" ? Folder : File;
                  return (
                    <tr
                      key={item.path}
                      className={cn(
                        "cursor-default border-b last:border-0 hover:bg-accent/60",
                        selected && "bg-accent",
                      )}
                      onClick={() => setSelectedPath(item.path)}
                      onDoubleClick={() => void loadDirectory(item.path)}
                    >
                      <td className="max-w-0 px-4 py-2">
                        <div className="flex min-w-0 items-center gap-2">
                          <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                          <span className="truncate font-medium">{item.name}</span>
                        </div>
                        <p className="mt-1 truncate font-mono text-xs text-muted-foreground">
                          {item.path}
                        </p>
                      </td>
                      <td className="whitespace-nowrap px-3 py-2 text-muted-foreground">
                        {item.kind}
                      </td>
                      <td className="whitespace-nowrap px-3 py-2 text-muted-foreground">
                        {formatModified(item.modified_at)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          <DialogFooter className="border-t pt-4 sm:items-center sm:justify-between sm:space-x-0">
            <p className="break-all font-mono text-xs text-muted-foreground">
              当前目录：{path || "-"}
            </p>
            <div className="flex flex-col-reverse gap-2 sm:flex-row">
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                取消
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={() => selectPath(path)}
                disabled={!path || loading}
              >
                选择当前目录
              </Button>
              <Button
                type="button"
                onClick={() => {
                  if (selectedPath) {
                    selectPath(selectedPath);
                  }
                }}
                disabled={!selectedPath || loading}
              >
                <Check className="size-4" aria-hidden="true" />
                选择
              </Button>
            </div>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}
