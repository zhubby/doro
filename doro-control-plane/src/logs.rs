use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;

static CONTROL_PLANE_LOG_HUB: OnceLock<LogHub> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct LogHub {
    inner: Arc<StdMutex<LogHubInner>>,
    sender: broadcast::Sender<RuntimeLogEntry>,
}

#[derive(Debug, Default)]
pub(crate) struct LogHubInner {
    control_plane: VecDeque<RuntimeLogEntry>,
    agents: HashMap<Uuid, VecDeque<RuntimeLogEntry>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeLogScope {
    ControlPlane,
    Agent(Uuid),
}

impl Default for LogHub {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self {
            inner: Arc::new(StdMutex::new(LogHubInner::default())),
            sender,
        }
    }
}

impl LogHub {
    pub fn register_control_plane_global(&self) {
        let _ = CONTROL_PLANE_LOG_HUB.set(self.clone());
    }

    pub fn push(&self, entry: RuntimeLogEntry) {
        {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if entry.source == "control_plane" {
                push_limited(&mut inner.control_plane, entry.clone(), RUNTIME_LOG_LIMIT);
            } else if let Some(host_id) = entry.host_id {
                push_limited(
                    inner.agents.entry(host_id).or_default(),
                    entry.clone(),
                    RUNTIME_LOG_LIMIT,
                );
            }
        }
        let _ = self.sender.send(entry);
    }

    pub fn control_plane_tail(&self, limit: usize) -> Vec<RuntimeLogEntry> {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        tail(&inner.control_plane, limit)
    }

    pub fn agent_tail(&self, host_id: Uuid, limit: usize) -> Vec<RuntimeLogEntry> {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner
            .agents
            .get(&host_id)
            .map(|entries| tail(entries, limit))
            .unwrap_or_default()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeLogEntry> {
        self.sender.subscribe()
    }
}

pub(crate) fn push_limited(
    entries: &mut VecDeque<RuntimeLogEntry>,
    entry: RuntimeLogEntry,
    limit: usize,
) {
    entries.push_back(entry);
    while entries.len() > limit {
        entries.pop_front();
    }
}

pub(crate) fn tail(entries: &VecDeque<RuntimeLogEntry>, limit: usize) -> Vec<RuntimeLogEntry> {
    let start = entries.len().saturating_sub(limit);
    entries.iter().skip(start).cloned().collect()
}

pub fn publish_control_plane_runtime_log(
    level: impl Into<String>,
    target: impl Into<String>,
    message: impl Into<String>,
    fields: Value,
) {
    let Some(hub) = CONTROL_PLANE_LOG_HUB.get() else {
        return;
    };
    hub.push(RuntimeLogEntry {
        id: Uuid::new_v4(),
        source: "control_plane".to_string(),
        host_id: None,
        agent_id: None,
        level: level.into(),
        target: target.into(),
        message: message.into(),
        fields,
        recorded_at: Utc::now(),
    });
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeLogQuery {
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeLogStreamQuery {
    scope: String,
    host_id: Option<Uuid>,
    token: String,
}

pub(crate) async fn list_control_plane_logs(
    State(state): State<AppState>,
    Query(query): Query<RuntimeLogQuery>,
) -> Json<ListRuntimeLogsResponse> {
    Json(ListRuntimeLogsResponse {
        items: state.logs.control_plane_tail(
            query
                .limit
                .unwrap_or(RUNTIME_LOG_LIMIT)
                .min(RUNTIME_LOG_LIMIT),
        ),
    })
}

pub(crate) async fn list_agent_logs(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
    Query(query): Query<RuntimeLogQuery>,
) -> Json<ListRuntimeLogsResponse> {
    Json(ListRuntimeLogsResponse {
        items: state.logs.agent_tail(
            host_id,
            query
                .limit
                .unwrap_or(RUNTIME_LOG_LIMIT)
                .min(RUNTIME_LOG_LIMIT),
        ),
    })
}

pub(crate) async fn runtime_log_stream(
    State(state): State<AppState>,
    Query(query): Query<RuntimeLogStreamQuery>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, AppError> {
    state.auth.verify_access_token(&query.token)?;
    let scope = runtime_log_scope(&query)?;
    let receiver = state.logs.subscribe();
    let stream =
        futures_util::stream::unfold((receiver, scope), |(mut receiver, scope)| async move {
            loop {
                match receiver.recv().await {
                    Ok(entry) if runtime_log_matches(scope, &entry) => {
                        let event = match serde_json::to_string(&entry) {
                            Ok(data) => Event::default().event("runtime_log").data(data),
                            Err(_) => continue,
                        };
                        return Some((Ok(event), (receiver, scope)));
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

pub(crate) fn runtime_log_scope(
    query: &RuntimeLogStreamQuery,
) -> Result<RuntimeLogScope, AppError> {
    match query.scope.as_str() {
        "control_plane" => Ok(RuntimeLogScope::ControlPlane),
        "agent" => query
            .host_id
            .map(RuntimeLogScope::Agent)
            .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "host_id is required")),
        _ => Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "scope must be control_plane or agent",
        )),
    }
}

pub(crate) fn runtime_log_matches(scope: RuntimeLogScope, entry: &RuntimeLogEntry) -> bool {
    match scope {
        RuntimeLogScope::ControlPlane => entry.source == "control_plane",
        RuntimeLogScope::Agent(host_id) => {
            entry.source == "agent"
                && entry
                    .host_id
                    .is_some_and(|entry_host_id| entry_host_id == host_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_hub_keeps_control_plane_tail_in_insertion_order() {
        let hub = LogHub::default();
        for index in 0..600 {
            hub.push(RuntimeLogEntry {
                id: Uuid::new_v4(),
                source: "control_plane".to_string(),
                host_id: None,
                agent_id: None,
                level: "INFO".to_string(),
                target: "doro_control_plane".to_string(),
                message: format!("line {index}"),
                fields: serde_json::json!({}),
                recorded_at: Utc::now(),
            });
        }

        let entries = hub.control_plane_tail(500);
        assert_eq!(entries.len(), 500);
        assert_eq!(
            entries.first().map(|entry| entry.message.as_str()),
            Some("line 100")
        );
        assert_eq!(
            entries.last().map(|entry| entry.message.as_str()),
            Some("line 599")
        );
    }

    #[test]
    fn log_hub_keeps_agent_logs_by_host() {
        let hub = LogHub::default();
        let first_host = Uuid::new_v4();
        let second_host = Uuid::new_v4();
        hub.push(RuntimeLogEntry {
            id: Uuid::new_v4(),
            source: "agent".to_string(),
            host_id: Some(first_host),
            agent_id: Some(Uuid::new_v4()),
            level: "INFO".to_string(),
            target: "doro_agent".to_string(),
            message: "first".to_string(),
            fields: serde_json::json!({}),
            recorded_at: Utc::now(),
        });
        hub.push(RuntimeLogEntry {
            id: Uuid::new_v4(),
            source: "agent".to_string(),
            host_id: Some(second_host),
            agent_id: Some(Uuid::new_v4()),
            level: "INFO".to_string(),
            target: "doro_agent".to_string(),
            message: "second".to_string(),
            fields: serde_json::json!({}),
            recorded_at: Utc::now(),
        });

        let entries = hub.agent_tail(first_host, 500);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "first");
    }

    #[test]
    fn runtime_log_matches_filters_by_scope_and_host() {
        let host_id = Uuid::new_v4();
        let other_host_id = Uuid::new_v4();
        let entry = RuntimeLogEntry {
            id: Uuid::new_v4(),
            source: "agent".to_string(),
            host_id: Some(host_id),
            agent_id: Some(Uuid::new_v4()),
            level: "WARN".to_string(),
            target: "doro_agent".to_string(),
            message: "agent warning".to_string(),
            fields: serde_json::json!({}),
            recorded_at: Utc::now(),
        };

        assert!(runtime_log_matches(RuntimeLogScope::Agent(host_id), &entry));
        assert!(!runtime_log_matches(
            RuntimeLogScope::Agent(other_host_id),
            &entry
        ));
        assert!(!runtime_log_matches(RuntimeLogScope::ControlPlane, &entry));
    }
}
