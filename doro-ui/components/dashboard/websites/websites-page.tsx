"use client";

import {
  Play,
  Plus,
  RefreshCw,
  RotateCw,
  Search,
  Settings,
  Square,
  Trash2,
} from "lucide-react";
import { type FormEvent, type ReactNode, useEffect, useMemo, useState } from "react";

import { DataTable, ResourceStatusBadge, TruncatedText } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  createWebsite,
  deleteWebsite,
  getWebsites,
  updateWebsite,
  websiteAction,
} from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import type { CreateWebsiteRequest, Website } from "@/types/api";
import type { ResourceColumn, ResourceStatus } from "@/types/dashboard";

type WebsiteFormState = {
  id: string | null;
  name: string;
  primaryDomain: string;
  aliases: string;
  listenPort: string;
  upstreamUrl: string;
  notes: string;
};

type WebsiteRow = Website & {
  resourceStatus: ResourceStatus;
  domainLabel: string;
  typeLabel: string;
  protocolLabel: string;
  sslLabel: string;
  updatedLabel: string;
};

const emptyForm: WebsiteFormState = {
  id: null,
  name: "",
  primaryDomain: "",
  aliases: "",
  listenPort: "8080",
  upstreamUrl: "http://127.0.0.1:8787",
  notes: "",
};

const statusLabels: Record<Website["status"], ResourceStatus> = {
  running: "running",
  stopped: "stopped",
  warning: "warning",
};

const columns: ResourceColumn<WebsiteRow>[] = [
  {
    key: "domainLabel",
    label: "名称 / 域名",
    width: "24%",
    render: (row) => (
      <div className="min-w-0">
        <p className="truncate font-medium" title={row.name}>
          {row.name}
        </p>
        <p className="truncate text-xs text-muted-foreground" title={row.domainLabel}>
          {row.domainLabel}
        </p>
      </div>
    ),
  },
  {
    key: "typeLabel",
    label: "类型",
    width: "7rem",
    render: (row) => <TruncatedText value={row.typeLabel} />,
  },
  {
    key: "upstream",
    label: "代理目标",
    width: "24%",
    render: (row) => <TruncatedText value={row.upstream.url} />,
  },
  {
    key: "resourceStatus",
    label: "状态",
    width: "7rem",
    render: (row) => <ResourceStatusBadge status={row.resourceStatus} />,
  },
  {
    key: "protocolLabel",
    label: "协议",
    width: "6rem",
    render: (row) => <TruncatedText value={row.protocolLabel} />,
  },
  {
    key: "sslLabel",
    label: "SSL",
    width: "9rem",
    render: (row) => <TruncatedText value={row.sslLabel} />,
  },
  {
    key: "updatedLabel",
    label: "更新时间",
    width: "9rem",
    render: (row) => <TruncatedText value={row.updatedLabel} />,
  },
  {
    key: "notes",
    label: "备注",
    width: "16%",
    render: (row) => <TruncatedText value={row.last_runtime_error ?? row.notes ?? "-"} />,
  },
];

export function WebsitesPage() {
  const [websites, setWebsites] = useState<Website[]>([]);
  const [query, setQuery] = useState("");
  const [apiError, setApiError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [form, setForm] = useState<WebsiteFormState>(emptyForm);

  const loadWebsites = async () => {
    setLoading(true);
    const result = await getWebsites();
    if (result.data) {
      setWebsites(result.data.items);
      setApiError(null);
    } else {
      setApiError(result.error);
    }
    setLoading(false);
  };

  useEffect(() => {
    void loadWebsites();
  }, []);

  const rows = useMemo(
    () =>
      websites.map((website) => ({
        ...website,
        resourceStatus: statusLabels[website.status],
        domainLabel: [website.primary_domain, ...website.aliases].join(", "),
        typeLabel: "反向代理",
        protocolLabel: website.protocol.toUpperCase(),
        sslLabel: "未启用（v1）",
        updatedLabel: formatRelativeTime(website.updated_at),
      })),
    [websites],
  );

  const filteredRows = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) {
      return rows;
    }
    return rows.filter((row) => {
      return (
        row.name.toLowerCase().includes(normalizedQuery) ||
        row.primary_domain.toLowerCase().includes(normalizedQuery) ||
        row.aliases.some((alias) => alias.toLowerCase().includes(normalizedQuery)) ||
        row.upstream.url.toLowerCase().includes(normalizedQuery)
      );
    });
  }, [query, rows]);

  const openCreateDialog = () => {
    setForm(emptyForm);
    setDialogOpen(true);
  };

  const openEditDialog = (website: Website) => {
    setForm({
      id: website.id,
      name: website.name,
      primaryDomain: website.primary_domain,
      aliases: website.aliases.join("\n"),
      listenPort: String(website.listen_port),
      upstreamUrl: website.upstream.url,
      notes: website.notes ?? "",
    });
    setDialogOpen(true);
  };

  const submitWebsite = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setNotice(null);
    const request = websiteRequest(form);
    const result = form.id
      ? await updateWebsite(form.id, request)
      : await createWebsite(request);

    if (result.data) {
      setDialogOpen(false);
      setNotice(form.id ? "网站配置已保存。" : "网站已创建，默认保持停止状态。");
      await loadWebsites();
    } else {
      setApiError(result.error);
    }
  };

  const runAction = async (
    website: Website,
    action: "start" | "stop" | "restart",
  ) => {
    setBusyId(website.id);
    setNotice(null);
    const result = await websiteAction(website.id, action, {
      reason: "operator requested from websites page",
    });
    if (result.data) {
      if (result.data.task) {
        setNotice("已创建网络暴露审批任务，批准后 Pingora 路由会生效。");
      } else {
        setNotice(action === "stop" ? "网站已停止。" : "操作已提交。");
      }
      await loadWebsites();
    } else {
      setApiError(result.error);
    }
    setBusyId(null);
  };

  const removeWebsite = async (website: Website) => {
    if (!window.confirm(`删除网站 ${website.primary_domain}？`)) {
      return;
    }
    setBusyId(website.id);
    const result = await deleteWebsite(website.id);
    if (!result.error) {
      setNotice("网站已删除。");
      await loadWebsites();
    } else {
      setApiError(result.error);
    }
    setBusyId(null);
  };

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      {notice ? (
        <div className="rounded-lg border border-primary/30 p-4 text-sm text-muted-foreground">
          {notice}
        </div>
      ) : null}

      <PageSection
        title="网站"
        description="通过控制平面管理 Pingora 反向代理站点。"
        contentClassName="space-y-4"
      >
        <Toolbar
          left={
            <Button onClick={openCreateDialog}>
              <Plus className="size-4" aria-hidden="true" />
              创建
            </Button>
          }
          right={
            <div className="flex w-full flex-col gap-2 sm:w-auto sm:flex-row">
              <label className="relative min-w-0 sm:w-72">
                <Search
                  className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
                  aria-hidden="true"
                />
                <span className="sr-only">搜索网站</span>
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="搜索域名或代理目标"
                  className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
                />
              </label>
              <Button
                variant="outline"
                size="icon"
                aria-label="刷新"
                disabled={loading}
                onClick={loadWebsites}
              >
                <RefreshCw className="size-4" aria-hidden="true" />
              </Button>
            </div>
          }
        />
        <DataTable
          columns={columns}
          rows={filteredRows}
          actionsWidth="17rem"
          emptyText={loading ? "正在加载网站..." : "暂无网站"}
          renderActions={(row) => (
            <>
              {row.status === "running" ? (
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busyId === row.id}
                  onClick={() => runAction(row, "stop")}
                >
                  <Square className="size-4" aria-hidden="true" />
                  停止
                </Button>
              ) : (
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busyId === row.id}
                  onClick={() => runAction(row, "start")}
                >
                  <Play className="size-4" aria-hidden="true" />
                  启动
                </Button>
              )}
              <Button
                variant="outline"
                size="sm"
                disabled={busyId === row.id}
                onClick={() => runAction(row, "restart")}
              >
                <RotateCw className="size-4" aria-hidden="true" />
                重启
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={row.status !== "stopped" || busyId === row.id}
                onClick={() => openEditDialog(row)}
              >
                <Settings className="size-4" aria-hidden="true" />
                配置
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={busyId === row.id}
                onClick={() => removeWebsite(row)}
              >
                <Trash2 className="size-4" aria-hidden="true" />
                删除
              </Button>
            </>
          )}
        />
      </PageSection>

      <WebsiteDialog
        open={dialogOpen}
        form={form}
        onOpenChange={setDialogOpen}
        onFormChange={setForm}
        onSubmit={submitWebsite}
      />
    </PageContainer>
  );
}

function WebsiteDialog({
  open,
  form,
  onOpenChange,
  onFormChange,
  onSubmit,
}: {
  open: boolean;
  form: WebsiteFormState;
  onOpenChange: (open: boolean) => void;
  onFormChange: (form: WebsiteFormState) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const editing = Boolean(form.id);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>{editing ? "配置网站" : "创建网站"}</DialogTitle>
            <DialogDescription>
              首版仅创建 HTTP 反向代理路由；HTTPS、静态站点和证书管理后续扩展。
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="名称">
              <input
                required
                value={form.name}
                onChange={(event) => onFormChange({ ...form, name: event.target.value })}
                className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </Field>
            <Field label="主域名">
              <input
                required
                value={form.primaryDomain}
                onChange={(event) =>
                  onFormChange({ ...form, primaryDomain: event.target.value })
                }
                placeholder="example.com"
                className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </Field>
            <Field label="监听端口">
              <input
                required
                type="number"
                min={1}
                max={65535}
                value={form.listenPort}
                onChange={(event) =>
                  onFormChange({ ...form, listenPort: event.target.value })
                }
                className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </Field>
            <Field label="代理目标">
              <input
                required
                value={form.upstreamUrl}
                onChange={(event) =>
                  onFormChange({ ...form, upstreamUrl: event.target.value })
                }
                placeholder="http://127.0.0.1:8787"
                className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </Field>
          </div>

          <Field label="别名">
            <textarea
              value={form.aliases}
              onChange={(event) => onFormChange({ ...form, aliases: event.target.value })}
              placeholder="每行一个域名"
              className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
            />
          </Field>

          <Field label="备注">
            <textarea
              value={form.notes}
              onChange={(event) => onFormChange({ ...form, notes: event.target.value })}
              className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
            />
          </Field>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              取消
            </Button>
            <Button type="submit">{editing ? "保存" : "创建"}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <label className="block space-y-2 text-sm">
      <span className="font-medium">{label}</span>
      {children}
    </label>
  );
}

function websiteRequest(form: WebsiteFormState): CreateWebsiteRequest {
  const aliases = form.aliases
    .split(/\r?\n|,/)
    .map((alias) => alias.trim())
    .filter(Boolean);
  return {
    name: form.name.trim(),
    primary_domain: form.primaryDomain.trim(),
    aliases,
    listen_port: Number(form.listenPort),
    upstream_url: form.upstreamUrl.trim(),
    notes: form.notes.trim() || null,
  };
}
