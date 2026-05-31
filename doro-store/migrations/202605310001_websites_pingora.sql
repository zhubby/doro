ALTER TABLE websites
    ALTER COLUMN host_id DROP NOT NULL;

ALTER TABLE websites
    ADD COLUMN IF NOT EXISTS name TEXT;

UPDATE websites
SET name = primary_domain
WHERE name IS NULL OR name = '';

ALTER TABLE websites
    ALTER COLUMN name SET NOT NULL;

ALTER TABLE websites
    ADD COLUMN IF NOT EXISTS aliases JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'reverse_proxy',
    ADD COLUMN IF NOT EXISTS listen_port INTEGER NOT NULL DEFAULT 80,
    ADD COLUMN IF NOT EXISTS upstream_url TEXT NOT NULL DEFAULT 'http://127.0.0.1:80',
    ADD COLUMN IF NOT EXISTS notes TEXT,
    ADD COLUMN IF NOT EXISTS last_runtime_error TEXT,
    ADD COLUMN IF NOT EXISTS last_checked_at TIMESTAMPTZ;

UPDATE websites
SET protocol = LOWER(protocol)
WHERE protocol <> LOWER(protocol);

CREATE UNIQUE INDEX IF NOT EXISTS idx_websites_primary_domain_listen_port
    ON websites (LOWER(primary_domain), listen_port);
