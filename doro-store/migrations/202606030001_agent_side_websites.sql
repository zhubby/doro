DROP INDEX IF EXISTS idx_websites_primary_domain_listen_port;

CREATE UNIQUE INDEX IF NOT EXISTS idx_websites_host_primary_domain_listen_port
    ON websites (host_id, LOWER(primary_domain), listen_port)
    WHERE host_id IS NOT NULL;
