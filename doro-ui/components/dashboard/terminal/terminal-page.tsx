"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import { Plug, Server, Terminal as TerminalIcon, Unplug } from "lucide-react";
import { useTranslations } from "next-intl";

import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Button } from "@/components/ui/button";
import { getHosts, terminalSessionWebSocketUrl } from "@/lib/control-plane-api";
import type { Host } from "@/types/api";

const TERMINAL_COLS = 100;
const TERMINAL_ROWS = 28;

function fitTerminal(fitAddon: FitAddon | null) {
  try {
    fitAddon?.fit();
  } catch (error) {
    if (!(error instanceof TypeError)) {
      throw error;
    }
  }
}

function hostLabel(host: Host) {
  return host.display_name || host.hostname;
}

function hasShellCapability(host: Host) {
  return host.capabilities.some((capability) => capability.name === "shell_execute");
}

export function TerminalPage() {
  const t = useTranslations("dashboard.terminal");
  const tCommon = useTranslations("common");
  const terminalNode = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const [hosts, setHosts] = useState<Host[]>([]);
  const [selectedHostId, setSelectedHostId] = useState("");
  const [loading, setLoading] = useState(true);
  const [connected, setConnected] = useState(false);
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
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;
    if (terminalNode.current) {
      terminal.open(terminalNode.current);
      requestAnimationFrame(() => fitTerminal(fitAddon));
      terminal.writeln(t("chooseAgent"));
    }
    function handleResize() {
      fitTerminal(fitAddon);
      const socket = socketRef.current;
      if (socket?.readyState === WebSocket.OPEN) {
        socket.send(
          JSON.stringify({
            type: "resize",
            cols: terminal.cols,
            rows: terminal.rows,
          }),
        );
      }
    }
    window.addEventListener("resize", handleResize);
    const disposable = terminal.onData((data) => {
      const socket = socketRef.current;
      if (socket?.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: "input", data }));
      }
    });
    return () => {
      window.removeEventListener("resize", handleResize);
      disposable.dispose();
      socketRef.current?.close();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, [t]);

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
        setError(result.error ?? t("loadingAgentsFailed"));
      }
      setLoading(false);
    }
    loadHosts();
    return () => {
      cancelled = true;
    };
  }, [t]);

  const selectedHost = useMemo(
    () => hosts.find((host) => host.id === selectedHostId) ?? null,
    [hosts, selectedHostId],
  );
  const canRun =
    Boolean(selectedHost) && selectedHost?.status === "online" && !connected;

  async function handleConnect() {
    if (!selectedHost || connected) {
      return;
    }
    setError(null);
    terminalRef.current?.reset();
    terminalRef.current?.writeln(t("connecting"));
    fitTerminal(fitAddonRef.current);
    const cols = terminalRef.current?.cols ?? TERMINAL_COLS;
    const rows = terminalRef.current?.rows ?? TERMINAL_ROWS;
    const url = await terminalSessionWebSocketUrl(
      selectedHost.id,
      cols,
      rows,
    );
    if (!url) {
      setError(tCommon("errors.notSignedIn"));
      return;
    }
    const socket = new WebSocket(url);
    socketRef.current = socket;
    socket.onopen = () => {
      setConnected(true);
      terminalRef.current?.reset();
      terminalRef.current?.focus();
    };
    socket.onmessage = (event) => {
      terminalRef.current?.write(String(event.data));
    };
    socket.onerror = () => {
      const message = t("connectionFailed");
      setError(message);
      terminalRef.current?.writeln(`\r\n[${message}]`);
    };
    socket.onclose = () => {
      setConnected(false);
      socketRef.current = null;
      terminalRef.current?.writeln(`\r\n[${t("disconnected")}]`);
    };
  }

  function handleDisconnect() {
    socketRef.current?.close();
  }

  return (
    <PageContainer>
      <PageSection>
        <div className="grid gap-4 xl:grid-cols-[280px_minmax(0,1fr)]">
          <div className="space-y-4 rounded-lg border bg-card p-4">
            <div className="flex items-center gap-2 text-sm font-medium">
              <Server className="size-4" aria-hidden="true" />
              Agent
            </div>
            <select
              value={selectedHostId}
              onChange={(event) => {
                handleDisconnect();
                setSelectedHostId(event.target.value);
              }}
              className="h-10 w-full rounded-md border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
              disabled={loading || connected}
            >
              {hosts.length === 0 ? <option value="">{t("noAgents")}</option> : null}
              {hosts.map((host) => (
                <option key={host.id} value={host.id}>
                  {hostLabel(host)}
                </option>
              ))}
            </select>
            <div className="rounded-md border bg-background p-3 text-xs text-muted-foreground">
              <p className="font-medium text-foreground">
                {selectedHost ? hostLabel(selectedHost) : t("unselected")}
              </p>
              <p className="mt-1">
                {t("status", { value: selectedHost?.status ?? "unknown" })}
              </p>
              <p className="mt-1">
                {t("host", { value: selectedHost?.hostname ?? "-" })}
              </p>
              <p className="mt-1">
                {t("terminal", {
                  value: connected ? t("connected") : t("notConnected"),
                })}
              </p>
            </div>
            <Button
              type="button"
              className="w-full"
              variant={connected ? "outline" : "default"}
              disabled={!canRun && !connected}
              onClick={connected ? handleDisconnect : handleConnect}
            >
              {connected ? (
                <Unplug className="size-4" aria-hidden="true" />
              ) : (
                <Plug className="size-4" aria-hidden="true" />
              )}
              {connected ? tCommon("actions.disconnect") : tCommon("actions.connect")}
            </Button>
            {error ? <p className="text-sm text-destructive">{error}</p> : null}
          </div>

          <div className="min-w-0 overflow-hidden rounded-lg border bg-[#0b0f14]">
            <div className="flex h-11 items-center gap-2 border-b border-white/10 px-4 text-sm text-slate-200">
              <TerminalIcon className="size-4" aria-hidden="true" />
              <span className="truncate">{selectedHost ? hostLabel(selectedHost) : "Terminal"}</span>
            </div>
            <div ref={terminalNode} className="h-[520px] p-3" />
          </div>
        </div>
      </PageSection>
    </PageContainer>
  );
}
