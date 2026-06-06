CREATE TABLE IF NOT EXISTS host_tags (
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (host_id, tag)
);

INSERT INTO host_tags (host_id, tag, created_at)
SELECT
    hosts.id,
    lower(trim(labels.value #>> '{}')) AS tag,
    hosts.created_at
FROM hosts
CROSS JOIN LATERAL jsonb_array_elements(hosts.labels) AS labels(value)
WHERE jsonb_typeof(labels.value) = 'string'
    AND trim(labels.value #>> '{}') <> ''
ON CONFLICT (host_id, tag) DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_host_tags_tag_host_id ON host_tags(tag, host_id);

ALTER TABLE hosts DROP COLUMN IF EXISTS labels;
