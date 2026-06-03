use crate::agent_streams::file_command_app_error;
use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FilePathQuery {
    path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FileSearchQuery {
    path: Option<String>,
    query: String,
    limit: Option<u32>,
}

pub(crate) async fn list_files(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(host_id): AxumPath<Uuid>,
    Query(query): Query<FilePathQuery>,
) -> Result<Json<FileDirectoryResponse>, AppError> {
    let path = query.path.unwrap_or_else(|| "/".to_string());
    ensure_file_capability(&state, host_id, CapabilityName::FilesRead).await?;
    record_file_event(
        &state,
        host_id,
        "file.list_requested",
        serde_json::json!({
            "path": path,
            "requested_by": current_user.username,
        }),
    )
    .await?;
    let result = state
        .agent_streams
        .list_directory(host_id, path)
        .await
        .map_err(file_command_app_error)?;
    Ok(Json(file_result_json(&result)?))
}

pub(crate) async fn search_files(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(host_id): AxumPath<Uuid>,
    Query(query): Query<FileSearchQuery>,
) -> Result<Json<FileSearchResponse>, AppError> {
    let path = query.path.unwrap_or_else(|| "/".to_string());
    let search_query = query.query.trim().to_string();
    if search_query.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "file search query is required",
        ));
    }
    ensure_file_capability(&state, host_id, CapabilityName::FilesRead).await?;
    let limit = query.limit.unwrap_or(DEFAULT_FILE_SEARCH_LIMIT).min(500);
    record_file_event(
        &state,
        host_id,
        "file.search_requested",
        serde_json::json!({
            "path": path,
            "query": search_query,
            "limit": limit,
            "requested_by": current_user.username,
        }),
    )
    .await?;
    let result = state
        .agent_streams
        .search_files(host_id, path, search_query, limit)
        .await
        .map_err(file_command_app_error)?;
    Ok(Json(file_result_json(&result)?))
}

pub(crate) async fn download_file(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(host_id): AxumPath<Uuid>,
    Query(query): Query<FilePathQuery>,
) -> Result<Json<FileDownloadResponse>, AppError> {
    let path = required_query_path(query.path)?;
    ensure_file_capability(&state, host_id, CapabilityName::FilesRead).await?;
    record_file_event(
        &state,
        host_id,
        "file.download_requested",
        serde_json::json!({
            "path": path,
            "requested_by": current_user.username,
        }),
    )
    .await?;
    let result = state
        .agent_streams
        .read_file(host_id, path)
        .await
        .map_err(file_command_app_error)?;
    let metadata: Value = parse_file_result_value(&result)?;
    Ok(Json(FileDownloadResponse {
        path: metadata
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: metadata
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("download")
            .to_string(),
        content_base64: base64::engine::general_purpose::STANDARD.encode(result.content),
        size_bytes: metadata
            .get("size_bytes")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
    }))
}

pub(crate) async fn upload_file(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(host_id): AxumPath<Uuid>,
    Json(request): Json<FileUploadRequest>,
) -> Result<Json<FileUploadResponse>, AppError> {
    ensure_file_capability(&state, host_id, CapabilityName::FilesWrite).await?;
    let content = base64::engine::general_purpose::STANDARD
        .decode(request.content_base64.as_bytes())
        .map_err(|_| AppError::status(StatusCode::BAD_REQUEST, "invalid base64 file content"))?;
    if content.len() > MAX_FILE_TRANSFER_BYTES {
        return Err(AppError::status(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file upload exceeds the 64 MiB transfer limit",
        ));
    }
    record_file_event(
        &state,
        host_id,
        "file.upload_requested",
        serde_json::json!({
            "path": request.path,
            "content_bytes": content.len(),
            "overwrite": request.overwrite.unwrap_or(false),
            "requested_by": current_user.username,
        }),
    )
    .await?;
    let result = state
        .agent_streams
        .run_file_operation(
            host_id,
            grpc::RunFileOperationCommand {
                command_id: String::new(),
                operation: "upload".to_string(),
                path: request.path,
                target_path: String::new(),
                name: String::new(),
                content,
                overwrite: request.overwrite.unwrap_or(false),
            },
        )
        .await
        .map_err(file_command_app_error)?;
    let response: FileOperationResponse = file_result_json(&result)?;
    let item = response
        .item
        .ok_or_else(|| AppError::status(StatusCode::BAD_GATEWAY, "agent did not return file"))?;
    record_file_event(
        &state,
        host_id,
        "file.upload_completed",
        serde_json::json!({
            "path": item.path,
            "requested_by": current_user.username,
        }),
    )
    .await?;
    Ok(Json(FileUploadResponse { item }))
}

pub(crate) async fn run_file_operation(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(host_id): AxumPath<Uuid>,
    Json(request): Json<FileOperationRequest>,
) -> Result<Json<FileOperationResponse>, AppError> {
    ensure_file_capability(&state, host_id, CapabilityName::FilesWrite).await?;
    let operation = file_operation_label(request.operation);
    record_file_event(
        &state,
        host_id,
        "file.operation_requested",
        serde_json::json!({
            "operation": operation,
            "path": request.path,
            "target_path": request.target_path,
            "name": request.name,
            "overwrite": request.overwrite.unwrap_or(false),
            "requested_by": current_user.username,
        }),
    )
    .await?;
    let result = state
        .agent_streams
        .run_file_operation(
            host_id,
            grpc::RunFileOperationCommand {
                command_id: String::new(),
                operation: operation.to_string(),
                path: request.path,
                target_path: request.target_path.unwrap_or_default(),
                name: request.name.unwrap_or_default(),
                content: Vec::new(),
                overwrite: request.overwrite.unwrap_or(false),
            },
        )
        .await
        .map_err(file_command_app_error)?;
    let response: FileOperationResponse = file_result_json(&result)?;
    record_file_event(
        &state,
        host_id,
        "file.operation_completed",
        serde_json::json!({
            "operation": operation,
            "message": response.message,
            "requested_by": current_user.username,
        }),
    )
    .await?;
    Ok(Json(response))
}

pub(crate) async fn ensure_file_capability(
    state: &AppState,
    host_id: Uuid,
    capability: CapabilityName,
) -> Result<(), AppError> {
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
        .any(|declared| declared.name == capability)
    {
        return Err(AppError::status(
            StatusCode::FORBIDDEN,
            "agent does not declare required file capability",
        ));
    }
    Ok(())
}

pub(crate) async fn record_file_event(
    state: &AppState,
    host_id: Uuid,
    event_type: impl Into<String>,
    event_json: Value,
) -> Result<(), AppError> {
    state
        .store
        .events()
        .record(NewAgentEvent {
            agent_id: None,
            host_id: Some(host_id),
            event_type: event_type.into(),
            event_json,
            recorded_at: Utc::now(),
        })
        .await?;
    Ok(())
}

pub(crate) fn file_operation_label(operation: FileOperationKind) -> &'static str {
    match operation {
        FileOperationKind::CreateDirectory => "create_directory",
        FileOperationKind::Rename => "rename",
        FileOperationKind::Move => "move",
        FileOperationKind::Copy => "copy",
        FileOperationKind::Delete => "delete",
    }
}

pub(crate) fn required_query_path(path: Option<String>) -> Result<String, AppError> {
    path.map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "path is required"))
}

pub(crate) fn parse_file_result_value(
    result: &grpc::FileCommandResultEvent,
) -> Result<Value, AppError> {
    serde_json::from_str(&result.result_json)
        .map_err(|_| AppError::status(StatusCode::BAD_GATEWAY, "agent returned invalid file JSON"))
}

pub(crate) fn file_result_json<T: for<'de> Deserialize<'de>>(
    result: &grpc::FileCommandResultEvent,
) -> Result<T, AppError> {
    serde_json::from_str(&result.result_json)
        .map_err(|_| AppError::status(StatusCode::BAD_GATEWAY, "agent returned invalid file JSON"))
}
