"use client";

import {
  Bot,
  CheckCircle2,
  Clock,
  MessageSquarePlus,
  RefreshCw,
  Send,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import {
  type FormEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
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
  const [selectedHostId, setSelectedHostId] = useState("");
  const [selectedProviderId, setSelectedProviderId] = useState("");
  const [model, setModel] = useState("");
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(true);
  const [sending, setSending] = useState(false);
  const [streamingMessageId, setStreamingMessageId] = useState<string | null>(null);
  const [apiError, setApiError] = useState<string | null>(null);
  const queuesRef = useRef<Record<string, string>>({});
  const eventSourceRef = useRef<EventSource | null>(null);

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
  const selectedProvider = useMemo(
    () => providers.find((provider) => provider.id === selectedProviderId) ?? null,
    [providers, selectedProviderId],
  );
  const eventsByMessage = useMemo(() => {
    const grouped: Record<string, AiChatEvent[]> = {};
    for (const event of events) {
      grouped[event.message_id] = [...(grouped[event.message_id] ?? []), event];
    }
    return grouped;
  }, [events]);

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
      const items = hostsResult.data.items;
      setHosts(items);
      const firstHost = items.find(
        (host) =>
          host.status === "online" &&
          host.capabilities.some((capability) => capability.name === "agent_run"),
      );
      setSelectedHostId((current) => current || firstHost?.id || "");
    }
    if (providersResult.data) {
      const items = providersResult.data.items;
      setProviders(items);
      const firstProvider = items.find(
        (provider) => provider.enabled && provider.has_api_key,
      );
      setSelectedProviderId((current) => current || firstProvider?.id || "");
      setModel((current) => current || firstProvider?.default_model || "");
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
      return;
    }
    const result = await getAiConversation(conversationId);
    if (result.data) {
      setMessages(result.data.messages);
      setEvents(result.data.events);
      setDisplayedContent(
        Object.fromEntries(
          result.data.messages.map((message) => [message.id, message.content]),
        ),
      );
    } else {
      setApiError(result.error ?? "无法加载 AI 对话");
    }
  }

  useEffect(() => {
    void loadShell();
    return () => eventSourceRef.current?.close();
  }, []);

  useEffect(() => {
    void loadConversation(selectedConversationId);
  }, [selectedConversationId]);

  useEffect(() => {
    if (selectedProvider && !model.trim()) {
      setModel(selectedProvider.default_model);
    }
  }, [selectedProvider, model]);

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

  async function ensureConversation() {
    if (selectedConversationId) {
      return selectedConversationId;
    }
    const result = await createAiConversation({
      title: input.trim().slice(0, 32) || "新 AI 对话",
    });
    if (!result.data) {
      setApiError(result.error ?? "无法创建 AI 对话");
      return null;
    }
    const conversation = result.data.item;
    setConversations((current) => [conversation, ...current]);
    setSelectedConversationId(conversation.id);
    return conversation.id;
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
    if (!content || !selectedHostId || !selectedProviderId || !model.trim() || sending) {
      return;
    }
    setSending(true);
    setApiError(null);
    const conversationId = await ensureConversation();
    if (!conversationId) {
      setSending(false);
      return;
    }
    const result = await createAiChatTurn(conversationId, {
      host_id: selectedHostId,
      ai_provider_id: selectedProviderId,
      model: model.trim(),
      content,
    });
    const turn = result.data;
    if (turn) {
      setInput("");
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

  async function createConversation() {
    const result = await createAiConversation({ title: "新 AI 对话" });
    const created = result.data;
    if (created) {
      setConversations((current) => [created.item, ...current]);
      setSelectedConversationId(created.item.id);
      setMessages([]);
      setEvents([]);
      setDisplayedContent({});
    } else {
      setApiError(result.error ?? "无法创建 AI 对话");
    }
  }

  return (
    <div className="grid min-h-0 flex-1 overflow-hidden lg:grid-cols-[18rem_1fr]">
      <aside className="min-h-0 border-b bg-card lg:border-b-0 lg:border-r">
        <div className="flex h-full min-h-0 flex-col">
          <div className="flex h-16 items-center justify-between border-b px-4">
            <div>
              <p className="text-sm font-semibold">AI 对话</p>
              <p className="text-xs text-muted-foreground">持久化 Agent 聊天</p>
            </div>
            <Button
              variant="outline"
              size="icon"
              aria-label="新建对话"
              onClick={() => void createConversation()}
            >
              <MessageSquarePlus className="size-4" aria-hidden="true" />
            </Button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-3">
            <div className="grid gap-1">
              {conversations.map((conversation) => (
                <button
                  key={conversation.id}
                  type="button"
                  onClick={() => setSelectedConversationId(conversation.id)}
                  className={cn(
                    "rounded-md px-3 py-2 text-left text-sm outline-none transition-colors hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring",
                    selectedConversationId === conversation.id && "bg-accent font-medium",
                  )}
                >
                  <span className="block truncate">{conversationTitle(conversation)}</span>
                  <span className="block truncate text-xs text-muted-foreground">
                    {conversation.updated_at}
                  </span>
                </button>
              ))}
              {!conversations.length && !loading ? (
                <div className="rounded-md border p-3 text-sm text-muted-foreground">
                  暂无对话
                </div>
              ) : null}
            </div>
          </div>
        </div>
      </aside>

      <section className="flex min-h-0 flex-col overflow-hidden">
        <div className="grid gap-3 border-b bg-background p-4 xl:grid-cols-[1fr_14rem_18rem_16rem]">
          <div className="min-w-0">
            <p className="text-sm font-semibold">Agent AI</p>
            <p className="truncate text-xs text-muted-foreground">
              控制平面下发请求和模型，Agent 流式回传结果。
            </p>
          </div>
          <Select
            value={selectedHostId}
            onValueChange={setSelectedHostId}
            options={[
              { value: "", label: "未选择 Agent" },
              ...onlineAgentHosts.map((host) => ({
                value: host.id,
                label: hostLabel(host),
              })),
            ]}
          />
          <Select
            value={selectedProviderId}
            onValueChange={(value) => {
              setSelectedProviderId(value);
              const provider = providers.find((item) => item.id === value);
              if (provider) {
                setModel(provider.default_model);
              }
            }}
            options={[
              { value: "", label: "未选择模型供应商" },
              ...enabledProviders.map((provider) => ({
                value: provider.id,
                label: `${provider.name} · ${provider.default_model}`,
              })),
            ]}
          />
          <input
            value={model}
            onChange={(event) => setModel(event.target.value)}
            placeholder="模型名"
            className="h-10 rounded-md border bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
        </div>

        {apiError ? (
          <div className="border-b border-destructive/30 px-4 py-3 text-sm text-muted-foreground">
            控制平面暂不可用：{apiError}
          </div>
        ) : null}

        <div className="min-h-0 flex-1 overflow-y-auto bg-muted/20 p-4">
          <div className="mx-auto flex max-w-5xl flex-col gap-4">
            {messages.map((message) => (
              <article
                key={message.id}
                className={cn(
                  "max-w-[86%] rounded-lg border bg-background p-4 shadow-sm",
                  message.role === "user" && "ml-auto bg-primary text-primary-foreground",
                )}
              >
                <div className="mb-2 flex items-center gap-2 text-xs">
                  {message.role === "assistant" ? (
                    <Bot className="size-4" aria-hidden="true" />
                  ) : null}
                  <span className="font-medium">
                    {message.role === "assistant" ? "AI" : "用户"}
                  </span>
                  {message.role === "assistant" ? (
                    <Badge variant="outline" className="ml-auto">
                      {messageStatusLabel(message)}
                    </Badge>
                  ) : null}
                </div>
                <p className="whitespace-pre-wrap text-sm leading-6">
                  {displayedContent[message.id] || (message.role === "assistant" ? "" : message.content)}
                  {message.id === streamingMessageId ? (
                    <span className="ml-1 inline-block h-4 w-1 animate-pulse bg-current align-[-2px]" />
                  ) : null}
                </p>
                {eventsByMessage[message.id]?.length ? (
                  <div className="mt-3 grid gap-2">
                    {eventsByMessage[message.id].map((event) => (
                      <ChatEventLine key={event.id} event={event} />
                    ))}
                  </div>
                ) : null}
              </article>
            ))}
            {!messages.length ? (
              <div className="rounded-lg border bg-background p-6 text-sm text-muted-foreground">
                选择 Agent 和模型后发送第一条消息。Agent 可以使用本机工具，高风险操作会等待审批。
              </div>
            ) : null}
          </div>
        </div>

        <form onSubmit={handleSubmit} className="border-t bg-background p-4">
          <div className="mx-auto flex max-w-5xl items-end gap-3">
            <textarea
              value={input}
              onChange={(event) => setInput(event.target.value)}
              placeholder="输入要交给 Agent 的请求..."
              className="min-h-14 flex-1 resize-none rounded-md border bg-background p-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            <Button
              type="submit"
              disabled={
                !input.trim() ||
                !selectedHostId ||
                !selectedProviderId ||
                !model.trim() ||
                sending
              }
            >
              {sending ? (
                <RefreshCw className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Send className="size-4" aria-hidden="true" />
              )}
              发送
            </Button>
          </div>
        </form>
      </section>
    </div>
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

  return (
    <div className="flex items-start gap-2 rounded-md border bg-muted/40 p-2 text-xs text-muted-foreground">
      {icon}
      <div className="min-w-0 flex-1">
        <p className="font-medium text-foreground">{label}</p>
        <p className="truncate" title={event.content ?? ""}>
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
