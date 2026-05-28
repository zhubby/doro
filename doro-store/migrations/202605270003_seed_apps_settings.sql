INSERT INTO apps (id, key, name, category, status, description, metadata)
VALUES
    ('00000000-0000-0000-0000-000000000101', 'mysql', 'MySQL', 'database', 'planned', 'Relational database service', '{}'::jsonb),
    ('00000000-0000-0000-0000-000000000102', 'openresty', 'OpenResty', 'website', 'planned', 'Web server and reverse proxy', '{}'::jsonb),
    ('00000000-0000-0000-0000-000000000103', 'redis', 'Redis', 'database', 'planned', 'In-memory data store', '{}'::jsonb)
ON CONFLICT (key) DO NOTHING;

INSERT INTO settings (key, value, description)
VALUES
    ('approval_policy', '"policy_and_human_approval"'::jsonb, 'Control-plane approval mode'),
    ('agent_transport', '"grpc_protobuf"'::jsonb, 'Agent transport protocol'),
    ('database', '"postgres"'::jsonb, 'Configured store backend')
ON CONFLICT (key) DO NOTHING;
