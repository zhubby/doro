"use client";

import { useEffect, useMemo, useState } from "react";
import { Bot, Play, RefreshCw } from "lucide-react";

import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { createTask, getHosts, getTasks } from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import type { Host, Task } from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";

function taskStatusBadge(status: Task["status"]) {
  if (status === "succeeded") {
    return <Badge className="min-w-16 justify-center">成功</Badge>;
  }
  if (status === "running" || status === "queued") {
    return <Badge variant="secondary" className="min-w-16 justify-center">运行中</Badge>;
  }
  if (status === "waiting_approval") {
    return <Badge variant="secondary" className="min-w-16 justify-center">待审批</Badge>;
  }
  if (status === "failed" || status === "cancelled") {
    return <Badge variant="destructive" className="min-w-16 justify-center">失败</Badge>;
  }
  return <Badge variant="outline" className="min-w-16 justify-center">草稿</Badge>;
}

function hostLabel(host: Host) {
  const labels = host.labels.length ? ` · ${host.labels.join(", ")}` : "";
  return `${host.display_name || host.hostname}${labels}`;
}

function isAiTask(task: Task) {
  return task.steps.some((step) => step.capability === "agent_run");
}

export function AiPage() {
  const [hosts, setHosts] = useState<Host[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [selectedHostId, setSelectedHostId] = useState("");
  const [title, setTitle] = useState("AI 运维任务");
  const [prompt, setPrompt] = useState("");
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onlineAgentHosts = useMemo(
    () =>
      hosts.filter(
        (host) =>
          host.status === "online" &&
          host.capabilities.some((capability) => capability.name === "agent_run"),
      ),
    [hosts],
  );
  const aiTasks = useMemo(() => tasks.filter(isAiTask).slice(0, 12), [tasks]);

  async function load() {
    setLoading(true);
    const [hostsResult, tasksResult] = await Promise.all([getHosts(), getTasks()]);
    if (hostsResult.data) {
      setHosts(hostsResult.data.items);
      const firstHost = hostsResult.data.items.find(
        (host) =>
          host.status === "online" &&
          host.capabilities.some((capability) => capability.name === "agent_run"),
      );
      setSelectedHostId((current) => current || firstHost?.id || "");
    }
    if (tasksResult.data) {
      setTasks(tasksResult.data.items);
    }
    setError(hostsResult.error ?? tasksResult.error);
    setLoading(false);
  }

  useEffect(() => {
    void load();
  }, []);

  async function submitTask() {
    const normalizedPrompt = prompt.trim();
    if (!selectedHostId || !normalizedPrompt || submitting) {
      return;
    }
    setSubmitting(true);
    setError(null);
    const result = await createTask({
      title: title.trim() || "AI 运维任务",
      host_id: selectedHostId,
      prompt: normalizedPrompt,
    });
    if (result.data) {
      setTasks((current) => [result.data as Task, ...current]);
      setPrompt("");
    } else {
      setError(result.error ?? "任务创建失败");
    }
    setSubmitting(false);
  }

  const columns: ResourceColumn<Task>[] = [
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
      label: "Host",
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
        <PageSection title="运行状态">
          <div className="space-y-3">
            <div className="flex items-center justify-between rounded-lg border p-3">
              <span className="text-sm">在线 Agent</span>
              <Badge variant="secondary">{onlineAgentHosts.length}</Badge>
            </div>
            <div className="flex items-center justify-between rounded-lg border p-3">
              <span className="text-sm">等待审批</span>
              <Badge variant="secondary">
                {aiTasks.filter((task) => task.status === "waiting_approval").length}
              </Badge>
            </div>
            <div className="flex items-center justify-between rounded-lg border p-3">
              <span className="text-sm">运行中</span>
              <Badge variant="secondary">
                {aiTasks.filter((task) => task.status === "running").length}
              </Badge>
            </div>
          </div>
        </PageSection>
      }
    >
      <PageSection
        title="AI 运维"
        toolbar={
          <Button variant="outline" onClick={() => void load()} disabled={loading}>
            <RefreshCw className="size-4" aria-hidden="true" />
            刷新
          </Button>
        }
      >
        <div className="rounded-lg border p-4">
          <div className="grid gap-3 md:grid-cols-[1fr_16rem]">
            <label className="grid gap-1.5 text-sm">
              <span className="font-medium">任务标题</span>
              <input
                className="h-10 rounded-md border bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                value={title}
                onChange={(event) => setTitle(event.target.value)}
              />
            </label>
            <label className="grid gap-1.5 text-sm">
              <span className="font-medium">目标 Host</span>
              <select
                className="h-10 rounded-md border bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                value={selectedHostId}
                onChange={(event) => setSelectedHostId(event.target.value)}
              >
                <option value="">未选择</option>
                {onlineAgentHosts.map((host) => (
                  <option key={host.id} value={host.id}>
                    {hostLabel(host)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <label className="mt-3 grid gap-1.5 text-sm">
            <span className="font-medium">自然语言任务</span>
            <textarea
              className="min-h-28 resize-y rounded-md border bg-background p-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              value={prompt}
              onChange={(event) => setPrompt(event.target.value)}
            />
          </label>
          <div className="mt-3 flex items-center justify-between gap-3">
            <div className="text-sm text-destructive">{error}</div>
            <Button
              onClick={() => void submitTask()}
              disabled={!selectedHostId || !prompt.trim() || submitting}
            >
              {submitting ? (
                <RefreshCw className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Play className="size-4" aria-hidden="true" />
              )}
              提交
            </Button>
          </div>
        </div>
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
          columns={columns}
          rows={aiTasks}
          actions={[]}
          emptyText={loading ? "加载中" : "暂无任务"}
        />
      </PageSection>
    </PageContainer>
  );
}
