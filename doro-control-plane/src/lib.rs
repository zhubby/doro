use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::response::Sse;
use axum::response::sse::Event;
use axum::routing::get;
use chrono::Utc;
use doro_ai::AiPlanRequest;
use doro_ai::DeterministicPlanner;
use doro_ai::PlanProvider;
use doro_protocol::AgentCapability;
use doro_protocol::AgentEvent;
use doro_protocol::ApprovalRequest;
use doro_protocol::CapabilityName;
use doro_protocol::CapabilityRisk;
use doro_protocol::Host;
use doro_protocol::HostStatus;
use doro_protocol::MetricSnapshot;
use doro_protocol::Task;
use doro_protocol::TaskStatus;
use futures_util::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_stream::wrappers::IntervalStream;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct AppState {
    hosts: Arc<RwLock<Vec<Host>>>,
    tasks: Arc<RwLock<Vec<Task>>>,
    approvals: Arc<RwLock<Vec<ApprovalRequest>>>,
}

pub fn app() -> Router {
    let state = AppState::seeded();

    Router::new()
        .route("/health", get(health))
        .route("/api/v1/hosts", get(list_hosts))
        .route("/api/v1/tasks", get(list_tasks).post(create_task))
        .route("/api/v1/approvals", get(list_approvals))
        .route("/api/v1/apps", get(list_apps))
        .route("/api/v1/settings", get(settings))
        .route("/api/v1/events", get(events))
        .route("/api/v1/agent/connect", get(agent_connect))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

impl AppState {
    fn seeded() -> Self {
        let host_id = Uuid::new_v4();
        Self {
            hosts: Arc::new(RwLock::new(vec![Host {
                id: host_id,
                hostname: "doro-local".to_string(),
                labels: vec!["control-plane".to_string(), "development".to_string()],
                status: HostStatus::Online,
                last_seen_at: Some(Utc::now()),
                capabilities: default_capabilities(),
            }])),
            tasks: Arc::new(RwLock::new(Vec::new())),
            approvals: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "doro-control-plane" }))
}

async fn list_hosts(State(state): State<AppState>) -> Json<Vec<Host>> {
    Json(state.hosts.read().await.clone())
}

async fn list_tasks(State(state): State<AppState>) -> Json<Vec<Task>> {
    Json(state.tasks.read().await.clone())
}

#[derive(Debug, Deserialize)]
struct CreateTaskRequest {
    title: String,
    host_id: Option<Uuid>,
    prompt: Option<String>,
}

async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Json<Task> {
    let planner = DeterministicPlanner;
    let plan = match request.prompt {
        Some(prompt) => planner.plan(AiPlanRequest { prompt }).ok(),
        None => None,
    };
    let steps = plan.map(|plan| plan.steps).unwrap_or_default();
    let status = if steps.iter().any(|step| step.risk >= CapabilityRisk::High) {
        TaskStatus::WaitingApproval
    } else {
        TaskStatus::Queued
    };
    let task = Task {
        id: Uuid::new_v4(),
        host_id: request.host_id,
        title: request.title,
        status,
        created_at: Utc::now(),
        steps,
    };

    state.tasks.write().await.push(task.clone());
    Json(task)
}

async fn list_approvals(State(state): State<AppState>) -> Json<Vec<ApprovalRequest>> {
    Json(state.approvals.read().await.clone())
}

async fn list_apps() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "items": [
            { "id": "openresty", "name": "OpenResty", "category": "website", "status": "planned" },
            { "id": "mysql", "name": "MySQL", "category": "database", "status": "planned" },
            { "id": "redis", "name": "Redis", "category": "database", "status": "planned" }
        ]
    }))
}

async fn settings() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "approval_policy": "policy_and_human_approval",
        "agent_transport": "websocket_json",
        "database": "sqlite"
    }))
}

async fn events() -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let stream = IntervalStream::new(tokio::time::interval(Duration::from_secs(15))).map(|_| {
        Ok(Event::default().event("heartbeat").data(
            serde_json::json!({
                "type": "control_plane_heartbeat",
                "at": Utc::now(),
            })
            .to_string(),
        ))
    });
    Sse::new(stream)
}

async fn agent_connect(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_agent_socket)
}

async fn handle_agent_socket(mut socket: WebSocket) {
    let welcome = AgentEvent::Heartbeat {
        host_id: Uuid::nil(),
        at: Utc::now(),
    };
    if let Ok(body) = serde_json::to_string(&welcome) {
        let _ = socket.send(Message::Text(body)).await;
    }
}

fn default_capabilities() -> Vec<AgentCapability> {
    vec![
        AgentCapability {
            name: CapabilityName::MetricsRead,
            risk: CapabilityRisk::Low,
            description: "Read CPU, memory, disk, and load metrics".to_string(),
        },
        AgentCapability {
            name: CapabilityName::LogsRead,
            risk: CapabilityRisk::Low,
            description: "Read service and task logs".to_string(),
        },
        AgentCapability {
            name: CapabilityName::ShellExecute,
            risk: CapabilityRisk::High,
            description: "Execute shell commands with approval".to_string(),
        },
    ]
}

pub fn example_metric(host_id: Uuid) -> MetricSnapshot {
    MetricSnapshot {
        host_id,
        captured_at: Utc::now(),
        cpu_percent: 0.0,
        memory_percent: 0.0,
        disk_percent: 0.0,
        load_average: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn router_builds() {
        let _router = app();
    }
}
