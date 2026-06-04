ALTER TABLE ai_conversations
    ADD COLUMN IF NOT EXISTS host_id UUID REFERENCES hosts(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS ai_provider_id UUID REFERENCES ai_model_providers(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_ai_conversations_host
    ON ai_conversations (host_id);

CREATE INDEX IF NOT EXISTS idx_ai_conversations_provider
    ON ai_conversations (ai_provider_id);
