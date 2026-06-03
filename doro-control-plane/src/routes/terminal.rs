use crate::agent_events::timestamp_to_utc;
use crate::agent_streams::{command_status_label, terminal_command_app_error};
use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TerminalSocketQuery {
    token: String,
    cols: Option<u32>,
    rows: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum TerminalClientMessage {
    Input { data: String },
    Resize { cols: u32, rows: u32 },
}

pub(crate) async fn terminal_session_ws(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
    Query(query): Query<TerminalSocketQuery>,
    ws: WebSocketUpgrade,
) -> Result<AxumResponse, AppError> {
    let current_user = state.auth.verify_access_token(&query.token)?;
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
        .any(|capability| capability.name == CapabilityName::ShellExecute)
    {
        return Err(AppError::status(
            StatusCode::FORBIDDEN,
            "agent does not declare shell execution capability",
        ));
    }

    Ok(ws.on_upgrade(move |socket| {
        handle_terminal_socket(
            socket,
            state,
            current_user,
            host_id,
            query.cols.unwrap_or(100),
            query.rows.unwrap_or(28),
        )
    }))
}

pub(crate) async fn handle_terminal_socket(
    socket: WebSocket,
    state: AppState,
    current_user: CurrentUser,
    host_id: Uuid,
    cols: u32,
    rows: u32,
) {
    let session_id = Uuid::new_v4().to_string();
    let (output_sender, mut output_receiver) = mpsc::unbounded_channel();
    if let Err(error) = state
        .agent_streams
        .open_terminal_session(host_id, session_id.clone(), cols, rows, output_sender)
        .await
    {
        tracing::warn!(?error, host_id = %host_id, "failed to open terminal websocket session");
        return;
    }
    let _ = state
        .store
        .events()
        .record(NewAgentEvent {
            agent_id: None,
            host_id: Some(host_id),
            event_type: "terminal.session_opened".to_string(),
            event_json: serde_json::json!({
                "session_id": session_id,
                "host_id": host_id,
                "requested_by": current_user.username,
                "cols": cols,
                "rows": rows,
            }),
            recorded_at: Utc::now(),
        })
        .await;

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let output_task = tokio::spawn(async move {
        while let Some(output) = output_receiver.recv().await {
            if ws_sender.send(Message::Text(output)).await.is_err() {
                break;
            }
        }
    });

    while let Some(message) = ws_receiver.next().await {
        let Ok(message) = message else {
            break;
        };
        match message {
            Message::Text(text) => {
                if let Ok(message) = serde_json::from_str::<TerminalClientMessage>(&text) {
                    match message {
                        TerminalClientMessage::Input { data } => {
                            if state
                                .agent_streams
                                .send_terminal_input(host_id, session_id.clone(), data)
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        TerminalClientMessage::Resize { cols, rows } => {
                            let _ = state
                                .agent_streams
                                .resize_terminal_session(host_id, session_id.clone(), cols, rows)
                                .await;
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    state
        .agent_streams
        .close_terminal_session(host_id, session_id.clone(), "websocket closed".to_string())
        .await;
    output_task.abort();
    let _ = state
        .store
        .events()
        .record(NewAgentEvent {
            agent_id: None,
            host_id: Some(host_id),
            event_type: "terminal.session_closed".to_string(),
            event_json: serde_json::json!({
                "session_id": session_id,
                "host_id": host_id,
                "reason": "websocket closed",
            }),
            recorded_at: Utc::now(),
        })
        .await;
}

pub(crate) async fn run_terminal_command(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<TerminalCommandRequest>,
) -> Result<Json<TerminalCommandResponse>, AppError> {
    let input = request.input.trim();
    if input.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "terminal command input is required",
        ));
    }
    if request
        .timeout_seconds
        .unwrap_or(DEFAULT_TERMINAL_TIMEOUT_SECONDS)
        > MAX_TERMINAL_TIMEOUT_SECONDS
    {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!(
                "terminal command timeout must be {MAX_TERMINAL_TIMEOUT_SECONDS} seconds or less"
            ),
        ));
    }

    let hosts = state.store.hosts().list().await?;
    let host = hosts
        .into_iter()
        .find(|host| host.id == request.host_id)
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
        .any(|capability| capability.name == CapabilityName::ShellExecute)
    {
        return Err(AppError::status(
            StatusCode::FORBIDDEN,
            "agent does not declare shell execution capability",
        ));
    }

    state
        .store
        .events()
        .record(NewAgentEvent {
            agent_id: None,
            host_id: Some(request.host_id),
            event_type: "terminal.command_requested".to_string(),
            event_json: serde_json::json!({
                "host_id": request.host_id,
                "input": request.input,
                "requested_by": current_user.username,
                "timeout_seconds": request.timeout_seconds.unwrap_or(DEFAULT_TERMINAL_TIMEOUT_SECONDS),
            }),
            recorded_at: Utc::now(),
        })
        .await?;

    let result = state
        .agent_streams
        .run_terminal_command(&request)
        .await
        .map_err(terminal_command_app_error)?;
    let started_at = result
        .started_at
        .as_ref()
        .and_then(timestamp_to_utc)
        .unwrap_or_else(Utc::now);
    let finished_at = result
        .finished_at
        .as_ref()
        .and_then(timestamp_to_utc)
        .unwrap_or_else(Utc::now);
    let status = command_status_label(result.status);
    let exit_code = if result.exit_code < 0 {
        None
    } else {
        Some(result.exit_code)
    };

    state
        .store
        .events()
        .record(NewAgentEvent {
            agent_id: None,
            host_id: Some(request.host_id),
            event_type: "terminal.command_completed".to_string(),
            event_json: serde_json::json!({
                "command_id": result.command_id,
                "host_id": request.host_id,
                "status": status,
                "exit_code": exit_code,
                "output_bytes": result.output.len(),
            }),
            recorded_at: finished_at,
        })
        .await?;

    Ok(Json(TerminalCommandResponse {
        command_id: result.command_id,
        host_id: request.host_id,
        status: status.to_string(),
        output: result.output,
        exit_code,
        started_at,
        finished_at,
    }))
}
