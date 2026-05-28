ALTER TABLE hosts
    ADD COLUMN IF NOT EXISTS system_profile JSONB NOT NULL DEFAULT '{}'::jsonb;
