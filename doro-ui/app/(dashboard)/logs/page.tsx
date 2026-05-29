"use client";

import { useEffect, useMemo, useState } from "react";
import { ChevronDown, RefreshCw } from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  getAgentLogs,
  getControlPlaneLogs,
  getHosts,
  runtimeLogStreamUrl,
} from "@/lib/control-plane-api";
import type { Host, RuntimeLogEntry } from "@/types/api";

const LOG_LIMIT = 500;
const MAX_RENDERED_LOGS = 1000;

type StreamState = "connecting" | "connected" | "closed" | "error";

type LogViewerProps = {
  entries: RuntimeLogEntry[];
  state: StreamState;
  error: string | null;
  emptyText: string;
};

function appendLogs(current: RuntimeLogEntry[], incoming: RuntimeLogEntry[]) {
  const byId = new Map(current.map((entry) => [entry.id, entry]));
  for (const entry of incoming) {
    byId.set(entry.id, entry);
  }
  return Array.from(byId.values())
    .sort(
      (left, right) =>
        new Date(left.recorded_at).getTime() - new Date(right.recorded_at).getTime(),
    )
    .slice(-MAX_RENDERED_LOGS);
}

function streamLabel(state: StreamState) {
  if (state === "connected") {
    return "实时连接";
  }
  if (state === "connecting") {
    return "正在连接";
  }
  if (state === "error") {
    return "连接异常";
  }
  return "未连接";
}

function levelVariant(level: string) {
  const normalized = level.toLowerCase();
  if (normalized === "error") {
    return "destructive" as const;
  }
  if (normalized === "warn") {
    return "outline" as const;
  }
  return "secondary" as const;
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

function LogViewer({ entries, state, error, emptyText }: LogViewerProps) {
  return (
    <div className="flex min-h-[520px] flex-col overflow-hidden rounded-md border bg-background">
      <div className="flex items-center justify-between border-b px-4 py-3">
        <div className="text-sm text-muted-foreground">
          {entries.length} 条日志
        </div>
        <Badge variant={state === "connected" ? "secondary" : "outline"}>
          {streamLabel(state)}
        </Badge>
      </div>
      {error ? (
        <div className="border-b border-destructive/30 px-4 py-2 text-sm text-muted-foreground">
          {error}
        </div>
      ) : null}
      <div className="min-h-0 flex-1 overflow-auto bg-zinc-950 p-3 text-zinc-100">
        {entries.length === 0 ? (
          <div className="flex h-full items-center justify-center text-sm text-zinc-400">
            {emptyText}
          </div>
        ) : (
          <div className="space-y-1 font-mono text-xs leading-5">
            {entries.map((entry) => (
              <div
                className="grid grid-cols-[72px_72px_minmax(120px,220px)_1fr] gap-3 rounded px-2 py-1 hover:bg-white/5"
                key={entry.id}
              >
                <span className="text-zinc-400">{formatTime(entry.recorded_at)}</span>
                <Badge
                  className="h-5 justify-center px-1.5 font-mono uppercase"
                  variant={levelVariant(entry.level)}
                >
                  {entry.level}
                </Badge>
                <span className="truncate text-zinc-400" title={entry.target}>
                  {entry.target}
                </span>
                <span className="min-w-0 break-words text-zinc-100">
                  {entry.message || JSON.stringify(entry.fields)}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default function LogsRoute() {
  const [tab, setTab] = useState("control-plane");
  const [hosts, setHosts] = useState<Host[]>([]);
  const [selectedHostId, setSelectedHostId] = useState<string>("");
  const [controlPlaneLogs, setControlPlaneLogs] = useState<RuntimeLogEntry[]>([]);
  const [agentLogs, setAgentLogs] = useState<RuntimeLogEntry[]>([]);
  const [controlPlaneState, setControlPlaneState] =
    useState<StreamState>("connecting");
  const [agentState, setAgentState] = useState<StreamState>("closed");
  const [error, setError] = useState<string | null>(null);

  const selectedHost = useMemo(
    () => hosts.find((host) => host.id === selectedHostId) ?? null,
    [hosts, selectedHostId],
  );

  useEffect(() => {
    let cancelled = false;
    async function loadHosts() {
      const result = await getHosts();
      if (cancelled) {
        return;
      }
      if (result.error) {
        setError(result.error);
        return;
      }
      const items = result.data?.items ?? [];
      setHosts(items);
      setSelectedHostId((current) => current || items[0]?.id || "");
    }
    void loadHosts();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let eventSource: EventSource | null = null;
    setControlPlaneState("connecting");

    async function connect() {
      const initial = await getControlPlaneLogs(LOG_LIMIT);
      if (cancelled) {
        return;
      }
      if (initial.error) {
        setError(initial.error);
      } else {
        setControlPlaneLogs(initial.data?.items ?? []);
      }

      const url = await runtimeLogStreamUrl("control_plane");
      if (cancelled) {
        return;
      }
      if (!url) {
        setControlPlaneState("error");
        setError("未登录，无法连接控制平面日志流");
        return;
      }

      eventSource = new EventSource(url);
      eventSource.onopen = () => setControlPlaneState("connected");
      eventSource.onerror = () => setControlPlaneState("error");
      eventSource.addEventListener("runtime_log", (event) => {
        const entry = JSON.parse((event as MessageEvent).data) as RuntimeLogEntry;
        setControlPlaneLogs((current) => appendLogs(current, [entry]));
      });
    }

    void connect();
    return () => {
      cancelled = true;
      eventSource?.close();
      setControlPlaneState("closed");
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let eventSource: EventSource | null = null;
    setAgentLogs([]);

    if (!selectedHostId) {
      setAgentState("closed");
      return () => {
        cancelled = true;
      };
    }

    setAgentState("connecting");

    async function connect() {
      const initial = await getAgentLogs(selectedHostId, LOG_LIMIT);
      if (cancelled) {
        return;
      }
      if (initial.error) {
        setError(initial.error);
      } else {
        setAgentLogs(initial.data?.items ?? []);
      }

      const url = await runtimeLogStreamUrl("agent", selectedHostId);
      if (cancelled) {
        return;
      }
      if (!url) {
        setAgentState("error");
        setError("未登录，无法连接 Agent 日志流");
        return;
      }

      eventSource = new EventSource(url);
      eventSource.onopen = () => setAgentState("connected");
      eventSource.onerror = () => setAgentState("error");
      eventSource.addEventListener("runtime_log", (event) => {
        const entry = JSON.parse((event as MessageEvent).data) as RuntimeLogEntry;
        setAgentLogs((current) => appendLogs(current, [entry]));
      });
    }

    void connect();
    return () => {
      cancelled = true;
      eventSource?.close();
      setAgentState("closed");
    };
  }, [selectedHostId]);

  return (
    <PageContainer>
      <PageSection
        title="日志"
        description="控制平面与 Agent 的实时程序日志。"
      >
        <Tabs value={tab} onValueChange={setTab}>
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <TabsList>
              <TabsTrigger value="control-plane">控制平面日志</TabsTrigger>
              <TabsTrigger value="agent">Agent 日志</TabsTrigger>
            </TabsList>
            {tab === "agent" ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline">
                    {selectedHost?.display_name ?? "选择 Agent"}
                    <ChevronDown />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-64">
                  <DropdownMenuLabel>Agent</DropdownMenuLabel>
                  <DropdownMenuRadioGroup
                    value={selectedHostId}
                    onValueChange={setSelectedHostId}
                  >
                    {hosts.map((host) => (
                      <DropdownMenuRadioItem key={host.id} value={host.id}>
                        {host.display_name}
                      </DropdownMenuRadioItem>
                    ))}
                  </DropdownMenuRadioGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            ) : (
              <Button size="sm" variant="outline" onClick={() => window.location.reload()}>
                <RefreshCw />
                刷新
              </Button>
            )}
          </div>
          <TabsContent value="control-plane">
            <LogViewer
              entries={controlPlaneLogs}
              state={controlPlaneState}
              error={error}
              emptyText="暂无控制平面日志"
            />
          </TabsContent>
          <TabsContent value="agent">
            <LogViewer
              entries={agentLogs}
              state={agentState}
              error={error}
              emptyText={selectedHostId ? "暂无 Agent 日志" : "请选择 Agent"}
            />
          </TabsContent>
        </Tabs>
      </PageSection>
    </PageContainer>
  );
}
