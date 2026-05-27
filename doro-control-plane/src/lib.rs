use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::response::Sse;
use axum::response::sse::Event;
use axum::routing::get;
use chrono::Utc;
use doro_ai::AiPlanRequest;
use doro_ai::DeterministicPlanner;
use doro_ai::PlanProvider;
use doro_protocol::AgentCapability;
use doro_protocol::ApprovalRequest;
use doro_protocol::CapabilityName;
use doro_protocol::CapabilityRisk;
use doro_protocol::Host;
use doro_protocol::HostStatus;
use doro_protocol::MetricSnapshot;
use doro_protocol::Task;
use doro_protocol::TaskStatus;
use doro_protocol::grpc;
use doro_protocol::grpc::agent_control_plane_server::AgentControlPlane;
use futures_util::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;
use tonic::Response;
use tonic::Status;
use tonic::Streaming;
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
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

#[derive(Debug, Default)]
pub struct GrpcAgentService;

#[tonic::async_trait]
impl AgentControlPlane for GrpcAgentService {
    type OpenAgentStreamStream = ReceiverStream<Result<grpc::ControlPlaneCommand, Status>>;

    async fn enroll(
        &self,
        request: Request<grpc::EnrollRequest>,
    ) -> Result<Response<grpc::EnrollResponse>, Status> {
        let request = request.into_inner();
        if request.enrollment_token.trim().is_empty() {
            return Err(Status::invalid_argument("enrollment token is required"));
        }

        Ok(Response::new(grpc::EnrollResponse {
            agent_id: Uuid::new_v4().to_string(),
            host_id: Uuid::new_v4().to_string(),
            control_plane_id: "doro-control-plane-local".to_string(),
        }))
    }

    async fn report_heartbeat(
        &self,
        request: Request<grpc::HeartbeatRequest>,
    ) -> Result<Response<grpc::HeartbeatResponse>, Status> {
        let request = request.into_inner();
        if request.agent_id.trim().is_empty() || request.host_id.trim().is_empty() {
            return Err(Status::invalid_argument(
                "agent_id and host_id are required",
            ));
        }

        Ok(Response::new(grpc::HeartbeatResponse {
            accepted: true,
            message: "heartbeat accepted".to_string(),
        }))
    }

    async fn open_agent_stream(
        &self,
        request: Request<Streaming<grpc::AgentEvent>>,
    ) -> Result<Response<Self::OpenAgentStreamStream>, Status> {
        let mut inbound = request.into_inner();
        tokio::spawn(async move {
            while let Ok(Some(event)) = inbound.message().await {
                tracing::info!(
                    event_id = %event.event_id,
                    host_id = %event.host_id,
                    kind = %event.kind,
                    "agent event received"
                );
            }
        });

        let (sender, receiver) = mpsc::channel(8);
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            kind: "ack".to_string(),
            payload_json: serde_json::json!({
                "message": "grpc agent stream connected"
            })
            .to_string(),
            requires_approval: false,
        };
        if sender.send(Ok(command)).await.is_err() {
            tracing::warn!("failed to enqueue initial grpc command");
        }

        Ok(Response::new(ReceiverStream::new(receiver)))
    }
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
        "agent_transport": "grpc_protobuf",
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
