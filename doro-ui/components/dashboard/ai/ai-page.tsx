"use client";

import {
  AlertTriangle,
  Bot,
  CheckCircle2,
  Clock,
  History,
  KeyRound,
  MessageSquarePlus,
  RefreshCw,
  Send,
  Server,
  ShieldCheck,
  Sparkles,
  Wrench,
} from "lucide-react";
import {
  type FormEvent,
  type ReactNode,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Select } from "@/components/ui/select";
import { useToastMessage } from "@/components/ui/use-toast-message";
import { Link } from "@/i18n/navigation";
import {
  aiChatStreamUrl,
  createAiChatTurn,
  createAiConversation,
  getAiConversation,
  getAiConversations,
  getAiModelProviders,
  getHosts,
} from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import { cn } from "@/lib/utils";
import type {
  AiChatEvent,
  AiChatMessage,
  AiChatStreamEvent,
  AiConversation,
  AiModelProvider,
  Host,
} from "@/types/api";

type DisplayMap = Record<string, string>;

const promptSuggestions = [
  "检查当前主机的 CPU、内存、磁盘和网络状态，列出需要关注的异常。",
  "查看 Docker 容器运行情况，按风险和资源占用排序。",
  "检查最近的服务日志，汇总错误、告警和可执行的处理建议。",
];

function hostLabel(host: Host) {
  const labels = host.labels.length ? ` · ${host.labels.join(", ")}` : "";
  return `${host.display_name || host.hostname}${labels}`;
}

function conversationTitle(conversation: AiConversation) {
  return conversation.title || "新 AI 对话";
}

function messageStatusLabel(message: AiChatMessage) {
  if (message.status === "waiting_approval") {
    return "等待审批";
  }
  if (message.status === "running" || message.status === "pending") {
    return "生成中";
  }
  if (message.status === "failed") {
    return "失败";
  }
  return "完成";
}

function messageStatusTone(message: AiChatMessage) {
  if (message.status === "waiting_approval") {
    return "border-amber-500/35 bg-amber-500/10 text-amber-700 dark:text-amber-300";
  }
  if (message.status === "running" || message.status === "pending") {
    return "border-sky-500/35 bg-sky-500/10 text-sky-700 dark:text-sky-300";
  }
  if (message.status === "failed") {
    return "border-destructive/40 bg-destructive/10 text-destructive";
  }
  return "border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
}

function messageRoleLabel(role: AiChatMessage["role"]) {
  if (role === "assistant") {
    return "Doro Agent";
  }
  if (role === "tool") {
    return "工具";
  }
  return "用户";
}

function formatMessageTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function streamEventToChatEvent(event: AiChatStreamEvent): AiChatEvent {
  return {
    id: event.event_id,
    conversation_id: event.conversation_id,
    message_id: event.message_id,
    kind: event.kind,
    content: event.content,
    payload: event.payload,
    created_at: event.created_at,
  };
}

function jsonStringField(value: unknown, key: string) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const field = (value as Record<string, unknown>)[key];
  return typeof field === "string" ? field : null;
}

function eventApprovalId(event: AiChatEvent) {
  if (!event.payload || typeof event.payload !== "object" || Array.isArray(event.payload)) {
    return null;
  }
  const approval = (event.payload as Record<string, unknown>).approval;
  if (!approval || typeof approval !== "object" || Array.isArray(approval)) {
    return null;
  }
  const id = (approval as Record<string, unknown>).id;
  return typeof id === "string" ? id : null;
}

export function AiPage() {
  const [conversations, setConversations] = useState<AiConversation[]>([]);
  const [selectedConversationId, setSelectedConversationId] = useState("");
  const [messages, setMessages] = useState<AiChatMessage[]>([]);
  const [events, setEvents] = useState<AiChatEvent[]>([]);
  const [displayedContent, setDisplayedContent] = useState<DisplayMap>({});
  const [hosts, setHosts] = useState<Host[]>([]);
  const [providers, setProviders] = useState<AiModelProvider[]>([]);
  const [model, setModel] = useState("");
  const [input, setInput] = useState("");
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [draftTitle, setDraftTitle] = useState("");
  const [draftHostId, setDraftHostId] = useState("");
  const [draftProviderId, setDraftProviderId] = useState("");
  const [creatingConversation, setCreatingConversation] = useState(false);
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [streamingMessageId, setStreamingMessageId] = useState<string | null>(null);
  const [apiError, setApiError] = useState<string | null>(null);
  const queuesRef = useRef<Record<string, string>>({});
  const eventSourceRef = useRef<EventSource | null>(null);
  const messagesEndRef = useRef<HTMLDivElement | null>(null);
  const initialConversationRenderRef = useRef(false);

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
  const selectedConversation = useMemo(
    () =>
      conversations.find((conversation) => conversation.id === selectedConversationId) ??
      null,
    [conversations, selectedConversationId],
  );
  const selectedHost = useMemo(
    () =>
      selectedConversation?.host_id
        ? hosts.find((host) => host.id === selectedConversation.host_id) ?? null
        : null,
    [hosts, selectedConversation],
  );
  const selectedProvider = useMemo(
    () =>
      selectedConversation?.ai_provider_id
        ? providers.find((provider) => provider.id === selectedConversation.ai_provider_id) ??
          null
        : null,
    [providers, selectedConversation],
  );
  const selectedHostReady = Boolean(
    selectedHost &&
      selectedHost.status === "online" &&
      selectedHost.capabilities.some((capability) => capability.name === "agent_run"),
  );
  const selectedProviderReady = Boolean(
    selectedProvider?.enabled && selectedProvider.has_api_key,
  );
  const bindingReady = Boolean(
    selectedConversation?.host_id &&
      selectedConversation.ai_provider_id &&
      selectedHostReady &&
      selectedProviderReady,
  );
  const eventsByMessage = useMemo(() => {
    const grouped: Record<string, AiChatEvent[]> = {};
    for (const event of events) {
      grouped[event.message_id] = [...(grouped[event.message_id] ?? []), event];
    }
    return grouped;
  }, [events]);
  const latestAssistantMessage = useMemo(
    () => messages.findLast((message) => message.role === "assistant") ?? null,
    [messages],
  );
  const canCreateConversation = Boolean(draftHostId && draftProviderId && !creatingConversation);
  const canSend = Boolean(input.trim() && bindingReady && model.trim() && !sending);

  useToastMessage(apiError, {
    id: "ai-api-error",
    kind: "error",
  });

  async function loadShell() {
    setLoading(true);
    const [conversationsResult, hostsResult, providersResult] = await Promise.all([
      getAiConversations(),
      getHosts(),
      getAiModelProviders(),
    ]);
    if (conversationsResult.data) {
      const items = conversationsResult.data.items;
      setConversations(items);
      setSelectedConversationId((current) => current || items[0]?.id || "");
    }
    if (hostsResult.data) {
      setHosts(hostsResult.data.items);
    }
    if (providersResult.data) {
      setProviders(providersResult.data.items);
    }
    setApiError(
      conversationsResult.error ?? hostsResult.error ?? providersResult.error ?? null,
    );
    setLoading(false);
  }

  async function loadConversation(conversationId: string) {
    if (!conversationId) {
      setMessages([]);
      setEvents([]);
      setDisplayedContent({});
      setModel("");
      return;
    }
    const result = await getAiConversation(conversationId);
    if (result.data) {
      const response = result.data;
      setConversations((current) =>
        current.map((conversation) =>
          conversation.id === response.item.id ? response.item : conversation,
        ),
      );
      setMessages(response.messages);
      setEvents(response.events);
      setDisplayedContent(
        Object.fromEntries(
          response.messages.map((message) => [message.id, message.content]),
        ),
      );
      const latestModel =
        response.messages.findLast((message) => Boolean(message.model))?.model ?? "";
      setModel(latestModel);
    } else {
      setApiError(result.error ?? "无法加载 AI 对话");
    }
  }

  useEffect(() => {
    void loadShell();
    return () => eventSourceRef.current?.close();
  }, []);

  useEffect(() => {
    eventSourceRef.current?.close();
    setStreamingMessageId(null);
    setModel("");
    void loadConversation(selectedConversationId);
    initialConversationRenderRef.current = false;
  }, [selectedConversationId]);

  useEffect(() => {
    if (selectedProvider && !model.trim()) {
      setModel(selectedProvider.default_model);
    }
  }, [selectedProvider, model]);

  useEffect(() => {
    if (!createDialogOpen) {
      return;
    }
    setDraftHostId((current) => current || onlineAgentHosts[0]?.id || "");
    setDraftProviderId((current) => current || enabledProviders[0]?.id || "");
  }, [createDialogOpen, onlineAgentHosts, enabledProviders]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      const entries = Object.entries(queuesRef.current).filter(([, queue]) => queue);
      if (!entries.length) {
        return;
      }
      setDisplayedContent((current) => {
        const next = { ...current };
        for (const [messageId, queue] of entries) {
          const chunk = queue.slice(0, 3);
          queuesRef.current[messageId] = queue.slice(chunk.length);
          next[messageId] = `${next[messageId] ?? ""}${chunk}`;
        }
        return next;
      });
    }, 24);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!messages.length) {
      initialConversationRenderRef.current = false;
      return;
    }
    if (!streamingMessageId && !initialConversationRenderRef.current) {
      initialConversationRenderRef.current = true;
      return;
    }
    messagesEndRef.current?.scrollIntoView({ block: "end" });
  }, [displayedContent, messages.length, streamingMessageId]);

  function openCreateConversationDialog() {
    setDraftTitle("");
    setDraftHostId(onlineAgentHosts[0]?.id || "");
    setDraftProviderId(enabledProviders[0]?.id || "");
    setCreateDialogOpen(true);
  }

  async function submitCreateConversation(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draftHostId || !draftProviderId || creatingConversation) {
      return;
    }
    setCreatingConversation(true);
    setApiError(null);
    const provider = providers.find((item) => item.id === draftProviderId) ?? null;
    const result = await createAiConversation({
      title: draftTitle.trim() || null,
      host_id: draftHostId,
      ai_provider_id: draftProviderId,
    });
    const created = result.data;
    if (created) {
      setConversations((current) => [created.item, ...current]);
      setSelectedConversationId(created.item.id);
      setMessages([]);
      setEvents([]);
      setDisplayedContent({});
      setModel(provider?.default_model || "");
      setInput("");
      setCreateDialogOpen(false);
    } else {
      setApiError(result.error ?? "无法创建 AI 对话");
    }
    setCreatingConversation(false);
  }

  async function startStream(conversationId: string, messageId: string) {
    eventSourceRef.current?.close();
    const url = await aiChatStreamUrl(conversationId, messageId);
    if (!url) {
      setApiError("未登录，无法连接 AI 流");
      return;
    }
    setStreamingMessageId(messageId);
    const eventSource = new EventSource(url);
    eventSourceRef.current = eventSource;
    eventSource.onerror = () => {
      setStreamingMessageId(null);
      eventSource.close();
    };
    eventSource.addEventListener("ai_chat", (event) => {
      const streamEvent = JSON.parse((event as MessageEvent).data) as AiChatStreamEvent;
      if (streamEvent.kind === "text_delta" && streamEvent.content) {
        queuesRef.current[messageId] =
          `${queuesRef.current[messageId] ?? ""}${streamEvent.content}`;
        setMessages((current) =>
          current.map((message) =>
            message.id === messageId
              ? { ...message, content: `${message.content}${streamEvent.content}` }
              : message,
          ),
        );
        return;
      }

      setEvents((current) => [...current, streamEventToChatEvent(streamEvent)]);
      if (streamEvent.kind === "done" || streamEvent.kind === "error") {
        const queued = queuesRef.current[messageId] ?? "";
        queuesRef.current[messageId] = "";
        setStreamingMessageId(null);
        setDisplayedContent((current) => {
          return { ...current, [messageId]: `${current[messageId] ?? ""}${queued}` };
        });
        eventSource.close();
      }
    });
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const content = input.trim();
    if (!content || !selectedConversation || !bindingReady || !model.trim() || sending) {
      return;
    }
    setSending(true);
    setApiError(null);
    const conversationId = selectedConversation.id;
    const result = await createAiChatTurn(conversationId, {
      model: model.trim(),
      content,
    });
    const turn = result.data;
    if (turn) {
      setInput("");
      setConversations((current) =>
        current.map((conversation) =>
          conversation.id === conversationId
            ? { ...conversation, updated_at: new Date().toISOString() }
            : conversation,
        ),
      );
      setMessages((current) => [
        ...current,
        turn.user_message,
        turn.assistant_message,
      ]);
      setDisplayedContent((current) => ({
        ...current,
        [turn.user_message.id]: turn.user_message.content,
        [turn.assistant_message.id]: "",
      }));
      void startStream(conversationId, turn.assistant_message.id);
    } else {
      setApiError(result.error ?? "发送失败");
    }
    setSending(false);
  }

  return (
    <div className="grid min-h-0 flex-1 grid-rows-[13.5rem_minmax(0,1fr)] overflow-hidden bg-background lg:grid-cols-[19rem_minmax(0,1fr)] lg:grid-rows-1 2xl:grid-cols-[20rem_minmax(0,1fr)]">
      <aside className="min-h-0 overflow-hidden border-b bg-muted/25 lg:border-b-0 lg:border-r">
        <div className="flex h-full min-h-0 flex-col">
          <div className="flex h-16 items-center justify-between border-b bg-background/65 px-4 backdrop-blur">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <p className="text-sm font-semibold">AI 对话</p>
                <Badge variant="secondary" className="px-2 py-0 text-[11px]">
                  {conversations.length}
                </Badge>
              </div>
              <p className="truncate text-xs text-muted-foreground">
                Agent 回合与执行事件
              </p>
            </div>
            <Button
              variant="outline"
              size="icon"
              aria-label="新建对话"
              onClick={openCreateConversationDialog}
            >
              <MessageSquarePlus className="size-4" aria-hidden="true" />
            </Button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-3">
            <div className="grid gap-1.5">
              {conversations.map((conversation) => (
                <button
                  key={conversation.id}
                  type="button"
                  onClick={() => setSelectedConversationId(conversation.id)}
                  className={cn(
                    "group relative rounded-lg border border-transparent px-3 py-2.5 text-left text-sm outline-none transition-colors hover:border-border hover:bg-background/80 focus-visible:ring-2 focus-visible:ring-ring",
                    selectedConversationId === conversation.id &&
                      "border-primary/25 bg-primary/10 text-foreground shadow-sm",
                  )}
                >
                  <span
                    className={cn(
                      "absolute left-0 top-2.5 h-8 w-0.5 rounded-r-full bg-transparent transition-colors",
                      selectedConversationId === conversation.id && "bg-primary",
                    )}
                    aria-hidden="true"
                  />
                  <span className="block truncate font-medium">
                    {conversationTitle(conversation)}
                  </span>
                  <span className="mt-1 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                    <History className="size-3.5 shrink-0" aria-hidden="true" />
                    <span className="truncate">
                      {formatRelativeTime(conversation.updated_at)}
                    </span>
                  </span>
                </button>
              ))}
              {loading ? (
                <div className="rounded-lg border border-dashed bg-background/50 p-3 text-sm text-muted-foreground">
                  正在同步对话...
                </div>
              ) : null}
              {!conversations.length && !loading ? (
                <div className="rounded-lg border border-dashed bg-background/50 p-3 text-sm text-muted-foreground">
                  暂无对话
                </div>
              ) : null}
            </div>
          </div>
        </div>
      </aside>

      <section className="flex min-h-0 min-w-0 flex-col overflow-hidden">
        <ConversationHeader
          conversation={selectedConversation}
          host={selectedHost}
          provider={selectedProvider}
          latestAssistantMessage={latestAssistantMessage}
          onlineAgentCount={onlineAgentHosts.length}
          providerCount={enabledProviders.length}
          hostReady={selectedHostReady}
          providerReady={selectedProviderReady}
          onCreateConversation={openCreateConversationDialog}
        />

        <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden bg-muted/20 px-3 py-4 sm:px-4 xl:px-5 2xl:px-6">
          <div className="mx-auto flex w-full max-w-[102rem] min-w-0 flex-col gap-4">
            {!messages.length ? (
              <EmptyConversation
                hasConversation={Boolean(selectedConversation)}
                bindingReady={bindingReady}
                onCreateConversation={openCreateConversationDialog}
                onPickSuggestion={setInput}
              />
            ) : null}
            {messages.map((message) => (
              <ChatMessageCard
                key={message.id}
                message={message}
                content={
                  displayedContent[message.id] ||
                  (message.role === "assistant" ? "" : message.content)
                }
                events={eventsByMessage[message.id] ?? []}
                streaming={message.id === streamingMessageId}
              />
            ))}
            <div ref={messagesEndRef} aria-hidden="true" />
          </div>
        </div>

        <form onSubmit={handleSubmit} className="border-t bg-background/95 p-3 backdrop-blur sm:p-4">
          <div className="mx-auto w-full max-w-[102rem] rounded-lg border bg-card shadow-sm">
            <textarea
              value={input}
              onChange={(event) => setInput(event.target.value)}
              placeholder={
                selectedConversation ? "输入要交给 Agent 的请求..." : "先新建一个 AI 对话..."
              }
              className="max-h-40 min-h-20 w-full resize-none rounded-t-lg bg-transparent p-4 text-sm leading-6 outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
            />
            <div className="flex flex-col gap-3 border-t px-3 py-3 lg:flex-row lg:items-center lg:justify-between">
              <div className="flex min-w-0 flex-wrap gap-2 text-[11px] text-muted-foreground">
                <span className="inline-flex max-w-full items-center gap-1 rounded-md bg-muted px-2 py-1">
                  <Server className="size-3.5 shrink-0" aria-hidden="true" />
                  <span className="truncate">
                    {selectedHost ? hostLabel(selectedHost) : "未绑定 Agent"}
                  </span>
                </span>
                <span className="inline-flex max-w-full items-center gap-1 rounded-md bg-muted px-2 py-1">
                  <KeyRound className="size-3.5 shrink-0" aria-hidden="true" />
                  <span className="truncate">
                    {selectedProvider ? selectedProvider.name : "未绑定模型供应商"}
                  </span>
                </span>
              </div>
              <div className="flex min-w-0 flex-col gap-2 sm:flex-row sm:items-center sm:justify-end">
                <label className="flex h-9 min-w-0 items-center rounded-md border bg-background pl-3 text-sm focus-within:ring-2 focus-within:ring-ring sm:w-80">
                  <span className="shrink-0 text-xs font-medium text-muted-foreground">
                    模型
                  </span>
                  <input
                    value={model}
                    onChange={(event) => setModel(event.target.value)}
                    placeholder={selectedProvider?.default_model ?? "模型名"}
                    className="min-w-0 flex-1 bg-transparent px-2 text-sm outline-none placeholder:text-muted-foreground"
                  />
                </label>
                <Button type="submit" disabled={!canSend} className="sm:min-w-24">
                  {sending ? (
                    <RefreshCw className="size-4 animate-spin" aria-hidden="true" />
                  ) : (
                    <Send className="size-4" aria-hidden="true" />
                  )}
                  发送
                </Button>
              </div>
            </div>
          </div>
        </form>
      </section>

      <CreateConversationDialog
        open={createDialogOpen}
        title={draftTitle}
        hostId={draftHostId}
        providerId={draftProviderId}
        hosts={onlineAgentHosts}
        providers={enabledProviders}
        submitting={creatingConversation}
        canSubmit={canCreateConversation}
        onOpenChange={setCreateDialogOpen}
        onTitleChange={setDraftTitle}
        onHostChange={setDraftHostId}
        onProviderChange={setDraftProviderId}
        onSubmit={submitCreateConversation}
      />
    </div>
  );
}

function ConversationHeader({
  conversation,
  host,
  provider,
  latestAssistantMessage,
  onlineAgentCount,
  providerCount,
  hostReady,
  providerReady,
  onCreateConversation,
}: {
  conversation: AiConversation | null;
  host: Host | null;
  provider: AiModelProvider | null;
  latestAssistantMessage: AiChatMessage | null;
  onlineAgentCount: number;
  providerCount: number;
  hostReady: boolean;
  providerReady: boolean;
  onCreateConversation: () => void;
}) {
  const hasMissingBinding = Boolean(
    conversation && (!conversation.host_id || !conversation.ai_provider_id),
  );
  const statusText = latestAssistantMessage
    ? messageStatusLabel(latestAssistantMessage)
    : conversation
      ? "待命"
      : "未选择对话";

  return (
    <div className="border-b bg-background/95 px-4 py-3 backdrop-blur sm:px-6">
      <div className="flex min-w-0 flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <span className="flex size-8 shrink-0 items-center justify-center rounded-md border bg-card text-primary">
              <Sparkles className="size-4" aria-hidden="true" />
            </span>
            <div className="min-w-0">
              <p className="truncate text-sm font-semibold">
                {conversation ? conversationTitle(conversation) : "AI 对话"}
              </p>
              <p className="truncate text-xs text-muted-foreground">
                {conversation
                  ? `由 ${conversation.created_by} 创建 · ${formatRelativeTime(conversation.updated_at)} 更新`
                  : "选择一个对话，或创建新的 Agent 会话"}
              </p>
            </div>
          </div>
          <div className="mt-2 flex min-w-0 flex-wrap gap-2 text-[11px] text-muted-foreground">
            <MetadataPill
              icon={<Server className="size-3.5" aria-hidden="true" />}
              label={
                host
                  ? `${hostLabel(host)} · ${hostReady ? "在线" : "不可用"}`
                  : conversation?.host_id
                    ? "Agent 未载入"
                    : "缺少 Agent 绑定"
              }
              tone={hostReady ? "default" : "warning"}
            />
            <MetadataPill
              icon={<KeyRound className="size-3.5" aria-hidden="true" />}
              label={
                provider
                  ? `${provider.name} · ${provider.default_model}`
                  : conversation?.ai_provider_id
                    ? "模型供应商未载入"
                    : "缺少模型供应商绑定"
              }
              tone={providerReady ? "default" : "warning"}
            />
            <MetadataPill
              icon={<Clock className="size-3.5" aria-hidden="true" />}
              label={statusText}
              tone={latestAssistantMessage?.status === "failed" ? "danger" : "default"}
            />
            <MetadataPill label={`可用 Agent ${onlineAgentCount}`} />
            <MetadataPill label={`可用供应商 ${providerCount}`} />
          </div>
          {hasMissingBinding ? (
            <div className="mt-2 inline-flex items-center gap-1.5 rounded-md border border-amber-500/25 bg-amber-500/10 px-2 py-1 text-xs text-amber-700 dark:text-amber-300">
              <AlertTriangle className="size-3.5" aria-hidden="true" />
              旧对话缺少绑定，需新建对话后继续发送。
            </div>
          ) : null}
        </div>
        <Button variant="outline" onClick={onCreateConversation} className="xl:shrink-0">
          <MessageSquarePlus className="size-4" aria-hidden="true" />
          新建对话
        </Button>
      </div>
    </div>
  );
}

function MetadataPill({
  icon,
  label,
  tone = "default",
}: {
  icon?: ReactNode;
  label: string;
  tone?: "default" | "warning" | "danger";
}) {
  return (
    <span
      className={cn(
        "inline-flex max-w-full items-center gap-1 rounded-md border px-2 py-1",
        tone === "default" && "border-border bg-muted/40",
        tone === "warning" &&
          "border-amber-500/25 bg-amber-500/10 text-amber-700 dark:text-amber-300",
        tone === "danger" && "border-destructive/30 bg-destructive/10 text-destructive",
      )}
    >
      {icon ? <span className="shrink-0">{icon}</span> : null}
      <span className="truncate">{label}</span>
    </span>
  );
}

function CreateConversationDialog({
  open,
  title,
  hostId,
  providerId,
  hosts,
  providers,
  submitting,
  canSubmit,
  onOpenChange,
  onTitleChange,
  onHostChange,
  onProviderChange,
  onSubmit,
}: {
  open: boolean;
  title: string;
  hostId: string;
  providerId: string;
  hosts: Host[];
  providers: AiModelProvider[];
  submitting: boolean;
  canSubmit: boolean;
  onOpenChange: (open: boolean) => void;
  onTitleChange: (value: string) => void;
  onHostChange: (value: string) => void;
  onProviderChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <form onSubmit={onSubmit} className="grid gap-5">
          <DialogHeader>
            <DialogTitle>新建 AI 对话</DialogTitle>
            <DialogDescription>
              Agent 与模型供应商会绑定到这个对话。
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4">
            <label className="grid gap-1.5">
              <span className="text-sm font-medium">对话名称</span>
              <input
                value={title}
                onChange={(event) => onTitleChange(event.target.value)}
                placeholder="新 AI 对话"
                className="h-10 rounded-md border bg-background px-3 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
              />
            </label>

            <div className="grid gap-4 sm:grid-cols-2">
              <div className="grid gap-1.5">
                <p className="text-sm font-medium">Agent</p>
                <Select
                  value={hostId}
                  onValueChange={onHostChange}
                  className="h-10"
                  placeholder="选择在线 Agent"
                  options={[
                    { value: "", label: "选择在线 Agent", disabled: true },
                    ...hosts.map((host) => ({
                      value: host.id,
                      label: hostLabel(host),
                    })),
                  ]}
                />
                {!hosts.length ? (
                  <p className="text-xs text-amber-700 dark:text-amber-300">
                    当前没有可运行 Agent 回合的在线主机。
                  </p>
                ) : null}
              </div>

              <div className="grid gap-1.5">
                <p className="text-sm font-medium">模型供应商</p>
                <Select
                  value={providerId}
                  onValueChange={onProviderChange}
                  className="h-10"
                  placeholder="选择模型供应商"
                  options={[
                    { value: "", label: "选择模型供应商", disabled: true },
                    ...providers.map((provider) => ({
                      value: provider.id,
                      label: `${provider.name} · ${provider.default_model}`,
                    })),
                  ]}
                />
                {!providers.length ? (
                  <p className="text-xs text-amber-700 dark:text-amber-300">
                    当前没有已启用且配置 API Key 的供应商。
                  </p>
                ) : null}
              </div>
            </div>
          </div>

          <DialogFooter>
            <DialogClose asChild>
              <Button type="button" variant="outline">
                取消
              </Button>
            </DialogClose>
            <Button type="submit" disabled={!canSubmit}>
              {submitting ? (
                <RefreshCw className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <MessageSquarePlus className="size-4" aria-hidden="true" />
              )}
              创建
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function EmptyConversation({
  hasConversation,
  bindingReady,
  onCreateConversation,
  onPickSuggestion,
}: {
  hasConversation: boolean;
  bindingReady: boolean;
  onCreateConversation: () => void;
  onPickSuggestion: (value: string) => void;
}) {
  if (!hasConversation) {
    return (
      <div className="rounded-lg border border-dashed bg-card/70 p-5 shadow-sm">
        <div className="flex items-start gap-3">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <Bot className="size-5" aria-hidden="true" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-semibold">还没有选中对话</p>
            <p className="mt-1 text-sm text-muted-foreground">
              新对话需要先绑定 Agent 和模型供应商。
            </p>
            <Button onClick={onCreateConversation} className="mt-4">
              <MessageSquarePlus className="size-4" aria-hidden="true" />
              新建对话
            </Button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-dashed bg-card/70 p-5 shadow-sm">
      <div className="flex items-start gap-3">
        <span className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Bot className="size-5" aria-hidden="true" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold">
            {bindingReady ? "Agent 待命" : "对话绑定不可用"}
          </p>
          <p className="mt-1 text-sm text-muted-foreground">
            {bindingReady
              ? "当前回合会记录模型、主机、工具调用和审批事件。"
              : "请新建一个已绑定 Agent 和模型供应商的对话。"}
          </p>
          {bindingReady ? (
            <div className="mt-4 grid gap-2 md:grid-cols-3">
              {promptSuggestions.map((suggestion) => (
                <button
                  key={suggestion}
                  type="button"
                  onClick={() => onPickSuggestion(suggestion)}
                  className="rounded-md border bg-background px-3 py-2 text-left text-xs leading-5 text-muted-foreground transition-colors hover:border-primary/30 hover:bg-primary/5 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  {suggestion}
                </button>
              ))}
            </div>
          ) : (
            <Button onClick={onCreateConversation} className="mt-4">
              <MessageSquarePlus className="size-4" aria-hidden="true" />
              新建对话
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

function ChatMessageCard({
  message,
  content,
  events,
  streaming,
}: {
  message: AiChatMessage;
  content: string;
  events: AiChatEvent[];
  streaming: boolean;
}) {
  const isUser = message.role === "user";
  const isAssistant = message.role === "assistant";

  return (
    <article className={cn("flex w-full min-w-0 gap-3", isUser && "justify-end")}>
      {!isUser ? <MessageAvatar role={message.role} /> : null}
      <div
        className={cn(
          "min-w-0 overflow-hidden rounded-lg border p-4 shadow-sm",
          isUser
            ? "max-w-[min(60rem,calc(100%-3rem))] border-primary/20 bg-primary/10 text-foreground"
            : "w-full max-w-[min(94rem,calc(100%-3rem))] border-border/80 bg-card/95",
        )}
      >
        <div className="mb-2 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <span className="font-semibold text-foreground">
            {messageRoleLabel(message.role)}
          </span>
          <span>{formatMessageTime(message.created_at)}</span>
          {message.model ? (
            <span className="max-w-full truncate rounded bg-muted px-1.5 py-0.5">
              {message.model}
            </span>
          ) : null}
          {isAssistant ? (
            <Badge
              variant="outline"
              className={cn("ml-auto font-medium", messageStatusTone(message))}
            >
              {messageStatusLabel(message)}
            </Badge>
          ) : null}
        </div>
        <p className="whitespace-pre-wrap break-words text-sm leading-7 [overflow-wrap:anywhere]">
          {content}
          {streaming ? (
            <span className="ml-1 inline-block h-4 w-1 animate-pulse rounded-full bg-current align-[-2px]" />
          ) : null}
        </p>
        {events.length ? (
          <div className="mt-4 min-w-0 max-w-full border-t pt-3">
            <p className="mb-2 text-[11px] font-medium text-muted-foreground">
              执行记录
            </p>
            <div className="grid min-w-0 gap-2">
              {events.map((event) => (
                <ChatEventLine key={event.id} event={event} />
              ))}
            </div>
          </div>
        ) : null}
      </div>
      {isUser ? <MessageAvatar role={message.role} /> : null}
    </article>
  );
}

function MessageAvatar({ role }: { role: AiChatMessage["role"] }) {
  const isUser = role === "user";

  return (
    <span
      className={cn(
        "mt-1 flex size-9 shrink-0 items-center justify-center rounded-lg border",
        isUser
          ? "border-primary/25 bg-primary/10 text-primary"
          : "border-border bg-background text-muted-foreground",
      )}
      aria-hidden="true"
    >
      {isUser ? <span className="text-xs font-semibold">我</span> : <Bot className="size-4" />}
    </span>
  );
}

function ChatEventLine({ event }: { event: AiChatEvent }) {
  const approvalId = event.kind === "approval_required" ? eventApprovalId(event) : null;
  const icon =
    event.kind === "approval_required" ? (
      <ShieldCheck className="size-4" aria-hidden="true" />
    ) : event.kind === "tool_result" ? (
      <CheckCircle2 className="size-4" aria-hidden="true" />
    ) : event.kind === "done" ? (
      <Clock className="size-4" aria-hidden="true" />
    ) : (
      <Wrench className="size-4" aria-hidden="true" />
    );
  const label =
    event.kind === "approval_required"
      ? "等待审批"
      : event.kind === "tool_result"
        ? "工具结果"
        : event.kind === "done"
          ? "回合完成"
          : event.kind === "error"
            ? "错误"
            : "工具调用";

  const tone =
    event.kind === "approval_required"
      ? "border-amber-500/25 bg-amber-500/10 text-amber-700 dark:text-amber-300"
      : event.kind === "error"
        ? "border-destructive/30 bg-destructive/10 text-destructive"
        : "border-border bg-muted/45 text-muted-foreground";

  return (
    <div
      className={cn(
        "flex min-w-0 max-w-full items-start gap-2 overflow-hidden rounded-md border p-2 text-xs",
        tone,
      )}
    >
      <span className="mt-0.5 shrink-0">{icon}</span>
      <div className="min-w-0 flex-1">
        <p className="font-medium text-foreground">{label}</p>
        <p
          className="whitespace-pre-wrap break-words leading-5 [overflow-wrap:anywhere]"
          title={event.content ?? ""}
        >
          {event.content ?? jsonStringField(event.payload, "tool_name") ?? event.created_at}
        </p>
        {approvalId ? (
          <Button asChild variant="link" className="mt-1 h-auto p-0 text-xs">
            <Link href="/approvals">打开审批</Link>
          </Button>
        ) : null}
      </div>
    </div>
  );
}
