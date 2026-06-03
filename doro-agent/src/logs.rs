use crate::constants::AGENT_RUNTIME_LOG_LIMIT;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use tokio::sync::broadcast;
use uuid::Uuid;

static AGENT_RUNTIME_LOGS: OnceLock<AgentRuntimeLogHub> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AgentRuntimeLog {
    pub id: Uuid,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: Value,
}

#[derive(Debug, Clone)]
struct AgentRuntimeLogHub {
    entries: Arc<StdMutex<VecDeque<AgentRuntimeLog>>>,
    sender: broadcast::Sender<AgentRuntimeLog>,
}

impl Default for AgentRuntimeLogHub {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(512);
        Self {
            entries: Arc::new(StdMutex::new(VecDeque::new())),
            sender,
        }
    }
}

impl AgentRuntimeLogHub {
    fn push(&self, entry: AgentRuntimeLog) {
        {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            entries.push_back(entry.clone());
            while entries.len() > AGENT_RUNTIME_LOG_LIMIT {
                entries.pop_front();
            }
        }
        let _ = self.sender.send(entry);
    }

    fn snapshot(&self) -> Vec<AgentRuntimeLog> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    fn subscribe(&self) -> broadcast::Receiver<AgentRuntimeLog> {
        self.sender.subscribe()
    }
}

pub fn init_runtime_log_capture() {
    let _ = AGENT_RUNTIME_LOGS.set(AgentRuntimeLogHub::default());
}

pub fn publish_runtime_log(
    level: impl Into<String>,
    target: impl Into<String>,
    message: impl Into<String>,
    fields: Value,
) {
    let Some(hub) = AGENT_RUNTIME_LOGS.get() else {
        return;
    };
    hub.push(AgentRuntimeLog {
        id: Uuid::new_v4(),
        level: level.into(),
        target: target.into(),
        message: message.into(),
        fields,
    });
}

pub(crate) struct RuntimeLogSubscription {
    pub(crate) snapshot: Vec<AgentRuntimeLog>,
    pub(crate) receiver: broadcast::Receiver<AgentRuntimeLog>,
}

pub(crate) fn runtime_log_subscription() -> Option<RuntimeLogSubscription> {
    AGENT_RUNTIME_LOGS.get().map(|hub| RuntimeLogSubscription {
        snapshot: hub.snapshot(),
        receiver: hub.subscribe(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_log_hub_keeps_bounded_tail() {
        let hub = AgentRuntimeLogHub::default();
        for index in 0..250 {
            hub.push(AgentRuntimeLog {
                id: Uuid::new_v4(),
                level: "INFO".to_string(),
                target: "doro_agent".to_string(),
                message: format!("line {index}"),
                fields: serde_json::json!({}),
            });
        }

        let snapshot = hub.snapshot();
        assert_eq!(snapshot.len(), AGENT_RUNTIME_LOG_LIMIT);
        assert_eq!(
            snapshot.first().map(|entry| entry.message.as_str()),
            Some("line 50")
        );
        assert_eq!(
            snapshot.last().map(|entry| entry.message.as_str()),
            Some("line 249")
        );
    }
}
