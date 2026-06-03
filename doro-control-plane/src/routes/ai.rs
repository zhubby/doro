use crate::error::{AppError, ai_model_provider_store_app_error, normalize_optional_text};
use crate::prelude::*;
use crate::routes::scheduled_tasks::required_text;
use crate::state::AppState;
use url::Url;

pub(crate) async fn list_ai_model_providers(
    State(state): State<AppState>,
) -> Result<Json<ListAiModelProvidersResponse>, AppError> {
    Ok(Json(ListAiModelProvidersResponse {
        items: state.store.ai_model_providers().list().await?,
    }))
}

pub(crate) async fn get_ai_model_provider(
    State(state): State<AppState>,
    AxumPath(provider_id): AxumPath<Uuid>,
) -> Result<Json<AiModelProviderResponse>, AppError> {
    let item = state
        .store
        .ai_model_providers()
        .get(provider_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "ai provider not found"))?;
    Ok(Json(AiModelProviderResponse { item }))
}

pub(crate) async fn create_ai_model_provider(
    State(state): State<AppState>,
    Json(request): Json<CreateAiModelProviderRequest>,
) -> Result<Json<AiModelProviderResponse>, AppError> {
    let now = Utc::now();
    let item = state
        .store
        .ai_model_providers()
        .create(NewAiModelProvider {
            id: Uuid::new_v4(),
            name: required_text(request.name, "ai provider name is required")?,
            base_url: validate_provider_base_url(request.base_url)?,
            default_model: required_text(
                request.default_model,
                "ai provider default_model is required",
            )?,
            timeout_seconds: request.timeout_seconds,
            api_key_secret: required_text(request.api_key, "ai provider api_key is required")?,
            enabled: request.enabled,
            created_at: now,
        })
        .await
        .map_err(ai_model_provider_store_app_error)?;

    Ok(Json(AiModelProviderResponse { item }))
}

pub(crate) async fn update_ai_model_provider(
    State(state): State<AppState>,
    AxumPath(provider_id): AxumPath<Uuid>,
    Json(request): Json<UpdateAiModelProviderRequest>,
) -> Result<Json<AiModelProviderResponse>, AppError> {
    let item = state
        .store
        .ai_model_providers()
        .update(
            provider_id,
            AiModelProviderChanges {
                name: request.name.map(|name| name.trim().to_string()),
                base_url: request
                    .base_url
                    .map(validate_provider_base_url)
                    .transpose()?,
                default_model: request
                    .default_model
                    .map(|default_model| default_model.trim().to_string()),
                timeout_seconds: request.timeout_seconds,
                api_key_secret: normalize_optional_text(request.api_key),
                enabled: request.enabled,
            },
        )
        .await
        .map_err(ai_model_provider_store_app_error)?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "ai provider not found"))?;

    Ok(Json(AiModelProviderResponse { item }))
}

pub(crate) async fn delete_ai_model_provider(
    State(state): State<AppState>,
    AxumPath(provider_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if !state.store.ai_model_providers().delete(provider_id).await? {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "ai provider not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

fn validate_provider_base_url(value: String) -> Result<String, AppError> {
    let value = required_text(value, "ai provider base_url is required")?
        .trim_end_matches('/')
        .to_string();
    let parsed = Url::parse(&value).map_err(|_| {
        AppError::status(StatusCode::BAD_REQUEST, "ai provider base_url is invalid")
    })?;
    match parsed.scheme() {
        "http" | "https" => Ok(value),
        _ => Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "ai provider base_url must use http or https",
        )),
    }
}
