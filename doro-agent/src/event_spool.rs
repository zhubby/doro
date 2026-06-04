use doro_protocol::grpc;
use prost::Message;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Default)]
pub(crate) struct EventSpoolMetrics {
    pub pending_files: usize,
    pub pending_bytes: u64,
    pub spooled_events: u64,
    pub drained_events: u64,
    pub dropped_events: u64,
    pub corrupt_events: u64,
    pub last_drain_status: String,
}

#[derive(Debug)]
pub(crate) struct EventSpool {
    enabled: bool,
    path: PathBuf,
    max_files: usize,
    max_bytes: u64,
    metrics: EventSpoolMetrics,
}

impl EventSpool {
    pub(crate) fn from_config(config: &doro_config::AgentReliabilityConfig) -> Self {
        let path = config
            .event_spool_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(default_spool_path);
        Self {
            enabled: config.event_spool_enabled,
            path,
            max_files: config.event_spool_max_files.max(1) as usize,
            max_bytes: config.event_spool_max_bytes.max(1024),
            metrics: EventSpoolMetrics {
                last_drain_status: "not_started".to_string(),
                ..EventSpoolMetrics::default()
            },
        }
    }

    pub(crate) fn spool(&mut self, event: &grpc::AgentEvent) {
        if !self.enabled || !should_spool(event) {
            return;
        }
        if let Err(error) = fs::create_dir_all(&self.path) {
            tracing::warn!(%error, path = %self.path.display(), "failed to create agent event spool");
            self.metrics.dropped_events += 1;
            return;
        }

        self.enforce_limits();
        let path = self.path.join(format!(
            "{}-{}.pb",
            event
                .recorded_at
                .as_ref()
                .map(|timestamp| timestamp.seconds)
                .unwrap_or_default(),
            event.event_id
        ));
        let mut bytes = Vec::new();
        if let Err(error) = event.encode(&mut bytes) {
            tracing::warn!(%error, event_id = event.event_id, "failed to encode agent event for spool");
            self.metrics.dropped_events += 1;
            return;
        }
        if bytes.len() as u64 > self.max_bytes {
            self.metrics.dropped_events += 1;
            return;
        }
        if let Err(error) = fs::write(&path, bytes) {
            tracing::warn!(%error, path = %path.display(), "failed to write agent event spool file");
            self.metrics.dropped_events += 1;
            return;
        }
        self.metrics.spooled_events += 1;
        self.refresh_metrics();
    }

    pub(crate) fn drain(&mut self, limit: usize) -> Vec<grpc::AgentEvent> {
        if !self.enabled {
            return Vec::new();
        }
        let mut drained = Vec::new();
        let mut paths = self.spool_files();
        while drained.len() < limit {
            let Some(path) = paths.pop_front() else {
                break;
            };
            match fs::read(&path)
                .ok()
                .and_then(|bytes| grpc::AgentEvent::decode(bytes.as_slice()).ok())
            {
                Some(event) => {
                    if fs::remove_file(&path).is_err() {
                        tracing::warn!(path = %path.display(), "failed to remove drained event spool file");
                    }
                    drained.push(event);
                    self.metrics.drained_events += 1;
                }
                None => {
                    let _ = fs::remove_file(&path);
                    self.metrics.corrupt_events += 1;
                }
            }
        }
        self.metrics.last_drain_status = if drained.is_empty() {
            "empty".to_string()
        } else {
            "drained".to_string()
        };
        self.refresh_metrics();
        drained
    }

    pub(crate) fn metrics(&self) -> EventSpoolMetrics {
        self.metrics.clone()
    }

    fn enforce_limits(&mut self) {
        let mut files = self.spool_files();
        while files.len() >= self.max_files {
            let Some(path) = files.pop_front() else {
                break;
            };
            if fs::remove_file(&path).is_ok() {
                self.metrics.dropped_events += 1;
            }
        }
        self.refresh_metrics();
        while self.metrics.pending_bytes > self.max_bytes {
            let mut files = self.spool_files();
            let Some(path) = files.pop_front() else {
                break;
            };
            if fs::remove_file(&path).is_ok() {
                self.metrics.dropped_events += 1;
            }
            self.refresh_metrics();
        }
    }

    fn refresh_metrics(&mut self) {
        let files = self.spool_files();
        self.metrics.pending_files = files.len();
        self.metrics.pending_bytes = files
            .iter()
            .filter_map(|path| fs::metadata(path).ok().map(|metadata| metadata.len()))
            .sum();
    }

    fn spool_files(&self) -> VecDeque<PathBuf> {
        let mut entries = fs::read_dir(&self.path)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(Result::ok))
            .filter_map(|entry| {
                let path = entry.path();
                let modified = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                path.extension()
                    .is_some_and(|extension| extension == "pb")
                    .then_some((modified, path))
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|(modified, path)| (*modified, path.clone()));
        entries.into_iter().map(|(_, path)| path).collect()
    }
}

fn should_spool(event: &grpc::AgentEvent) -> bool {
    !matches!(
        event.event,
        Some(grpc::agent_event::Event::LogLine(_))
            | Some(grpc::agent_event::Event::AgentChatTextDelta(_))
    )
}

fn default_spool_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join(".doro")
        .join("agent-event-spool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use doro_protocol::protobuf_timestamp_now;

    fn test_event(id: &str) -> grpc::AgentEvent {
        grpc::AgentEvent {
            event_id: id.to_string(),
            agent_id: "00000000-0000-0000-0000-000000000001".to_string(),
            host_id: "00000000-0000-0000-0000-000000000002".to_string(),
            recorded_at: Some(protobuf_timestamp_now()),
            event: Some(grpc::agent_event::Event::CommandResult(
                grpc::CommandResultEvent {
                    command_id: "command".to_string(),
                    status: grpc::CommandStatus::Succeeded as i32,
                    message: "ok".to_string(),
                },
            )),
        }
    }

    #[test]
    fn spools_and_drains_events() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let mut spool = EventSpool {
            enabled: true,
            path: temp.path().to_path_buf(),
            max_files: 8,
            max_bytes: 1024 * 1024,
            metrics: EventSpoolMetrics::default(),
        };

        spool.spool(&test_event("event-1"));
        let drained = spool.drain(16);

        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].event_id, "event-1");
        assert_eq!(spool.metrics().pending_files, 0);
        assert_eq!(spool.metrics().drained_events, 1);
        Ok(())
    }

    #[test]
    fn skips_corrupt_spool_files() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        fs::write(temp.path().join("bad.pb"), b"not protobuf")?;
        let mut spool = EventSpool {
            enabled: true,
            path: temp.path().to_path_buf(),
            max_files: 8,
            max_bytes: 1024 * 1024,
            metrics: EventSpoolMetrics::default(),
        };

        let drained = spool.drain(16);

        assert!(drained.is_empty());
        assert_eq!(spool.metrics().corrupt_events, 1);
        Ok(())
    }
}
