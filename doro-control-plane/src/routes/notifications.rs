use crate::error::AppError;
use crate::notifications::{public_email_settings, save_email_settings, send_test_email};
use crate::prelude::*;
use crate::state::AppState;

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
