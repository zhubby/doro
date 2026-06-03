use crate::auth::CurrentUser;
use crate::error::{AppError, normalize_optional_text, website_store_app_error};
use crate::prelude::*;
use crate::state::AppState;

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
    let changes = validate_website_request(
        request.name,
        request.primary_domain,
        request.aliases,
        request.listen_port.unwrap_or(state.website_http_port),
        request.upstream_url,
        request.notes,
    )?;
    let now = Utc::now();
    let item = match state
        .store
        .websites()
        .create(NewWebsite {
            id: Uuid::new_v4(),
            host_id: None,
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
        request.name,
        request.primary_domain,
        request.aliases,
        request.listen_port.unwrap_or(state.website_http_port),
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
    let item = state
        .store
        .websites()
        .set_status(website_id, WebsiteStatus::Stopped, None)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    reload_website_routes(&state).await?;
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
    if !state.store.websites().delete(website_id).await? {
        return Err(AppError::status(StatusCode::NOT_FOUND, "website not found"));
    }
    reload_website_routes(&state).await?;
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
    let task = create_website_task(
        &state,
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
            host_id: None,
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
    }
}

pub(crate) async fn apply_website_route(
    state: &AppState,
    website_id: Uuid,
) -> Result<(), AppError> {
    let item = state
        .store
        .websites()
        .set_status(website_id, WebsiteStatus::Running, None)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "website not found"))?;
    if let Err(error) = reload_website_routes(state).await {
        let message = error.0.to_string();
        let _ = state
            .store
            .websites()
            .set_status(website_id, WebsiteStatus::Warning, Some(message))
            .await;
        let _ = reload_website_routes(state).await;
        return Err(error);
    }
    record_website_event(state, "website.running", &item).await?;
    Ok(())
}

pub(crate) async fn reload_website_routes(state: &AppState) -> Result<(), AppError> {
    let websites = state.store.websites().running().await?;
    state
        .website_runtime
        .reload(&websites)
        .map(|route_count| {
            tracing::info!(route_count, "reloaded website proxy routes");
        })
        .map_err(|error| AppError::status(StatusCode::BAD_REQUEST, error.to_string()))
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
    let validation_website = Website {
        id: Uuid::new_v4(),
        host_id: None,
        name: name.clone(),
        primary_domain: primary_domain.clone(),
        aliases: aliases.clone(),
        status: WebsiteStatus::Running,
        kind: WebsiteKind::ReverseProxy,
        protocol: WebsiteProtocol::Http,
        listen_port,
        upstream: doro_protocol::WebsiteProxyTarget {
            url: upstream_url.clone(),
        },
        app_install_id: None,
        tls_certificate_id: None,
        config: serde_json::json!({}),
        notes: None,
        last_runtime_error: None,
        last_checked_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    doro_website::WebsiteRoute::from_website(&validation_website)
        .map_err(|error| AppError::status(StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(WebsiteChanges {
        name,
        primary_domain,
        aliases,
        listen_port,
        upstream_url,
        notes: normalize_optional_text(notes),
    })
}

pub(crate) fn normalize_domain_input(value: &str) -> Option<String> {
    let domain = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() || domain.contains('/') || domain.contains(':') {
        None
    } else {
        Some(domain)
    }
}
