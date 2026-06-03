use crate::agent_tools::apply_agent_tool_approval_decision;
use crate::auth::CurrentUser;
use crate::error::{AppError, approval_store_app_error, normalize_optional_text};
use crate::prelude::*;
use crate::routes::docker::apply_approved_docker_task;
use crate::routes::scheduled_tasks::apply_approved_scheduled_task;
use crate::routes::virtual_machines::apply_approved_virtual_machine_task;
use crate::routes::websites::apply_approved_website_task;
use crate::state::AppState;

pub(crate) async fn list_approvals(
    State(state): State<AppState>,
) -> Result<Json<ListApprovalsResponse>, AppError> {
    Ok(Json(ListApprovalsResponse {
        items: state.store.approvals().list().await?,
    }))
}

pub(crate) async fn create_approval(
    State(state): State<AppState>,
    Json(request): Json<CreateApprovalRequest>,
) -> Result<Json<CreateApprovalResponse>, AppError> {
    let requested_at = Utc::now();
    let reason = request.reason.trim().to_string();
    if reason.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "approval reason is required",
        ));
    }
    let expires_at = request
        .expires_at
        .unwrap_or_else(|| requested_at + ChronoDuration::hours(DEFAULT_APPROVAL_TTL_HOURS));
    if expires_at <= requested_at {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "approval expires_at must be in the future",
        ));
    }

    match state
        .store
        .approvals()
        .create(NewApproval {
            id: Uuid::new_v4(),
            task_id: request.task_id,
            step_id: request.step_id,
            reason,
            requested_at,
            expires_at,
        })
        .await
    {
        Ok(item) => Ok(Json(CreateApprovalResponse { item })),
        Err(error) => Err(approval_store_app_error(error)),
    }
}

pub(crate) async fn delete_approval(
    State(state): State<AppState>,
    AxumPath(approval_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if state.store.approvals().delete(approval_id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }

    Err(AppError::status(
        StatusCode::NOT_FOUND,
        "approval not found",
    ))
}

pub(crate) async fn approve_approval(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(approval_id): AxumPath<Uuid>,
    Json(request): Json<ResolveApprovalRequest>,
) -> Result<Json<ResolveApprovalResponse>, AppError> {
    let decision_note = normalize_optional_text(request.decision_note);
    match state
        .store
        .approvals()
        .approve(
            approval_id,
            current_user.username,
            decision_note,
            Utc::now(),
        )
        .await
    {
        Ok(item) => {
            apply_approved_website_task(&state, item.task_id, item.step_id).await;
            apply_approved_docker_task(&state, item.task_id, item.step_id).await;
            apply_approved_virtual_machine_task(&state, item.task_id, item.step_id).await;
            apply_approved_scheduled_task(&state, &item).await;
            apply_agent_tool_approval_decision(&state, &item, true).await;
            Ok(Json(ResolveApprovalResponse { item }))
        }
        Err(error) => Err(approval_store_app_error(error)),
    }
}

pub(crate) async fn deny_approval(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(approval_id): AxumPath<Uuid>,
    Json(request): Json<ResolveApprovalRequest>,
) -> Result<Json<ResolveApprovalResponse>, AppError> {
    let decision_note = normalize_optional_text(request.decision_note);
    match state
        .store
        .approvals()
        .deny(
            approval_id,
            current_user.username,
            decision_note,
            Utc::now(),
        )
        .await
    {
        Ok(item) => {
            apply_agent_tool_approval_decision(&state, &item, false).await;
            Ok(Json(ResolveApprovalResponse { item }))
        }
        Err(error) => Err(approval_store_app_error(error)),
    }
}
