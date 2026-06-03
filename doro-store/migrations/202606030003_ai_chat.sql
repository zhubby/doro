CREATE TABLE IF NOT EXISTS ai_conversations (
    id UUID PRIMARY KEY,
    title TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS ai_chat_messages (
    id UUID PRIMARY KEY,
    conversation_id UUID NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    status TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    host_id UUID REFERENCES hosts(id) ON DELETE SET NULL,
    ai_provider_id UUID REFERENCES ai_model_providers(id) ON DELETE SET NULL,
    model TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ai_chat_messages_conversation_created
    ON ai_chat_messages (conversation_id, created_at);

CREATE INDEX IF NOT EXISTS idx_ai_chat_messages_task
    ON ai_chat_messages (task_id);

CREATE TABLE IF NOT EXISTS ai_chat_events (
    id UUID PRIMARY KEY,
    conversation_id UUID NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES ai_chat_messages(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    content TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ai_chat_events_message_created
    ON ai_chat_events (message_id, created_at);
