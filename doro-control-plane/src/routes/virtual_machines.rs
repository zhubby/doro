use crate::agent_events::{ingest_agent_event, virtual_machine_snapshot_payload};
use crate::agent_streams::container_refresh_app_error;
use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;

pub(crate) async fn refresh_virtual_machines(
    State(state): State<AppState>,
) -> Result<Json<ListVirtualMachinesResponse>, AppError> {
    let hosts = state.store.hosts().list().await?;
    let online_hosts = hosts
        .into_iter()
        .filter(|host| host.status == HostStatus::Online)
        .filter(|host| {
            host.capabilities
                .iter()
                .any(|capability| capability.name == CapabilityName::VirtualMachinesManage)
        })
        .collect::<Vec<_>>();
    if online_hosts.is_empty() {
        return Ok(Json(ListVirtualMachinesResponse {
            items: state.store.virtual_machines().list().await?,
        }));
    }

    for host in &online_hosts {
        let snapshot = state
            .agent_streams
            .collect_virtual_machines(host.id)
            .await
            .map_err(container_refresh_app_error)?;
        let payload = virtual_machine_snapshot_payload(&snapshot);
        ingest_agent_event(
            &state.store,
            Some(host.id),
            "virtual_machine.snapshot",
            &payload,
            Utc::now(),
        )
        .await?;
    }

    Ok(Json(ListVirtualMachinesResponse {
        items: state.store.virtual_machines().list().await?,
    }))
}

pub(crate) async fn list_host_virtual_machines(
    State(state): State<AppState>,
    AxumPath(host_id): AxumPath<Uuid>,
) -> Result<Json<ListVirtualMachinesResponse>, AppError> {
    Ok(Json(ListVirtualMachinesResponse {
        items: state.store.virtual_machines().list_by_host(host_id).await?,
    }))
}

pub(crate) async fn list_virtual_machine_images(
    State(state): State<AppState>,
) -> Result<Json<ListVirtualMachineImagesResponse>, AppError> {
    Ok(Json(ListVirtualMachineImagesResponse {
        items: state.store.virtual_machines().images().await?,
    }))
}

pub(crate) async fn list_virtual_machine_templates(
    State(state): State<AppState>,
) -> Result<Json<ListVirtualMachineTemplatesResponse>, AppError> {
    Ok(Json(ListVirtualMachineTemplatesResponse {
        items: state.store.virtual_machines().templates().await?,
    }))
}

pub(crate) async fn create_virtual_machine(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<CreateVirtualMachineRequest>,
) -> Result<Json<VirtualMachineActionResponse>, AppError> {
    validate_virtual_machine_create_request(&request)?;
    let task = create_virtual_machine_task(
        &state,
        current_user.username,
        request.host_id,
        format!("create virtual machine {}", request.name),
        "Create QEMU virtual machine",
        serde_json::to_value(request)?,
    )
    .await?;
    Ok(Json(VirtualMachineActionResponse { task }))
}

pub(crate) async fn start_virtual_machine(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(vm_id): AxumPath<Uuid>,
    Json(request): Json<VirtualMachineActionRequest>,
) -> Result<Json<VirtualMachineActionResponse>, AppError> {
    vm_action_task(state, current_user, vm_id, "start", request).await
}

pub(crate) async fn stop_virtual_machine(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(vm_id): AxumPath<Uuid>,
    Json(request): Json<VirtualMachineActionRequest>,
) -> Result<Json<VirtualMachineActionResponse>, AppError> {
    vm_action_task(state, current_user, vm_id, "stop", request).await
}

pub(crate) async fn restart_virtual_machine(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(vm_id): AxumPath<Uuid>,
    Json(request): Json<VirtualMachineActionRequest>,
) -> Result<Json<VirtualMachineActionResponse>, AppError> {
    vm_action_task(state, current_user, vm_id, "restart", request).await
}

pub(crate) async fn delete_virtual_machine(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(vm_id): AxumPath<Uuid>,
    Json(request): Json<VirtualMachineActionRequest>,
) -> Result<Json<VirtualMachineActionResponse>, AppError> {
    vm_action_task(state, current_user, vm_id, "delete", request).await
}

pub(crate) async fn vm_action_task(
    state: AppState,
    current_user: CurrentUser,
    vm_id: Uuid,
    action: &'static str,
    request: VirtualMachineActionRequest,
) -> Result<Json<VirtualMachineActionResponse>, AppError> {
    let vm = state
        .store
        .virtual_machines()
        .list()
        .await?
        .into_iter()
        .find(|vm| vm.id == vm_id)
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "virtual machine not found"))?;
    let task = create_virtual_machine_task(
        &state,
        current_user.username,
        vm.host_id,
        format!("{action} virtual machine {}", vm.name),
        format!("{action} QEMU virtual machine"),
        serde_json::json!({
            "action": action,
            "vm_id": vm.id,
            "vm_ref": vm.vm_ref,
            "reason": request.reason,
        }),
    )
    .await?;
    Ok(Json(VirtualMachineActionResponse { task }))
}

pub(crate) async fn list_virtual_machine_snapshots(
    State(_state): State<AppState>,
    AxumPath(_vm_id): AxumPath<Uuid>,
) -> Result<Json<ListVirtualMachineSnapshotsResponse>, AppError> {
    Ok(Json(ListVirtualMachineSnapshotsResponse {
        items: Vec::new(),
    }))
}

pub(crate) async fn create_virtual_machine_snapshot(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(vm_id): AxumPath<Uuid>,
    Json(request): Json<CreateVirtualMachineSnapshotRequest>,
) -> Result<Json<VirtualMachineActionResponse>, AppError> {
    let vm = state
        .store
        .virtual_machines()
        .list()
        .await?
        .into_iter()
        .find(|vm| vm.id == vm_id)
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "virtual machine not found"))?;
    let task = create_virtual_machine_task(
        &state,
        current_user.username,
        vm.host_id,
        format!("snapshot virtual machine {}", vm.name),
        "Create QEMU virtual machine snapshot",
        serde_json::json!({
            "action": "snapshot",
            "vm_id": vm.id,
            "vm_ref": vm.vm_ref,
            "name": request.name,
            "description": request.description,
        }),
    )
    .await?;
    Ok(Json(VirtualMachineActionResponse { task }))
}

pub(crate) async fn virtual_machine_console(
    State(state): State<AppState>,
    AxumPath(vm_id): AxumPath<Uuid>,
) -> Result<Json<VirtualMachineConsoleResponse>, AppError> {
    let vm = state
        .store
        .virtual_machines()
        .list()
        .await?
        .into_iter()
        .find(|vm| vm.id == vm_id)
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "virtual machine not found"))?;
    Ok(Json(VirtualMachineConsoleResponse {
        item: vm.console.unwrap_or_else(|| serde_json::json!({})),
    }))
}

pub(crate) fn validate_virtual_machine_create_request(
    request: &CreateVirtualMachineRequest,
) -> Result<(), AppError> {
    if request.name.trim().is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "virtual machine name is required",
        ));
    }
    if request.cpu_cores == 0 || request.memory_mib < 128 || request.disk_gb == 0 {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "virtual machine resources are invalid",
        ));
    }
    if request.networks.iter().any(|network| {
        network.mode == VirtualMachineNetworkMode::BridgeTap && network.bridge.is_none()
    }) {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "bridge network requires bridge name",
        ));
    }
    Ok(())
}

pub(crate) async fn create_virtual_machine_task(
    state: &AppState,
    created_by: String,
    host_id: Uuid,
    title: String,
    summary: impl Into<String>,
    payload: Value,
) -> Result<Task, AppError> {
    let step_id = Uuid::new_v4();
    let task = state
        .store
        .tasks()
        .create_with_steps(NewTask {
            id: Uuid::new_v4(),
            host_id: Some(host_id),
            title,
            prompt: None,
            status: TaskStatus::WaitingApproval,
            created_by,
            created_at: Utc::now(),
            metadata: serde_json::json!({}),
            create_step_approvals: true,
            steps: vec![TaskStep {
                id: step_id,
                capability: CapabilityName::VirtualMachinesManage,
                risk: CapabilityRisk::High,
                summary: summary.into(),
                status: TaskStepStatus::Pending,
                payload,
            }],
        })
        .await?;
    Ok(task)
}
