CREATE TABLE IF NOT EXISTS ai_model_providers (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    default_model TEXT NOT NULL,
    timeout_seconds INTEGER NOT NULL DEFAULT 60,
    api_key_secret TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_model_providers_name_lower
    ON ai_model_providers (LOWER(name));
