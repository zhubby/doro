CREATE TABLE IF NOT EXISTS virtual_machines (
    id UUID PRIMARY KEY,
    host_id UUID NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    vm_ref TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    image TEXT NOT NULL,
    cpu_cores INTEGER NOT NULL,
    memory_mib INTEGER NOT NULL,
    disk_gb INTEGER NOT NULL,
    networks JSONB NOT NULL DEFAULT '[]'::jsonb,
    console JSONB,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ,
    observed_at TIMESTAMPTZ NOT NULL,
    UNIQUE(host_id, provider, vm_ref)
);
CREATE INDEX IF NOT EXISTS idx_virtual_machines_host_id ON virtual_machines(host_id);
CREATE INDEX IF NOT EXISTS idx_virtual_machines_observed_at ON virtual_machines(observed_at DESC);

CREATE TABLE IF NOT EXISTS virtual_machine_images (
    id UUID PRIMARY KEY,
    host_id UUID REFERENCES hosts(id) ON DELETE CASCADE,
    image_ref TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    os_family TEXT,
    architecture TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    UNIQUE(host_id, image_ref)
);

CREATE TABLE IF NOT EXISTS virtual_machine_templates (
    id UUID PRIMARY KEY,
    template_ref TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    image_ref TEXT NOT NULL,
    cpu_cores INTEGER NOT NULL,
    memory_mib INTEGER NOT NULL,
    disk_gb INTEGER NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS virtual_machine_snapshots (
    id UUID PRIMARY KEY,
    vm_id UUID NOT NULL REFERENCES virtual_machines(id) ON DELETE CASCADE,
    snapshot_ref TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE(vm_id, snapshot_ref)
);
