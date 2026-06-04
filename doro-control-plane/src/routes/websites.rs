use crate::agent_streams::{AgentStreamRegistry, agent_task_error_message};
use crate::auth::CurrentUser;
use crate::error::{AppError, normalize_optional_text, website_store_app_error};
use crate::prelude::*;
use crate::state::AppState;
use url::Url;

pub(crate) async fn list_websites(
    State(state): State<AppState>,
) -> Result<Json<ListWebsitesResponse>, AppError> {
    Ok(Json(ListWebsitesResponse {
        items: state.store.websites().list().await?,
    }))
}

pub(crate) async fn get_website(
    State(state): State<AppState>,
    AxumPath(website_id): AxumPath<Uuid>,
) -> Result<Json<WebsiteActionResponse>, AppError> {
    let item = state
        .store
        .websites()
        .get(website_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    Ok(Json(WebsiteActionResponse { item, task: None }))
}

pub(crate) async fn create_website(
    State(state): State<AppState>,
    Json(request): Json<CreateWebsiteRequest>,
) -> Result<Json<WebsiteActionResponse>, AppError> {
    ensure_network_expose_ready(&state, request.host_id).await?;
    let changes = validate_website_request(
        request.host_id,
        request.name,
        request.primary_domain,
        request.aliases,
        request.listen_port,
        request.upstream_url,
        request.notes,
    )?;
    let now = Utc::now();
    let item = match state
        .store
        .websites()
        .create(NewWebsite {
            id: Uuid::new_v4(),
            host_id: changes.host_id,
            name: changes.name,
            primary_domain: changes.primary_domain,
            aliases: changes.aliases,
            status: WebsiteStatus::Stopped,
            kind: WebsiteKind::ReverseProxy,
            protocol: WebsiteProtocol::Http,
            listen_port: changes.listen_port,
            upstream_url: changes.upstream_url,
            app_install_id: None,
            tls_certificate_id: None,
            config: serde_json::json!({}),
            notes: changes.notes,
            created_at: now,
        })
        .await
    {
        Ok(item) => item,
        Err(error) => return Err(website_store_app_error(error)),
    };
    record_website_event(&state, "website.created", &item).await?;
    Ok(Json(WebsiteActionResponse { item, task: None }))
}

pub(crate) async fn update_website(
    State(state): State<AppState>,
    AxumPath(website_id): AxumPath<Uuid>,
    Json(request): Json<UpdateWebsiteRequest>,
) -> Result<Json<WebsiteActionResponse>, AppError> {
    ensure_network_expose_ready(&state, request.host_id).await?;
    let existing = state
        .store
        .websites()
        .get(website_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    if existing.status != WebsiteStatus::Stopped {
        return Err(AppError::status(
            StatusCode::CONFLICT,
            "website must be stopped before configuration changes",
        ));
    }
    let changes = validate_website_request(
        request.host_id,
        request.name,
        request.primary_domain,
        request.aliases,
        request.listen_port,
        request.upstream_url,
        request.notes,
    )?;
    let item = match state
        .store
        .websites()
        .update_stopped(website_id, changes)
        .await
    {
        Ok(Some(item)) => item,
        Ok(None) => return Err(AppError::status(StatusCode::NOT_FOUND, "website not found")),
        Err(error) => return Err(website_store_app_error(error)),
    };
    record_website_event(&state, "website.updated", &item).await?;
    Ok(Json(WebsiteActionResponse { item, task: None }))
}

pub(crate) async fn start_website(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(website_id): AxumPath<Uuid>,
    Json(request): Json<WebsiteActionRequest>,
) -> Result<Json<WebsiteActionResponse>, AppError> {
    website_network_expose_task(state, current_user, website_id, "start", request).await
}

pub(crate) async fn restart_website(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(website_id): AxumPath<Uuid>,
    Json(request): Json<WebsiteActionRequest>,
) -> Result<Json<WebsiteActionResponse>, AppError> {
    website_network_expose_task(state, current_user, website_id, "restart", request).await
}

pub(crate) async fn stop_website(
    State(state): State<AppState>,
    AxumPath(website_id): AxumPath<Uuid>,
    Json(_request): Json<WebsiteActionRequest>,
) -> Result<Json<WebsiteActionResponse>, AppError> {
    let existing = state
        .store
        .websites()
        .get(website_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    let host_id = website_host_id(&existing)?;
    let item = state
        .store
        .websites()
        .set_status(website_id, WebsiteStatus::Stopped, None)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    if let Err(error) = sync_website_routes_for_host(&state, host_id).await {
        let message = error.0.to_string();
        let item = state
            .store
            .websites()
            .set_status(website_id, WebsiteStatus::Warning, Some(message))
            .await?
            .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
        record_website_event(&state, "website.warning", &item).await?;
        return Err(error);
    }
    record_website_event(&state, "website.stopped", &item).await?;
    Ok(Json(WebsiteActionResponse { item, task: None }))
}

pub(crate) async fn delete_website(
    State(state): State<AppState>,
    AxumPath(website_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let item = state
        .store
        .websites()
        .get(website_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    let host_id = website_host_id(&item)?;
    if item.status == WebsiteStatus::Running || item.status == WebsiteStatus::Warning {
        let stopped = state
            .store
            .websites()
            .set_status(website_id, WebsiteStatus::Stopped, None)
            .await?
            .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
        if let Err(error) = sync_website_routes_for_host(&state, host_id).await {
            let message = error.0.to_string();
            let _ = state
                .store
                .websites()
                .set_status(website_id, WebsiteStatus::Warning, Some(message))
                .await;
            record_website_event(&state, "website.warning", &stopped).await?;
            return Err(error);
        }
    }
    if !state.store.websites().delete(website_id).await? {
        return Err(AppError::status(StatusCode::NOT_FOUND, "website not found"));
    }
    record_website_event(&state, "website.deleted", &item).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn website_network_expose_task(
    state: AppState,
    current_user: CurrentUser,
    website_id: Uuid,
    action: &'static str,
    request: WebsiteActionRequest,
) -> Result<Json<WebsiteActionResponse>, AppError> {
    let item = state
        .store
        .websites()
        .get(website_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    let host_id = website_host_id(&item)?;
    ensure_network_expose_ready(&state, host_id).await?;
    let task = create_website_task(
        &state,
        host_id,
        current_user.username,
        format!("{action} website {}", item.primary_domain),
        format!("{action} website reverse proxy route"),
        serde_json::json!({
            "resource": "website",
            "action": action,
            "website_id": website_id,
            "reason": request.reason,
        }),
    )
    .await?;
    Ok(Json(WebsiteActionResponse {
        item,
        task: Some(task),
    }))
}

pub(crate) async fn create_website_task(
    state: &AppState,
    host_id: Uuid,
    created_by: String,
    title: String,
    summary: impl Into<String>,
    payload: Value,
) -> Result<Task, AppError> {
    let step_id = Uuid::new_v4();
    state
        .store
        .tasks()
        .create_with_steps(NewTask {
            id: Uuid::new_v4(),
            host_id: Some(host_id),
            title,
            prompt: None,
            status: TaskStatus::WaitingApproval,
            created_by,
            created_at: Utc::now(),
            metadata: serde_json::json!({}),
            create_step_approvals: true,
            steps: vec![TaskStep {
                id: step_id,
                capability: CapabilityName::NetworkExpose,
                risk: CapabilityRisk::High,
                summary: summary.into(),
                status: TaskStepStatus::Pending,
                payload,
            }],
        })
        .await
        .map_err(AppError::from)
}

pub(crate) async fn apply_approved_website_task(state: &AppState, task_id: Uuid, step_id: Uuid) {
    let Ok(tasks) = state.store.tasks().list().await else {
        tracing::warn!("failed to inspect task after approval");
        return;
    };
    let Some(task) = tasks.into_iter().find(|task| task.id == task_id) else {
        return;
    };
    let Some(step) = task.steps.into_iter().find(|step| step.id == step_id) else {
        return;
    };
    if step.capability != CapabilityName::NetworkExpose {
        return;
    }
    if step.payload.get("resource").and_then(Value::as_str) != Some("website") {
        return;
    }
    let Some(website_id) = step
        .payload
        .get("website_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
    else {
        tracing::warn!(task_id = %task.id, "approved website task has invalid website_id");
        return;
    };
    if let Err(error) = apply_website_route(state, website_id).await {
        tracing::warn!(?error, website_id = %website_id, "failed to apply approved website route");
        let message = error.0.to_string();
        let _ = state
            .store
            .tasks()
            .update_step_status(step_id, "failed")
            .await;
        let _ = state
            .store
            .tasks()
            .update_status(task.id, TaskStatus::Failed, Some(Utc::now()), Some(message))
            .await;
        return;
    }
    let _ = state
        .store
        .tasks()
        .update_step_status(step_id, "succeeded")
        .await;
    let _ = state
        .store
        .tasks()
        .update_status(task.id, TaskStatus::Succeeded, Some(Utc::now()), None)
        .await;
}

pub(crate) async fn apply_website_route(
    state: &AppState,
    website_id: Uuid,
) -> Result<(), AppError> {
    let existing = state
        .store
        .websites()
        .get(website_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    let host_id = website_host_id(&existing)?;
    ensure_network_expose_ready(state, host_id).await?;
    let item = state
        .store
        .websites()
        .set_status(website_id, WebsiteStatus::Running, None)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    if let Err(error) = sync_website_routes_for_host(state, host_id).await {
        let message = error.0.to_string();
        let _ = state
            .store
            .websites()
            .set_status(website_id, WebsiteStatus::Warning, Some(message))
            .await;
        return Err(error);
    }
    record_website_event(state, "website.running", &item).await?;
    Ok(())
}

pub(crate) async fn sync_website_routes_for_host(
    state: &AppState,
    host_id: Uuid,
) -> Result<grpc::WebsiteRoutesAppliedEvent, AppError> {
    ensure_network_expose_ready(state, host_id).await?;
    let result = apply_website_routes_for_host(&state.store, &state.agent_streams, host_id).await?;
    tracing::info!(
        host_id = %host_id,
        route_count = result.route_count,
        "applied website routes on agent"
    );
    Ok(result)
}

pub(crate) async fn sync_running_websites_for_connected_host(
    store: Store,
    agent_streams: AgentStreamRegistry,
    host_id: Uuid,
) {
    if let Err(error) = apply_website_routes_for_host(&store, &agent_streams, host_id).await {
        tracing::warn!(
            ?error,
            host_id = %host_id,
            "failed to sync running website routes for connected agent"
        );
    }
}

pub(crate) async fn apply_website_routes_for_host(
    store: &Store,
    agent_streams: &AgentStreamRegistry,
    host_id: Uuid,
) -> Result<grpc::WebsiteRoutesAppliedEvent, AppError> {
    let websites = store.websites().running_by_host(host_id).await?;
    let routes = websites
        .iter()
        .map(website_to_grpc_route)
        .collect::<Vec<_>>();
    agent_streams
        .apply_website_routes(
            host_id,
            grpc::ApplyWebsiteRoutesCommand {
                command_id: String::new(),
                routes,
            },
        )
        .await
        .map_err(|error| AppError::status(StatusCode::BAD_GATEWAY, agent_task_error_message(error)))
}

pub(crate) async fn ensure_network_expose_ready(
    state: &AppState,
    host_id: Uuid,
) -> Result<Uuid, AppError> {
    let hosts = state.store.hosts().list().await?;
    let host = hosts
        .into_iter()
        .find(|host| host.id == host_id)
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "host not found"))?;
    if host.status != HostStatus::Online {
        return Err(AppError::status(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent is not online",
        ));
    }
    if !host
        .capabilities
        .iter()
        .any(|capability| capability.name == CapabilityName::NetworkExpose)
    {
        return Err(AppError::status(
            StatusCode::FORBIDDEN,
            "agent does not declare NetworkExpose capability",
        ));
    }
    state
        .agent_streams
        .agent_id_for_host(host_id)
        .await
        .ok_or_else(|| {
            AppError::status(
                StatusCode::SERVICE_UNAVAILABLE,
                "agent stream is not connected",
            )
        })
}

pub(crate) fn website_to_grpc_route(website: &Website) -> grpc::WebsiteRoute {
    grpc::WebsiteRoute {
        website_id: website.id.to_string(),
        primary_domain: website.primary_domain.clone(),
        aliases: website.aliases.clone(),
        status: serialize_website_status(website.status).to_string(),
        kind: serialize_website_kind(website.kind).to_string(),
        protocol: serialize_website_protocol(website.protocol).to_string(),
        listen_port: u32::from(website.listen_port),
        upstream_url: website.upstream.url.clone(),
        config_json: website.config.to_string(),
    }
}

pub(crate) async fn record_website_event(
    state: &AppState,
    event_type: &str,
    website: &Website,
) -> Result<(), AppError> {
    state
        .store
        .events()
        .record(NewAgentEvent {
            agent_id: None,
            host_id: website.host_id,
            external_event_id: None,
            event_type: event_type.to_string(),
            event_json: serde_json::json!({
                "website_id": website.id,
                "primary_domain": website.primary_domain,
                "status": website.status,
            }),
            recorded_at: Utc::now(),
        })
        .await?;
    Ok(())
}

pub(crate) fn validate_website_request(
    host_id: Uuid,
    name: String,
    primary_domain: String,
    aliases: Vec<String>,
    listen_port: u16,
    upstream_url: String,
    notes: Option<String>,
) -> Result<WebsiteChanges, AppError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "website name is required",
        ));
    }
    let primary_domain = normalize_domain_input(&primary_domain)
        .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "website domain is required"))?;
    let aliases = aliases
        .into_iter()
        .filter_map(|alias| normalize_domain_input(&alias))
        .filter(|alias| alias != &primary_domain)
        .collect::<Vec<_>>();
    if listen_port == 0 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "website listen port is invalid",
        ));
    }
    let upstream_url = upstream_url.trim().to_string();
    validate_http_upstream(&upstream_url)?;
    Ok(WebsiteChanges {
        host_id,
        name,
        primary_domain,
        aliases,
        listen_port,
        upstream_url,
        notes: normalize_optional_text(notes),
    })
}

pub(crate) fn website_host_id(website: &Website) -> Result<Uuid, AppError> {
    website.host_id.ok_or_else(|| {
        AppError::status(
            StatusCode::CONFLICT,
            "website must be bound to a host before agent route operations",
        )
    })
}

pub(crate) fn validate_http_upstream(upstream_url: &str) -> Result<(), AppError> {
    if upstream_url.trim().is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "website upstream URL is required",
        ));
    }
    let url = Url::parse(upstream_url).map_err(|_| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            "website upstream URL must be an absolute http or https URL",
        )
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "website upstream URL must be an absolute http or https URL",
        ));
    }
    if url.host_str().is_none() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "website upstream URL must include a host",
        ));
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "website upstream URL path, query, and fragment are not supported in v1",
        ));
    }
    Ok(())
}

pub(crate) fn serialize_website_status(status: WebsiteStatus) -> &'static str {
    match status {
        WebsiteStatus::Stopped => "stopped",
        WebsiteStatus::Running => "running",
        WebsiteStatus::Warning => "warning",
    }
}

pub(crate) fn serialize_website_kind(kind: WebsiteKind) -> &'static str {
    match kind {
        WebsiteKind::ReverseProxy => "reverse_proxy",
        WebsiteKind::StaticSite => "static_site",
        WebsiteKind::TcpProxy => "tcp_proxy",
        WebsiteKind::UdpProxy => "udp_proxy",
    }
}

pub(crate) fn serialize_website_protocol(protocol: WebsiteProtocol) -> &'static str {
    match protocol {
        WebsiteProtocol::Http => "http",
        WebsiteProtocol::Https => "https",
        WebsiteProtocol::Tcp => "tcp",
        WebsiteProtocol::Udp => "udp",
    }
}

pub(crate) fn normalize_domain_input(value: &str) -> Option<String> {
    let domain = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() || domain.contains('/') || domain.contains(':') {
        None
    } else {
        Some(domain)
    }
}
