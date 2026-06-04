use doro_protocol::grpc;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::task::{AbortHandle, JoinHandle};

#[derive(Debug, Clone, Default)]
pub(crate) struct CommandRegistry {
    inner: Arc<Mutex<CommandRegistryInner>>,
}

#[derive(Debug, Default)]
struct CommandRegistryInner {
    running: HashMap<String, RunningCommand>,
    cancel_count: u64,
}

#[derive(Debug)]
struct RunningCommand {
    kind: String,
    abort: AbortHandle,
    cancel_signal: Option<CommandCancellationSignal>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CommandRegistryMetrics {
    pub pending_commands: usize,
    pub cancel_count: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct CommandCancellationSignal {
    cancelled: Arc<AtomicBool>,
}

impl CommandCancellationSignal {
    pub(crate) fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CancelResult {
    Cancelled { kind: String },
    NotFound,
}

#[derive(Debug)]
pub(crate) struct CancellationEvents {
    pub target_event: Option<grpc::AgentEvent>,
    pub cancel_event: grpc::AgentEvent,
}

impl CommandRegistry {
    pub(crate) async fn track_spawn<F>(
        &self,
        command_id: String,
        kind: impl Into<String>,
        future: F,
    ) where
        F: Future<Output = ()> + Send + 'static,
    {
        self.track_spawn_with_cancellation(command_id, kind, None, future)
            .await;
    }

    pub(crate) async fn track_spawn_with_cancellation<F>(
        &self,
        command_id: String,
        kind: impl Into<String>,
        cancel_signal: Option<CommandCancellationSignal>,
        future: F,
    ) where
        F: Future<Output = ()> + Send + 'static,
    {
        let kind = kind.into();
        let handle = tokio::spawn(future);
        let abort = handle.abort_handle();
        self.inner.lock().await.running.insert(
            command_id.clone(),
            RunningCommand {
                kind,
                abort,
                cancel_signal,
            },
        );
        spawn_completion_cleanup(self.clone(), command_id, handle);
    }

    pub(crate) async fn complete(&self, command_id: &str) {
        self.inner.lock().await.running.remove(command_id);
    }

    pub(crate) async fn cancel(&self, command_id: &str) -> CancelResult {
        let running = self.inner.lock().await.running.remove(command_id);
        match running {
            Some(running) => {
                if let Some(cancel_signal) = &running.cancel_signal {
                    cancel_signal.cancel();
                }
                running.abort.abort();
                let mut inner = self.inner.lock().await;
                inner.cancel_count += 1;
                CancelResult::Cancelled { kind: running.kind }
            }
            None => CancelResult::NotFound,
        }
    }

    #[cfg(test)]
    pub(crate) async fn metrics(&self) -> CommandRegistryMetrics {
        let inner = self.inner.lock().await;
        CommandRegistryMetrics {
            pending_commands: inner.running.len(),
            cancel_count: inner.cancel_count,
        }
    }

    pub(crate) fn try_metrics(&self) -> Option<CommandRegistryMetrics> {
        let inner = self.inner.try_lock().ok()?;
        Some(CommandRegistryMetrics {
            pending_commands: inner.running.len(),
            cancel_count: inner.cancel_count,
        })
    }

    pub(crate) async fn cancellation_events(
        &self,
        agent: &crate::runtime::Agent,
        agent_id: uuid::Uuid,
        cancel_command_id: String,
        target_command_id: String,
        reason: String,
    ) -> CancellationEvents {
        let result = self.cancel(&target_command_id).await;
        let message = match &result {
            CancelResult::Cancelled { kind } => {
                if reason.trim().is_empty() {
                    format!("cancelled running {kind} command {target_command_id}")
                } else {
                    format!("cancelled running {kind} command {target_command_id}: {reason}")
                }
            }
            CancelResult::NotFound => {
                format!("command {target_command_id} is not running")
            }
        };
        match result {
            CancelResult::Cancelled { .. } => {
                let target_event = agent.command_result_event(
                    agent_id,
                    target_command_id,
                    grpc::CommandStatus::Cancelled,
                    message.clone(),
                );
                let cancel_event = agent.command_result_event(
                    agent_id,
                    cancel_command_id,
                    grpc::CommandStatus::Succeeded,
                    message,
                );
                CancellationEvents {
                    target_event: Some(target_event),
                    cancel_event,
                }
            }
            CancelResult::NotFound => CancellationEvents {
                target_event: None,
                cancel_event: agent.command_result_event(
                    agent_id,
                    cancel_command_id,
                    grpc::CommandStatus::Failed,
                    message,
                ),
            },
        }
    }
}

fn spawn_completion_cleanup(registry: CommandRegistry, command_id: String, handle: JoinHandle<()>) {
    tokio::spawn(async move {
        let _ = handle.await;
        registry.complete(&command_id).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Agent;
    use crate::test_support::test_agent_config;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    #[tokio::test]
    async fn tracks_and_cancels_running_command() {
        let registry = CommandRegistry::default();
        let (sender, receiver) = oneshot::channel::<()>();

        registry
            .track_spawn("command-1".to_string(), "test", async move {
                let _ = receiver.await;
            })
            .await;

        assert_eq!(registry.metrics().await.pending_commands, 1);
        assert_eq!(
            registry.cancel("command-1").await,
            CancelResult::Cancelled {
                kind: "test".to_string()
            }
        );
        let _ = sender.send(());
        assert_eq!(registry.metrics().await.pending_commands, 0);
        assert_eq!(registry.metrics().await.cancel_count, 1);
    }

    #[tokio::test]
    async fn cancellation_events_report_cancelled_target_command() {
        let registry = CommandRegistry::default();
        let (_sender, receiver) = oneshot::channel::<()>();
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(test_agent_config(agent_id));

        registry
            .track_spawn(
                "target-command".to_string(),
                "terminal_command",
                async move {
                    let _ = receiver.await;
                },
            )
            .await;

        let events = registry
            .cancellation_events(
                &agent,
                agent_id,
                "cancel-command".to_string(),
                "target-command".to_string(),
                "operator requested".to_string(),
            )
            .await;
        let target_event = match events.target_event.and_then(|event| event.event) {
            Some(grpc::agent_event::Event::CommandResult(result)) => result,
            other => panic!("expected target command result event, got {other:?}"),
        };
        assert_eq!(target_event.command_id, "target-command");
        assert_eq!(target_event.status, grpc::CommandStatus::Cancelled as i32);

        let cancel_event = match events.cancel_event.event {
            Some(grpc::agent_event::Event::CommandResult(result)) => result,
            other => panic!("expected cancel command result event, got {other:?}"),
        };
        assert_eq!(cancel_event.command_id, "cancel-command");
        assert_eq!(cancel_event.status, grpc::CommandStatus::Succeeded as i32);
        assert_eq!(registry.metrics().await.cancel_count, 1);
    }
}
