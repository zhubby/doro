use crate::agent_events::parse_event_payload;
use crate::agent_streams::agent_task_error_message;
use crate::prelude::*;
use crate::state::AppState;

pub(crate) async fn create_agent_tool_approval(
    store: &Store,
    host_id: Uuid,
    request: grpc::AgentToolApprovalRequestEvent,
    requested_at: DateTime<Utc>,
) -> Result<(), sea_orm::DbErr> {
    let task_id = doro_store::parse_uuid(&request.task_id).map_err(|_| {
        sea_orm::DbErr::Custom("agent tool approval task_id is invalid".to_string())
    })?;
    let risk = grpc_risk_to_protocol(&request.risk);
    let capability = agent_tool_capability(&request.tool_name);
    let step_id = Uuid::new_v4();
    let summary = if request.summary.trim().is_empty() {
        format!("Approve AI tool {}", request.tool_name)
    } else {
        request.summary.clone()
    };

    store
        .tasks()
        .append_step_with_approval(
            task_id,
            TaskStep {
                id: step_id,
                capability,
                risk,
                summary: summary.clone(),
                status: TaskStepStatus::WaitingApproval,
                payload: serde_json::json!({
                    "resource": "agent_ai_tool",
                    "host_id": host_id,
                    "request_id": request.request_id,
                    "command_id": request.command_id,
                    "tool_call_id": request.tool_call_id,
                    "tool_name": request.tool_name,
                    "risk": request.risk,
                    "arguments": parse_event_payload(&request.arguments_json),
                }),
            },
            summary,
            requested_at,
            requested_at + ChronoDuration::hours(DEFAULT_APPROVAL_TTL_HOURS),
        )
        .await?;
    store
        .tasks()
        .update_status(task_id, TaskStatus::WaitingApproval, None, None)
        .await?;
    Ok(())
}

pub(crate) async fn apply_agent_tool_approval_decision(
    state: &AppState,
    approval: &doro_protocol::ApprovalRequest,
    approved: bool,
) {
    let Ok(tasks) = state.store.tasks().list().await else {
        tracing::warn!("failed to inspect task for agent tool approval");
        return;
    };
    let Some(task) = tasks.into_iter().find(|task| task.id == approval.task_id) else {
        return;
    };
    let Some(step) = task
        .steps
        .into_iter()
        .find(|step| step.id == approval.step_id)
    else {
        return;
    };
    if step.payload.get("resource").and_then(Value::as_str) != Some("agent_ai_tool") {
        return;
    }

    let Some(host_id) = step
        .payload
        .get("host_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
    else {
        tracing::warn!(step_id = %step.id, "agent tool approval step is missing host_id");
        return;
    };
    let Some(request_id) = step.payload.get("request_id").and_then(Value::as_str) else {
        tracing::warn!(step_id = %step.id, "agent tool approval step is missing request_id");
        return;
    };

    let message = approval
        .decision_note
        .clone()
        .unwrap_or_else(|| if approved { "approved" } else { "denied" }.to_string());
    let decision = grpc::AgentToolApprovalDecisionCommand {
        request_id: request_id.to_string(),
        task_id: approval.task_id.to_string(),
        step_id: step.id.to_string(),
        approved,
        message,
    };

    if let Err(error) = state
        .agent_streams
        .send_agent_tool_approval_decision(host_id, decision)
        .await
    {
        tracing::warn!(
            ?error,
            host_id = %host_id,
            step_id = %step.id,
            "failed to send agent tool approval decision"
        );
        let _ = state
            .store
            .tasks()
            .update_step_status(step.id, "failed")
            .await;
        let _ = state
            .store
            .tasks()
            .update_status(
                approval.task_id,
                TaskStatus::Failed,
                Some(Utc::now()),
                Some(agent_task_error_message(error)),
            )
            .await;
        return;
    }

    let step_status = if approved { "running" } else { "failed" };
    if let Err(error) = state
        .store
        .tasks()
        .update_step_status(step.id, step_status)
        .await
    {
        tracing::warn!(%error, step_id = %step.id, "failed to update agent tool step status");
    }
    if approved {
        let _ = state
            .store
            .tasks()
            .update_status(approval.task_id, TaskStatus::Running, None, None)
            .await;
    }
}

pub(crate) fn agent_tool_capability(tool_name: &str) -> CapabilityName {
    match tool_name {
        "run_shell" => CapabilityName::ShellExecute,
        "write_file" | "file_operation" => CapabilityName::FilesWrite,
        "container_snapshot" => CapabilityName::ContainersManage,
        "virtual_machine_snapshot" => CapabilityName::VirtualMachinesManage,
        _ => CapabilityName::AgentRun,
    }
}

pub(crate) fn grpc_risk_to_protocol(risk: &str) -> CapabilityRisk {
    match risk {
        "Low" | "low" => CapabilityRisk::Low,
        "High" | "high" => CapabilityRisk::High,
        _ => CapabilityRisk::Medium,
    }
}

pub(crate) fn normalize_task_step_status(status: &str) -> Option<&str> {
    match status {
        "pending" | "waiting_approval" | "running" | "succeeded" | "failed" | "cancelled" => {
            Some(status)
        }
        _ => None,
    }
}
