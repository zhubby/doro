use crate::error::AppError;
use crate::notifications::{
    list_system_notifications, mark_system_notification_read, public_email_settings,
    public_system_notification_settings, save_email_settings, save_system_notification_settings,
    send_test_email,
};
use crate::prelude::*;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub(crate) struct ListSystemNotificationsQuery {
    status: Option<SystemNotificationStatus>,
    limit: Option<u64>,
}

pub(crate) async fn get_email_notification_settings(
    State(state): State<AppState>,
) -> Result<Json<EmailNotificationSettingsResponse>, AppError> {
    Ok(Json(EmailNotificationSettingsResponse {
        item: public_email_settings(&state.store).await?,
    }))
}

pub(crate) async fn update_email_notification_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateEmailNotificationSettingsRequest>,
) -> Result<Json<EmailNotificationSettingsResponse>, AppError> {
    Ok(Json(EmailNotificationSettingsResponse {
        item: save_email_settings(&state.store, request).await?,
    }))
}

pub(crate) async fn get_system_notification_settings(
    State(state): State<AppState>,
) -> Result<Json<SystemNotificationSettingsResponse>, AppError> {
    Ok(Json(SystemNotificationSettingsResponse {
        item: public_system_notification_settings(&state.store).await?,
    }))
}

pub(crate) async fn update_system_notification_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateSystemNotificationSettingsRequest>,
) -> Result<Json<SystemNotificationSettingsResponse>, AppError> {
    Ok(Json(SystemNotificationSettingsResponse {
        item: save_system_notification_settings(&state.store, request).await?,
    }))
}

pub(crate) async fn list_system_notifications_route(
    State(state): State<AppState>,
    Query(query): Query<ListSystemNotificationsQuery>,
) -> Result<Json<ListSystemNotificationsResponse>, AppError> {
    Ok(Json(ListSystemNotificationsResponse {
        items: list_system_notifications(&state.store, query.status, query.limit.unwrap_or(100))
            .await?,
    }))
}

pub(crate) async fn mark_system_notification_read_route(
    State(state): State<AppState>,
    AxumPath(notification_id): AxumPath<Uuid>,
) -> Result<Json<SystemNotificationResponse>, AppError> {
    Ok(Json(SystemNotificationResponse {
        item: mark_system_notification_read(&state.store, notification_id).await?,
    }))
}

pub(crate) async fn test_email_notification(
    State(state): State<AppState>,
    Json(request): Json<TestEmailNotificationRequest>,
) -> Result<Json<TestEmailNotificationResponse>, AppError> {
    send_test_email(&state.store, request.recipient).await?;
    Ok(Json(TestEmailNotificationResponse {
        sent: true,
        message: "test email sent".to_string(),
    }))
}
