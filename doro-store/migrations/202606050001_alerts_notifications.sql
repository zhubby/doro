CREATE TABLE IF NOT EXISTS alert_rules (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    severity TEXT NOT NULL,
    metric_source TEXT NOT NULL,
    metric_key TEXT NOT NULL,
    operator TEXT NOT NULL,
    threshold REAL NOT NULL,
    host_id UUID REFERENCES hosts(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    for_seconds INTEGER NOT NULL DEFAULT 60,
    cooldown_seconds INTEGER NOT NULL DEFAULT 600,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_alert_rules_enabled ON alert_rules(enabled);
CREATE INDEX IF NOT EXISTS idx_alert_rules_host_id ON alert_rules(host_id);

CREATE TABLE IF NOT EXISTS alert_rule_states (
    id UUID PRIMARY KEY,
    alert_rule_id UUID NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    state TEXT NOT NULL,
    first_matched_at TIMESTAMPTZ,
    last_matched_at TIMESTAMPTZ,
    last_fired_at TIMESTAMPTZ,
    active_incident_id UUID,
    last_resolved_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE(alert_rule_id, host_id)
);
CREATE INDEX IF NOT EXISTS idx_alert_rule_states_rule_host ON alert_rule_states(alert_rule_id, host_id);
CREATE INDEX IF NOT EXISTS idx_alert_rule_states_active_incident ON alert_rule_states(active_incident_id);

CREATE TABLE IF NOT EXISTS alert_incidents (
    id UUID PRIMARY KEY,
    alert_rule_id UUID NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    rule_name TEXT NOT NULL,
    severity TEXT NOT NULL,
    metric_source TEXT NOT NULL,
    metric_key TEXT NOT NULL,
    operator TEXT NOT NULL,
    threshold REAL NOT NULL,
    observed_value REAL NOT NULL,
    status TEXT NOT NULL,
    triggered_at TIMESTAMPTZ NOT NULL,
    resolved_at TIMESTAMPTZ,
    last_observed_at TIMESTAMPTZ NOT NULL,
    notification_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_alert_incidents_status_triggered ON alert_incidents(status, triggered_at DESC);
CREATE INDEX IF NOT EXISTS idx_alert_incidents_host_triggered ON alert_incidents(host_id, triggered_at DESC);
CREATE INDEX IF NOT EXISTS idx_alert_incidents_rule_triggered ON alert_incidents(alert_rule_id, triggered_at DESC);

ALTER TABLE alert_rule_states
    ADD CONSTRAINT fk_alert_rule_states_active_incident
    FOREIGN KEY (active_incident_id)
    REFERENCES alert_incidents(id)
    ON DELETE SET NULL;

CREATE TABLE IF NOT EXISTS alert_notifications (
    id UUID PRIMARY KEY,
    alert_incident_id UUID REFERENCES alert_incidents(id) ON DELETE SET NULL,
    alert_rule_id UUID REFERENCES alert_rules(id) ON DELETE SET NULL,
    channel TEXT NOT NULL,
    status TEXT NOT NULL,
    recipient TEXT NOT NULL,
    subject TEXT NOT NULL,
    error_message TEXT,
    sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_alert_notifications_incident_created ON alert_notifications(alert_incident_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_alert_notifications_status_created ON alert_notifications(status, created_at DESC);
