ALTER TABLE approvals
ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;

UPDATE approvals
SET expires_at = requested_at + INTERVAL '24 hours'
WHERE expires_at IS NULL;

ALTER TABLE approvals
ALTER COLUMN expires_at SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_approvals_expires_at ON approvals(expires_at);
