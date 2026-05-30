"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import {
  Copy,
  Download,
  File,
  Folder,
  FolderPlus,
  HardDrive,
  Home,
  MoveRight,
  RefreshCw,
  Search,
  Server,
  Trash2,
  Upload,
} from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  downloadFile,
  getHosts,
  listFiles,
  runFileOperation,
  searchFiles,
  uploadFile,
} from "@/lib/control-plane-api";
import { cn } from "@/lib/utils";
import type { FileEntry, Host } from "@/types/api";

const ROOTS = ["/", "/Users", "/home", "/var", "/tmp"];

function hasFileCapability(host: Host, capability: "files_read" | "files_write") {
  return host.capabilities.some((item) => item.name === capability);
}

function hostLabel(host: Host) {
  return host.display_name || host.hostname;
}

function formatBytes(value: FileEntry["size_bytes"]) {
  if (value === null || value === undefined) {
    return "-";
  }
  const bytes = Number(value);
  if (!Number.isFinite(bytes)) {
    return "-";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let size = bytes / 1024;
  let unit = units[0];
  for (const nextUnit of units.slice(1)) {
    if (size < 1024) {
      break;
    }
    size /= 1024;
    unit = nextUnit;
  }
  return `${size >= 10 ? size.toFixed(0) : size.toFixed(1)} ${unit}`;
}

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

function parentPath(path: string) {
  if (!path || path === "/") {
    return "/";
  }
  const trimmed = path.replace(/\/+$/, "");
  const index = trimmed.lastIndexOf("/");
  if (index <= 0) {
    return "/";
  }
  return trimmed.slice(0, index);
}

function joinPath(base: string, name: string) {
  if (base === "/") {
    return `/${name}`;
  }
  return `${base.replace(/\/+$/, "")}/${name}`;
}

async function fileToBase64(file: globalThis.File) {
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const chunkSize = 8192;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.slice(index, index + chunkSize));
  }
  return btoa(binary);
}

function saveDownload(name: string, contentBase64: string) {
  const binary = atob(contentBase64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  const url = URL.createObjectURL(new Blob([bytes]));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
}

export function FilesPage() {
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [hosts, setHosts] = useState<Host[]>([]);
  const [selectedHostId, setSelectedHostId] = useState("");
  const [path, setPath] = useState("/");
  const [typedPath, setTypedPath] = useState("/");
  const [items, setItems] = useState<FileEntry[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchMode, setSearchMode] = useState(false);

  const selectedHost = useMemo(
    () => hosts.find((host) => host.id === selectedHostId) ?? null,
    [hosts, selectedHostId],
  );
  const selectedItem = useMemo(
    () => items.find((item) => item.path === selectedPath) ?? null,
    [items, selectedPath],
  );
  const canWrite = Boolean(selectedHost && hasFileCapability(selectedHost, "files_write"));

  async function loadHosts() {
    const result = await getHosts();
    if (result.data) {
      const fileHosts = result.data.items.filter((host) =>
        hasFileCapability(host, "files_read"),
      );
      setHosts(fileHosts);
      setSelectedHostId((current) => current || fileHosts[0]?.id || "");
      setError(null);
    } else {
      setError(result.error ?? "无法加载 Agent");
    }
  }

  async function loadDirectory(nextPath = path) {
    if (!selectedHostId) {
      setItems([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    const result = await listFiles(selectedHostId, nextPath);
    if (result.data) {
      setPath(result.data.path);
      setTypedPath(result.data.path);
      setItems(result.data.items);
      setSelectedPath(null);
      setSearchMode(false);
      setError(null);
    } else {
      setError(result.error ?? "无法读取目录");
    }
    setLoading(false);
  }

  useEffect(() => {
    loadHosts();
  }, []);

  useEffect(() => {
    if (selectedHostId) {
      loadDirectory("/");
    }
  }, [selectedHostId]);

  async function handleSearch() {
    if (!selectedHostId || !searchQuery.trim()) {
      await loadDirectory(path);
      return;
    }
    setBusy(true);
    const result = await searchFiles(selectedHostId, path, searchQuery.trim());
    if (result.data) {
      setItems(result.data.items);
      setSelectedPath(null);
      setSearchMode(true);
      setError(null);
    } else {
      setError(result.error ?? "搜索失败");
    }
    setBusy(false);
  }

  async function handleOpen(item: FileEntry) {
    if (item.kind === "directory" || item.kind === "symlink") {
      await loadDirectory(item.path);
      return;
    }
    await handleDownload(item);
  }

  async function handleDownload(item = selectedItem) {
    if (!selectedHostId || !item || item.kind === "directory") {
      return;
    }
    setBusy(true);
    const result = await downloadFile(selectedHostId, item.path);
    if (result.data) {
      saveDownload(result.data.name, result.data.content_base64);
      setError(null);
    } else {
      setError(result.error ?? "下载失败");
    }
    setBusy(false);
  }

  async function handleUpload(files: FileList | null) {
    const file = files?.[0];
    if (!selectedHostId || !file) {
      return;
    }
    setBusy(true);
    const contentBase64 = await fileToBase64(file);
    const result = await uploadFile(selectedHostId, {
      path: joinPath(path, file.name),
      content_base64: contentBase64,
      overwrite: true,
    });
    if (result.data) {
      await loadDirectory(path);
    } else {
      setError(result.error ?? "上传失败");
    }
    setBusy(false);
    if (fileInputRef.current) {
      fileInputRef.current.value = "";
    }
  }

  async function executeOperation(
    operation: Parameters<typeof runFileOperation>[1]["operation"],
    operationPath: string,
    options: {
      target_path?: string | null;
      name?: string | null;
      overwrite?: boolean | null;
    } = {},
  ) {
    if (!selectedHostId) {
      return;
    }
    setBusy(true);
    const result = await runFileOperation(selectedHostId, {
      operation,
      path: operationPath,
      target_path: options.target_path ?? null,
      name: options.name ?? null,
      overwrite: options.overwrite ?? false,
    });
    if (result.data) {
      await loadDirectory(path);
    } else {
      setError(result.error ?? "操作失败");
    }
    setBusy(false);
  }

  async function handleCreateDirectory() {
    const name = window.prompt("文件夹名称");
    if (!name?.trim()) {
      return;
    }
    await executeOperation("create_directory", joinPath(path, name.trim()));
  }

  async function handleRename() {
    if (!selectedItem) {
      return;
    }
    const name = window.prompt("新名称", selectedItem.name);
    if (!name?.trim() || name === selectedItem.name) {
      return;
    }
    await executeOperation("rename", selectedItem.path, {
      name: name.trim(),
    });
  }

  async function handleMove() {
    if (!selectedItem) {
      return;
    }
    const target = window.prompt("目标路径", selectedItem.path);
    if (!target?.trim() || target === selectedItem.path) {
      return;
    }
    await executeOperation("move", selectedItem.path, {
      target_path: target.trim(),
    });
  }

  async function handleCopy() {
    if (!selectedItem) {
      return;
    }
    const target = window.prompt("复制到", `${selectedItem.path}.copy`);
    if (!target?.trim()) {
      return;
    }
    await executeOperation("copy", selectedItem.path, {
      target_path: target.trim(),
    });
  }

  async function handleDelete() {
    if (!selectedItem) {
      return;
    }
    if (!window.confirm(`删除 ${selectedItem.name}？`)) {
      return;
    }
    await executeOperation("delete", selectedItem.path);
  }

  return (
    <PageContainer className="space-y-4">
      <PageSection>
        <div className="grid min-h-[calc(100vh-8rem)] gap-4 xl:grid-cols-[260px_minmax(0,1fr)_280px]">
          <aside className="min-h-0 rounded-lg border bg-card p-4">
            <div className="mb-3 flex items-center gap-2 text-sm font-medium">
              <Server className="size-4" aria-hidden="true" />
              Agent
            </div>
            <select
              value={selectedHostId}
              onChange={(event) => setSelectedHostId(event.target.value)}
              className="h-10 w-full rounded-md border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
            >
              {hosts.length === 0 ? <option value="">暂无文件 Agent</option> : null}
              {hosts.map((host) => (
                <option key={host.id} value={host.id}>
                  {hostLabel(host)}
                </option>
              ))}
            </select>

            <div className="mt-5 space-y-2">
              <p className="text-xs font-medium text-muted-foreground">位置</p>
              {ROOTS.map((root) => (
                <Button
                  key={root}
                  type="button"
                  variant={path === root ? "secondary" : "ghost"}
                  className="w-full justify-start"
                  onClick={() => loadDirectory(root)}
                >
                  {root === "/" ? (
                    <HardDrive className="size-4" aria-hidden="true" />
                  ) : (
                    <Folder className="size-4" aria-hidden="true" />
                  )}
                  {root}
                </Button>
              ))}
            </div>
          </aside>

          <main className="min-w-0 rounded-lg border bg-card">
            <div className="flex flex-wrap items-center gap-2 border-b p-3">
              <Button
                type="button"
                variant="outline"
                size="icon"
                onClick={() => loadDirectory(parentPath(path))}
                disabled={path === "/" || loading}
                aria-label="返回上级"
              >
                <Home className="size-4" aria-hidden="true" />
              </Button>
              <form
                className="min-w-48 flex-1"
                onSubmit={(event) => {
                  event.preventDefault();
                  loadDirectory(typedPath || "/");
                }}
              >
                <input
                  value={typedPath}
                  onChange={(event) => setTypedPath(event.target.value)}
                  className="h-10 w-full rounded-md border bg-background px-3 font-mono text-sm outline-none focus:ring-2 focus:ring-ring"
                />
              </form>
              <form
                className="flex min-w-52 items-center gap-2"
                onSubmit={(event) => {
                  event.preventDefault();
                  handleSearch();
                }}
              >
                <input
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  className="h-10 w-full rounded-md border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                  placeholder="搜索文件名"
                />
                <Button type="submit" variant="outline" size="icon" disabled={busy}>
                  <Search className="size-4" aria-hidden="true" />
                </Button>
              </form>
              <Button
                type="button"
                variant="outline"
                size="icon"
                onClick={() => loadDirectory(path)}
                disabled={loading}
                aria-label="刷新"
              >
                <RefreshCw className="size-4" aria-hidden="true" />
              </Button>
              <input
                ref={fileInputRef}
                type="file"
                className="hidden"
                onChange={(event) => handleUpload(event.target.files)}
              />
              <Button
                type="button"
                variant="outline"
                onClick={() => fileInputRef.current?.click()}
                disabled={!canWrite || busy}
              >
                <Upload className="size-4" aria-hidden="true" />
                上传
              </Button>
              <Button
                type="button"
                onClick={handleCreateDirectory}
                disabled={!canWrite || busy}
              >
                <FolderPlus className="size-4" aria-hidden="true" />
                新建
              </Button>
            </div>

            {error ? (
              <div className="border-b bg-destructive/10 px-4 py-2 text-sm text-destructive">
                {error}
              </div>
            ) : null}
            {searchMode ? (
              <div className="border-b px-4 py-2 text-xs text-muted-foreground">
                搜索结果 · {items.length} 项
              </div>
            ) : null}

            <div className="min-h-0 overflow-auto">
              <table className="w-full text-sm">
                <thead className="sticky top-0 bg-card text-left text-xs text-muted-foreground">
                  <tr className="border-b">
                    <th className="h-9 px-4 font-medium">名称</th>
                    <th className="h-9 px-3 font-medium">大小</th>
                    <th className="h-9 px-3 font-medium">修改时间</th>
                    <th className="h-9 px-3 font-medium">权限</th>
                  </tr>
                </thead>
                <tbody>
                  {loading ? (
                    <tr>
                      <td className="px-4 py-8 text-center text-muted-foreground" colSpan={4}>
                        正在加载
                      </td>
                    </tr>
                  ) : null}
                  {!loading && items.length === 0 ? (
                    <tr>
                      <td className="px-4 py-8 text-center text-muted-foreground" colSpan={4}>
                        空目录
                      </td>
                    </tr>
                  ) : null}
                  {items.map((item) => {
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
                        onDoubleClick={() => handleOpen(item)}
                      >
                        <td className="max-w-0 px-4 py-2">
                          <div className="flex min-w-0 items-center gap-2">
                            <Icon className="size-4 shrink-0 text-muted-foreground" />
                            <span className="truncate font-medium">{item.name}</span>
                            {item.kind === "symlink" ? (
                              <Badge variant="outline">link</Badge>
                            ) : null}
                          </div>
                        </td>
                        <td className="whitespace-nowrap px-3 py-2 text-muted-foreground">
                          {formatBytes(item.size_bytes)}
                        </td>
                        <td className="whitespace-nowrap px-3 py-2 text-muted-foreground">
                          {formatModified(item.modified_at)}
                        </td>
                        <td className="whitespace-nowrap px-3 py-2 text-muted-foreground">
                          {item.readonly ? "只读" : "读写"}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </main>

          <aside className="min-h-0 rounded-lg border bg-card p-4">
            <div className="mb-4 flex items-center justify-between">
              <h2 className="text-sm font-semibold">详情</h2>
              {selectedItem ? <Badge variant="outline">{selectedItem.kind}</Badge> : null}
            </div>
            {selectedItem ? (
              <div className="space-y-4">
                <div>
                  <p className="truncate text-sm font-medium">{selectedItem.name}</p>
                  <p className="mt-1 break-all font-mono text-xs text-muted-foreground">
                    {selectedItem.path}
                  </p>
                </div>
                <dl className="space-y-2 text-sm">
                  <div className="flex justify-between gap-4">
                    <dt className="text-muted-foreground">大小</dt>
                    <dd>{formatBytes(selectedItem.size_bytes)}</dd>
                  </div>
                  <div className="flex justify-between gap-4">
                    <dt className="text-muted-foreground">修改</dt>
                    <dd>{formatModified(selectedItem.modified_at)}</dd>
                  </div>
                  <div className="flex justify-between gap-4">
                    <dt className="text-muted-foreground">权限</dt>
                    <dd>{selectedItem.readonly ? "只读" : "读写"}</dd>
                  </div>
                </dl>
                {selectedItem.symlink_target ? (
                  <p className="break-all rounded-md border bg-background p-3 font-mono text-xs text-muted-foreground">
                    {selectedItem.symlink_target}
                  </p>
                ) : null}
                <div className="grid gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => handleDownload()}
                    disabled={selectedItem.kind === "directory" || busy}
                  >
                    <Download className="size-4" aria-hidden="true" />
                    下载
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    onClick={handleRename}
                    disabled={!canWrite || busy}
                  >
                    <MoveRight className="size-4" aria-hidden="true" />
                    重命名
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    onClick={handleCopy}
                    disabled={!canWrite || busy}
                  >
                    <Copy className="size-4" aria-hidden="true" />
                    复制
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    onClick={handleMove}
                    disabled={!canWrite || busy}
                  >
                    <MoveRight className="size-4" aria-hidden="true" />
                    移动
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    className="text-destructive hover:text-destructive"
                    onClick={handleDelete}
                    disabled={!canWrite || busy}
                  >
                    <Trash2 className="size-4" aria-hidden="true" />
                    删除
                  </Button>
                </div>
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">未选择项目</p>
            )}
          </aside>
        </div>
      </PageSection>
    </PageContainer>
  );
}
