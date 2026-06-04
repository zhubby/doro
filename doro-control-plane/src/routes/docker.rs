use crate::agent_streams::{docker_command_app_error, docker_command_error_message};
use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::prelude::*;
use crate::state::AppState;
use doro_container::{
    ContainerCommand, ContainerComposeCommand, ContainerListFilter, ContainerNetworkCommand,
    ContainerRuntimeCommand, ContainerRuntimeCommandEnvelope, ContainerVolumeCommand,
    CreateContainerRequest, CreateNetworkRequest, CreateVolumeRequest, PullImageRequest,
    RemoveContainerRequest, RemoveImageRequest, RemoveVolumeRequest, RestartContainerRequest,
    StopContainerRequest,
};
use doro_protocol::{
    DockerActionRequest, DockerActionResponse, DockerComposeProject, DockerComposeProjectRequest,
    DockerComposeProjectResponse, DockerContainerCreateRequest, DockerContainerSummary,
    DockerImagePullRequest, DockerImageRemoveRequest, DockerImageSummary,
    DockerNetworkContainerRequest, DockerNetworkCreateRequest, DockerNetworkSummary,
    DockerVolumeCreateRequest, DockerVolumeSummary, ListDockerComposeProjectsResponse,
    ListDockerContainersResponse, ListDockerImagesResponse, ListDockerNetworksResponse,
    ListDockerVolumesResponse,
};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub(crate) struct DockerHostQuery {
    host_id: Option<Uuid>,
}

pub(crate) async fn list_docker_containers(
    State(state): State<AppState>,
    Query(query): Query<DockerHostQuery>,
) -> Result<Json<ListDockerContainersResponse>, AppError> {
    let hosts = docker_hosts(&state, query.host_id).await?;
    let mut items = Vec::new();
    for host in hosts {
        let details = run_docker_query(
            &state,
            host.id,
            ContainerRuntimeCommand::Container(ContainerCommand::List {
                filter: ContainerListFilter { all: true },
            }),
        )
        .await?;
        let mut summaries: Vec<doro_container::ContainerSummary> =
            serde_json::from_value(details).map_err(AppError::from)?;
        items.extend(summaries.drain(..).map(|container| DockerContainerSummary {
            host_id: host.id,
            runtime: "docker".to_string(),
            id: container.id,
            names: container.names,
            image: container.image,
            image_id: container.image_id,
            command: container.command,
            created: container.created,
            ports: container.ports,
            labels: container.labels,
            state: container.state,
            status: container.status,
        }));
    }
    Ok(Json(ListDockerContainersResponse { items }))
}

pub(crate) async fn create_docker_container(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<DockerContainerCreateRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let labels = string_map_from_value(request.labels, "labels")?;
    let command =
        ContainerRuntimeCommand::Container(ContainerCommand::Create(CreateContainerRequest {
            name: required_text(request.name, "container name is required")?,
            image: required_text(request.image, "container image is required")?,
            command: request.command,
            env: request.env,
            labels,
        }));
    docker_task_response(
        &state,
        current_user.username,
        request.host_id,
        "create Docker container",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn start_docker_container(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(container): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    docker_container_action(state, current_user, container, "start", request).await
}

pub(crate) async fn stop_docker_container(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(container): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let id_or_name = required_text(container, "container id or name is required")?;
    let host_id = require_host_id(request.host_id)?;
    let command =
        ContainerRuntimeCommand::Container(ContainerCommand::Stop(StopContainerRequest {
            id_or_name,
            timeout_seconds: 10,
        }));
    docker_task_response(
        &state,
        current_user.username,
        host_id,
        "stop Docker container",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn restart_docker_container(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(container): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let id_or_name = required_text(container, "container id or name is required")?;
    let host_id = require_host_id(request.host_id)?;
    let command =
        ContainerRuntimeCommand::Container(ContainerCommand::Restart(RestartContainerRequest {
            id_or_name,
            timeout_seconds: 10,
        }));
    docker_task_response(
        &state,
        current_user.username,
        host_id,
        "restart Docker container",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn delete_docker_container(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(container): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let id_or_name = required_text(container, "container id or name is required")?;
    let host_id = require_host_id(request.host_id)?;
    let command =
        ContainerRuntimeCommand::Container(ContainerCommand::Remove(RemoveContainerRequest {
            id_or_name,
            force: true,
            remove_volumes: false,
        }));
    docker_task_response(
        &state,
        current_user.username,
        host_id,
        "delete Docker container",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn list_docker_images(
    State(state): State<AppState>,
    Query(query): Query<DockerHostQuery>,
) -> Result<Json<ListDockerImagesResponse>, AppError> {
    let hosts = docker_hosts(&state, query.host_id).await?;
    let mut items = Vec::new();
    for host in hosts {
        let details = run_docker_query(
            &state,
            host.id,
            ContainerRuntimeCommand::Image(doro_container::ContainerImageCommand::List),
        )
        .await?;
        let images: Vec<doro_container::ImageSummary> =
            serde_json::from_value(details).map_err(AppError::from)?;
        items.extend(images.into_iter().map(|image| DockerImageSummary {
            host_id: host.id,
            id: image.id,
            repo_tags: image.repo_tags,
            repo_digests: image.repo_digests,
            created: image.created,
            size: image.size,
            labels: image.labels,
        }));
    }
    Ok(Json(ListDockerImagesResponse { items }))
}

pub(crate) async fn pull_docker_image(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<DockerImagePullRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let command = ContainerRuntimeCommand::Image(doro_container::ContainerImageCommand::Pull(
        PullImageRequest {
            reference: required_text(request.reference, "image reference is required")?,
            tag: request.tag,
            platform: request.platform,
        },
    ));
    docker_task_response(
        &state,
        current_user.username,
        request.host_id,
        "pull Docker image",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn remove_docker_image(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<DockerImageRemoveRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let command = ContainerRuntimeCommand::Image(doro_container::ContainerImageCommand::Remove(
        RemoveImageRequest {
            reference: required_text(request.reference, "image reference is required")?,
            force: request.force,
            noprune: request.noprune,
        },
    ));
    docker_task_response(
        &state,
        current_user.username,
        request.host_id,
        "remove Docker image",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn list_docker_networks(
    State(state): State<AppState>,
    Query(query): Query<DockerHostQuery>,
) -> Result<Json<ListDockerNetworksResponse>, AppError> {
    let hosts = docker_hosts(&state, query.host_id).await?;
    let mut items = Vec::new();
    for host in hosts {
        let details = run_docker_query(
            &state,
            host.id,
            ContainerRuntimeCommand::Network(ContainerNetworkCommand::List),
        )
        .await?;
        let networks: Vec<doro_container::NetworkSummary> =
            serde_json::from_value(details).map_err(AppError::from)?;
        items.extend(networks.into_iter().map(|network| DockerNetworkSummary {
            host_id: host.id,
            id: network.id,
            name: network.name,
            driver: network.driver,
            scope: network.scope,
            internal: network.internal,
            attachable: network.attachable,
            ingress: network.ingress,
        }));
    }
    Ok(Json(ListDockerNetworksResponse { items }))
}

pub(crate) async fn create_docker_network(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<DockerNetworkCreateRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let command =
        ContainerRuntimeCommand::Network(ContainerNetworkCommand::Create(CreateNetworkRequest {
            name: required_text(request.name, "network name is required")?,
            driver: if request.driver.trim().is_empty() {
                "bridge".to_string()
            } else {
                request.driver
            },
            internal: request.internal,
            attachable: request.attachable,
            labels: string_map_from_value(request.labels, "labels")?,
        }));
    docker_task_response(
        &state,
        current_user.username,
        request.host_id,
        "create Docker network",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn remove_docker_network(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(network): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let host_id = require_host_id(request.host_id)?;
    let command = ContainerRuntimeCommand::Network(ContainerNetworkCommand::Remove {
        name_or_id: required_text(network, "network name or id is required")?,
    });
    docker_task_response(
        &state,
        current_user.username,
        host_id,
        "remove Docker network",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn connect_docker_network(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(network): AxumPath<String>,
    Json(request): Json<DockerNetworkContainerRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    docker_network_container_action(state, current_user, network, request, "connect").await
}

pub(crate) async fn disconnect_docker_network(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(network): AxumPath<String>,
    Json(request): Json<DockerNetworkContainerRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    docker_network_container_action(state, current_user, network, request, "disconnect").await
}

pub(crate) async fn list_docker_volumes(
    State(state): State<AppState>,
    Query(query): Query<DockerHostQuery>,
) -> Result<Json<ListDockerVolumesResponse>, AppError> {
    let hosts = docker_hosts(&state, query.host_id).await?;
    let mut items = Vec::new();
    for host in hosts {
        let details = run_docker_query(
            &state,
            host.id,
            ContainerRuntimeCommand::Volume(ContainerVolumeCommand::List),
        )
        .await?;
        let volumes: Vec<doro_container::VolumeSummary> =
            serde_json::from_value(details).map_err(AppError::from)?;
        items.extend(volumes.into_iter().map(|volume| DockerVolumeSummary {
            host_id: host.id,
            name: volume.name,
            driver: volume.driver,
            mountpoint: volume.mountpoint,
            labels: volume.labels,
            usage_size: volume.usage_size,
            usage_ref_count: volume.usage_ref_count,
        }));
    }
    Ok(Json(ListDockerVolumesResponse { items }))
}

pub(crate) async fn create_docker_volume(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<DockerVolumeCreateRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let command =
        ContainerRuntimeCommand::Volume(ContainerVolumeCommand::Create(CreateVolumeRequest {
            name: required_text(request.name, "volume name is required")?,
            driver: if request.driver.trim().is_empty() {
                "local".to_string()
            } else {
                request.driver
            },
            driver_opts: string_map_from_value(request.driver_opts, "driver_opts")?,
            labels: string_map_from_value(request.labels, "labels")?,
        }));
    docker_task_response(
        &state,
        current_user.username,
        request.host_id,
        "create Docker volume",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn remove_docker_volume(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(volume): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let host_id = require_host_id(request.host_id)?;
    let command =
        ContainerRuntimeCommand::Volume(ContainerVolumeCommand::Remove(RemoveVolumeRequest {
            name: required_text(volume, "volume name is required")?,
            force: true,
        }));
    docker_task_response(
        &state,
        current_user.username,
        host_id,
        "remove Docker volume",
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn list_docker_compose_projects(
    State(state): State<AppState>,
    Query(query): Query<DockerHostQuery>,
) -> Result<Json<ListDockerComposeProjectsResponse>, AppError> {
    let hosts = docker_hosts(&state, query.host_id).await?;
    let mut items = Vec::new();
    for host in hosts {
        let details = run_docker_query(
            &state,
            host.id,
            ContainerRuntimeCommand::Compose(ContainerComposeCommand::List),
        )
        .await?;
        let projects: Vec<ComposeProjectPayload> =
            serde_json::from_value(details).map_err(AppError::from)?;
        items.extend(
            projects
                .into_iter()
                .map(|project| project.into_protocol(host.id)),
        );
    }
    Ok(Json(ListDockerComposeProjectsResponse { items }))
}

pub(crate) async fn create_docker_compose_project(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<DockerComposeProjectRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let project = required_text(request.name, "compose project name is required")?;
    let command = ContainerRuntimeCommand::Compose(ContainerComposeCommand::CreateOrUpdate {
        project: project.clone(),
        compose_yaml: request.compose_yaml,
        env_file: request.env_file,
    });
    docker_task_response(
        &state,
        current_user.username,
        request.host_id,
        format!("create or update Docker Compose project {project}"),
        request.reason,
        command,
    )
    .await
}

pub(crate) async fn read_docker_compose_project(
    State(state): State<AppState>,
    Query(query): Query<DockerHostQuery>,
    AxumPath(project): AxumPath<String>,
) -> Result<Json<DockerComposeProjectResponse>, AppError> {
    let host_id = require_host_id(query.host_id)?;
    ensure_docker_ready(&state, host_id).await?;
    let details = run_docker_query(
        &state,
        host_id,
        ContainerRuntimeCommand::Compose(ContainerComposeCommand::Read { project }),
    )
    .await?;
    let project: ComposeProjectPayload = serde_json::from_value(details).map_err(AppError::from)?;
    Ok(Json(DockerComposeProjectResponse {
        item: project.into_protocol(host_id),
    }))
}

pub(crate) async fn update_docker_compose_project(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(project): AxumPath<String>,
    Json(request): Json<DockerComposeProjectRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    let path_project = required_text(project, "compose project name is required")?;
    if path_project != request.name {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "compose project path and request name must match",
        ));
    }
    create_docker_compose_project(State(state), Extension(current_user), Json(request)).await
}

pub(crate) async fn up_docker_compose_project(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(project): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    docker_compose_action(state, current_user, project, request, "up").await
}

pub(crate) async fn down_docker_compose_project(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(project): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    docker_compose_action(state, current_user, project, request, "down").await
}

pub(crate) async fn restart_docker_compose_project(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(project): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    docker_compose_action(state, current_user, project, request, "restart").await
}

pub(crate) async fn pull_docker_compose_project(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(project): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    docker_compose_action(state, current_user, project, request, "pull").await
}

pub(crate) async fn delete_docker_compose_project(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(project): AxumPath<String>,
    Json(request): Json<DockerActionRequest>,
) -> Result<Json<DockerActionResponse>, AppError> {
    docker_compose_action(state, current_user, project, request, "delete").await
}

pub(crate) async fn apply_approved_docker_task(state: &AppState, task_id: Uuid, step_id: Uuid) {
    let Ok(tasks) = state.store.tasks().list().await else {
        tracing::warn!("failed to inspect Docker task after approval");
        return;
    };
    let Some(task) = tasks.into_iter().find(|task| task.id == task_id) else {
        return;
    };
    let Some(step) = task.steps.iter().find(|step| step.id == step_id).cloned() else {
        return;
    };
    if step.capability != CapabilityName::ContainersManage {
        return;
    }
    if step.payload.get("resource").and_then(Value::as_str) != Some("docker") {
        return;
    }
    let Some(host_id) = task.host_id else {
        mark_docker_task_failed(state, task.id, step_id, "Docker task is missing host_id").await;
        return;
    };
    if let Err(error) = dispatch_docker_task(state, task.id, step_id, host_id, step.payload).await {
        tracing::warn!(?error, task_id = %task.id, "failed to dispatch approved Docker task");
        mark_docker_task_failed(state, task.id, step_id, &error.0.to_string()).await;
    }
}

async fn docker_container_action(
    state: AppState,
    current_user: CurrentUser,
    container: String,
    action: &'static str,
    request: DockerActionRequest,
) -> Result<Json<DockerActionResponse>, AppError> {
    let id_or_name = required_text(container, "container id or name is required")?;
    let host_id = require_host_id(request.host_id)?;
    let command = ContainerRuntimeCommand::Container(ContainerCommand::Start { id_or_name });
    let command = match action {
        "start" => command,
        _ => unreachable!("unsupported container action"),
    };
    docker_task_response(
        &state,
        current_user.username,
        host_id,
        format!("{action} Docker container"),
        request.reason,
        command,
    )
    .await
}

async fn docker_network_container_action(
    state: AppState,
    current_user: CurrentUser,
    network: String,
    request: DockerNetworkContainerRequest,
    action: &'static str,
) -> Result<Json<DockerActionResponse>, AppError> {
    let network = required_text(network, "network name or id is required")?;
    let container = required_text(request.container, "container id or name is required")?;
    let command = match action {
        "connect" => ContainerRuntimeCommand::Network(ContainerNetworkCommand::Connect(
            doro_container::NetworkContainerRequest {
                network,
                container,
                force: request.force,
            },
        )),
        "disconnect" => ContainerRuntimeCommand::Network(ContainerNetworkCommand::Disconnect(
            doro_container::NetworkContainerRequest {
                network,
                container,
                force: request.force,
            },
        )),
        _ => unreachable!("unsupported network action"),
    };
    docker_task_response(
        &state,
        current_user.username,
        request.host_id,
        format!("{action} Docker network"),
        request.reason,
        command,
    )
    .await
}

async fn docker_compose_action(
    state: AppState,
    current_user: CurrentUser,
    project: String,
    request: DockerActionRequest,
    action: &'static str,
) -> Result<Json<DockerActionResponse>, AppError> {
    let project = required_text(project, "compose project name is required")?;
    let host_id = require_host_id(request.host_id)?;
    let command = match action {
        "up" => ContainerRuntimeCommand::Compose(ContainerComposeCommand::Up { project }),
        "down" => ContainerRuntimeCommand::Compose(ContainerComposeCommand::Down { project }),
        "restart" => ContainerRuntimeCommand::Compose(ContainerComposeCommand::Restart { project }),
        "pull" => ContainerRuntimeCommand::Compose(ContainerComposeCommand::Pull { project }),
        "delete" => ContainerRuntimeCommand::Compose(ContainerComposeCommand::Delete { project }),
        _ => unreachable!("unsupported compose action"),
    };
    docker_task_response(
        &state,
        current_user.username,
        host_id,
        format!("{action} Docker Compose project"),
        request.reason,
        command,
    )
    .await
}

async fn docker_task_response(
    state: &AppState,
    created_by: String,
    host_id: Uuid,
    title: impl Into<String>,
    reason: Option<String>,
    command: ContainerRuntimeCommand,
) -> Result<Json<DockerActionResponse>, AppError> {
    ensure_docker_ready(state, host_id).await?;
    let task =
        create_docker_task(state, created_by, host_id, title.into(), reason, command).await?;
    Ok(Json(DockerActionResponse { task }))
}

async fn create_docker_task(
    state: &AppState,
    created_by: String,
    host_id: Uuid,
    title: String,
    reason: Option<String>,
    command: ContainerRuntimeCommand,
) -> Result<Task, AppError> {
    let step_id = Uuid::new_v4();
    let command_id = Uuid::new_v4();
    let envelope = ContainerRuntimeCommandEnvelope {
        command_id,
        task_id: None,
        step_id: Some(step_id),
        command,
    };
    let payload = json!({
        "resource": "docker",
        "reason": reason,
        "command": envelope,
    });
    Ok(state
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
            metadata: json!({ "resource": "docker" }),
            create_step_approvals: true,
            steps: vec![TaskStep {
                id: step_id,
                capability: CapabilityName::ContainersManage,
                risk: CapabilityRisk::High,
                summary: "Run approved Docker management command".to_string(),
                status: TaskStepStatus::Pending,
                payload,
            }],
        })
        .await?)
}

async fn dispatch_docker_task(
    state: &AppState,
    task_id: Uuid,
    step_id: Uuid,
    host_id: Uuid,
    payload: Value,
) -> Result<(), AppError> {
    let agent_id = ensure_docker_ready(state, host_id).await?;
    let mut envelope: ContainerRuntimeCommandEnvelope = serde_json::from_value(
        payload
            .get("command")
            .cloned()
            .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "Docker command missing"))?,
    )
    .map_err(AppError::from)?;
    envelope.task_id = Some(task_id);
    envelope.step_id = Some(step_id);
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

    let command_json = serde_json::to_string(&envelope)?;
    let result = match state
        .agent_streams
        .run_docker_command(host_id, command_json)
        .await
    {
        Ok(result) => result,
        Err(error) => {
            let finished_at = Utc::now();
            let message = docker_command_error_message(error);
            finish_failed_docker_run(state, task_id, step_id, task_run_id, finished_at, &message)
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
            external_event_id: None,
            event_type: "docker.command".to_string(),
            event_json: json!({
                "task_id": task_id,
                "step_id": step_id,
                "status": step_status,
            }),
            recorded_at: finished_at,
        })
        .await?;
    Ok(())
}

async fn finish_failed_docker_run(
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

async fn mark_docker_task_failed(state: &AppState, task_id: Uuid, step_id: Uuid, message: &str) {
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

async fn run_docker_query(
    state: &AppState,
    host_id: Uuid,
    command: ContainerRuntimeCommand,
) -> Result<Value, AppError> {
    ensure_docker_ready(state, host_id).await?;
    let envelope = ContainerRuntimeCommandEnvelope {
        command_id: Uuid::new_v4(),
        task_id: None,
        step_id: None,
        command,
    };
    let result = state
        .agent_streams
        .run_docker_command(host_id, serde_json::to_string(&envelope)?)
        .await
        .map_err(docker_command_app_error)?;
    if result.status == grpc::CommandStatus::Failed as i32 {
        return Err(AppError::status(
            StatusCode::BAD_GATEWAY,
            docker_command_error_message(crate::agent_streams::DockerCommandError::AgentFailed(
                result.message,
            )),
        ));
    }
    Ok(parse_json_value(&result.details_json))
}

async fn ensure_docker_ready(state: &AppState, host_id: Uuid) -> Result<Uuid, AppError> {
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
        .any(|capability| capability.name == CapabilityName::ContainersManage)
    {
        return Err(AppError::status(
            StatusCode::FORBIDDEN,
            "agent does not declare ContainersManage capability",
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

async fn docker_hosts(
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
                .any(|capability| capability.name == CapabilityName::ContainersManage)
        })
        .collect::<Vec<_>>();
    if hosts.is_empty() && host_id.is_some() {
        return Err(AppError::status(
            StatusCode::SERVICE_UNAVAILABLE,
            "Docker-capable agent is not online",
        ));
    }
    Ok(hosts)
}

fn require_host_id(host_id: Option<Uuid>) -> Result<Uuid, AppError> {
    host_id.ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "host_id is required"))
}

fn required_text(value: String, message: &str) -> Result<String, AppError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::status(StatusCode::BAD_REQUEST, message));
    }
    Ok(value)
}

fn string_map_from_value(
    value: Value,
    field: &str,
) -> Result<std::collections::HashMap<String, String>, AppError> {
    if value.is_null() {
        return Ok(std::collections::HashMap::new());
    }
    let Some(object) = value.as_object() else {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!("{field} must be an object"),
        ));
    };
    let mut map = std::collections::HashMap::new();
    for (key, value) in object {
        let Some(value) = value.as_str() else {
            return Err(AppError::status(
                StatusCode::BAD_REQUEST,
                format!("{field} values must be strings"),
            ));
        };
        map.insert(key.clone(), value.to_string());
    }
    Ok(map)
}

fn parse_json_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| json!({}))
}

#[derive(Debug, Deserialize)]
struct ComposeProjectPayload {
    name: String,
    status: String,
    path: String,
    services: Vec<String>,
    compose_yaml: Option<String>,
    env_file: Option<String>,
}

impl ComposeProjectPayload {
    fn into_protocol(self, host_id: Uuid) -> DockerComposeProject {
        DockerComposeProject {
            host_id,
            name: self.name,
            status: self.status,
            path: self.path,
            services: self.services,
            compose_yaml: self.compose_yaml,
            env_file: self.env_file,
        }
    }
}
