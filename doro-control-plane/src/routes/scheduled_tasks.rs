use crate::agent_events::timestamp_to_utc;
use crate::agent_streams::{
    AgentStreamRegistry, agent_task_error_message, terminal_command_error_message,
};
use crate::auth::CurrentUser;
use crate::error::{AppError, scheduled_task_store_app_error};
use crate::prelude::*;
use crate::state::AppState;

pub(crate) async fn list_scheduled_tasks(
    State(state): State<AppState>,
) -> Result<Json<ListScheduledTasksResponse>, AppError> {
    Ok(Json(ListScheduledTasksResponse {
        items: state.store.scheduled_tasks().list().await?,
    }))
}

pub(crate) async fn create_scheduled_task(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    Json(request): Json<CreateScheduledTaskRequest>,
) -> Result<Json<CreateScheduledTaskResponse>, AppError> {
    let now = Utc::now();
    let name = required_text(request.name, "scheduled task name is required")?;
    let schedule = normalize_cron_expression(&request.schedule)?;
    let label_selector = normalize_label_selector(request.label_selector);
    let (required_capability, task_template) = scheduled_task_template(
        request.kind,
        request.script,
        request.prompt,
        request.timeout_seconds,
    )?;
    let status = match request.kind {
        ScheduledTaskKind::Script => ScheduledTaskStatus::PendingApproval,
        ScheduledTaskKind::AgentRun => ScheduledTaskStatus::Active,
    };
    let next_run_at = if status == ScheduledTaskStatus::Active {
        Some(next_cron_run_at(&schedule, now)?)
    } else {
        None
    };
    let scheduled_task_id = Uuid::new_v4();
    let item = state
        .store
        .scheduled_tasks()
        .create(NewScheduledTask {
            id: scheduled_task_id,
            name: name.clone(),
            kind: request.kind,
            schedule: schedule.clone(),
            status,
            required_capability,
            label_selector: label_selector.clone(),
            task_template: task_template.clone(),
            next_run_at,
            approval_task_id: None,
            created_at: now,
        })
        .await?;

    let approval_task = if request.kind == ScheduledTaskKind::Script {
        let approval_task = create_scheduled_task_approval_task(
            &state,
            current_user.username,
            scheduled_task_id,
            &name,
            &label_selector,
            &task_template,
        )
        .await?;
        let item = state
            .store
            .scheduled_tasks()
            .update(
                scheduled_task_id,
                ScheduledTaskChanges {
                    approval_task_id: Some(Some(approval_task.id)),
                    updated_at: Some(Utc::now()),
                    ..ScheduledTaskChanges::default()
                },
            )
            .await?;
        return Ok(Json(CreateScheduledTaskResponse {
            item,
            approval_task: Some(approval_task),
        }));
    } else {
        None
    };

    Ok(Json(CreateScheduledTaskResponse {
        item,
        approval_task,
    }))
}

pub(crate) async fn update_scheduled_task(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(scheduled_task_id): AxumPath<Uuid>,
    Json(request): Json<UpdateScheduledTaskRequest>,
) -> Result<Json<UpdateScheduledTaskResponse>, AppError> {
    let existing = state
        .store
        .scheduled_tasks()
        .get(scheduled_task_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "scheduled task not found"))?;
    let mut template = existing.task_template.clone();
    let mut changes = ScheduledTaskChanges::default();
    if let Some(name) = request.name {
        changes.name = Some(required_text(name, "scheduled task name is required")?);
    }
    if let Some(schedule) = request.schedule {
        let schedule = normalize_cron_expression(&schedule)?;
        changes.schedule = Some(schedule.clone());
        if existing.status == ScheduledTaskStatus::Active {
            changes.next_run_at = Some(Some(next_cron_run_at(&schedule, Utc::now())?));
        }
    }
    if let Some(label_selector) = request.label_selector {
        changes.label_selector = Some(normalize_label_selector(label_selector));
    }

    let mut script_changed = false;
    match existing.kind {
        ScheduledTaskKind::Script => {
            if let Some(script) = request.script {
                template["script"] = Value::String(required_text(
                    script,
                    "script scheduled task requires a script",
                )?);
                script_changed = true;
            }
            if let Some(timeout_seconds) = request.timeout_seconds {
                template["timeout_seconds"] =
                    serde_json::json!(timeout_seconds.clamp(1, MAX_TERMINAL_TIMEOUT_SECONDS));
            }
        }
        ScheduledTaskKind::AgentRun => {
            if let Some(prompt) = request.prompt {
                template["prompt"] = Value::String(required_text(
                    prompt,
                    "agent scheduled task requires a prompt",
                )?);
            }
        }
    }
    changes.task_template = Some(template.clone());
    if script_changed {
        changes.status = Some(ScheduledTaskStatus::PendingApproval);
        changes.next_run_at = Some(None);
        changes.approved_at = Some(None);
        changes.approved_by = Some(None);
        changes.approval_task_id = Some(None);
    }
    changes.updated_at = Some(Utc::now());
    let mut item = state
        .store
        .scheduled_tasks()
        .update(scheduled_task_id, changes)
        .await?;

    if script_changed {
        let labels = item.label_selector.clone();
        let approval_task = create_scheduled_task_approval_task(
            &state,
            current_user.username,
            scheduled_task_id,
            &item.name,
            &labels,
            &template,
        )
        .await?;
        item = state
            .store
            .scheduled_tasks()
            .update(
                scheduled_task_id,
                ScheduledTaskChanges {
                    approval_task_id: Some(Some(approval_task.id)),
                    updated_at: Some(Utc::now()),
                    ..ScheduledTaskChanges::default()
                },
            )
            .await?;
    }

    Ok(Json(UpdateScheduledTaskResponse { item }))
}

pub(crate) async fn delete_scheduled_task(
    State(state): State<AppState>,
    AxumPath(scheduled_task_id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if state
        .store
        .scheduled_tasks()
        .delete(scheduled_task_id)
        .await?
    {
        return Ok(StatusCode::NO_CONTENT);
    }
    Err(AppError::status(
        StatusCode::NOT_FOUND,
        "scheduled task not found",
    ))
}

pub(crate) async fn enable_scheduled_task(
    State(state): State<AppState>,
    Extension(current_user): Extension<CurrentUser>,
    AxumPath(scheduled_task_id): AxumPath<Uuid>,
) -> Result<Json<ScheduledTaskActionResponse>, AppError> {
    let item = state
        .store
        .scheduled_tasks()
        .get(scheduled_task_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "scheduled task not found"))?;

    if item.kind == ScheduledTaskKind::Script && item.approved_at.is_none() {
        let approval_task = create_scheduled_task_approval_task(
            &state,
            current_user.username,
            item.id,
            &item.name,
            &item.label_selector,
            &item.task_template,
        )
        .await?;
        let item = state
            .store
            .scheduled_tasks()
            .update(
                item.id,
                ScheduledTaskChanges {
                    status: Some(ScheduledTaskStatus::PendingApproval),
                    next_run_at: Some(None),
                    approval_task_id: Some(Some(approval_task.id)),
                    updated_at: Some(Utc::now()),
                    ..ScheduledTaskChanges::default()
                },
            )
            .await?;
        return Ok(Json(ScheduledTaskActionResponse {
            item,
            task: Some(approval_task),
            runs: Vec::new(),
        }));
    }

    let next_run_at = next_cron_run_at(&item.schedule, Utc::now())?;
    let item = state
        .store
        .scheduled_tasks()
        .update(
            item.id,
            ScheduledTaskChanges {
                status: Some(ScheduledTaskStatus::Active),
                next_run_at: Some(Some(next_run_at)),
                updated_at: Some(Utc::now()),
                ..ScheduledTaskChanges::default()
            },
        )
        .await?;
    Ok(Json(ScheduledTaskActionResponse {
        item,
        task: None,
        runs: Vec::new(),
    }))
}

pub(crate) async fn disable_scheduled_task(
    State(state): State<AppState>,
    AxumPath(scheduled_task_id): AxumPath<Uuid>,
) -> Result<Json<ScheduledTaskActionResponse>, AppError> {
    let item = state
        .store
        .scheduled_tasks()
        .update(
            scheduled_task_id,
            ScheduledTaskChanges {
                status: Some(ScheduledTaskStatus::Paused),
                next_run_at: Some(None),
                updated_at: Some(Utc::now()),
                ..ScheduledTaskChanges::default()
            },
        )
        .await
        .map_err(scheduled_task_store_app_error)?;
    Ok(Json(ScheduledTaskActionResponse {
        item,
        task: None,
        runs: Vec::new(),
    }))
}

pub(crate) async fn run_scheduled_task_now(
    State(state): State<AppState>,
    AxumPath(scheduled_task_id): AxumPath<Uuid>,
) -> Result<Json<ScheduledTaskActionResponse>, AppError> {
    let item = state
        .store
        .scheduled_tasks()
        .get(scheduled_task_id)
        .await?
        .ok_or_else(|| AppError::status(StatusCode::NOT_FOUND, "scheduled task not found"))?;
    ensure_scheduled_task_approved(&item)?;
    let trigger = trigger_scheduled_task(&state.store, &state.agent_streams, item.clone()).await?;
    let item = state
        .store
        .scheduled_tasks()
        .get(scheduled_task_id)
        .await?
        .unwrap_or(item);
    Ok(Json(ScheduledTaskActionResponse {
        item,
        task: trigger.first_task,
        runs: trigger.runs,
    }))
}

pub(crate) async fn list_scheduled_task_runs(
    State(state): State<AppState>,
    AxumPath(scheduled_task_id): AxumPath<Uuid>,
) -> Result<Json<ListScheduledTaskRunsResponse>, AppError> {
    Ok(Json(ListScheduledTaskRunsResponse {
        items: state
            .store
            .scheduled_tasks()
            .list_runs(scheduled_task_id)
            .await?,
    }))
}

#[derive(Debug, Default)]
pub(crate) struct ScheduledTaskTrigger {
    runs: Vec<ScheduledTaskRun>,
    first_task: Option<Task>,
}

pub(crate) fn required_text(value: String, message: &'static str) -> Result<String, AppError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::status(StatusCode::BAD_REQUEST, message));
    }
    Ok(value)
}

pub(crate) fn normalize_label_selector(labels: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for label in labels {
        let label = label.trim();
        if label.is_empty() || normalized.iter().any(|existing| existing == label) {
            continue;
        }
        normalized.push(label.to_string());
    }
    normalized
}

pub(crate) fn normalize_cron_expression(expression: &str) -> Result<String, AppError> {
    let fields = expression
        .split_whitespace()
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let normalized = match fields.len() {
        5 => format!("0 {}", fields.join(" ")),
        6 => fields.join(" "),
        _ => {
            return Err(AppError::status(
                StatusCode::BAD_REQUEST,
                "schedule must be a 5-field cron expression",
            ));
        }
    };
    Schedule::from_str(&normalized).map_err(|error| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            format!("schedule is not a valid cron expression: {error}"),
        )
    })?;
    Ok(normalized)
}

pub(crate) fn next_cron_run_at(
    schedule_expression: &str,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>, AppError> {
    let schedule = Schedule::from_str(schedule_expression).map_err(|error| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            format!("schedule is not a valid cron expression: {error}"),
        )
    })?;
    schedule.after(&after).next().ok_or_else(|| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            "schedule does not produce a future run time",
        )
    })
}

pub(crate) fn scheduled_task_template(
    kind: ScheduledTaskKind,
    script: Option<String>,
    prompt: Option<String>,
    timeout_seconds: Option<u32>,
) -> Result<(CapabilityName, Value), AppError> {
    match kind {
        ScheduledTaskKind::Script => {
            let script = required_text(
                script.unwrap_or_default(),
                "script scheduled task requires a script",
            )?;
            Ok((
                CapabilityName::ShellExecute,
                serde_json::json!({
                    "script": script,
                    "timeout_seconds": timeout_seconds
                        .unwrap_or(DEFAULT_TERMINAL_TIMEOUT_SECONDS)
                        .clamp(1, MAX_TERMINAL_TIMEOUT_SECONDS),
                }),
            ))
        }
        ScheduledTaskKind::AgentRun => {
            let prompt = required_text(
                prompt.unwrap_or_default(),
                "agent scheduled task requires a prompt",
            )?;
            Ok((
                CapabilityName::AgentRun,
                serde_json::json!({
                    "prompt": prompt,
                }),
            ))
        }
    }
}

pub(crate) async fn create_scheduled_task_approval_task(
    state: &AppState,
    created_by: String,
    scheduled_task_id: Uuid,
    scheduled_task_name: &str,
    label_selector: &[String],
    task_template: &Value,
) -> Result<Task, AppError> {
    let step_id = Uuid::new_v4();
    let script = task_template
        .get("script")
        .and_then(Value::as_str)
        .unwrap_or_default();
    state
        .store
        .tasks()
        .create_with_steps(NewTask {
            id: Uuid::new_v4(),
            host_id: None,
            title: format!("enable scheduled script {scheduled_task_name}"),
            prompt: None,
            status: TaskStatus::WaitingApproval,
            created_by,
            created_at: Utc::now(),
            metadata: serde_json::json!({
                "resource": "scheduled_task",
                "scheduled_task_id": scheduled_task_id,
                "action": "enable",
            }),
            create_step_approvals: true,
            steps: vec![TaskStep {
                id: step_id,
                capability: CapabilityName::ShellExecute,
                risk: CapabilityRisk::High,
                summary: "Approve scheduled shell execution".to_string(),
                status: TaskStepStatus::Pending,
                payload: serde_json::json!({
                    "resource": "scheduled_task",
                    "action": "enable",
                    "scheduled_task_id": scheduled_task_id,
                    "label_selector": label_selector,
                    "script": script,
                }),
            }],
        })
        .await
        .map_err(AppError::from)
}

pub(crate) fn ensure_scheduled_task_approved(task: &ScheduledTask) -> Result<(), AppError> {
    if task.kind == ScheduledTaskKind::Script && task.approved_at.is_none() {
        return Err(AppError::status(
            StatusCode::CONFLICT,
            "scheduled script task is waiting for approval",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_five_field_cron_to_seconds_field() {
        let schedule = match normalize_cron_expression("0 3 * * *") {
            Ok(schedule) => schedule,
            Err(error) => panic!("schedule should parse: {error:?}"),
        };

        assert_eq!(schedule, "0 0 3 * * *");
    }
}

pub(crate) async fn apply_approved_scheduled_task(
    state: &AppState,
    approval: &doro_protocol::ApprovalRequest,
) {
    let Ok(Some(task)) = state
        .store
        .scheduled_tasks()
        .find_by_approval_task(approval.task_id)
        .await
    else {
        return;
    };
    if task.kind != ScheduledTaskKind::Script {
        return;
    }
    let now = Utc::now();
    let Ok(next_run_at) = next_cron_run_at(&task.schedule, now) else {
        tracing::warn!(scheduled_task_id = %task.id, "approved scheduled task has invalid schedule");
        return;
    };
    if let Err(error) = state
        .store
        .scheduled_tasks()
        .update(
            task.id,
            ScheduledTaskChanges {
                status: Some(ScheduledTaskStatus::Active),
                next_run_at: Some(Some(next_run_at)),
                approved_at: Some(Some(now)),
                approved_by: Some(approval.resolved_by.clone()),
                updated_at: Some(now),
                ..ScheduledTaskChanges::default()
            },
        )
        .await
    {
        tracing::warn!(%error, scheduled_task_id = %task.id, "failed to activate approved scheduled task");
    }
}

pub(crate) async fn trigger_scheduled_task(
    store: &Store,
    agent_streams: &AgentStreamRegistry,
    scheduled_task: ScheduledTask,
) -> Result<ScheduledTaskTrigger, AppError> {
    ensure_scheduled_task_approved(&scheduled_task)?;
    let started_at = Utc::now();
    let hosts = store.hosts().list().await?;
    let mut matches = Vec::new();
    for host in hosts {
        if host.status != HostStatus::Online {
            continue;
        }
        if !scheduled_task
            .label_selector
            .iter()
            .all(|required| host.labels.iter().any(|label| label == required))
        {
            continue;
        }
        if !host
            .capabilities
            .iter()
            .any(|capability| capability.name == scheduled_task.required_capability)
        {
            continue;
        }
        let Some(agent_id) = agent_streams.agent_id_for_host(host.id).await else {
            continue;
        };
        matches.push((host.id, agent_id, host.display_name));
    }

    if matches.is_empty() {
        let run = store
            .scheduled_tasks()
            .create_run(NewScheduledTaskRun {
                id: Uuid::new_v4(),
                scheduled_task_id: scheduled_task.id,
                task_id: None,
                status: ScheduledTaskRunStatus::Skipped,
                started_at,
                finished_at: Some(started_at),
                message: Some("no online agent matched required tags and capability".to_string()),
            })
            .await?;
        store
            .scheduled_tasks()
            .update(
                scheduled_task.id,
                ScheduledTaskChanges {
                    last_run_at: Some(Some(started_at)),
                    last_run_status: Some(Some(ScheduledTaskRunStatus::Skipped)),
                    updated_at: Some(Utc::now()),
                    ..ScheduledTaskChanges::default()
                },
            )
            .await?;
        return Ok(ScheduledTaskTrigger {
            runs: vec![run],
            first_task: None,
        });
    }

    let mut trigger = ScheduledTaskTrigger::default();
    let mut saw_failed = false;
    for (host_id, agent_id, host_name) in matches {
        let (task, run) = dispatch_scheduled_task_to_host(
            store,
            agent_streams,
            &scheduled_task,
            host_id,
            agent_id,
            host_name,
        )
        .await?;
        saw_failed |= run.status == ScheduledTaskRunStatus::Failed;
        if trigger.first_task.is_none() {
            trigger.first_task = Some(task);
        }
        trigger.runs.push(run);
    }

    store
        .scheduled_tasks()
        .update(
            scheduled_task.id,
            ScheduledTaskChanges {
                last_run_at: Some(Some(started_at)),
                last_run_status: Some(Some(if saw_failed {
                    ScheduledTaskRunStatus::Failed
                } else {
                    ScheduledTaskRunStatus::Succeeded
                })),
                updated_at: Some(Utc::now()),
                ..ScheduledTaskChanges::default()
            },
        )
        .await?;
    Ok(trigger)
}

pub(crate) async fn dispatch_scheduled_task_to_host(
    store: &Store,
    agent_streams: &AgentStreamRegistry,
    scheduled_task: &ScheduledTask,
    host_id: Uuid,
    agent_id: Uuid,
    host_name: String,
) -> Result<(Task, ScheduledTaskRun), AppError> {
    let now = Utc::now();
    let step_id = Uuid::new_v4();
    let (summary, capability, risk) = match scheduled_task.kind {
        ScheduledTaskKind::Script => (
            "Run scheduled shell script",
            CapabilityName::ShellExecute,
            CapabilityRisk::High,
        ),
        ScheduledTaskKind::AgentRun => (
            "Run scheduled agent placeholder",
            CapabilityName::AgentRun,
            CapabilityRisk::Medium,
        ),
    };
    let task = store
        .tasks()
        .create_with_steps(NewTask {
            id: Uuid::new_v4(),
            host_id: Some(host_id),
            title: format!("scheduled task {} on {host_name}", scheduled_task.name),
            prompt: None,
            status: TaskStatus::Queued,
            created_by: "scheduler".to_string(),
            created_at: now,
            metadata: serde_json::json!({
                "resource": "scheduled_task",
                "scheduled_task_id": scheduled_task.id,
                "kind": scheduled_task.kind,
            }),
            create_step_approvals: false,
            steps: vec![TaskStep {
                id: step_id,
                capability,
                risk,
                summary: summary.to_string(),
                status: TaskStepStatus::Pending,
                payload: serde_json::json!({
                    "scheduled_task_id": scheduled_task.id,
                    "template": scheduled_task.task_template.clone(),
                }),
            }],
        })
        .await?;
    store
        .tasks()
        .update_status(task.id, TaskStatus::Running, None, None)
        .await?;
    store.tasks().update_step_status(step_id, "running").await?;
    let scheduled_run = store
        .scheduled_tasks()
        .create_run(NewScheduledTaskRun {
            id: Uuid::new_v4(),
            scheduled_task_id: scheduled_task.id,
            task_id: Some(task.id),
            status: ScheduledTaskRunStatus::Running,
            started_at: now,
            finished_at: None,
            message: None,
        })
        .await?;
    let task_run_id = Uuid::new_v4();
    store
        .tasks()
        .create_run(NewTaskRun {
            id: task_run_id,
            task_id: task.id,
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

    let outcome = match scheduled_task.kind {
        ScheduledTaskKind::Script => {
            execute_scheduled_script(agent_streams, host_id, scheduled_task).await
        }
        ScheduledTaskKind::AgentRun => {
            execute_scheduled_agent_task(agent_streams, host_id, scheduled_task, task.id).await
        }
    };
    let finished_at = Utc::now();
    let (run_status, command_id, result_json, message) = match outcome {
        Ok(outcome) => (
            if outcome.succeeded {
                ScheduledTaskRunStatus::Succeeded
            } else {
                ScheduledTaskRunStatus::Failed
            },
            outcome.command_id,
            outcome.result_json,
            outcome.message,
        ),
        Err(message) => (
            ScheduledTaskRunStatus::Failed,
            None,
            serde_json::json!({}),
            Some(message),
        ),
    };
    let task_status = if run_status == ScheduledTaskRunStatus::Succeeded {
        TaskStatus::Succeeded
    } else {
        TaskStatus::Failed
    };
    let step_status = if run_status == ScheduledTaskRunStatus::Succeeded {
        "succeeded"
    } else {
        "failed"
    };
    store
        .tasks()
        .update_step_status(step_id, step_status)
        .await?;
    store
        .tasks()
        .update_status(
            task.id,
            task_status,
            Some(finished_at),
            if run_status == ScheduledTaskRunStatus::Failed {
                message.clone()
            } else {
                None
            },
        )
        .await?;
    store
        .tasks()
        .finish_run(
            task_run_id,
            step_status.to_string(),
            command_id,
            finished_at,
            result_json,
            if run_status == ScheduledTaskRunStatus::Failed {
                message.clone()
            } else {
                None
            },
        )
        .await?;
    let scheduled_run = store
        .scheduled_tasks()
        .finish_run(scheduled_run.id, run_status, finished_at, message)
        .await?;
    Ok((task, scheduled_run))
}

#[derive(Debug)]
pub(crate) struct ScheduledCommandOutcome {
    succeeded: bool,
    command_id: Option<String>,
    result_json: Value,
    message: Option<String>,
}

pub(crate) async fn execute_scheduled_script(
    agent_streams: &AgentStreamRegistry,
    host_id: Uuid,
    scheduled_task: &ScheduledTask,
) -> Result<ScheduledCommandOutcome, String> {
    let script = scheduled_task
        .task_template
        .get("script")
        .and_then(Value::as_str)
        .ok_or_else(|| "scheduled script template is missing script".to_string())?;
    let timeout_seconds = scheduled_task
        .task_template
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(DEFAULT_TERMINAL_TIMEOUT_SECONDS)
        .clamp(1, MAX_TERMINAL_TIMEOUT_SECONDS);
    let result = agent_streams
        .run_terminal_command(&TerminalCommandRequest {
            host_id,
            input: script.to_string(),
            cols: None,
            rows: None,
            timeout_seconds: Some(timeout_seconds),
        })
        .await
        .map_err(terminal_command_error_message)?;
    let succeeded = result.status == grpc::CommandStatus::Succeeded as i32;
    Ok(ScheduledCommandOutcome {
        succeeded,
        command_id: Some(result.command_id),
        result_json: serde_json::json!({
            "output": result.output,
            "exit_code": if result.exit_code < 0 { Value::Null } else { serde_json::json!(result.exit_code) },
            "started_at": result.started_at.as_ref().and_then(timestamp_to_utc),
            "finished_at": result.finished_at.as_ref().and_then(timestamp_to_utc),
        }),
        message: if succeeded {
            Some("script completed".to_string())
        } else {
            Some("script failed".to_string())
        },
    })
}

pub(crate) async fn execute_scheduled_agent_task(
    agent_streams: &AgentStreamRegistry,
    host_id: Uuid,
    scheduled_task: &ScheduledTask,
    task_id: Uuid,
) -> Result<ScheduledCommandOutcome, String> {
    let prompt = scheduled_task
        .task_template
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let result = agent_streams
        .run_agent_task(
            host_id,
            grpc::RunAgentTaskCommand {
                command_id: String::new(),
                task_id: task_id.to_string(),
                scheduled_task_id: scheduled_task.id.to_string(),
                prompt,
                template_json: scheduled_task.task_template.to_string(),
                ai_provider: None,
            },
        )
        .await
        .map_err(agent_task_error_message)?;
    let succeeded = result.status == grpc::CommandStatus::Succeeded as i32;
    Ok(ScheduledCommandOutcome {
        succeeded,
        command_id: Some(result.command_id),
        result_json: serde_json::json!({
            "message": result.message,
        }),
        message: Some(result.message),
    })
}

pub(crate) async fn run_scheduled_task_scheduler(
    store: Store,
    agent_streams: AgentStreamRegistry,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(SCHEDULED_TASK_TICK_SECONDS));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(error) = run_due_scheduled_tasks(&store, &agent_streams).await {
                    tracing::warn!(?error, "scheduled task tick failed");
                }
            }
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
        }
    }
}

pub(crate) async fn run_due_scheduled_tasks(
    store: &Store,
    agent_streams: &AgentStreamRegistry,
) -> Result<(), AppError> {
    let now = Utc::now();
    let due = store.scheduled_tasks().due(now).await?;
    for scheduled_task in due {
        if let Err(error) = ensure_scheduled_task_approved(&scheduled_task) {
            tracing::warn!(?error, scheduled_task_id = %scheduled_task.id, "scheduled task is not approved");
            continue;
        }
        let next_run_at = match next_cron_run_at(&scheduled_task.schedule, now) {
            Ok(next_run_at) => next_run_at,
            Err(error) => {
                tracing::warn!(?error, scheduled_task_id = %scheduled_task.id, "scheduled task has invalid schedule");
                let _ = store
                    .scheduled_tasks()
                    .update(
                        scheduled_task.id,
                        ScheduledTaskChanges {
                            status: Some(ScheduledTaskStatus::Paused),
                            next_run_at: Some(None),
                            updated_at: Some(Utc::now()),
                            ..ScheduledTaskChanges::default()
                        },
                    )
                    .await;
                continue;
            }
        };
        store
            .scheduled_tasks()
            .update(
                scheduled_task.id,
                ScheduledTaskChanges {
                    next_run_at: Some(Some(next_run_at)),
                    updated_at: Some(Utc::now()),
                    ..ScheduledTaskChanges::default()
                },
            )
            .await?;
        if let Err(error) = trigger_scheduled_task(store, agent_streams, scheduled_task).await {
            tracing::warn!(?error, "scheduled task dispatch failed");
        }
    }
    Ok(())
}
