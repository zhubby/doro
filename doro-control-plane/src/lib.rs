use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response as AxumResponse;
use axum::response::Sse;
use axum::response::sse::Event;
use axum::routing::get;
use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;
use doro_ai::AiPlanRequest;
use doro_ai::DeterministicPlanner;
use doro_ai::PlanProvider;
use doro_protocol::AgentCapability;
use doro_protocol::CapabilityRisk;
use doro_protocol::CreateTaskRequest;
use doro_protocol::HealthResponse;
use doro_protocol::ListApprovalsResponse;
use doro_protocol::ListAppsResponse;
use doro_protocol::ListHostsResponse;
use doro_protocol::ListTasksResponse;
use doro_protocol::MetricSnapshot;
use doro_protocol::SettingsResponse;
use doro_protocol::Task;
use doro_protocol::TaskStatus;
use doro_protocol::grpc;
use doro_protocol::grpc::agent_control_plane_server::AgentControlPlane;
use doro_protocol::grpc::agent_control_plane_server::AgentControlPlaneServer;
use doro_store::AgentHeartbeat;
use doro_store::AgentRegistration;
use doro_store::NewAgentEvent;
use doro_store::NewTask;
use doro_store::Store;
use futures_util::StreamExt;
use prost_types::Timestamp;
use serde_json::Value;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;
use tonic::Response;
use tonic::Status;
use tonic::Streaming;
use tonic::transport::Server;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AppState {
    store: Store,
}

#[derive(Debug)]
pub struct AppError(anyhow::Error);

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> AxumResponse {
        tracing::error!(error = %self.0, "control-plane request failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "internal server error"
            })),
        )
            .into_response()
    }
}

pub fn app(store: Store) -> Router {
    let state = AppState { store };

    Router::new()
        .route("/health", get(health))
        .route("/api/v1/hosts", get(list_hosts))
        .route("/api/v1/tasks", get(list_tasks).post(create_task))
        .route("/api/v1/approvals", get(list_approvals))
        .route("/api/v1/apps", get(list_apps))
        .route("/api/v1/settings", get(settings))
        .route("/api/v1/events", get(events))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

pub async fn run(config: doro_config::DoroConfig) -> anyhow::Result<()> {
    let http_addr: SocketAddr = config.server.http_bind.parse()?;
    let grpc_addr: SocketAddr = config.server.grpc_bind.parse()?;
    let store = Store::connect_with_config(&config.store).await?;
    store.migrate().await?;

    let http_listener = tokio::net::TcpListener::bind(http_addr).await?;
    tracing::info!("doro control-plane http listening on http://{http_addr}");
    tracing::info!("doro control-plane grpc listening on http://{grpc_addr}");

    let http_store = store.clone();
    let grpc_store = store.clone();
    let http_server = async move {
        axum::serve(http_listener, app(http_store))
            .await
            .map_err(anyhow::Error::from)
    };
    let grpc_server = async move {
        Server::builder()
            .add_service(AgentControlPlaneServer::new(GrpcAgentService {
                store: grpc_store,
            }))
            .serve(grpc_addr)
            .await
            .map_err(anyhow::Error::from)
    };

    tokio::try_join!(http_server, grpc_server)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct GrpcAgentService {
    store: Store,
}

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

        let agent_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let observed_at = Utc::now();
        let hostname = if request.hostname.trim().is_empty() {
            format!("doro-agent-{host_id}")
        } else {
            request.hostname
        };
        let capabilities = request
            .capabilities
            .into_iter()
            .filter_map(grpc_capability_to_protocol)
            .collect();

        self.store
            .agents()
            .register(AgentRegistration {
                agent_id,
                host_id,
                hostname,
                capabilities,
                observed_at,
            })
            .await
            .map_err(store_status)?;
        self.store
            .events()
            .record(NewAgentEvent {
                agent_id: Some(agent_id),
                host_id: Some(host_id),
                event_type: "agent_enrolled".to_string(),
                event_json: serde_json::json!({
                    "agent_id": agent_id,
                    "host_id": host_id
                }),
                recorded_at: observed_at,
            })
            .await
            .map_err(store_status)?;

        Ok(Response::new(grpc::EnrollResponse {
            agent_id: agent_id.to_string(),
            host_id: host_id.to_string(),
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

        let agent_id = doro_store::parse_uuid(&request.agent_id)
            .map_err(|_| Status::invalid_argument("agent_id must be a uuid"))?;
        let host_id = doro_store::parse_uuid(&request.host_id)
            .map_err(|_| Status::invalid_argument("host_id must be a uuid"))?;
        let observed_at = request
            .observed_at
            .as_ref()
            .and_then(timestamp_to_utc)
            .unwrap_or_else(Utc::now);
        let capabilities = request
            .capabilities
            .into_iter()
            .filter_map(grpc_capability_to_protocol)
            .collect();

        self.store
            .agents()
            .heartbeat(AgentHeartbeat {
                agent_id,
                host_id,
                capabilities,
                observed_at,
            })
            .await
            .map_err(store_status)?;
        self.store
            .events()
            .record(NewAgentEvent {
                agent_id: Some(agent_id),
                host_id: Some(host_id),
                event_type: "heartbeat".to_string(),
                event_json: serde_json::json!({
                    "agent_id": agent_id,
                    "host_id": host_id,
                    "observed_at": observed_at
                }),
                recorded_at: observed_at,
            })
            .await
            .map_err(store_status)?;

        Ok(Response::new(grpc::HeartbeatResponse {
            accepted: true,
            message: "heartbeat accepted".to_string(),
        }))
    }

    async fn open_agent_stream(
        &self,
        request: Request<Streaming<grpc::AgentEvent>>,
    ) -> Result<Response<Self::OpenAgentStreamStream>, Status> {
        let store = self.store.clone();
        let mut inbound = request.into_inner();
        tokio::spawn(async move {
            while let Ok(Some(event)) = inbound.message().await {
                let recorded_at = event
                    .recorded_at
                    .as_ref()
                    .and_then(timestamp_to_utc)
                    .unwrap_or_else(Utc::now);
                let agent_id = parse_optional_uuid(&event.agent_id);
                let host_id = parse_optional_uuid(&event.host_id);
                let payload = parse_event_payload(&event.payload_json);
                let event_type = if event.kind.trim().is_empty() {
                    "unknown".to_string()
                } else {
                    event.kind.clone()
                };

                if let Err(error) = store
                    .events()
                    .record(NewAgentEvent {
                        agent_id,
                        host_id,
                        event_type: event_type.clone(),
                        event_json: serde_json::json!({
                            "event_id": event.event_id,
                            "kind": event_type,
                            "payload": payload
                        }),
                        recorded_at,
                    })
                    .await
                {
                    tracing::warn!(%error, "failed to persist agent stream event");
                }
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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "doro-control-plane".to_string(),
    })
}

async fn list_hosts(State(state): State<AppState>) -> Result<Json<ListHostsResponse>, AppError> {
    Ok(Json(ListHostsResponse {
        items: state.store.hosts().list().await?,
    }))
}

async fn list_tasks(State(state): State<AppState>) -> Result<Json<ListTasksResponse>, AppError> {
    Ok(Json(ListTasksResponse {
        items: state.store.tasks().list().await?,
    }))
}

async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<Task>, AppError> {
    let planner = DeterministicPlanner;
    let prompt = request.prompt.clone();
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

    let task = state
        .store
        .tasks()
        .create_with_steps(NewTask {
            id: Uuid::new_v4(),
            host_id: request.host_id,
            title: request.title,
            prompt,
            status,
            created_by: "api".to_string(),
            created_at: Utc::now(),
            steps,
        })
        .await?;

    Ok(Json(task))
}

async fn list_approvals(
    State(state): State<AppState>,
) -> Result<Json<ListApprovalsResponse>, AppError> {
    Ok(Json(ListApprovalsResponse {
        items: state.store.approvals().list().await?,
    }))
}

async fn list_apps(State(state): State<AppState>) -> Result<Json<ListAppsResponse>, AppError> {
    Ok(Json(ListAppsResponse {
        items: state.store.apps().list().await?,
    }))
}

async fn settings(State(state): State<AppState>) -> Result<Json<SettingsResponse>, AppError> {
    Ok(Json(SettingsResponse {
        approval_policy: setting_string(&state.store, "approval_policy", "policy_and_human_approval")
            .await?,
        agent_transport: setting_string(&state.store, "agent_transport", "grpc_protobuf").await?,
        database: setting_string(&state.store, "database", "postgres").await?,
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

async fn setting_string(store: &Store, key: &str, fallback: &str) -> Result<String, AppError> {
    let value = store.settings().get_json(key).await?;
    Ok(value
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| fallback.to_string()))
}

fn grpc_capability_to_protocol(capability: grpc::AgentCapability) -> Option<AgentCapability> {
    doro_store::parse_agent_capability(&capability.name, &capability.risk, capability.description)
}

fn timestamp_to_utc(timestamp: &Timestamp) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(timestamp.seconds, timestamp.nanos as u32)
        .single()
}

fn parse_optional_uuid(value: &str) -> Option<Uuid> {
    if value.trim().is_empty() {
        return None;
    }
    doro_store::parse_uuid(value).ok()
}

fn parse_event_payload(payload_json: &str) -> Value {
    if payload_json.trim().is_empty() {
        return serde_json::json!({});
    }
    serde_json::from_str(payload_json).unwrap_or_else(|_| {
        serde_json::json!({
            "raw": payload_json
        })
    })
}

fn store_status(error: sea_orm::DbErr) -> Status {
    tracing::error!(%error, "store operation failed");
    Status::internal("store operation failed")
}

#[cfg(test)]
fn default_capabilities() -> Vec<AgentCapability> {
    vec![
        AgentCapability {
            name: doro_protocol::CapabilityName::MetricsRead,
            risk: CapabilityRisk::Low,
            description: "Read CPU, memory, disk, and load metrics".to_string(),
        },
        AgentCapability {
            name: doro_protocol::CapabilityName::LogsRead,
            risk: CapabilityRisk::Low,
            description: "Read service and task logs".to_string(),
        },
        AgentCapability {
            name: doro_protocol::CapabilityName::ShellExecute,
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
    use doro_protocol::CapabilityName;
    use sea_orm::DatabaseBackend;
    use sea_orm::MockDatabase;

    #[tokio::test]
    async fn router_builds() {
        let connection = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);
        let _router = app(store);
    }

    #[test]
    fn parses_event_payload_json() {
        assert_eq!(
            parse_event_payload(r#"{"ok":true}"#),
            serde_json::json!({"ok": true})
        );
        assert_eq!(
            parse_event_payload("not json"),
            serde_json::json!({"raw": "not json"})
        );
    }

    #[test]
    fn default_capabilities_include_high_risk_shell() {
        assert!(
            default_capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::ShellExecute)
        );
    }
}
