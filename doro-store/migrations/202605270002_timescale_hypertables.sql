CREATE EXTENSION IF NOT EXISTS timescaledb;

SELECT create_hypertable(
    'metric_snapshots',
    'captured_at',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);
SELECT create_hypertable(
    'agent_events',
    'recorded_at',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

SELECT add_retention_policy(
    'metric_snapshots',
    INTERVAL '30 days',
    if_not_exists => TRUE
);
SELECT add_retention_policy(
    'agent_events',
    INTERVAL '30 days',
    if_not_exists => TRUE
);
