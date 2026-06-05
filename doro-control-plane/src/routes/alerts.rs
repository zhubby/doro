use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AlertIncidentQuery {
    limit: Option<u64>,
}

pub(crate) async fn list_alert_rules(
    State(state): State<AppState>,
) -> Result<Json<ListAlertRulesResponse>, AppError> {
    Ok(Json(ListAlertRulesResponse {
        items: state.store.alerts().list_rules().await?,
    }))
}

pub(crate) async fn create_alert_rule(
    State(state): State<AppState>,
    Json(request): Json<CreateAlertRuleRequest>,
) -> Result<Json<AlertRuleResponse>, AppError> {
    let item = state
        .store
        .alerts()
        .create_rule(NewAlertRule {
            id: Uuid::new_v4(),
            name: request.name,
            description: request.description,
            severity: request.severity,
            metric: request.metric,
            operator: request.operator,
            threshold: request.threshold,
            host_id: request.host_id,
            enabled: request.enabled,
            for_seconds: request.for_seconds,
            cooldown_seconds: request.cooldown_seconds,
            created_at: Utc::now(),
        })
        .await
        .map_err(alert_store_app_error)?;
    Ok(Json(AlertRuleResponse { item }))
}

pub(crate) async fn update_alert_rule(
    State(state): State<AppState>,
    AxumPath(rule_id): AxumPath<Uuid>,
    Json(request): Json<UpdateAlertRuleRequest>,
) -> Result<Json<AlertRuleResponse>, AppError> {
    let item = state
        .store
        .alerts()
        .update_rule(
            rule_id,
            AlertRuleChanges {
                name: Some(request.name),
                description: Some(request.description),
                severity: Some(request.severity),
                metric: Some(request.metric),
                operator: Some(request.operator),
                threshold: Some(request.threshold),
                host_id: Some(request.host_id),
                enabled: Some(request.enabled),
                for_seconds: Some(request.for_seconds),
                cooldown_seconds: Some(request.cooldown_seconds),
            },
        )
        .await
        .map_err(alert_store_app_error)?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "alert rule not found"))?;
    Ok(Json(AlertRuleResponse { item }))
}

pub(crate) async fn delete_alert_rule(
    State(state): State<AppState>,
    AxumPath(rule_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if state.store.alerts().delete_rule(rule_id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }
    Err(AppError::status(
        StatusCode::NOT_FOUND,
        "alert rule not found",
    ))
}

pub(crate) async fn list_alert_incidents(
    State(state): State<AppState>,
    Query(query): Query<AlertIncidentQuery>,
) -> Result<Json<ListAlertIncidentsResponse>, AppError> {
    Ok(Json(ListAlertIncidentsResponse {
        items: state
            .store
            .alerts()
            .list_incidents(query.limit.unwrap_or(100))
            .await?,
    }))
}

fn alert_store_app_error(error: sea_orm::DbErr) -> AppError {
    match &error {
        sea_orm::DbErr::Custom(message)
            if message.contains("is required")
                || message.contains("unsupported")
                || message.contains("JSON pointer") =>
        {
            AppError::status(StatusCode::BAD_REQUEST, message.clone())
        }
        _ => error.into(),
    }
}
