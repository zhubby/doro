"use client";

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, RefreshCw, Search, X } from "lucide-react";

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
import { useToastMessage } from "@/components/ui/use-toast-message";
import {
  getAgentLogs,
  getControlPlaneLogs,
  getHosts,
  runtimeLogStreamUrl,
} from "@/lib/control-plane-api";
import type { Host, RuntimeLogEntry } from "@/types/api";

const LOG_LIMIT = 500;
const MAX_RENDERED_LOGS = 1000;
const ALL_LEVELS = "all";
const KNOWN_LOG_LEVELS = ["error", "warn", "info", "debug", "trace"];

type StreamState = "connecting" | "connected" | "closed" | "error";

type LogViewerProps = {
  entries: RuntimeLogEntry[];
  totalCount: number;
  state: StreamState;
  emptyText: string;
  noMatchesText: string;
  hasActiveFilters: boolean;
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
  const normalized = normalizeLogLevel(level);
  if (normalized === "error") {
    return "destructive" as const;
  }
  if (normalized === "warn") {
    return "outline" as const;
  }
  return "secondary" as const;
}

function normalizeLogLevel(level: string) {
  const normalized = level.trim().toLowerCase();
  if (!normalized) {
    return "unknown";
  }
  if (normalized === "warning") {
    return "warn";
  }
  return normalized;
}

function logLevelLabel(level: string) {
  if (level === ALL_LEVELS) {
    return "全部";
  }
  return level.toUpperCase();
}

function searchableLogText(entry: RuntimeLogEntry) {
  return [
    entry.message,
    entry.target,
    JSON.stringify(entry.fields),
  ]
    .filter(Boolean)
    .join("\n")
    .toLowerCase();
}

function filterLogs(
  entries: RuntimeLogEntry[],
  levelFilter: string,
  searchQuery: string,
) {
  const normalizedLevelFilter = normalizeLogLevel(levelFilter);
  const normalizedQuery = searchQuery.trim().toLowerCase();

  if (normalizedLevelFilter === ALL_LEVELS && !normalizedQuery) {
    return entries;
  }

  return entries.filter((entry) => {
    if (
      normalizedLevelFilter !== ALL_LEVELS &&
      normalizeLogLevel(entry.level) !== normalizedLevelFilter
    ) {
      return false;
    }

    if (!normalizedQuery) {
      return true;
    }

    return searchableLogText(entry).includes(normalizedQuery);
  });
}

function levelCounts(entries: RuntimeLogEntry[]) {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    const level = normalizeLogLevel(entry.level);
    counts.set(level, (counts.get(level) ?? 0) + 1);
  }
  return counts;
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(value));
}

function LogViewer({
  entries,
  totalCount,
  state,
  emptyText,
  noMatchesText,
  hasActiveFilters,
}: LogViewerProps) {
  const frameRef = useRef<HTMLDivElement | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [height, setHeight] = useState<number | null>(null);

  useLayoutEffect(() => {
    function updateHeight() {
      const element = frameRef.current;
      if (!element) {
        return;
      }
      const viewportHeight = window.visualViewport?.height ?? window.innerHeight;
      const top = element.getBoundingClientRect().top;
      setHeight(Math.max(320, Math.floor(viewportHeight - top - 24)));
    }

    updateHeight();
    window.addEventListener("resize", updateHeight);
    window.visualViewport?.addEventListener("resize", updateHeight);

    const observer = new ResizeObserver(updateHeight);
    if (frameRef.current?.parentElement) {
      observer.observe(frameRef.current.parentElement);
    }

    return () => {
      window.removeEventListener("resize", updateHeight);
      window.visualViewport?.removeEventListener("resize", updateHeight);
      observer.disconnect();
    };
  }, []);

  useLayoutEffect(() => {
    const element = scrollRef.current;
    if (!element) {
      return;
    }

    function scrollToBottom() {
      const current = scrollRef.current;
      if (!current) {
        return;
      }
      current.scrollTop = current.scrollHeight;
    }

    scrollToBottom();
    const frame = window.requestAnimationFrame(scrollToBottom);

    return () => {
      window.cancelAnimationFrame(frame);
    };
  }, [entries, height]);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) {
      return;
    }
    element.scrollTop = element.scrollHeight;
  }, [entries.length]);

  const displayEmptyText =
    hasActiveFilters && totalCount > 0 ? noMatchesText : emptyText;

  return (
    <div
      className="flex min-h-0 flex-col overflow-hidden rounded-md border bg-background"
      ref={frameRef}
      style={height ? { height } : undefined}
    >
      <div className="flex items-center justify-between border-b px-4 py-3">
        <div className="text-sm text-muted-foreground">
          {entries.length === totalCount
            ? `${totalCount} 条日志`
            : `${entries.length} / ${totalCount} 条日志`}
        </div>
        <Badge variant={state === "connected" ? "secondary" : "outline"}>
          {streamLabel(state)}
        </Badge>
      </div>
      <div
        className="min-h-0 flex-1 overflow-y-auto overscroll-contain bg-background p-3 text-foreground dark:bg-zinc-950 dark:text-zinc-100"
        ref={scrollRef}
      >
        {entries.length === 0 ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground dark:text-zinc-400">
            {displayEmptyText}
          </div>
        ) : (
          <div className="space-y-1 font-mono text-xs leading-5">
            {entries.map((entry) => (
              <div
                className="grid grid-cols-[72px_72px_minmax(120px,220px)_1fr] gap-3 rounded px-2 py-1 hover:bg-muted/60 dark:hover:bg-white/5"
                key={entry.id}
              >
                <span className="text-muted-foreground dark:text-zinc-400">
                  {formatTime(entry.recorded_at)}
                </span>
                <Badge
                  className="h-5 justify-center px-1.5 font-mono uppercase"
                  variant={levelVariant(entry.level)}
                >
                  {entry.level || "unknown"}
                </Badge>
                <span
                  className="truncate text-muted-foreground dark:text-zinc-400"
                  title={entry.target}
                >
                  {entry.target}
                </span>
                <span className="min-w-0 break-words text-foreground dark:text-zinc-100">
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

type LogFiltersProps = {
  entries: RuntimeLogEntry[];
  levelFilter: string;
  searchQuery: string;
  onLevelFilterChange: (value: string) => void;
  onSearchQueryChange: (value: string) => void;
  onClear: () => void;
};

function LogFilters({
  entries,
  levelFilter,
  searchQuery,
  onLevelFilterChange,
  onSearchQueryChange,
  onClear,
}: LogFiltersProps) {
  const counts = useMemo(() => levelCounts(entries), [entries]);
  const normalizedLevelFilter = normalizeLogLevel(levelFilter);
  const dynamicLevels = Array.from(counts.keys())
    .filter((level) => !KNOWN_LOG_LEVELS.includes(level))
    .sort();
  const levels = [
    ALL_LEVELS,
    ...KNOWN_LOG_LEVELS,
    ...dynamicLevels,
    ...(normalizedLevelFilter !== ALL_LEVELS &&
    !KNOWN_LOG_LEVELS.includes(normalizedLevelFilter) &&
    !dynamicLevels.includes(normalizedLevelFilter)
      ? [normalizedLevelFilter]
      : []),
  ];
  const hasActiveFilters =
    normalizedLevelFilter !== ALL_LEVELS || searchQuery.trim().length > 0;

  return (
    <div className="flex flex-col gap-3 rounded-md border bg-background p-3">
      <div className="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
        <label className="relative min-w-0 lg:w-96">
          <Search
            className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <span className="sr-only">搜索日志</span>
          <input
            value={searchQuery}
            onChange={(event) => onSearchQueryChange(event.target.value)}
            placeholder="搜索消息、目标或结构化字段"
            className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
          />
        </label>
        {hasActiveFilters ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="w-full justify-center lg:w-auto"
            onClick={onClear}
          >
            <X className="size-4" aria-hidden="true" />
            清空筛选
          </Button>
        ) : null}
      </div>
      <div className="flex flex-wrap gap-2">
        {levels.map((level) => {
          const isActive = normalizedLevelFilter === level;
          const count = level === ALL_LEVELS ? entries.length : counts.get(level) ?? 0;

          return (
            <Button
              key={level}
              type="button"
              variant={isActive ? "default" : "outline"}
              size="sm"
              aria-pressed={isActive}
              onClick={() => onLevelFilterChange(level)}
            >
              {logLevelLabel(level)}
              <Badge
                variant={isActive ? "secondary" : "outline"}
                className="ml-1 font-mono"
              >
                {count}
              </Badge>
            </Button>
          );
        })}
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
  const [levelFilter, setLevelFilter] = useState(ALL_LEVELS);
  const [searchQuery, setSearchQuery] = useState("");
  const [controlPlaneState, setControlPlaneState] =
    useState<StreamState>("connecting");
  const [agentState, setAgentState] = useState<StreamState>("closed");
  const [error, setError] = useState<string | null>(null);

  useToastMessage(error, {
    id: "logs-error",
    kind: "error",
  });

  const selectedHost = useMemo(
    () => hosts.find((host) => host.id === selectedHostId) ?? null,
    [hosts, selectedHostId],
  );
  const activeLogs = tab === "agent" ? agentLogs : controlPlaneLogs;
  const hasActiveFilters =
    normalizeLogLevel(levelFilter) !== ALL_LEVELS || searchQuery.trim().length > 0;
  const filteredControlPlaneLogs = useMemo(
    () => filterLogs(controlPlaneLogs, levelFilter, searchQuery),
    [controlPlaneLogs, levelFilter, searchQuery],
  );
  const filteredAgentLogs = useMemo(
    () => filterLogs(agentLogs, levelFilter, searchQuery),
    [agentLogs, levelFilter, searchQuery],
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
    <div className="box-border min-h-0 flex-1 overflow-hidden p-6">
      <Tabs
        className="flex min-h-0 flex-col gap-3"
        value={tab}
        onValueChange={setTab}
      >
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
            <Button
              size="sm"
              variant="outline"
              onClick={() => window.location.reload()}
            >
              <RefreshCw />
              刷新
            </Button>
          )}
        </div>
        <LogFilters
          entries={activeLogs}
          levelFilter={levelFilter}
          searchQuery={searchQuery}
          onLevelFilterChange={setLevelFilter}
          onSearchQueryChange={setSearchQuery}
          onClear={() => {
            setLevelFilter(ALL_LEVELS);
            setSearchQuery("");
          }}
        />
        <TabsContent
          className="mt-0 min-h-0 overflow-hidden"
          value="control-plane"
        >
          <LogViewer
            entries={filteredControlPlaneLogs}
            totalCount={controlPlaneLogs.length}
            state={controlPlaneState}
            emptyText="暂无控制平面日志"
            noMatchesText="没有匹配的控制平面日志"
            hasActiveFilters={hasActiveFilters}
          />
        </TabsContent>
        <TabsContent
          className="mt-0 min-h-0 overflow-hidden"
          value="agent"
        >
          <LogViewer
            entries={filteredAgentLogs}
            totalCount={agentLogs.length}
            state={agentState}
            emptyText={selectedHostId ? "暂无 Agent 日志" : "请选择 Agent"}
            noMatchesText="没有匹配的 Agent 日志"
            hasActiveFilters={hasActiveFilters}
          />
        </TabsContent>
      </Tabs>
    </div>
  );
}
