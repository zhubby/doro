"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { Send, Server, Terminal as TerminalIcon } from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Button } from "@/components/ui/button";
import { getHosts, runTerminalCommand } from "@/lib/control-plane-api";
import type { Host } from "@/types/api";

const TERMINAL_COLS = 100;
const TERMINAL_ROWS = 28;

function hostLabel(host: Host) {
  return host.display_name || host.hostname;
}

function hasShellCapability(host: Host) {
  return host.capabilities.some((capability) => capability.name === "shell_execute");
}

export function TerminalPage() {
  const terminalNode = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<XTerm | null>(null);
  const [hosts, setHosts] = useState<Host[]>([]);
  const [selectedHostId, setSelectedHostId] = useState("");
  const [command, setCommand] = useState("");
  const [loading, setLoading] = useState(true);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const terminal = new XTerm({
      cols: TERMINAL_COLS,
      rows: TERMINAL_ROWS,
      cursorBlink: true,
      convertEol: true,
      fontFamily:
        'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
      fontSize: 13,
      theme: {
        background: "#0b0f14",
        foreground: "#d7dde8",
        cursor: "#d7dde8",
      },
    });
    terminalRef.current = terminal;
    if (terminalNode.current) {
      terminal.open(terminalNode.current);
      terminal.writeln("Doro terminal ready.");
    }
    return () => {
      terminal.dispose();
      terminalRef.current = null;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    async function loadHosts() {
      setLoading(true);
      const result = await getHosts();
      if (cancelled) {
        return;
      }
      if (result.data) {
        const shellHosts = result.data.items.filter(hasShellCapability);
        setHosts(shellHosts);
        setSelectedHostId((current) => current || shellHosts[0]?.id || "");
        setError(null);
      } else {
        setError(result.error ?? "无法加载 Agent");
      }
      setLoading(false);
    }
    loadHosts();
    return () => {
      cancelled = true;
    };
  }, []);

  const selectedHost = useMemo(
    () => hosts.find((host) => host.id === selectedHostId) ?? null,
    [hosts, selectedHostId],
  );
  const canRun =
    Boolean(selectedHost) && selectedHost?.status === "online" && command.trim().length > 0;

  async function handleRun() {
    if (!selectedHost || !canRun) {
      return;
    }
    const input = command.trim();
    setCommand("");
    setRunning(true);
    setError(null);
    terminalRef.current?.writeln(`\r\n$ ${input}`);

    const result = await runTerminalCommand({
      host_id: selectedHost.id,
      input,
      cols: TERMINAL_COLS,
      rows: TERMINAL_ROWS,
      timeout_seconds: 30,
    });

    if (result.data) {
      if (result.data.output.trim()) {
        terminalRef.current?.write(`${result.data.output.replace(/\n/g, "\r\n")}\r\n`);
      }
      const suffix =
        result.data.exit_code === null ? result.data.status : `${result.data.status}:${result.data.exit_code}`;
      terminalRef.current?.writeln(`[${suffix}]`);
    } else {
      const message = result.error ?? "终端命令执行失败";
      setError(message);
      terminalRef.current?.writeln(`[failed] ${message}`);
    }
    setRunning(false);
  }

  return (
    <PageContainer>
      <PageSection
        title="终端"
        description="选择在线 Agent，执行一次性命令并查看返回输出。"
      >
        <div className="grid gap-4 xl:grid-cols-[280px_minmax(0,1fr)]">
          <div className="space-y-4 rounded-lg border bg-card p-4">
            <div className="flex items-center gap-2 text-sm font-medium">
              <Server className="size-4" aria-hidden="true" />
              Agent
            </div>
            <select
              value={selectedHostId}
              onChange={(event) => setSelectedHostId(event.target.value)}
              className="h-10 w-full rounded-md border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
              disabled={loading || running}
            >
              {hosts.length === 0 ? <option value="">暂无可用 Agent</option> : null}
              {hosts.map((host) => (
                <option key={host.id} value={host.id}>
                  {hostLabel(host)}
                </option>
              ))}
            </select>
            <div className="rounded-md border bg-background p-3 text-xs text-muted-foreground">
              <p className="font-medium text-foreground">
                {selectedHost ? hostLabel(selectedHost) : "未选择 Agent"}
              </p>
              <p className="mt-1">状态：{selectedHost?.status ?? "unknown"}</p>
              <p className="mt-1">主机：{selectedHost?.hostname ?? "-"}</p>
            </div>
            {error ? <p className="text-sm text-destructive">{error}</p> : null}
          </div>

          <div className="min-w-0 overflow-hidden rounded-lg border bg-[#0b0f14]">
            <div className="flex h-11 items-center gap-2 border-b border-white/10 px-4 text-sm text-slate-200">
              <TerminalIcon className="size-4" aria-hidden="true" />
              <span className="truncate">{selectedHost ? hostLabel(selectedHost) : "Terminal"}</span>
            </div>
            <div ref={terminalNode} className="h-[520px] p-3" />
            <form
              className="flex gap-2 border-t border-white/10 bg-black/20 p-3"
              onSubmit={(event) => {
                event.preventDefault();
                handleRun();
              }}
            >
              <input
                value={command}
                onChange={(event) => setCommand(event.target.value)}
                className="h-10 min-w-0 flex-1 rounded-md border border-white/10 bg-black px-3 font-mono text-sm text-slate-100 outline-none focus:ring-2 focus:ring-primary"
                placeholder="输入命令"
                disabled={running || !selectedHost}
              />
              <Button type="submit" disabled={!canRun || running}>
                <Send className="size-4" aria-hidden="true" />
                {running ? "执行中" : "执行"}
              </Button>
            </form>
          </div>
        </div>
      </PageSection>
    </PageContainer>
  );
}
