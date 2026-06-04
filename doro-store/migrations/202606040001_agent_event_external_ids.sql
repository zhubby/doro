ALTER TABLE agent_events
    ADD COLUMN IF NOT EXISTS external_event_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_events_external_event_id
    ON agent_events(recorded_at, agent_id, host_id, external_event_id)
    WHERE external_event_id IS NOT NULL;
