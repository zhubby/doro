use argon2::Argon2;
use argon2::PasswordHash;
use argon2::PasswordHasher;
use argon2::PasswordVerifier;
use argon2::password_hash::SaltString;
use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::Path as AxumPath;
use axum::extract::Query;
use axum::extract::State;
use axum::http::Request as HttpRequest;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::response::Response as AxumResponse;
use axum::response::Sse;
use axum::response::sse::Event;
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::TimeZone;
use chrono::Utc;
use doro_ai::AiPlanRequest;
use doro_ai::DeterministicPlanner;
use doro_ai::PlanProvider;
use doro_protocol::AgentCapability;
use doro_protocol::AuthStatusResponse;
use doro_protocol::AuthTokenResponse;
use doro_protocol::CapabilityRisk;
use doro_protocol::CreateEnrollmentTokenRequest;
use doro_protocol::CreateEnrollmentTokenResponse;
use doro_protocol::CreateTaskRequest;
use doro_protocol::CurrentUserResponse;
use doro_protocol::EnrollmentToken;
use doro_protocol::HealthResponse;
use doro_protocol::HostStatus;
use doro_protocol::LatestMetricResponse;
use doro_protocol::ListApprovalsResponse;
use doro_protocol::ListAppsResponse;
use doro_protocol::ListHostContainersResponse;
use doro_protocol::ListHostsResponse;
use doro_protocol::ListMetricSnapshotsResponse;
use doro_protocol::ListTasksResponse;
use doro_protocol::LoginRequest;
use doro_protocol::MetricSnapshot;
use doro_protocol::RefreshTokenRequest;
use doro_protocol::RegisterRequest;
use doro_protocol::SettingsResponse;
use doro_protocol::Task;
use doro_protocol::TaskStatus;
use doro_protocol::UserSummary;
use doro_protocol::grpc;
use doro_protocol::grpc::agent_control_plane_server::AgentControlPlane;
use doro_protocol::grpc::agent_control_plane_server::AgentControlPlaneServer;
use doro_protocol::protobuf_timestamp_now;
use doro_store::AgentHeartbeat;
use doro_store::AgentRegistration;
use doro_store::NewAgentEvent;
use doro_store::NewContainerObservation;
use doro_store::NewEnrollmentToken;
use doro_store::NewMetricSnapshot;
use doro_store::NewRefreshToken;
use doro_store::NewTask;
use doro_store::NewUser;
use doro_store::Store;
use doro_store::StoredUser;
use futures_util::StreamExt;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use jsonwebtoken::Validation;
use prost_types::Timestamp;
use rand::RngCore;
use rand::rngs::OsRng;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
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

const CONTAINER_REFRESH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct AppState {
    store: Store,
    auth: AuthService,
    agent_streams: AgentStreamRegistry,
}

#[derive(Debug, Clone, Default)]
pub struct AgentStreamRegistry {
    streams: Arc<Mutex<HashMap<Uuid, AgentStreamHandle>>>,
}

#[derive(Debug, Clone)]
struct AgentStreamHandle {
    agent_id: Uuid,
    sender: mpsc::Sender<Result<grpc::ControlPlaneCommand, Status>>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<AgentCommandReply>>>>,
}

#[derive(Debug)]
enum AgentCommandReply {
    ContainerSnapshot(grpc::ContainerSnapshotEvent),
    Failed(String),
}

impl AgentStreamRegistry {
    async fn register(
        &self,
        host_id: Uuid,
        agent_id: Uuid,
        sender: mpsc::Sender<Result<grpc::ControlPlaneCommand, Status>>,
    ) -> Arc<Mutex<HashMap<String, oneshot::Sender<AgentCommandReply>>>> {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        self.streams.lock().await.insert(
            host_id,
            AgentStreamHandle {
                agent_id,
                sender,
                pending: pending.clone(),
            },
        );
        pending
    }

    async fn unregister(&self, host_id: Uuid, agent_id: Uuid) {
        let mut streams = self.streams.lock().await;
        if streams
            .get(&host_id)
            .is_some_and(|handle| handle.agent_id == agent_id)
        {
            streams.remove(&host_id);
        }
    }

    async fn shutdown_all(&self, reason: impl Into<String>) {
        let reason = reason.into();
        let handles = self
            .streams
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for handle in handles {
            let command = grpc::ControlPlaneCommand {
                command_id: Uuid::new_v4().to_string(),
                issued_at: Some(protobuf_timestamp_now()),
                command: Some(grpc::control_plane_command::Command::Shutdown(
                    grpc::ShutdownCommand {
                        reason: reason.clone(),
                    },
                )),
            };
            if handle.sender.send(Ok(command)).await.is_err() {
                tracing::debug!(
                    agent_id = %handle.agent_id,
                    "failed to enqueue agent stream shutdown command"
                );
            }
        }
    }

    async fn collect_containers(
        &self,
        host_id: Uuid,
    ) -> Result<grpc::ContainerSnapshotEvent, ContainerRefreshError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(ContainerRefreshError::NoStream)?;
        let command_id = Uuid::new_v4().to_string();
        let (reply_sender, reply_receiver) = oneshot::channel();
        handle
            .pending
            .lock()
            .await
            .insert(command_id.clone(), reply_sender);

        let command = grpc::ControlPlaneCommand {
            command_id: command_id.clone(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::CollectContainers(
                grpc::CollectContainersCommand {
                    runtime: "docker".to_string(),
                },
            )),
        };

        if handle.sender.send(Ok(command)).await.is_err() {
            handle.pending.lock().await.remove(&command_id);
            return Err(ContainerRefreshError::NoStream);
        }

        match tokio::time::timeout(CONTAINER_REFRESH_TIMEOUT, reply_receiver).await {
            Ok(Ok(AgentCommandReply::ContainerSnapshot(snapshot))) => Ok(snapshot),
            Ok(Ok(AgentCommandReply::Failed(message))) => {
                Err(ContainerRefreshError::AgentFailed(message))
            }
            Ok(Err(_)) => Err(ContainerRefreshError::NoStream),
            Err(_) => {
                handle.pending.lock().await.remove(&command_id);
                Err(ContainerRefreshError::Timeout)
            }
        }
    }
}

#[derive(Debug)]
enum ContainerRefreshError {
    NoOnlineHosts,
    NoStream,
    Timeout,
    AgentFailed(String),
}

#[derive(Debug, Clone)]
pub struct AuthService {
    jwt_secret: String,
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MetricHistoryQuery {
    limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    role: String,
    iat: i64,
    exp: i64,
    jti: String,
    typ: String,
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

pub fn app(store: Store) -> Router {
    app_with_auth(store, AuthService::development())
}

pub fn app_with_auth(store: Store, auth: AuthService) -> Router {
    app_with_auth_and_streams(store, auth, AgentStreamRegistry::default())
}

pub fn app_with_auth_and_streams(
    store: Store,
    auth: AuthService,
    agent_streams: AgentStreamRegistry,
) -> Router {
    let state = AppState {
        store,
        auth,
        agent_streams,
    };

    let protected_routes = Router::new()
        .route("/api/v1/hosts", get(list_hosts))
        .route(
            "/api/v1/hosts/enrollment-token",
            axum::routing::post(create_enrollment_token),
        )
        .route("/api/v1/hosts/:host_id", axum::routing::delete(delete_host))
        .route(
            "/api/v1/hosts/:host_id/metrics/latest",
            get(latest_host_metric),
        )
        .route("/api/v1/hosts/:host_id/metrics", get(list_host_metrics))
        .route(
            "/api/v1/hosts/:host_id/containers",
            get(list_host_containers),
        )
        .route("/api/v1/containers", get(refresh_containers))
        .route("/api/v1/tasks", get(list_tasks).post(create_task))
        .route("/api/v1/approvals", get(list_approvals))
        .route("/api/v1/apps", get(list_apps))
        .route("/api/v1/settings", get(settings))
        .route("/api/v1/events", get(events))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/logout", axum::routing::post(logout))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/status", get(auth_status))
        .route("/api/v1/auth/register", axum::routing::post(register))
        .route("/api/v1/auth/login", axum::routing::post(login))
        .route("/api/v1/auth/refresh", axum::routing::post(refresh))
        .merge(protected_routes)
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

pub async fn run(config: doro_config::ControlPlaneConfig) -> anyhow::Result<()> {
    let console_addr: SocketAddr = config.server.console_bind.parse()?;
    let agent_addr: SocketAddr = config.server.agent_bind.parse()?;
    let store = Store::connect_with_config(&config.store).await?;
    store.migrate().await?;
    let auth = AuthService::load_or_create(&store, config.security.jwt_secret.as_deref()).await?;

    let console_listener = tokio::net::TcpListener::bind(console_addr).await?;
    tracing::info!("doro control-plane console listening on http://{console_addr}");
    tracing::info!("doro control-plane agent listening on http://{agent_addr}");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!("shutdown signal received, stopping control-plane services");
        let _ = shutdown_tx.send(true);
    });

    let agent_streams = AgentStreamRegistry::default();
    let shutdown_streams = agent_streams.clone();
    let console_store = store.clone();
    let console_streams = agent_streams.clone();
    let agent_store = store.clone();
    let grpc_streams = agent_streams.clone();
    let console_shutdown = shutdown_rx.clone();
    let stream_shutdown = shutdown_rx.clone();
    let agent_shutdown = shutdown_rx;
    tokio::spawn(async move {
        wait_for_shutdown(stream_shutdown).await;
        shutdown_streams
            .shutdown_all("control-plane shutting down")
            .await;
    });
    let console_server = async move {
        axum::serve(
            console_listener,
            app_with_auth_and_streams(console_store, auth, console_streams),
        )
        .with_graceful_shutdown(wait_for_shutdown(console_shutdown))
        .await
        .map_err(anyhow::Error::from)
    };
    let agent_server = async move {
        Server::builder()
            .add_service(AgentControlPlaneServer::new(GrpcAgentService {
                store: agent_store,
                agent_streams: grpc_streams,
                shutdown_rx: agent_shutdown.clone(),
            }))
            .serve_with_shutdown(agent_addr, wait_for_shutdown(agent_shutdown))
            .await
            .map_err(anyhow::Error::from)
    };

    tokio::try_join!(console_server, agent_server)?;
    Ok(())
}

async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    while !*shutdown_rx.borrow_and_update() {
        if shutdown_rx.changed().await.is_err() {
            break;
        }
    }
}

fn shutdown_requested(shutdown_rx: &watch::Receiver<bool>) -> bool {
    *shutdown_rx.borrow()
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "failed to listen for ctrl-c shutdown signal");
        }
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::SignalKind;

        let terminate = async {
            match tokio::signal::unix::signal(SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to listen for terminate shutdown signal");
                    std::future::pending::<()>().await;
                }
            }
        };

        tokio::select! {
            () = ctrl_c => {}
            () = terminate => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}

#[derive(Debug, Clone)]
pub struct GrpcAgentService {
    store: Store,
    agent_streams: AgentStreamRegistry,
    shutdown_rx: watch::Receiver<bool>,
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
        let system_profile = parse_event_payload(&request.system_profile_json);

        self.store
            .agents()
            .register(AgentRegistration {
                agent_id,
                host_id,
                enrollment_token: request.enrollment_token,
                hostname,
                system_profile,
                capabilities,
                observed_at,
            })
            .await
            .map_err(enrollment_status)?;
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
        let agent_streams = self.agent_streams.clone();
        let shutdown_rx = self.shutdown_rx.clone();
        let mut inbound = request.into_inner();
        tracing::debug!("agent opened grpc stream");
        let (sender, receiver) = mpsc::channel(8);
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::Ack(
                grpc::AckCommand {
                    message: "grpc agent stream connected".to_string(),
                },
            )),
        };
        if sender.send(Ok(command)).await.is_err() {
            tracing::warn!("failed to enqueue initial grpc command");
        }

        tokio::spawn(async move {
            let command_sender = sender;
            let mut pending_commands: Option<
                Arc<Mutex<HashMap<String, oneshot::Sender<AgentCommandReply>>>>,
            > = None;
            let mut connected_agent: Option<(Uuid, Uuid)> = None;
            loop {
                let event = tokio::select! {
                    event = inbound.message() => {
                        match event {
                            Ok(Some(event)) => event,
                            Ok(None) => break,
                            Err(error) => {
                                if shutdown_requested(&shutdown_rx) {
                                    tracing::debug!(%error, "agent stream receive stopped during shutdown");
                                } else {
                                    tracing::warn!(%error, "agent stream receive failed");
                                }
                                break;
                            }
                        }
                    }
                    () = wait_for_shutdown(shutdown_rx.clone()) => break,
                };
                let recorded_at = event
                    .recorded_at
                    .as_ref()
                    .and_then(timestamp_to_utc)
                    .unwrap_or_else(Utc::now);
                let agent_id = parse_optional_uuid(&event.agent_id);
                let host_id = parse_optional_uuid(&event.host_id);
                let Some((event_type, payload)) = typed_agent_event_payload(&event) else {
                    tracing::warn!("agent stream event missing typed payload");
                    continue;
                };
                match event.event.clone() {
                    Some(grpc::agent_event::Event::Connected(_))
                    | Some(grpc::agent_event::Event::Heartbeat(_)) => {
                        if let (Some(agent_id), Some(host_id)) = (agent_id, host_id) {
                            if let Some((old_agent_id, old_host_id)) = connected_agent
                                && old_host_id != host_id
                            {
                                agent_streams.unregister(old_host_id, old_agent_id).await;
                            }
                            connected_agent = Some((agent_id, host_id));
                            pending_commands = Some(
                                agent_streams
                                    .register(host_id, agent_id, command_sender.clone())
                                    .await,
                            );
                            tracing::debug!(
                                agent_id = %agent_id,
                                host_id = %host_id,
                                event_type,
                                "agent stream registered"
                            );
                            if let Err(error) = store
                                .agents()
                                .mark_online(agent_id, host_id, recorded_at)
                                .await
                            {
                                tracing::warn!(%error, "failed to refresh streamed agent heartbeat");
                            }
                        }
                    }
                    Some(grpc::agent_event::Event::ContainerSnapshot(snapshot)) => {
                        if let Some(pending_commands) = &pending_commands
                            && !snapshot.command_id.is_empty()
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&snapshot.command_id)
                        {
                            let _ = reply_sender
                                .send(AgentCommandReply::ContainerSnapshot(snapshot.clone()));
                        }
                    }
                    Some(grpc::agent_event::Event::CommandResult(result)) => {
                        if result.status == grpc::CommandStatus::Failed as i32
                            && let Some(pending_commands) = &pending_commands
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&result.command_id)
                        {
                            let _ = reply_sender.send(AgentCommandReply::Failed(result.message));
                        }
                    }
                    _ => {}
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

                if let Err(error) =
                    ingest_agent_event(&store, host_id, &event_type, &payload, recorded_at).await
                {
                    tracing::warn!(%error, event_type, "failed to ingest agent stream event");
                }
            }

            if let Some((agent_id, host_id)) = connected_agent {
                agent_streams.unregister(host_id, agent_id).await;
                let recorded_at = Utc::now();
                if let Err(error) = store
                    .agents()
                    .mark_offline(agent_id, host_id, recorded_at)
                    .await
                {
                    tracing::warn!(%error, "failed to mark disconnected agent offline");
                }
                if let Err(error) = store
                    .events()
                    .record(NewAgentEvent {
                        agent_id: Some(agent_id),
                        host_id: Some(host_id),
                        event_type: "agent_disconnected".to_string(),
                        event_json: serde_json::json!({
                            "agent_id": agent_id,
                            "host_id": host_id
                        }),
                        recorded_at,
                    })
                    .await
                {
                    tracing::warn!(%error, "failed to persist agent disconnect event");
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(receiver)))
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "doro-control-plane".to_string(),
    })
}

async fn auth_status(State(state): State<AppState>) -> Result<Json<AuthStatusResponse>, AppError> {
    Ok(Json(AuthStatusResponse {
        registration_open: state.store.users().registration_open().await?,
    }))
}

async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<AuthTokenResponse>, AppError> {
    validate_username(&request.username)?;
    validate_password(&request.password)?;
    let now = Utc::now();
    let password_hash = hash_password(&request.password)?;
    let user = state
        .store
        .users()
        .create_first_admin(NewUser {
            id: Uuid::new_v4(),
            username: request.username.trim().to_lowercase(),
            display_name: display_name_or_username(&request.display_name, &request.username),
            password_hash,
            role: "admin".to_string(),
            created_at: now,
        })
        .await
        .map_err(|error| {
            if error.to_string().contains("registration is closed") {
                AppError::status(StatusCode::CONFLICT, "registration is closed")
            } else {
                AppError::from(error)
            }
        })?;
    state.store.users().mark_login(user.id, now).await?;

    Ok(Json(issue_token_pair(&state, user, now).await?))
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthTokenResponse>, AppError> {
    let username = request.username.trim().to_lowercase();
    let Some(user) = state.store.users().find_by_username(&username).await? else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    };
    if user.status != "active" || !verify_password(&request.password, &user.password_hash)? {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    }

    let now = Utc::now();
    state.store.users().mark_login(user.id, now).await?;
    Ok(Json(issue_token_pair(&state, user, now).await?))
}

async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<AuthTokenResponse>, AppError> {
    let now = Utc::now();
    let Some(stored_token) = state
        .store
        .refresh_tokens()
        .find_by_token(&request.refresh_token)
        .await?
    else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid refresh token",
        ));
    };
    if stored_token.status != "active" || stored_token.revoked_at.is_some() {
        state
            .store
            .refresh_tokens()
            .revoke_all_for_user(stored_token.user_id, now)
            .await?;
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid refresh token",
        ));
    }
    if stored_token.expires_at <= now {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "refresh token expired",
        ));
    }
    let Some(user) = state.store.users().find_by_id(stored_token.user_id).await? else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid refresh token",
        ));
    };
    if user.status != "active" {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "user is disabled",
        ));
    }

    let refresh_token = generate_refresh_token();
    state
        .store
        .refresh_tokens()
        .rotate(
            stored_token.id,
            NewRefreshToken {
                id: Uuid::new_v4(),
                user_id: user.id,
                token: refresh_token.clone(),
                created_at: now,
                expires_at: now + ChronoDuration::days(30),
            },
            now,
        )
        .await?;
    let (access_token, expires_at) = state.auth.issue_access_token(&user, now)?;
    Ok(Json(AuthTokenResponse {
        access_token,
        refresh_token,
        expires_at,
        user: user_summary(&user),
    }))
}

async fn me(Extension(current_user): Extension<CurrentUser>) -> Json<CurrentUserResponse> {
    Json(CurrentUserResponse {
        user: UserSummary {
            id: current_user.id,
            username: current_user.username.clone(),
            display_name: current_user.username,
            role: current_user.role,
        },
    })
}

async fn logout(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<StatusCode, AppError> {
    state
        .store
        .refresh_tokens()
        .revoke(&request.refresh_token, Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_hosts(State(state): State<AppState>) -> Result<Json<ListHostsResponse>, AppError> {
    Ok(Json(ListHostsResponse {
        items: state.store.hosts().list().await?,
    }))
}

async fn delete_host(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if state.store.hosts().delete(host_id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }

    Err(AppError::status(StatusCode::NOT_FOUND, "host not found"))
}

async fn create_enrollment_token(
    State(state): State<AppState>,
    Json(request): Json<CreateEnrollmentTokenRequest>,
) -> Result<Json<CreateEnrollmentTokenResponse>, AppError> {
    let now = Utc::now();
    let token = generate_enrollment_token();
    let label = request
        .label
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| format!("new-host-{}", now.format("%Y%m%d%H%M%S")));

    let stored = state
        .store
        .enrollment_tokens()
        .create(NewEnrollmentToken {
            id: Uuid::new_v4(),
            label: label.clone(),
            token: token.clone(),
            expires_at: None,
            created_at: now,
        })
        .await?;

    Ok(Json(CreateEnrollmentTokenResponse {
        item: EnrollmentToken {
            id: stored.id,
            label,
            token,
        },
    }))
}

async fn latest_host_metric(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
) -> Result<Json<LatestMetricResponse>, AppError> {
    Ok(Json(LatestMetricResponse {
        item: state.store.metrics().latest_for_host(host_id).await?,
    }))
}

async fn list_host_metrics(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
    Query(query): Query<MetricHistoryQuery>,
) -> Result<Json<ListMetricSnapshotsResponse>, AppError> {
    let limit = query.limit.unwrap_or(60).clamp(1, 240);
    Ok(Json(ListMetricSnapshotsResponse {
        items: state
            .store
            .metrics()
            .recent_for_host(host_id, limit)
            .await?,
    }))
}

async fn list_host_containers(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
) -> Result<Json<ListHostContainersResponse>, AppError> {
    Ok(Json(ListHostContainersResponse {
        items: state.store.containers().list_by_host(host_id).await?,
    }))
}

async fn refresh_containers(
    State(state): State<AppState>,
) -> Result<Json<ListHostContainersResponse>, AppError> {
    let hosts = state.store.hosts().list().await?;
    let online_hosts = hosts
        .into_iter()
        .filter(|host| host.status == HostStatus::Online)
        .collect::<Vec<_>>();
    if online_hosts.is_empty() {
        return Err(container_refresh_app_error(
            ContainerRefreshError::NoOnlineHosts,
        ));
    }

    let mut snapshots = Vec::with_capacity(online_hosts.len());
    for host in &online_hosts {
        let snapshot = state
            .agent_streams
            .collect_containers(host.id)
            .await
            .map_err(container_refresh_app_error)?;
        snapshots.push((host.id, snapshot));
    }

    for (host_id, snapshot) in snapshots {
        let payload = container_snapshot_payload(&snapshot);
        ingest_agent_event(
            &state.store,
            Some(host_id),
            "container.snapshot",
            &payload,
            Utc::now(),
        )
        .await?;
    }

    let mut items = Vec::new();
    for host in online_hosts {
        items.extend(state.store.containers().list_by_host(host.id).await?);
    }
    Ok(Json(ListHostContainersResponse { items }))
}

async fn list_tasks(State(state): State<AppState>) -> Result<Json<ListTasksResponse>, AppError> {
    Ok(Json(ListTasksResponse {
        items: state.store.tasks().list().await?,
    }))
}

async fn create_task(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
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
            created_by: current_user.username,
            created_at: Utc::now(),
            steps,
        })
        .await?;

    Ok(Json(task))
}

async fn auth_middleware(
    State(state): State<AppState>,
    mut request: HttpRequest<axum::body::Body>,
    next: Next,
) -> Result<AxumResponse, AppError> {
    let Some(header) = request.headers().get(AUTHORIZATION) else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "missing bearer token",
        ));
    };
    let header = header
        .to_str()
        .map_err(|_| AppError::status(StatusCode::UNAUTHORIZED, "invalid bearer token"))?;
    let Some(token) = header.strip_prefix("Bearer ") else {
        return Err(AppError::status(
            StatusCode::UNAUTHORIZED,
            "invalid bearer token",
        ));
    };
    let current_user = state.auth.verify_access_token(token)?;
    request.extensions_mut().insert(current_user);
    Ok(next.run(request).await)
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
        approval_policy: setting_string(
            &state.store,
            "approval_policy",
            "policy_and_human_approval",
        )
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

fn typed_agent_event_payload(event: &grpc::AgentEvent) -> Option<(String, Value)> {
    match event.event.as_ref()? {
        grpc::agent_event::Event::Connected(connected) => Some((
            "connected".to_string(),
            serde_json::json!({
                "protocol_version": connected.protocol_version,
                "hostname": connected.hostname
            }),
        )),
        grpc::agent_event::Event::Heartbeat(heartbeat) => Some((
            "heartbeat".to_string(),
            serde_json::json!({
                "protocol_version": heartbeat.protocol_version
            }),
        )),
        grpc::agent_event::Event::MetricsSnapshot(snapshot) => Some((
            "metrics.snapshot".to_string(),
            serde_json::json!({
                "host_id": snapshot.host_id,
                "captured_at": snapshot.captured_at.as_ref().and_then(timestamp_to_utc),
                "cpu_percent": snapshot.cpu_percent,
                "memory_percent": snapshot.memory_percent,
                "disk_percent": snapshot.disk_percent,
                "load_average": snapshot.load_average,
                "extra": parse_event_payload(&snapshot.extra_json),
            }),
        )),
        grpc::agent_event::Event::ContainerSnapshot(snapshot) => Some((
            "container.snapshot".to_string(),
            container_snapshot_payload(snapshot),
        )),
        grpc::agent_event::Event::CollectorError(error) => Some((
            "metrics.collector_error".to_string(),
            serde_json::json!({
                "command_id": error.command_id,
                "collector": error.collector,
                "message": error.message,
            }),
        )),
        grpc::agent_event::Event::CommandResult(result) => Some((
            "command.result".to_string(),
            serde_json::json!({
                "command_id": result.command_id,
                "status": result.status,
                "message": result.message,
            }),
        )),
    }
}

fn container_snapshot_payload(snapshot: &grpc::ContainerSnapshotEvent) -> Value {
    serde_json::json!({
        "command_id": snapshot.command_id,
        "runtime": snapshot.runtime,
        "containers": snapshot.containers.iter().map(container_observation_payload).collect::<Vec<_>>(),
        "extra": parse_event_payload(&snapshot.extra_json),
    })
}

fn container_observation_payload(container: &grpc::ContainerObservation) -> Value {
    serde_json::json!({
        "id": container.id,
        "names": container.names,
        "image": container.image,
        "image_id": container.image_id,
        "command": container.command,
        "created": container.created,
        "ports": parse_event_payload(&container.ports_json),
        "labels": parse_event_payload(&container.labels_json),
        "state": container.state,
        "status": container.status,
    })
}

async fn ingest_agent_event(
    store: &Store,
    host_id: Option<Uuid>,
    event_type: &str,
    payload: &Value,
    recorded_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    match event_type {
        "metrics.snapshot" => {
            if let Some(snapshot) = metric_snapshot_from_payload(host_id, payload, recorded_at) {
                store.metrics().record(snapshot).await?;
            }
        }
        "container.snapshot" => {
            if let Some(host_id) = host_id {
                let containers = container_observations_from_payload(host_id, payload, recorded_at);
                store.containers().upsert_many(containers).await?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn metric_snapshot_from_payload(
    fallback_host_id: Option<Uuid>,
    payload: &Value,
    recorded_at: DateTime<Utc>,
) -> Option<NewMetricSnapshot> {
    let host_id = payload
        .get("host_id")
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .or(fallback_host_id)?;
    Some(NewMetricSnapshot {
        host_id,
        captured_at: payload
            .get("captured_at")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or(recorded_at),
        cpu_percent: json_f32(payload, "cpu_percent")?,
        memory_percent: json_f32(payload, "memory_percent")?,
        disk_percent: json_f32(payload, "disk_percent")?,
        load_average: json_f32(payload, "load_average")?,
        extra: payload
            .get("extra")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    })
}

fn container_observations_from_payload(
    host_id: Uuid,
    payload: &Value,
    recorded_at: DateTime<Utc>,
) -> Vec<NewContainerObservation> {
    let runtime = payload
        .get("runtime")
        .and_then(Value::as_str)
        .unwrap_or("docker");
    payload
        .get("containers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|container| container_observation(host_id, runtime, container, recorded_at))
        .collect()
}

fn container_observation(
    host_id: Uuid,
    runtime: &str,
    container: &Value,
    recorded_at: DateTime<Utc>,
) -> Option<NewContainerObservation> {
    let container_ref = container.get("id").and_then(Value::as_str)?.to_string();
    let name = container
        .get("names")
        .and_then(Value::as_array)
        .and_then(|names| names.first())
        .and_then(Value::as_str)
        .map(|name| name.trim_start_matches('/').to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| container_ref.chars().take(12).collect());
    let image = container
        .get("image")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let status = container
        .get("state")
        .and_then(Value::as_str)
        .or_else(|| container.get("status").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    Some(NewContainerObservation {
        host_id,
        runtime: runtime.to_string(),
        container_ref,
        name,
        image,
        status,
        ports: container
            .get("ports")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
        labels: container
            .get("labels")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        observed_at: recorded_at,
    })
}

fn json_f32(payload: &Value, key: &str) -> Option<f32> {
    payload.get(key)?.as_f64().map(|value| value as f32)
}

fn store_status(error: sea_orm::DbErr) -> Status {
    if let sea_orm::DbErr::Custom(message) = &error
        && message.contains("is not enrolled")
    {
        tracing::warn!(%error, "agent identity is not enrolled");
        return Status::failed_precondition(message.clone());
    }

    tracing::error!(%error, "store operation failed");
    Status::internal("store operation failed")
}

fn enrollment_status(error: sea_orm::DbErr) -> Status {
    match &error {
        sea_orm::DbErr::Custom(message)
            if message.contains("enrollment token is invalid")
                || message.contains("enrollment token is not active")
                || message.contains("enrollment token is expired") =>
        {
            Status::permission_denied(message.clone())
        }
        _ => store_status(error),
    }
}

impl AppError {
    fn status(status: StatusCode, message: impl Into<String>) -> Self {
        Self(anyhow::anyhow!(ApiError {
            status,
            message: message.into(),
        }))
    }
}

fn container_refresh_app_error(error: ContainerRefreshError) -> AppError {
    match error {
        ContainerRefreshError::NoOnlineHosts => {
            AppError::status(StatusCode::SERVICE_UNAVAILABLE, "no online agents")
        }
        ContainerRefreshError::NoStream => AppError::status(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent stream is not connected",
        ),
        ContainerRefreshError::Timeout => AppError::status(
            StatusCode::GATEWAY_TIMEOUT,
            "agent container refresh timed out",
        ),
        ContainerRefreshError::AgentFailed(message) => AppError::status(
            StatusCode::BAD_GATEWAY,
            format!("agent container refresh failed: {message}"),
        ),
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> AxumResponse {
        (
            self.status,
            Json(serde_json::json!({
                "error": self.message
            })),
        )
            .into_response()
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> AxumResponse {
        if let Some(error) = self.0.downcast_ref::<ApiError>() {
            return (
                error.status,
                Json(serde_json::json!({
                    "error": error.message
                })),
            )
                .into_response();
        }
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

impl AuthService {
    async fn load_or_create(
        store: &Store,
        configured_secret: Option<&str>,
    ) -> anyhow::Result<Self> {
        if let Some(secret) = configured_secret
            && !secret.trim().is_empty()
        {
            return Ok(Self {
                jwt_secret: secret.to_string(),
            });
        }

        let existing = store.settings().get_json("jwt_secret").await?;
        if let Some(secret) = existing.and_then(|value| value.as_str().map(str::to_string))
            && !secret.trim().is_empty()
        {
            return Ok(Self { jwt_secret: secret });
        }
        let secret = generate_secret();
        store
            .settings()
            .upsert_json(
                "jwt_secret",
                serde_json::json!(secret),
                Some("JWT signing secret".to_string()),
            )
            .await?;
        Ok(Self { jwt_secret: secret })
    }

    fn development() -> Self {
        Self {
            jwt_secret: "doro-development-jwt-secret-change-before-production".to_string(),
        }
    }

    fn issue_access_token(
        &self,
        user: &StoredUser,
        issued_at: DateTime<Utc>,
    ) -> anyhow::Result<(String, DateTime<Utc>)> {
        let expires_at = issued_at + ChronoDuration::days(1);
        let claims = Claims {
            sub: user.id.to_string(),
            username: user.username.clone(),
            role: user.role.clone(),
            iat: issued_at.timestamp(),
            exp: expires_at.timestamp(),
            jti: Uuid::new_v4().to_string(),
            typ: "access".to_string(),
        };
        let token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;
        Ok((token, expires_at))
    }

    fn verify_access_token(&self, token: &str) -> Result<CurrentUser, AppError> {
        let mut validation = Validation::default();
        validation.validate_exp = true;
        let data = jsonwebtoken::decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| AppError::status(StatusCode::UNAUTHORIZED, "invalid bearer token"))?;
        if data.claims.typ != "access" {
            return Err(AppError::status(
                StatusCode::UNAUTHORIZED,
                "invalid bearer token",
            ));
        }
        let id = doro_store::parse_uuid(&data.claims.sub)
            .map_err(|_| AppError::status(StatusCode::UNAUTHORIZED, "invalid bearer token"))?;
        Ok(CurrentUser {
            id,
            username: data.claims.username,
            role: data.claims.role,
        })
    }
}

async fn issue_token_pair(
    state: &AppState,
    user: StoredUser,
    now: DateTime<Utc>,
) -> Result<AuthTokenResponse, AppError> {
    let refresh_token = generate_refresh_token();
    state
        .store
        .refresh_tokens()
        .create(NewRefreshToken {
            id: Uuid::new_v4(),
            user_id: user.id,
            token: refresh_token.clone(),
            created_at: now,
            expires_at: now + ChronoDuration::days(30),
        })
        .await?;
    let (access_token, expires_at) = state.auth.issue_access_token(&user, now)?;
    Ok(AuthTokenResponse {
        access_token,
        refresh_token,
        expires_at,
        user: user_summary(&user),
    })
}

fn user_summary(user: &StoredUser) -> UserSummary {
    UserSummary {
        id: user.id,
        username: user.username.clone(),
        display_name: user.display_name.clone(),
        role: user.role.clone(),
    }
}

fn validate_username(username: &str) -> Result<(), AppError> {
    let username = username.trim();
    if username.len() < 3 || username.len() > 64 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "invalid username",
        ));
    }
    if !username
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
    {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "invalid username",
        ));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.chars().count() < 10 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "password is too short",
        ));
    }
    Ok(())
}

fn display_name_or_username(display_name: &str, username: &str) -> String {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        username.trim().to_string()
    } else {
        display_name.to_string()
    }
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| anyhow::anyhow!("failed to hash password: {error}"))?
        .to_string())
}

fn verify_password(password: &str, password_hash: &str) -> anyhow::Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|error| anyhow::anyhow!("invalid password hash: {error}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn generate_refresh_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("doro_refresh_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn generate_enrollment_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("doro_enroll_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn generate_secret() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
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
        extra: serde_json::json!({}),
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
    fn metrics_snapshot_payload_maps_to_store_model() {
        let host_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "host_id": host_id,
            "captured_at": "2026-05-27T00:00:00Z",
            "cpu_percent": 12.5,
            "memory_percent": 34.5,
            "disk_percent": 56.5,
            "load_average": 1.5,
            "extra": {"networks": []}
        });

        let snapshot = match metric_snapshot_from_payload(None, &payload, Utc::now()) {
            Some(snapshot) => snapshot,
            None => panic!("valid metric payload should parse"),
        };

        assert_eq!(snapshot.host_id, host_id);
        assert_eq!(snapshot.cpu_percent, 12.5);
        assert_eq!(snapshot.extra, serde_json::json!({"networks": []}));
    }

    #[test]
    fn malformed_metrics_snapshot_payload_is_ignored() {
        let payload = serde_json::json!({
            "cpu_percent": "not-a-number",
            "memory_percent": 34.5,
            "disk_percent": 56.5,
            "load_average": 1.5
        });

        assert!(metric_snapshot_from_payload(Some(Uuid::new_v4()), &payload, Utc::now()).is_none());
    }

    #[test]
    fn container_snapshot_payload_maps_to_observations() {
        let host_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "runtime": "docker",
            "containers": [{
                "id": "abc123",
                "names": ["/postgres"],
                "image": "postgres:16",
                "state": "running",
                "ports": [],
                "labels": {"app": "db"}
            }]
        });

        let observations = container_observations_from_payload(host_id, &payload, Utc::now());

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].container_ref, "abc123");
        assert_eq!(observations[0].name, "postgres");
        assert_eq!(observations[0].runtime, "docker");
    }

    #[tokio::test]
    async fn stream_registry_dispatches_container_command_and_receives_snapshot() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel(1);
        let pending = registry.register(host_id, agent_id, sender).await;

        let collect = tokio::spawn({
            let registry = registry.clone();
            async move { registry.collect_containers(host_id).await }
        });
        let command = receiver
            .recv()
            .await
            .expect("registry should send collect command")
            .expect("command stream item should be ok");
        assert!(matches!(
            command.command,
            Some(grpc::control_plane_command::Command::CollectContainers(_))
        ));
        let reply_sender = pending
            .lock()
            .await
            .remove(&command.command_id)
            .expect("command should have pending waiter");
        reply_sender
            .send(AgentCommandReply::ContainerSnapshot(
                grpc::ContainerSnapshotEvent {
                    command_id: command.command_id,
                    runtime: "docker".to_string(),
                    containers: Vec::new(),
                    extra_json: "{}".to_string(),
                },
            ))
            .expect("waiter should receive snapshot");

        let snapshot = collect
            .await
            .expect("collect task should complete")
            .expect("snapshot should succeed");
        assert_eq!(snapshot.runtime, "docker");
    }

    #[tokio::test]
    async fn stream_registry_sends_shutdown_to_registered_streams() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel(1);
        registry.register(host_id, agent_id, sender).await;

        registry.shutdown_all("control-plane shutting down").await;

        let command = receiver
            .recv()
            .await
            .expect("registry should send shutdown command")
            .expect("command stream item should be ok");
        let Some(grpc::control_plane_command::Command::Shutdown(shutdown)) = command.command else {
            panic!("shutdown_all should send shutdown command");
        };
        assert_eq!(shutdown.reason, "control-plane shutting down");
    }

    #[test]
    fn default_capabilities_include_high_risk_shell() {
        assert!(
            default_capabilities()
                .iter()
                .any(|capability| capability.name == CapabilityName::ShellExecute)
        );
    }

    #[test]
    fn validates_usernames_and_passwords() {
        assert!(validate_username("admin.user-1").is_ok());
        assert!(validate_username("ad").is_err());
        assert!(validate_username("admin user").is_err());
        assert!(validate_password("1234567890").is_ok());
        assert!(validate_password("short").is_err());
    }

    #[test]
    fn password_hash_verifies_only_matching_password() -> anyhow::Result<()> {
        let hash = hash_password("correct-password")?;

        assert!(verify_password("correct-password", &hash)?);
        assert!(!verify_password("wrong-password", &hash)?);

        Ok(())
    }

    #[test]
    fn jwt_access_token_round_trips_current_user() -> anyhow::Result<()> {
        let auth = AuthService {
            jwt_secret: "test-secret".to_string(),
        };
        let user = StoredUser {
            id: Uuid::new_v4(),
            username: "admin".to_string(),
            display_name: "Admin".to_string(),
            password_hash: "hash".to_string(),
            role: "admin".to_string(),
            status: "active".to_string(),
        };
        let (token, expires_at) = auth.issue_access_token(&user, Utc::now())?;
        let current_user = auth
            .verify_access_token(&token)
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;

        assert_eq!(current_user.id, user.id);
        assert_eq!(current_user.username, "admin");
        assert_eq!(current_user.role, "admin");
        assert!(expires_at > Utc::now());

        Ok(())
    }

    #[test]
    fn enrollment_errors_map_to_permission_denied() {
        let status = enrollment_status(sea_orm::DbErr::Custom(
            "enrollment token is expired".to_string(),
        ));

        assert_eq!(status.code(), tonic::Code::PermissionDenied);
    }
}
