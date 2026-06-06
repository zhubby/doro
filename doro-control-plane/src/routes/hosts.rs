use crate::agent_events::{container_snapshot_payload, ingest_agent_event};
use crate::auth::generate_enrollment_token;
use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MetricHistoryQuery {
    limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ListHostsQuery {
    tags: Option<String>,
}

pub(crate) async fn list_hosts(
    State(state): State<AppState>,
    Query(query): Query<ListHostsQuery>,
) -> Result<Json<ListHostsResponse>, AppError> {
    let tags = query.tags.as_deref().map(split_tags).unwrap_or_default();
    Ok(Json(ListHostsResponse {
        items: state.store.hosts().list_by_tags(tags).await?,
    }))
}

pub(crate) async fn delete_host(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if state.store.hosts().delete(host_id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }

    Err(AppError::status(StatusCode::NOT_FOUND, "host not found"))
}

fn split_tags(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) async fn update_host(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
    Json(request): Json<UpdateHostRequest>,
) -> Result<Json<UpdateHostResponse>, AppError> {
    match state
        .store
        .hosts()
        .update(host_id, request.display_name, request.labels)
        .await
    {
        Ok(host) => Ok(Json(UpdateHostResponse { item: host })),
        Err(sea_orm::DbErr::RecordNotFound(_)) => {
            Err(AppError::status(StatusCode::NOT_FOUND, "host not found"))
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn create_enrollment_token(
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

pub(crate) async fn latest_host_metric(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
) -> Result<Json<LatestMetricResponse>, AppError> {
    Ok(Json(LatestMetricResponse {
        item: state.store.metrics().latest_for_host(host_id).await?,
    }))
}

pub(crate) async fn list_host_metrics(
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

pub(crate) async fn list_host_containers(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
) -> Result<Json<ListHostContainersResponse>, AppError> {
    Ok(Json(ListHostContainersResponse {
        items: state.store.containers().list_by_host(host_id).await?,
    }))
}

pub(crate) async fn refresh_containers(
    State(state): State<AppState>,
) -> Result<Json<ListHostContainersResponse>, AppError> {
    let hosts = state.store.hosts().list().await?;
    let online_hosts = hosts
        .into_iter()
        .filter(|host| host.status == HostStatus::Online)
        .collect::<Vec<_>>();
    if online_hosts.is_empty() {
        return Ok(Json(ListHostContainersResponse {
            items: state.store.containers().list().await?,
        }));
    }

    let mut snapshots = Vec::with_capacity(online_hosts.len());
    for host in &online_hosts {
        match state.agent_streams.collect_containers(host.id).await {
            Ok(snapshot) => snapshots.push((host.id, snapshot)),
            Err(error) => {
                tracing::warn!(
                    ?error,
                    host_id = %host.id,
                    hostname = %host.hostname,
                    "failed to refresh containers from agent"
                );
            }
        }
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

    Ok(Json(ListHostContainersResponse {
        items: state.store.containers().list().await?,
    }))
}
