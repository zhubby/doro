use crate::agent_events::{ingest_agent_event, virtual_machine_snapshot_payload};
use crate::agent_streams::{
    container_refresh_app_error, virtual_machine_command_app_error,
    virtual_machine_command_error_message,
};
use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;
use doro_protocol::VirtualMachineImage;
use doro_protocol::VirtualMachineSnapshot;
use doro_vm::{
    VmCommand, VmCommandEnvelope, VmDeleteMode, VmDiskSpec, VmId, VmImageRef, VmNetworkMode,
    VmNetworkSpec, VmPortForward, VmSnapshotRequest, VmSpec, VmStopMode,
};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub(crate) struct VirtualMachineHostQuery {
    host_id: Option<Uuid>,
}

struct NewVirtualMachineTask {
    task_id: Uuid,
    step_id: Uuid,
    created_by: String,
    host_id: Uuid,
    title: String,
    summary: String,
    payload: Value,
}

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
    Query(query): Query<VirtualMachineHostQuery>,
) -> Result<Json<ListVirtualMachineImagesResponse>, AppError> {
    let hosts = virtual_machine_hosts(&state, query.host_id).await?;
    let mut items = Vec::new();
    for host in hosts {
        let details = run_vm_query(
            &state,
            host.id,
            VmCommand::ListImages,
            "list virtual machine images",
        )
        .await?;
        let images: Vec<doro_vm::VmImageRef> = serde_json::from_value(details)
            .map_err(|error| AppError::status(StatusCode::BAD_GATEWAY, error.to_string()))?;
        items.extend(images.into_iter().map(|image| VirtualMachineImage {
            host_id: Some(host.id),
            id: image.id,
            name: image.name,
            path: image.path.display().to_string(),
            os_family: image.os_family,
            architecture: image.architecture,
        }));
    }
    if !items.is_empty() || query.host_id.is_some() {
        return Ok(Json(ListVirtualMachineImagesResponse { items }));
    }
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
    ensure_virtual_machine_ready(&state, request.host_id).await?;
    let task_id = Uuid::new_v4();
    let step_id = Uuid::new_v4();
    let vm_ref = stable_vm_ref(task_id);
    let reason = request.reason.clone();
    let task = create_virtual_machine_task(
        &state,
        NewVirtualMachineTask {
            task_id,
            step_id,
            created_by: current_user.username,
            host_id: request.host_id,
            title: format!("create virtual machine {}", request.name),
            summary: "Create QEMU virtual machine".to_string(),
            payload: json!({
                "resource": "virtual_machine",
                "action": "create",
                "reason": reason,
                "vm_ref": vm_ref,
                "request": request,
            }),
        },
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
        NewVirtualMachineTask {
            task_id: Uuid::new_v4(),
            step_id: Uuid::new_v4(),
            created_by: current_user.username,
            host_id: vm.host_id,
            title: format!("{action} virtual machine {}", vm.name),
            summary: format!("{action} QEMU virtual machine"),
            payload: json!({
                "resource": "virtual_machine",
                "action": action,
                "vm_id": vm.id,
                "vm_ref": vm.vm_ref,
                "reason": request.reason,
            }),
        },
    )
    .await?;
    Ok(Json(VirtualMachineActionResponse { task }))
}

pub(crate) async fn list_virtual_machine_snapshots(
    State(state): State<AppState>,
    AxumPath(vm_id): AxumPath<Uuid>,
) -> Result<Json<ListVirtualMachineSnapshotsResponse>, AppError> {
    let vm = find_virtual_machine(&state, vm_id).await?;
    let details = run_vm_query(
        &state,
        vm.host_id,
        VmCommand::ListSnapshots {
            id: VmId::new(vm.vm_ref),
        },
        "list virtual machine snapshots",
    )
    .await?;
    let snapshots: Vec<doro_vm::VmSnapshot> = serde_json::from_value(details)
        .map_err(|error| AppError::status(StatusCode::BAD_GATEWAY, error.to_string()))?;
    Ok(Json(ListVirtualMachineSnapshotsResponse {
        items: snapshots
            .into_iter()
            .map(|snapshot| VirtualMachineSnapshot {
                id: snapshot.id,
                vm_id,
                name: snapshot.name,
                description: snapshot.description,
                created_at: snapshot.created_at,
            })
            .collect(),
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
    let reason = request.reason.clone();
    let task = create_virtual_machine_task(
        &state,
        NewVirtualMachineTask {
            task_id: Uuid::new_v4(),
            step_id: Uuid::new_v4(),
            created_by: current_user.username,
            host_id: vm.host_id,
            title: format!("snapshot virtual machine {}", vm.name),
            summary: "Create QEMU virtual machine snapshot".to_string(),
            payload: json!({
                "resource": "virtual_machine",
                "action": "snapshot",
                "vm_id": vm.id,
                "vm_ref": vm.vm_ref,
                "name": request.name,
                "description": request.description,
                "reason": reason,
            }),
        },
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

async fn create_virtual_machine_task(
    state: &AppState,
    request: NewVirtualMachineTask,
) -> Result<Task, AppError> {
    let task = state
        .store
        .tasks()
        .create_with_steps(NewTask {
            id: request.task_id,
            host_id: Some(request.host_id),
            title: request.title,
            prompt: None,
            status: TaskStatus::WaitingApproval,
            created_by: request.created_by,
            created_at: Utc::now(),
            metadata: json!({ "resource": "virtual_machine" }),
            create_step_approvals: true,
            steps: vec![TaskStep {
                id: request.step_id,
                capability: CapabilityName::VirtualMachinesManage,
                risk: CapabilityRisk::High,
                summary: request.summary,
                status: TaskStepStatus::Pending,
                payload: request.payload,
            }],
        })
        .await?;
    Ok(task)
}

pub(crate) async fn apply_approved_virtual_machine_task(
    state: &AppState,
    task_id: Uuid,
    step_id: Uuid,
) {
    let Ok(tasks) = state.store.tasks().list().await else {
        tracing::warn!("failed to inspect virtual machine task after approval");
        return;
    };
    let Some(task) = tasks.into_iter().find(|task| task.id == task_id) else {
        return;
    };
    let Some(step) = task.steps.iter().find(|step| step.id == step_id).cloned() else {
        return;
    };
    if step.capability != CapabilityName::VirtualMachinesManage {
        return;
    }
    if step.payload.get("resource").and_then(Value::as_str) != Some("virtual_machine") {
        return;
    }
    let Some(host_id) = task.host_id else {
        mark_virtual_machine_task_failed(
            state,
            task.id,
            step_id,
            "virtual machine task is missing host_id",
        )
        .await;
        return;
    };
    if let Err(error) =
        dispatch_virtual_machine_task(state, task.id, step_id, host_id, step.payload).await
    {
        tracing::warn!(
            ?error,
            task_id = %task.id,
            "failed to dispatch approved virtual machine task"
        );
        mark_virtual_machine_task_failed(state, task.id, step_id, &error.0.to_string()).await;
    }
}

async fn dispatch_virtual_machine_task(
    state: &AppState,
    task_id: Uuid,
    step_id: Uuid,
    host_id: Uuid,
    payload: Value,
) -> Result<(), AppError> {
    let agent_id = ensure_virtual_machine_ready(state, host_id).await?;
    let envelope = VmCommandEnvelope {
        command_id: Uuid::new_v4(),
        task_id: Some(task_id),
        step_id: Some(step_id),
        command: vm_command_from_payload(state, host_id, &payload).await?,
    };

    let now = Utc::now();
    let task_run_id = Uuid::new_v4();
    state
        .store
        .tasks()
        .update_status(task_id, TaskStatus::Running, None, None)
        .await?;
    state
        .store
        .tasks()
        .update_step_status(step_id, "running")
        .await?;
    state
        .store
        .tasks()
        .create_run(NewTaskRun {
            id: task_run_id,
            task_id,
            step_id: Some(step_id),
            agent_id,
            status: "running".to_string(),
            command_id: None,
            started_at: Some(now),
            finished_at: None,
            result_json: json!({}),
            error_message: None,
        })
        .await?;

    let result = match state
        .agent_streams
        .run_virtual_machine_command(host_id, serde_json::to_string(&envelope)?)
        .await
    {
        Ok(result) => result,
        Err(error) => {
            let finished_at = Utc::now();
            let message = virtual_machine_command_error_message(error);
            finish_failed_virtual_machine_run(
                state,
                task_id,
                step_id,
                task_run_id,
                finished_at,
                &message,
            )
            .await?;
            return Err(AppError::status(StatusCode::BAD_GATEWAY, message));
        }
    };
    let finished_at = Utc::now();
    let succeeded = result.status == grpc::CommandStatus::Succeeded as i32;
    let step_status = if succeeded { "succeeded" } else { "failed" };
    let task_status = if succeeded {
        TaskStatus::Succeeded
    } else {
        TaskStatus::Failed
    };
    let error_message = (!succeeded).then_some(result.message.clone());
    let result_json = json!({
        "message": result.message,
        "details": parse_json_value(&result.details_json),
    });
    state
        .store
        .tasks()
        .update_step_status(step_id, step_status)
        .await?;
    state
        .store
        .tasks()
        .update_status(
            task_id,
            task_status,
            Some(finished_at),
            error_message.clone(),
        )
        .await?;
    state
        .store
        .tasks()
        .finish_run(
            task_run_id,
            step_status.to_string(),
            Some(result.command_id),
            finished_at,
            result_json,
            error_message,
        )
        .await?;
    state
        .store
        .events()
        .record(NewAgentEvent {
            agent_id: Some(agent_id),
            host_id: Some(host_id),
            event_type: "virtual_machine.command".to_string(),
            event_json: json!({
                "task_id": task_id,
                "step_id": step_id,
                "status": step_status,
            }),
            recorded_at: finished_at,
        })
        .await?;
    if succeeded {
        refresh_single_host_virtual_machines(state, host_id).await?;
    }
    Ok(())
}

async fn vm_command_from_payload(
    state: &AppState,
    host_id: Uuid,
    payload: &Value,
) -> Result<VmCommand, AppError> {
    let action = payload
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AppError::status(StatusCode::BAD_REQUEST, "virtual machine action missing")
        })?;
    match action {
        "create" => create_command_from_payload(state, host_id, payload).await,
        "start" => Ok(VmCommand::Start {
            id: vm_id_from_payload(payload)?,
        }),
        "stop" => Ok(VmCommand::Stop {
            id: vm_id_from_payload(payload)?,
            mode: VmStopMode::Graceful,
        }),
        "restart" => Ok(VmCommand::Restart {
            id: vm_id_from_payload(payload)?,
        }),
        "delete" => Ok(VmCommand::Delete {
            id: vm_id_from_payload(payload)?,
            mode: VmDeleteMode::DeleteDisks,
        }),
        "snapshot" => Ok(VmCommand::Snapshot {
            id: vm_id_from_payload(payload)?,
            request: VmSnapshotRequest {
                name: required_payload_text(payload, "name", "snapshot name is required")?,
                description: payload
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
        }),
        _ => Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "unsupported virtual machine action",
        )),
    }
}

async fn create_command_from_payload(
    state: &AppState,
    host_id: Uuid,
    payload: &Value,
) -> Result<VmCommand, AppError> {
    let request: CreateVirtualMachineRequest = serde_json::from_value(
        payload
            .get("request")
            .cloned()
            .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "create request missing"))?,
    )
    .map_err(AppError::from)?;
    let vm_ref = required_payload_text(payload, "vm_ref", "vm_ref is required")?;
    let images = images_for_host(state, host_id).await?;
    let image = images
        .into_iter()
        .find(|image| image.id == request.image_id)
        .ok_or_else(|| {
            AppError::status(StatusCode::NOT_FOUND, "virtual machine image not found")
        })?;
    Ok(VmCommand::Create {
        spec: Box::new(VmSpec {
            id: VmId::new(vm_ref),
            name: request.name,
            image: VmImageRef {
                id: image.id,
                name: image.name,
                path: image.path.into(),
                os_family: image.os_family,
                architecture: image.architecture,
            },
            cpu_cores: request.cpu_cores,
            memory_mib: request.memory_mib,
            disks: vec![VmDiskSpec {
                path: "disk.qcow2".into(),
                size_gb: request.disk_gb,
                format: "qcow2".to_string(),
                boot: true,
            }],
            networks: request
                .networks
                .into_iter()
                .map(protocol_network_to_vm)
                .collect(),
            cloud_init: request.cloud_init,
            metadata: json!({
                "created_by": "control_plane",
            }),
        }),
    })
}

fn protocol_network_to_vm(network: doro_protocol::VirtualMachineNetwork) -> VmNetworkSpec {
    VmNetworkSpec {
        mode: match network.mode {
            VirtualMachineNetworkMode::UserNat => VmNetworkMode::UserNat,
            VirtualMachineNetworkMode::BridgeTap => VmNetworkMode::BridgeTap,
        },
        bridge: network.bridge,
        mac_address: network.mac_address,
        port_forwards: network
            .port_forwards
            .into_iter()
            .map(|port| VmPortForward {
                host_port: port.host_port,
                guest_port: port.guest_port,
                protocol: port.protocol,
            })
            .collect(),
    }
}

async fn run_vm_query(
    state: &AppState,
    host_id: Uuid,
    command: VmCommand,
    message: &str,
) -> Result<Value, AppError> {
    ensure_virtual_machine_ready(state, host_id).await?;
    let envelope = VmCommandEnvelope {
        command_id: Uuid::new_v4(),
        task_id: None,
        step_id: None,
        command,
    };
    let result = state
        .agent_streams
        .run_virtual_machine_command(host_id, serde_json::to_string(&envelope)?)
        .await
        .map_err(virtual_machine_command_app_error)?;
    if result.status == grpc::CommandStatus::Failed as i32 {
        return Err(AppError::status(
            StatusCode::BAD_GATEWAY,
            virtual_machine_command_error_message(
                crate::agent_streams::VirtualMachineCommandError::AgentFailed(result.message),
            ),
        ));
    }
    let details = parse_json_value(&result.details_json);
    if details.is_null() {
        return Err(AppError::status(
            StatusCode::BAD_GATEWAY,
            message.to_string(),
        ));
    }
    Ok(details)
}

async fn images_for_host(
    state: &AppState,
    host_id: Uuid,
) -> Result<Vec<VirtualMachineImage>, AppError> {
    let details = run_vm_query(
        state,
        host_id,
        VmCommand::ListImages,
        "list virtual machine images",
    )
    .await?;
    let images: Vec<doro_vm::VmImageRef> = serde_json::from_value(details)
        .map_err(|error| AppError::status(StatusCode::BAD_GATEWAY, error.to_string()))?;
    Ok(images
        .into_iter()
        .map(|image| VirtualMachineImage {
            host_id: Some(host_id),
            id: image.id,
            name: image.name,
            path: image.path.display().to_string(),
            os_family: image.os_family,
            architecture: image.architecture,
        })
        .collect())
}

async fn refresh_single_host_virtual_machines(
    state: &AppState,
    host_id: Uuid,
) -> Result<(), AppError> {
    let snapshot = state
        .agent_streams
        .collect_virtual_machines(host_id)
        .await
        .map_err(container_refresh_app_error)?;
    let payload = virtual_machine_snapshot_payload(&snapshot);
    ingest_agent_event(
        &state.store,
        Some(host_id),
        "virtual_machine.snapshot",
        &payload,
        Utc::now(),
    )
    .await?;
    Ok(())
}

async fn ensure_virtual_machine_ready(state: &AppState, host_id: Uuid) -> Result<Uuid, AppError> {
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
        .any(|capability| capability.name == CapabilityName::VirtualMachinesManage)
    {
        return Err(AppError::status(
            StatusCode::FORBIDDEN,
            "agent does not declare VirtualMachinesManage capability",
        ));
    }
    state
        .agent_streams
        .agent_id_for_host(host_id)
        .await
        .ok_or_else(|| {
            AppError::status(
                StatusCode::SERVICE_UNAVAILABLE,
                "agent stream is not connected",
            )
        })
}

async fn virtual_machine_hosts(
    state: &AppState,
    host_id: Option<Uuid>,
) -> Result<Vec<doro_protocol::Host>, AppError> {
    let hosts = state.store.hosts().list().await?;
    let hosts = hosts
        .into_iter()
        .filter(|host| host_id.is_none_or(|id| host.id == id))
        .filter(|host| host.status == HostStatus::Online)
        .filter(|host| {
            host.capabilities
                .iter()
                .any(|capability| capability.name == CapabilityName::VirtualMachinesManage)
        })
        .collect::<Vec<_>>();
    if hosts.is_empty() && host_id.is_some() {
        return Err(AppError::status(
            StatusCode::SERVICE_UNAVAILABLE,
            "virtual machine capable agent is not online",
        ));
    }
    Ok(hosts)
}

async fn find_virtual_machine(
    state: &AppState,
    vm_id: Uuid,
) -> Result<doro_protocol::VirtualMachine, AppError> {
    state
        .store
        .virtual_machines()
        .list()
        .await?
        .into_iter()
        .find(|vm| vm.id == vm_id)
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "virtual machine not found"))
}

fn vm_id_from_payload(payload: &Value) -> Result<VmId, AppError> {
    required_payload_text(payload, "vm_ref", "vm_ref is required").map(VmId::new)
}

fn required_payload_text(
    payload: &Value,
    field: &str,
    message: &'static str,
) -> Result<String, AppError> {
    let value = payload
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if value.is_empty() {
        return Err(AppError::status(StatusCode::BAD_REQUEST, message));
    }
    Ok(value)
}

fn stable_vm_ref(task_id: Uuid) -> String {
    let simple = task_id.simple().to_string();
    format!("vm-{}", &simple[..12])
}

fn parse_json_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| json!({}))
}

async fn finish_failed_virtual_machine_run(
    state: &AppState,
    task_id: Uuid,
    step_id: Uuid,
    task_run_id: Uuid,
    finished_at: DateTime<Utc>,
    message: &str,
) -> Result<(), AppError> {
    state
        .store
        .tasks()
        .update_step_status(step_id, "failed")
        .await?;
    state
        .store
        .tasks()
        .update_status(
            task_id,
            TaskStatus::Failed,
            Some(finished_at),
            Some(message.to_string()),
        )
        .await?;
    state
        .store
        .tasks()
        .finish_run(
            task_run_id,
            "failed".to_string(),
            None,
            finished_at,
            json!({ "message": message }),
            Some(message.to_string()),
        )
        .await?;
    Ok(())
}

async fn mark_virtual_machine_task_failed(
    state: &AppState,
    task_id: Uuid,
    step_id: Uuid,
    message: &str,
) {
    let _ = state
        .store
        .tasks()
        .update_step_status(step_id, "failed")
        .await;
    let _ = state
        .store
        .tasks()
        .update_status(
            task_id,
            TaskStatus::Failed,
            Some(Utc::now()),
            Some(message.to_string()),
        )
        .await;
}
