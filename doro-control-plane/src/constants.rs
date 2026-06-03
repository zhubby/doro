use std::time::Duration;

pub(crate) const CONTAINER_REFRESH_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const DEFAULT_APPROVAL_TTL_HOURS: i64 = 24;
pub(crate) const DEFAULT_TERMINAL_TIMEOUT_SECONDS: u32 = 30;
pub(crate) const MAX_TERMINAL_TIMEOUT_SECONDS: u32 = 120;
pub(crate) const RUNTIME_LOG_LIMIT: usize = 500;
pub(crate) const FILE_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const MAX_FILE_TRANSFER_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const DEFAULT_FILE_SEARCH_LIMIT: u32 = 500;
pub(crate) const SCHEDULED_TASK_TICK_SECONDS: u64 = 30;
pub(crate) const AGENT_TASK_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);
