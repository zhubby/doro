ALTER TABLE cron_jobs
    ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'script',
    ADD COLUMN IF NOT EXISTS required_capability TEXT NOT NULL DEFAULT 'shell_execute',
    ADD COLUMN IF NOT EXISTS label_selector JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS next_run_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_run_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_run_status TEXT,
    ADD COLUMN IF NOT EXISTS approval_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS approved_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS approved_by TEXT;

CREATE INDEX IF NOT EXISTS idx_cron_jobs_status_next_run_at ON cron_jobs(status, next_run_at);
CREATE INDEX IF NOT EXISTS idx_cron_jobs_approval_task_id ON cron_jobs(approval_task_id);
CREATE INDEX IF NOT EXISTS idx_cron_job_runs_cron_job_id_started_at
    ON cron_job_runs(cron_job_id, started_at DESC);
