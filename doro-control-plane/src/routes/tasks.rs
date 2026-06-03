use crate::agent_streams::agent_task_error_message;
use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::prelude::*;
use crate::routes::scheduled_tasks::required_text;
use crate::state::AppState;

pub(crate) async fn list_tasks(
    State(state): State<AppState>,
) -> Result<Json<ListTasksResponse>, AppError> {
    Ok(Json(ListTasksResponse {
        items: state.store.tasks().list().await?,
    }))
}

pub(crate) async fn create_task(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<Task>, AppError> {
    if let Some(prompt) = request.prompt.clone() {
        let prompt = required_text(prompt, "task prompt is required")?;
        let host_id = request
            .host_id
            .ok_or_else(|| AppError::status(StatusCode::BAD_REQUEST, "host_id is required"))?;
        ensure_agent_run_ready(&state, host_id).await?;
        let step_id = Uuid::new_v4();
        let task = state
            .store
            .tasks()
            .create_with_steps(NewTask {
                id: Uuid::new_v4(),
                host_id: Some(host_id),
                title: required_text(request.title, "task title is required")?,
                prompt: Some(prompt.clone()),
                status: TaskStatus::Queued,
                created_by: current_user.username,
                created_at: Utc::now(),
                metadata: serde_json::json!({
                    "resource": "agent_ai_task",
                }),
                create_step_approvals: false,
                steps: vec![TaskStep {
                    id: step_id,
                    capability: CapabilityName::AgentRun,
                    risk: CapabilityRisk::Medium,
                    summary: "Run AI-guided agent operation".to_string(),
                    status: TaskStepStatus::Pending,
                    payload: serde_json::json!({
                        "prompt": prompt.clone(),
                    }),
                }],
            })
            .await?;

        let dispatch_state = state.clone();
        let dispatch_prompt = prompt.clone();
        let dispatch_task_id = task.id;
        tokio::spawn(async move {
            if let Err(error) = dispatch_agent_run_task(
                &dispatch_state,
                dispatch_task_id,
                step_id,
                host_id,
                dispatch_prompt,
                None,
            )
            .await
            {
                tracing::warn!(
                    ?error,
                    task_id = %dispatch_task_id,
                    "failed to dispatch agent AI task"
                );
            }
        });

        return Ok(Json(task));
    }

    let prompt = None;
    let steps = Vec::<TaskStep>::new();
    let status = if steps.iter().any(|step| step.risk >= CapabilityRisk::High) {
        TaskStatus::WaitingApproval
    } else {
        TaskStatus::Queued
    };

    let task = state
        .store
        .tasks()
        .create_with_steps(NewTask {
            id: Uuid::new_v4(),
            host_id: request.host_id,
            title: request.title,
            prompt,
            status,
            created_by: current_user.username,
            created_at: Utc::now(),
            metadata: serde_json::json!({}),
            create_step_approvals: true,
            steps,
        })
        .await?;

    Ok(Json(task))
}

pub(crate) async fn ensure_agent_run_ready(
    state: &AppState,
    host_id: Uuid,
) -> Result<Uuid, AppError> {
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
        .any(|capability| capability.name == CapabilityName::AgentRun)
    {
        return Err(AppError::status(
            StatusCode::FORBIDDEN,
            "agent does not declare AgentRun capability",
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

pub(crate) async fn dispatch_agent_run_task(
    state: &AppState,
    task_id: Uuid,
    step_id: Uuid,
    host_id: Uuid,
    prompt: String,
    scheduled_task_id: Option<Uuid>,
) -> Result<(), AppError> {
    let agent_id = ensure_agent_run_ready(state, host_id).await?;
    let now = Utc::now();
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
    let task_run_id = Uuid::new_v4();
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
            result_json: serde_json::json!({}),
            error_message: None,
        })
        .await?;

    let result = state
        .agent_streams
        .run_agent_task(
            host_id,
            grpc::RunAgentTaskCommand {
                command_id: String::new(),
                task_id: task_id.to_string(),
                scheduled_task_id: scheduled_task_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                prompt,
                template_json: serde_json::json!({
                    "source": "manual_task",
                })
                .to_string(),
            },
        )
        .await
        .map_err(|error| {
            AppError::status(StatusCode::BAD_GATEWAY, agent_task_error_message(error))
        })?;
    let finished_at = Utc::now();
    let succeeded = result.status == grpc::CommandStatus::Succeeded as i32;
    let task_status = if succeeded {
        TaskStatus::Succeeded
    } else {
        TaskStatus::Failed
    };
    let step_status = if succeeded { "succeeded" } else { "failed" };
    let error_message = if succeeded {
        None
    } else {
        Some(result.message.clone())
    };

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
            Some(result.command_id.clone()),
            finished_at,
            serde_json::json!({
                "message": result.message,
            }),
            error_message,
        )
        .await?;
    Ok(())
}
