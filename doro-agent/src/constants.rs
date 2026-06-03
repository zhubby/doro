use std::time::Duration;

pub(crate) const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(2);
pub(crate) const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
pub(crate) const AGENT_RUNTIME_LOG_LIMIT: usize = 200;
pub(crate) const MAX_FILE_TRANSFER_BYTES: usize = 64 * 1024 * 1024;
