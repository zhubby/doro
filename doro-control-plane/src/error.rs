use crate::prelude::*;

#[derive(Debug)]
pub struct AppError(pub(crate) anyhow::Error);

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

pub(crate) fn scheduled_task_store_app_error(error: sea_orm::DbErr) -> AppError {
    match error {
        sea_orm::DbErr::RecordNotFound(message) => AppError::status(StatusCode::NOT_FOUND, message),
        other => other.into(),
    }
}

pub(crate) fn store_status(error: sea_orm::DbErr) -> Status {
    if let sea_orm::DbErr::Custom(message) = &error
        && message.contains("is not enrolled")
    {
        tracing::warn!(%error, "agent identity is not enrolled");
        return Status::failed_precondition(message.clone());
    }

    tracing::error!(%error, "store operation failed");
    Status::internal("store operation failed")
}

pub(crate) fn enrollment_status(error: sea_orm::DbErr) -> Status {
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
    pub(crate) fn status(status: StatusCode, message: impl Into<String>) -> Self {
        Self(anyhow::anyhow!(ApiError {
            status,
            message: message.into(),
        }))
    }
}

pub(crate) fn approval_store_app_error(error: sea_orm::DbErr) -> AppError {
    match &error {
        sea_orm::DbErr::RecordNotFound(_) => {
            AppError::status(StatusCode::NOT_FOUND, "approval target not found")
        }
        sea_orm::DbErr::Custom(message)
            if message.contains("approval expired")
                || message.contains("approval already resolved") =>
        {
            AppError::status(StatusCode::CONFLICT, message.clone())
        }
        _ => error.into(),
    }
}

pub(crate) fn website_store_app_error(error: sea_orm::DbErr) -> AppError {
    match &error {
        sea_orm::DbErr::Custom(message)
            if message.contains("already exists") || message.contains("must be stopped") =>
        {
            AppError::status(StatusCode::CONFLICT, message.clone())
        }
        _ => error.into(),
    }
}

pub(crate) fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug)]
pub(crate) struct ApiError {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enrollment_errors_map_to_permission_denied() {
        let status = enrollment_status(sea_orm::DbErr::Custom(
            "enrollment token is expired".to_string(),
        ));

        assert_eq!(status.code(), tonic::Code::PermissionDenied);
    }
}
