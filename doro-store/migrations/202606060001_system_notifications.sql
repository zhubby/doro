CREATE TABLE IF NOT EXISTS system_notifications (
    id UUID PRIMARY KEY,
    source TEXT NOT NULL,
    severity TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    link_url TEXT,
    alert_incident_id UUID REFERENCES alert_incidents(id) ON DELETE SET NULL,
    alert_rule_id UUID REFERENCES alert_rules(id) ON DELETE SET NULL,
    host_id UUID REFERENCES hosts(id) ON DELETE SET NULL,
    status TEXT NOT NULL,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_system_notifications_status_created ON system_notifications(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_system_notifications_alert_incident ON system_notifications(alert_incident_id);
