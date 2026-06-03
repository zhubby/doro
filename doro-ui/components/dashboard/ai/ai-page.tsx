"use client";

import {
  Bot,
  Eye,
  KeyRound,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Send,
  Trash2,
} from "lucide-react";
import { type FormEvent, type ReactNode, useEffect, useMemo, useState } from "react";

import { DataTable, TruncatedText } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Select } from "@/components/ui/select";
import {
  createAiModelProvider,
  createTask,
  deleteAiModelProvider,
  getAiModelProviders,
  getHosts,
  getTasks,
  updateAiModelProvider,
} from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import type {
  AiModelProvider,
  CreateAiModelProviderRequest,
  Host,
  Task,
  UpdateAiModelProviderRequest,
} from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";

type ProviderFormState = {
  id: string | null;
  name: string;
  baseUrl: string;
  defaultModel: string;
  timeoutSeconds: string;
  apiKey: string;
  enabled: boolean;
};

type ProviderRow = AiModelProvider & {
  keyLabel: string;
  updatedLabel: string;
};

const emptyProviderForm: ProviderFormState = {
  id: null,
  name: "",
  baseUrl: "https://api.openai.com/v1",
  defaultModel: "gpt-4.1-mini",
  timeoutSeconds: "60",
  apiKey: "",
  enabled: true,
};

function taskStatusBadge(status: Task["status"]) {
  if (status === "succeeded") {
    return <Badge className="min-w-16 justify-center">成功</Badge>;
  }
  if (status === "running" || status === "queued") {
    return (
      <Badge variant="secondary" className="min-w-16 justify-center">
        运行中
      </Badge>
    );
  }
  if (status === "waiting_approval") {
    return (
      <Badge variant="secondary" className="min-w-16 justify-center">
        待审批
      </Badge>
    );
  }
  if (status === "failed" || status === "cancelled") {
    return (
      <Badge variant="destructive" className="min-w-16 justify-center">
        失败
      </Badge>
    );
  }
  return (
    <Badge variant="outline" className="min-w-16 justify-center">
      草稿
    </Badge>
  );
}

function providerStatusBadge(provider: AiModelProvider) {
  if (!provider.enabled) {
    return (
      <Badge variant="outline" className="min-w-14 justify-center">
        停用
      </Badge>
    );
  }
  if (!provider.has_api_key) {
    return (
      <Badge variant="secondary" className="min-w-14 justify-center">
        缺少密钥
      </Badge>
    );
  }
  return <Badge className="min-w-14 justify-center">启用</Badge>;
}

function hostLabel(host: Host) {
  const labels = host.labels.length ? ` · ${host.labels.join(", ")}` : "";
  return `${host.display_name || host.hostname}${labels}`;
}

function isAiTask(task: Task) {
  return task.steps.some((step) => step.capability === "agent_run");
}

export function AiPage() {
  const [providers, setProviders] = useState<AiModelProvider[]>([]);
  const [hosts, setHosts] = useState<Host[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [query, setQuery] = useState("");
  const [selectedHostId, setSelectedHostId] = useState("");
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [title, setTitle] = useState("AI 运维任务");
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(true);
  const [submittingTask, setSubmittingTask] = useState(false);
  const [submittingProvider, setSubmittingProvider] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [providerDialogOpen, setProviderDialogOpen] = useState(false);
  const [providerForm, setProviderForm] = useState<ProviderFormState>(emptyProviderForm);
  const [detailProvider, setDetailProvider] = useState<AiModelProvider | null>(null);
  const [apiError, setApiError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const onlineAgentHosts = useMemo(
    () =>
      hosts.filter(
        (host) =>
          host.status === "online" &&
          host.capabilities.some((capability) => capability.name === "agent_run"),
      ),
    [hosts],
  );
  const enabledProviders = useMemo(
    () => providers.filter((provider) => provider.enabled && provider.has_api_key),
    [providers],
  );
  const aiTasks = useMemo(() => tasks.filter(isAiTask).slice(0, 12), [tasks]);
  const providerRows = useMemo(
    () =>
      providers.map((provider) => ({
        ...provider,
        keyLabel: provider.has_api_key
          ? `已配置 ${provider.api_key_hint ?? ""}`.trim()
          : "未配置",
        updatedLabel: formatRelativeTime(provider.updated_at),
      })),
    [providers],
  );
  const filteredProviders = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) {
      return providerRows;
    }
    return providerRows.filter(
      (provider) =>
        provider.name.toLowerCase().includes(normalizedQuery) ||
        provider.base_url.toLowerCase().includes(normalizedQuery) ||
        provider.default_model.toLowerCase().includes(normalizedQuery),
    );
  }, [providerRows, query]);

  async function load() {
    setLoading(true);
    const [providersResult, hostsResult, tasksResult] = await Promise.all([
      getAiModelProviders(),
      getHosts(),
      getTasks(),
    ]);
    if (providersResult.data) {
      const nextProviders = providersResult.data.items;
      setProviders(nextProviders);
      const firstProvider = nextProviders.find(
        (provider) => provider.enabled && provider.has_api_key,
      );
      setSelectedProviderId((current) =>
        current &&
        nextProviders.some(
          (provider) =>
            provider.id === current && provider.enabled && provider.has_api_key,
        )
          ? current
          : firstProvider?.id ?? "",
      );
    }
    if (hostsResult.data) {
      const nextHosts = hostsResult.data.items;
      setHosts(nextHosts);
      const firstHost = nextHosts.find(
        (host) =>
          host.status === "online" &&
          host.capabilities.some((capability) => capability.name === "agent_run"),
      );
      setSelectedHostId((current) =>
        current &&
        nextHosts.some(
          (host) =>
            host.id === current &&
            host.status === "online" &&
            host.capabilities.some((capability) => capability.name === "agent_run"),
        )
          ? current
          : firstHost?.id ?? "",
      );
    }
    if (tasksResult.data) {
      setTasks(tasksResult.data.items);
    }
    setApiError(providersResult.error ?? hostsResult.error ?? tasksResult.error);
    setLoading(false);
  }

  useEffect(() => {
    void load();
  }, []);

  async function submitTask() {
    const normalizedPrompt = prompt.trim();
    if (!selectedHostId || !selectedProviderId || !normalizedPrompt || submittingTask) {
      return;
    }
    setSubmittingTask(true);
    setApiError(null);
    setNotice(null);
    const result = await createTask({
      title: title.trim() || "AI 运维任务",
      host_id: selectedHostId,
      prompt: normalizedPrompt,
      ai_provider_id: selectedProviderId,
    });
    if (result.data) {
      setTasks((current) => [result.data as Task, ...current]);
      setPrompt("");
      setNotice("AI 任务已提交，控制平面会使用选定供应商下发单次任务配置。");
    } else {
      setApiError(result.error ?? "任务创建失败");
    }
    setSubmittingTask(false);
  }

  function openCreateProviderDialog() {
    setProviderForm(emptyProviderForm);
    setProviderDialogOpen(true);
  }

  function openEditProviderDialog(provider: AiModelProvider) {
    setProviderForm({
      id: provider.id,
      name: provider.name,
      baseUrl: provider.base_url,
      defaultModel: provider.default_model,
      timeoutSeconds: String(provider.timeout_seconds),
      apiKey: "",
      enabled: provider.enabled,
    });
    setProviderDialogOpen(true);
  }

  async function submitProvider(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmittingProvider(true);
    setApiError(null);
    setNotice(null);

    const result = providerForm.id
      ? await updateAiModelProvider(providerForm.id, providerUpdateRequest(providerForm))
      : await createAiModelProvider(providerCreateRequest(providerForm));

    if (result.data) {
      setProviderDialogOpen(false);
      setNotice(providerForm.id ? "模型供应商配置已保存。" : "模型供应商已创建。");
      await load();
    } else {
      setApiError(result.error ?? "模型供应商保存失败");
    }
    setSubmittingProvider(false);
  }

  async function removeProvider(provider: AiModelProvider) {
    if (!window.confirm(`删除模型供应商 ${provider.name}？`)) {
      return;
    }
    setBusyId(provider.id);
    setApiError(null);
    setNotice(null);
    const result = await deleteAiModelProvider(provider.id);
    if (!result.error) {
      setNotice("模型供应商已删除。");
      await load();
    } else {
      setApiError(result.error);
    }
    setBusyId(null);
  }

  const providerColumns: ResourceColumn<ProviderRow>[] = [
    {
      key: "name",
      label: "名称 / Base URL",
      width: "30%",
      render: (row) => (
        <div className="min-w-0">
          <p className="truncate font-medium" title={row.name}>
            {row.name}
          </p>
          <p className="truncate text-xs text-muted-foreground" title={row.base_url}>
            {row.base_url}
          </p>
        </div>
      ),
    },
    {
      key: "default_model",
      label: "默认模型",
      width: "18%",
      render: (row) => <TruncatedText value={row.default_model} />,
    },
    {
      key: "enabled",
      label: "状态",
      width: "7rem",
      render: (row) => providerStatusBadge(row),
    },
    {
      key: "keyLabel",
      label: "密钥",
      width: "10rem",
      render: (row) => <TruncatedText value={row.keyLabel} />,
    },
    {
      key: "timeout_seconds",
      label: "超时",
      width: "6rem",
      render: (row) => <span>{row.timeout_seconds}s</span>,
    },
    {
      key: "updatedLabel",
      label: "更新时间",
      width: "9rem",
      render: (row) => <TruncatedText value={row.updatedLabel} />,
    },
  ];

  const taskColumns: ResourceColumn<Task>[] = [
    {
      key: "title",
      label: "任务",
      width: "34%",
      render: (row) => (
        <div className="min-w-0">
          <p className="truncate font-medium">{row.title}</p>
          <p className="truncate text-xs text-muted-foreground">{row.id}</p>
        </div>
      ),
    },
    {
      key: "status",
      label: "状态",
      width: "8rem",
      render: (row) => taskStatusBadge(row.status),
    },
    {
      key: "host_id",
      label: "Agent",
      width: "24%",
      render: (row) => {
        const host = hosts.find((item) => item.id === row.host_id);
        return <span className="block truncate">{host ? host.display_name : row.host_id}</span>;
      },
    },
    {
      key: "created_at",
      label: "创建时间",
      width: "10rem",
      render: (row) => <span>{formatRelativeTime(row.created_at)}</span>,
    },
  ];

  return (
    <PageContainer
      aside={
        <PageSection title="AI 状态">
          <div className="space-y-3">
            <StatusLine label="模型供应商" value={providers.length} />
            <StatusLine label="可用供应商" value={enabledProviders.length} />
            <StatusLine label="在线 Agent" value={onlineAgentHosts.length} />
            <StatusLine
              label="运行中任务"
              value={aiTasks.filter((task) => task.status === "running").length}
            />
          </div>
        </PageSection>
      }
    >
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
        title="手动 AI 任务"
        description="选择在线 Agent 和模型供应商后，下发一次 OpenAI Responses 兼容任务。"
      >
        <div className="rounded-lg border p-4">
          <div className="grid gap-3 md:grid-cols-[1fr_16rem_16rem]">
            <Field label="任务标题">
              <input
                className="h-10 w-full rounded-md border bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                value={title}
                onChange={(event) => setTitle(event.target.value)}
              />
            </Field>
            <Field label="目标 Agent">
              <Select
                value={selectedHostId}
                onValueChange={setSelectedHostId}
                options={[
                  { value: "", label: "未选择" },
                  ...onlineAgentHosts.map((host) => ({
                    value: host.id,
                    label: hostLabel(host),
                  })),
                ]}
              />
            </Field>
            <Field label="模型供应商">
              <Select
                value={selectedProviderId}
                onValueChange={setSelectedProviderId}
                options={[
                  { value: "", label: "未选择" },
                  ...enabledProviders.map((provider) => ({
                    value: provider.id,
                    label: `${provider.name} · ${provider.default_model}`,
                  })),
                ]}
              />
            </Field>
          </div>
          <Field label="自然语言任务" className="mt-3">
            <textarea
              className="min-h-28 w-full resize-y rounded-md border bg-background p-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
            />
          </Field>
          <div className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="text-sm text-muted-foreground">
              {enabledProviders.length === 0
                ? "暂无可用模型供应商，请先创建并配置 API Key。"
                : "任务会使用所选供应商配置，不会把 API Key 写入任务记录。"}
            </div>
            <Button
              onClick={() => void submitTask()}
              disabled={
                !selectedHostId ||
                !selectedProviderId ||
                !prompt.trim() ||
                submittingTask
              }
            >
              {submittingTask ? (
                <RefreshCw className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Send className="size-4" aria-hidden="true" />
              )}
              提交
            </Button>
          </div>
        </div>
      </PageSection>

      <PageSection
        title="模型供应商"
        description="仅支持 OpenAI Responses 兼容配置。"
        contentClassName="space-y-4"
      >
        <Toolbar
          left={
            <Button onClick={openCreateProviderDialog}>
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
                <span className="sr-only">搜索模型供应商</span>
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="搜索名称、URL 或模型"
                  className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
                />
              </label>
              <Button
                variant="outline"
                size="icon"
                aria-label="刷新"
                disabled={loading}
                onClick={() => void load()}
              >
                <RefreshCw className="size-4" aria-hidden="true" />
              </Button>
            </div>
          }
        />
        <DataTable
          columns={providerColumns}
          rows={filteredProviders}
          actionsWidth="14rem"
          emptyText={loading ? "正在加载模型供应商..." : "暂无模型供应商"}
          renderActions={(row) => (
            <>
              <Button variant="outline" size="sm" onClick={() => setDetailProvider(row)}>
                <Eye className="size-4" aria-hidden="true" />
                详情
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={busyId === row.id}
                onClick={() => openEditProviderDialog(row)}
              >
                <Pencil className="size-4" aria-hidden="true" />
                编辑
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={busyId === row.id}
                onClick={() => void removeProvider(row)}
              >
                <Trash2 className="size-4" aria-hidden="true" />
                删除
              </Button>
            </>
          )}
        />
      </PageSection>

      <PageSection contentClassName="space-y-4">
        <Toolbar
          left={
            <div className="flex items-center gap-2 text-sm font-medium">
              <Bot className="size-4" aria-hidden="true" />
              最近 AgentRun 任务
            </div>
          }
        />
        <DataTable
          columns={taskColumns}
          rows={aiTasks}
          actions={[]}
          emptyText={loading ? "加载中" : "暂无任务"}
        />
      </PageSection>

      <ProviderDialog
        open={providerDialogOpen}
        form={providerForm}
        submitting={submittingProvider}
        onOpenChange={setProviderDialogOpen}
        onFormChange={setProviderForm}
        onSubmit={submitProvider}
      />
      <ProviderDetailDialog
        provider={detailProvider}
        onOpenChange={(open) => {
          if (!open) {
            setDetailProvider(null);
          }
        }}
      />
    </PageContainer>
  );
}

function StatusLine({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center justify-between rounded-lg border p-3">
      <span className="text-sm">{label}</span>
      <Badge variant="secondary">{value}</Badge>
    </div>
  );
}

function ProviderDialog({
  open,
  form,
  submitting,
  onOpenChange,
  onFormChange,
  onSubmit,
}: {
  open: boolean;
  form: ProviderFormState;
  submitting: boolean;
  onOpenChange: (open: boolean) => void;
  onFormChange: (form: ProviderFormState) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const editing = Boolean(form.id);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>{editing ? "编辑模型供应商" : "创建模型供应商"}</DialogTitle>
            <DialogDescription>
              当前仅支持 OpenAI Responses 兼容接口；编辑时 API Key 留空表示保留现有密钥。
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
            <Field label="默认模型">
              <input
                required
                value={form.defaultModel}
                onChange={(event) =>
                  onFormChange({ ...form, defaultModel: event.target.value })
                }
                className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </Field>
            <Field label="Base URL">
              <input
                required
                value={form.baseUrl}
                onChange={(event) => onFormChange({ ...form, baseUrl: event.target.value })}
                placeholder="https://api.openai.com/v1"
                className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </Field>
            <Field label="超时秒数">
              <input
                required
                type="number"
                min={1}
                value={form.timeoutSeconds}
                onChange={(event) =>
                  onFormChange({ ...form, timeoutSeconds: event.target.value })
                }
                className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </Field>
          </div>

          <Field label="API Key">
            <div className="relative">
              <KeyRound
                className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <input
                required={!editing}
                type="password"
                value={form.apiKey}
                onChange={(event) => onFormChange({ ...form, apiKey: event.target.value })}
                placeholder={editing ? "留空表示不替换" : "sk-..."}
                className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </div>
          </Field>

          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(event) =>
                onFormChange({ ...form, enabled: event.target.checked })
              }
              className="size-4 rounded border"
            />
            <span className="font-medium">启用此供应商</span>
          </label>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              取消
            </Button>
            <Button type="submit" disabled={submitting}>
              {submitting ? (
                <RefreshCw className="size-4 animate-spin" aria-hidden="true" />
              ) : null}
              {editing ? "保存" : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ProviderDetailDialog({
  provider,
  onOpenChange,
}: {
  provider: AiModelProvider | null;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={Boolean(provider)} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>模型供应商详情</DialogTitle>
          <DialogDescription>接口不会返回 API Key 明文。</DialogDescription>
        </DialogHeader>
        {provider ? (
          <div className="grid gap-3 text-sm sm:grid-cols-2">
            <DetailItem label="ID" value={provider.id} />
            <DetailItem label="名称" value={provider.name} />
            <DetailItem label="Base URL" value={provider.base_url} />
            <DetailItem label="默认模型" value={provider.default_model} />
            <DetailItem label="超时" value={`${provider.timeout_seconds}s`} />
            <DetailItem label="状态" value={provider.enabled ? "启用" : "停用"} />
            <DetailItem
              label="密钥"
              value={
                provider.has_api_key
                  ? `已配置 ${provider.api_key_hint ?? ""}`.trim()
                  : "未配置"
              }
            />
            <DetailItem label="创建时间" value={provider.created_at} />
            <DetailItem label="更新时间" value={provider.updated_at} />
          </div>
        ) : null}
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            关闭
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function DetailItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md border p-3">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="mt-1 truncate font-medium" title={value}>
        {value}
      </p>
    </div>
  );
}

function Field({
  label,
  children,
  className,
}: {
  label: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={`block space-y-2 text-sm ${className ?? ""}`}>
      <span className="font-medium">{label}</span>
      {children}
    </label>
  );
}

function providerCreateRequest(form: ProviderFormState): CreateAiModelProviderRequest {
  return {
    name: form.name.trim(),
    base_url: form.baseUrl.trim(),
    default_model: form.defaultModel.trim(),
    timeout_seconds: Number(form.timeoutSeconds),
    api_key: form.apiKey.trim(),
    enabled: form.enabled,
  };
}

function providerUpdateRequest(form: ProviderFormState): UpdateAiModelProviderRequest {
  return {
    name: form.name.trim(),
    base_url: form.baseUrl.trim(),
    default_model: form.defaultModel.trim(),
    timeout_seconds: Number(form.timeoutSeconds),
    api_key: form.apiKey.trim() || null,
    enabled: form.enabled,
  };
}
