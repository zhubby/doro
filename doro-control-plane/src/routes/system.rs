use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "doro-control-plane".to_string(),
    })
}

pub(crate) async fn control_plane_environment(
    State(state): State<AppState>,
) -> Json<ControlPlaneEnvironmentResponse> {
    Json(ControlPlaneEnvironmentResponse {
        item: state.control_plane_environment,
    })
}

pub(crate) fn collect_control_plane_environment() -> ControlPlaneEnvironment {
    let uptime_seconds = System::uptime().min(u32::MAX as u64);
    ControlPlaneEnvironment {
        hostname: System::host_name().unwrap_or_else(|| "unknown".to_string()),
        os_version: System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
        architecture: std::env::consts::ARCH.to_string(),
        host_address: control_plane_host_address().unwrap_or_else(|| "unknown".to_string()),
        booted_at: Utc::now().checked_sub_signed(ChronoDuration::seconds(uptime_seconds as i64)),
        uptime_seconds: uptime_seconds as u32,
    }
}

pub(crate) fn control_plane_host_address() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

pub(crate) async fn list_apps(
    State(state): State<AppState>,
) -> Result<Json<ListAppsResponse>, AppError> {
    Ok(Json(ListAppsResponse {
        items: state.store.apps().list().await?,
    }))
}

pub(crate) async fn settings(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse>, AppError> {
    Ok(Json(SettingsResponse {
        approval_policy: setting_string(
            &state.store,
            "approval_policy",
            "policy_and_human_approval",
        )
        .await?,
        agent_transport: setting_string(&state.store, "agent_transport", "grpc_protobuf").await?,
        database: setting_string(&state.store, "database", "postgres").await?,
    }))
}

pub(crate) async fn events() -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let stream = IntervalStream::new(tokio::time::interval(Duration::from_secs(15))).map(|_| {
        Ok(Event::default().event("heartbeat").data(
            serde_json::json!({
                "type": "control_plane_heartbeat",
                "at": Utc::now(),
            })
            .to_string(),
        ))
    });
    Sse::new(stream)
}

pub(crate) async fn setting_string(
    store: &Store,
    key: &str,
    fallback: &str,
) -> Result<String, AppError> {
    let value = store.settings().get_json(key).await?;
    Ok(value
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| fallback.to_string()))
}
