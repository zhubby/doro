use crate::constants::MAX_FILE_TRANSFER_BYTES;
use crate::filesystem;
use crate::runtime::{Agent, VmRuntime};
use crate::terminal::{TerminalCommand, TerminalManager};
use crate::tools::{AgentCommandState, LocalAgentToolExecutor, parse_json_value};
use crate::website_routes::apply_website_routes;
use async_trait::async_trait;
use doro_ai::{
    AgentError, AgentRunEvent, AgentRunEventSink, AgentRunOutcome, AgentRunRequest, AgentRunStatus,
};
use doro_protocol::grpc;
use doro_vm::{VmCommand, VmCommandEnvelope, VmCommandStatus, VmProviderError};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentCommandAction {
    Continue,
    Reconnect,
}

pub(crate) async fn handle_command(
    command: grpc::ControlPlaneCommand,
    agent: &Agent,
    agent_id: Uuid,
    sender: &mpsc::Sender<grpc::AgentEvent>,
    terminal: &TerminalManager,
    command_state: &AgentCommandState,
) -> AgentCommandAction {
    let command_id = command.command_id.clone();
    match command.command {
        Some(grpc::control_plane_command::Command::Ack(_)) => {
            tracing::info!(command_id = %command_id, "control-plane acknowledged stream")
        }
        Some(grpc::control_plane_command::Command::CollectContainers(_)) => {
            tracing::info!(command_id = %command_id, "collecting containers by control-plane request");
            let event = match &agent.container_runtime {
                Some(runtime) => match runtime.snapshot().await {
                    Ok(snapshot) => agent.container_snapshot_event(agent_id, command_id, snapshot),
                    Err(error) => agent.command_result_event(
                        agent_id,
                        command_id,
                        grpc::CommandStatus::Failed,
                        error.to_string(),
                    ),
                },
                None => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    "container provider is not enabled",
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue command result event");
            }
        }
        Some(grpc::control_plane_command::Command::CollectVirtualMachines(_)) => {
            tracing::info!(command_id = %command_id, "collecting virtual machines by control-plane request");
            let event = match &agent.vm_runtime {
                Some(runtime) => match runtime.provider.list().await {
                    Ok(states) => {
                        agent.virtual_machine_snapshot_event(agent_id, command_id, states)
                    }
                    Err(error) => agent.command_result_event(
                        agent_id,
                        command_id,
                        grpc::CommandStatus::Failed,
                        error.to_string(),
                    ),
                },
                None => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    "virtual machine provider is not enabled",
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue virtual machine snapshot event");
            }
        }
        Some(grpc::control_plane_command::Command::RunVirtualMachineCommand(vm_command)) => {
            tracing::info!(command_id = %command_id, "executing virtual machine command by control-plane request");
            let event = match &agent.vm_runtime {
                Some(runtime) => {
                    match serde_json::from_str::<VmCommandEnvelope>(&vm_command.command_json) {
                        Ok(envelope) => match execute_vm_command(runtime, envelope).await {
                            Ok(result) => agent
                                .virtual_machine_command_result_event(agent_id, command_id, result),
                            Err(error) => agent.command_result_event(
                                agent_id,
                                command_id,
                                grpc::CommandStatus::Failed,
                                error.to_string(),
                            ),
                        },
                        Err(error) => agent.command_result_event(
                            agent_id,
                            command_id,
                            grpc::CommandStatus::Failed,
                            format!("invalid virtual machine command payload: {error}"),
                        ),
                    }
                }
                None => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    "virtual machine provider is not enabled",
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue virtual machine command result event");
            }
        }
        Some(grpc::control_plane_command::Command::ListDirectory(list_command)) => {
            tracing::info!(command_id = %command_id, path = list_command.path, "listing directory by control-plane request");
            let event = match filesystem::list_directory(&list_command.path) {
                Ok(output) => agent.file_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue file list result event");
            }
        }
        Some(grpc::control_plane_command::Command::ReadFile(read_command)) => {
            tracing::info!(command_id = %command_id, path = read_command.path, "reading file by control-plane request");
            let event = match filesystem::read_file(&read_command.path, MAX_FILE_TRANSFER_BYTES) {
                Ok(output) => agent.file_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue file read result event");
            }
        }
        Some(grpc::control_plane_command::Command::SearchFiles(search_command)) => {
            tracing::info!(command_id = %command_id, path = search_command.path, query = search_command.query, "searching files by control-plane request");
            let event = match filesystem::search_files(
                &search_command.path,
                &search_command.query,
                search_command.limit,
            ) {
                Ok(output) => agent.file_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue file search result event");
            }
        }
        Some(grpc::control_plane_command::Command::RunFileOperation(file_command)) => {
            tracing::info!(command_id = %command_id, operation = file_command.operation, path = file_command.path, "running file operation by control-plane request");
            let event = match filesystem::run_operation(file_command, MAX_FILE_TRANSFER_BYTES) {
                Ok(output) => agent.file_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue file operation result event");
            }
        }
        Some(grpc::control_plane_command::Command::ApplyWebsiteRoutes(route_command)) => {
            tracing::info!(
                command_id = %command_id,
                route_count = route_command.routes.len(),
                "applying website routes by control-plane request"
            );
            let website_ids = route_command
                .routes
                .iter()
                .map(|route| route.website_id.clone())
                .collect::<Vec<_>>();
            let result = match &agent.website_runtime {
                Some(runtime) => apply_website_routes(runtime, route_command.routes),
                None => Err("website runtime is not enabled".to_string()),
            };
            let event =
                agent.website_routes_applied_event(agent_id, command_id, result, website_ids);
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue website routes applied event");
            }
        }
        Some(grpc::control_plane_command::Command::RunAgentTask(agent_task)) => {
            tracing::info!(
                command_id = %command_id,
                task_id = agent_task.task_id,
                scheduled_task_id = agent_task.scheduled_task_id,
                "running agent AI task"
            );
            let task_agent = agent.clone();
            let task_sender = sender.clone();
            let task_terminal = terminal.clone();
            let task_state = command_state.clone();
            tokio::spawn(async move {
                run_agent_task_command(
                    task_agent,
                    agent_id,
                    command_id,
                    agent_task,
                    task_sender,
                    task_terminal,
                    task_state,
                )
                .await;
            });
        }
        Some(grpc::control_plane_command::Command::RunAgentChatTurn(chat_turn)) => {
            tracing::info!(
                command_id = %command_id,
                conversation_id = chat_turn.conversation_id,
                task_id = chat_turn.task_id,
                "running agent AI chat turn"
            );
            let task_agent = agent.clone();
            let task_sender = sender.clone();
            let task_terminal = terminal.clone();
            let task_state = command_state.clone();
            tokio::spawn(async move {
                run_agent_chat_turn_command(
                    task_agent,
                    agent_id,
                    command_id,
                    chat_turn,
                    task_sender,
                    task_terminal,
                    task_state,
                )
                .await;
            });
        }
        Some(grpc::control_plane_command::Command::AgentToolApprovalDecision(decision)) => {
            tracing::info!(
                request_id = decision.request_id,
                task_id = decision.task_id,
                approved = decision.approved,
                "received agent tool approval decision"
            );
            command_state.resolve_tool_approval(decision).await;
        }
        Some(grpc::control_plane_command::Command::RunTerminalCommand(terminal_command)) => {
            tracing::info!(command_id = %command_id, "executing terminal command by control-plane request");
            let event = match terminal
                .execute(TerminalCommand {
                    command_id: command_id.clone(),
                    input: terminal_command.input,
                    cols: terminal_command.cols.clamp(20, 300) as u16,
                    rows: terminal_command.rows.clamp(5, 120) as u16,
                    timeout: Duration::from_secs(
                        terminal_command.timeout_seconds.clamp(1, 120) as u64
                    ),
                })
                .await
            {
                Ok(output) => agent.terminal_command_result_event(agent_id, command_id, output),
                Err(error) => agent.command_result_event(
                    agent_id,
                    command_id,
                    grpc::CommandStatus::Failed,
                    error.to_string(),
                ),
            };
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue terminal command result event");
            }
        }
        Some(grpc::control_plane_command::Command::OpenTerminalSession(open)) => {
            tracing::info!(
                session_id = open.session_id,
                command_id = %command_id,
                "opening interactive terminal session"
            );
            match terminal
                .open_interactive(
                    open.session_id.clone(),
                    open.cols.clamp(20, 300) as u16,
                    open.rows.clamp(5, 120) as u16,
                )
                .await
            {
                Ok(mut output_rx) => {
                    let output_agent = agent.clone();
                    let output_sender = sender.clone();
                    let session_id = open.session_id;
                    tokio::spawn(async move {
                        while let Some(output) = output_rx.recv().await {
                            let event = output_agent.terminal_output_event(
                                agent_id,
                                session_id.clone(),
                                output,
                            );
                            if output_sender.send(event).await.is_err() {
                                return;
                            }
                        }
                        let event = output_agent.terminal_session_closed_event(
                            agent_id,
                            session_id,
                            "pty output ended",
                        );
                        let _ = output_sender.send(event).await;
                    });
                }
                Err(error) => {
                    let event = agent.terminal_session_closed_event(
                        agent_id,
                        open.session_id,
                        error.to_string(),
                    );
                    if sender.send(event).await.is_err() {
                        tracing::warn!("failed to enqueue terminal session error event");
                    }
                }
            }
        }
        Some(grpc::control_plane_command::Command::TerminalInput(input)) => {
            if let Err(error) = terminal
                .write_interactive(input.session_id.clone(), input.data)
                .await
            {
                let event = agent.terminal_session_closed_event(
                    agent_id,
                    input.session_id,
                    error.to_string(),
                );
                if sender.send(event).await.is_err() {
                    tracing::warn!("failed to enqueue terminal input error event");
                }
            }
        }
        Some(grpc::control_plane_command::Command::ResizeTerminalSession(resize)) => {
            if let Err(error) = terminal
                .resize_interactive(
                    resize.session_id.clone(),
                    resize.cols.clamp(20, 300) as u16,
                    resize.rows.clamp(5, 120) as u16,
                )
                .await
            {
                tracing::warn!(%error, session_id = resize.session_id, "failed to resize terminal session");
            }
        }
        Some(grpc::control_plane_command::Command::CloseTerminalSession(close)) => {
            let _ = terminal.close_interactive(close.session_id.clone()).await;
            let event =
                agent.terminal_session_closed_event(agent_id, close.session_id, close.reason);
            if sender.send(event).await.is_err() {
                tracing::warn!("failed to enqueue terminal close event");
            }
        }
        Some(grpc::control_plane_command::Command::Shutdown(shutdown)) => {
            tracing::info!(
                command_id = %command_id,
                reason = shutdown.reason,
                "control-plane requested agent stream reconnect"
            );
            return AgentCommandAction::Reconnect;
        }
        None => {
            tracing::warn!(command_id = %command_id, "control-plane command missing typed payload")
        }
    }
    AgentCommandAction::Continue
}

async fn run_agent_task_command(
    agent: Agent,
    agent_id: Uuid,
    command_id: String,
    agent_task: grpc::RunAgentTaskCommand,
    sender: mpsc::Sender<grpc::AgentEvent>,
    terminal: TerminalManager,
    command_state: AgentCommandState,
) {
    let task_id = agent_task.task_id.clone();
    let started = agent.agent_task_progress_event(
        agent_id,
        grpc::AgentTaskProgressEvent {
            command_id: command_id.clone(),
            task_id: task_id.clone(),
            step_id: String::new(),
            status: "running".to_string(),
            message: "agent AI task started".to_string(),
            details_json: json!({
                "scheduled_task_id": agent_task.scheduled_task_id.clone(),
            })
            .to_string(),
        },
    );
    if sender.send(started).await.is_err() {
        tracing::warn!("failed to enqueue agent task start event");
    }

    let outcome = match agent.ai_runner_for_command(agent_task.ai_provider.as_ref()) {
        Ok(runner) => {
            let executor = LocalAgentToolExecutor {
                agent: agent.clone(),
                agent_id,
                command_id: command_id.clone(),
                task_id: task_id.clone(),
                sender: sender.clone(),
                terminal,
                command_state,
                tool_timeout: Duration::from_secs(
                    agent.config.ai.agent.tool_timeout_seconds.max(1),
                ),
                shell_timeout: Duration::from_secs(
                    agent.config.ai.agent.shell_timeout_seconds.max(1),
                ),
                approval_timeout: Duration::from_secs(
                    agent.config.ai.agent.approval_timeout_seconds.max(1),
                ),
            };
            runner
                .run(
                    AgentRunRequest {
                        prompt: agent_task.prompt,
                        context: json!({
                            "agent_id": agent_id,
                            "host_id": agent.config.host_id,
                            "hostname": agent.config.hostname.clone(),
                            "scheduled_task_id": agent_task.scheduled_task_id.clone(),
                            "template": parse_json_value(&agent_task.template_json),
                        }),
                    },
                    &executor,
                )
                .await
        }
        Err(error) => Err(error),
    };

    let (status, message, result_outcome) = match outcome {
        Ok(outcome) => {
            let status = match outcome.status {
                AgentRunStatus::Succeeded => grpc::CommandStatus::Succeeded,
                AgentRunStatus::Failed => grpc::CommandStatus::Failed,
            };
            (status, outcome.summary.clone(), outcome)
        }
        Err(error) => {
            let message = error.to_string();
            (
                grpc::CommandStatus::Failed,
                message.clone(),
                AgentRunOutcome {
                    status: AgentRunStatus::Failed,
                    summary: message,
                    transcript: Vec::new(),
                },
            )
        }
    };

    let result_event =
        agent.agent_task_result_event(agent_id, command_id.clone(), task_id, &result_outcome);
    if sender.send(result_event).await.is_err() {
        tracing::warn!("failed to enqueue agent task result event");
    }

    let command_result = agent.command_result_event(agent_id, command_id, status, message);
    if sender.send(command_result).await.is_err() {
        tracing::warn!("failed to enqueue agent task command result event");
    }
}

async fn run_agent_chat_turn_command(
    agent: Agent,
    agent_id: Uuid,
    command_id: String,
    chat_turn: grpc::RunAgentChatTurnCommand,
    sender: mpsc::Sender<grpc::AgentEvent>,
    terminal: TerminalManager,
    command_state: AgentCommandState,
) {
    let task_id = chat_turn.task_id.clone();
    let sink = GrpcChatEventSink {
        agent: agent.clone(),
        agent_id,
        command_id: command_id.clone(),
        conversation_id: chat_turn.conversation_id.clone(),
        message_id: chat_turn.assistant_message_id.clone(),
        task_id: task_id.clone(),
        sender: sender.clone(),
    };

    let outcome = match agent.ai_runner_for_command(chat_turn.ai_provider.as_ref()) {
        Ok(runner) => {
            let executor = LocalAgentToolExecutor {
                agent: agent.clone(),
                agent_id,
                command_id: command_id.clone(),
                task_id: task_id.clone(),
                sender: sender.clone(),
                terminal,
                command_state,
                tool_timeout: Duration::from_secs(
                    agent.config.ai.agent.tool_timeout_seconds.max(1),
                ),
                shell_timeout: Duration::from_secs(
                    agent.config.ai.agent.shell_timeout_seconds.max(1),
                ),
                approval_timeout: Duration::from_secs(
                    agent.config.ai.agent.approval_timeout_seconds.max(1),
                ),
            };
            runner
                .run_streaming(
                    AgentRunRequest {
                        prompt: chat_prompt_from_messages(&chat_turn.messages_json),
                        context: json!({
                            "agent_id": agent_id,
                            "host_id": agent.config.host_id,
                            "hostname": agent.config.hostname.clone(),
                            "conversation_id": chat_turn.conversation_id.clone(),
                            "user_message_id": chat_turn.user_message_id.clone(),
                            "assistant_message_id": chat_turn.assistant_message_id.clone(),
                        }),
                    },
                    &executor,
                    &sink,
                )
                .await
        }
        Err(error) => Err(error),
    };

    let (status, message, result_outcome) = match outcome {
        Ok(outcome) => {
            let status = match outcome.status {
                AgentRunStatus::Succeeded => grpc::CommandStatus::Succeeded,
                AgentRunStatus::Failed => grpc::CommandStatus::Failed,
            };
            (status, outcome.summary.clone(), outcome)
        }
        Err(error) => {
            let message = error.to_string();
            (
                grpc::CommandStatus::Failed,
                message.clone(),
                AgentRunOutcome {
                    status: AgentRunStatus::Failed,
                    summary: message,
                    transcript: Vec::new(),
                },
            )
        }
    };

    let result_event = agent.agent_chat_turn_result_event(
        agent_id,
        grpc::AgentChatTurnResultEvent {
            command_id: command_id.clone(),
            conversation_id: chat_turn.conversation_id,
            message_id: chat_turn.assistant_message_id,
            task_id,
            status: status as i32,
            message,
            result_json: json!({
                "transcript": result_outcome.transcript,
            })
            .to_string(),
        },
    );
    if sender.send(result_event).await.is_err() {
        tracing::warn!("failed to enqueue agent chat turn result event");
    }
}

#[derive(Clone)]
struct GrpcChatEventSink {
    agent: Agent,
    agent_id: Uuid,
    command_id: String,
    conversation_id: String,
    message_id: String,
    task_id: String,
    sender: mpsc::Sender<grpc::AgentEvent>,
}

#[async_trait]
impl AgentRunEventSink for GrpcChatEventSink {
    async fn emit(&self, event: AgentRunEvent) -> Result<(), AgentError> {
        let event = match event {
            AgentRunEvent::TextDelta { delta } => self.agent.agent_chat_text_delta_event(
                self.agent_id,
                grpc::AgentChatTextDeltaEvent {
                    command_id: self.command_id.clone(),
                    conversation_id: self.conversation_id.clone(),
                    message_id: self.message_id.clone(),
                    task_id: self.task_id.clone(),
                    delta,
                },
            ),
            AgentRunEvent::ToolCall {
                call_id,
                name,
                arguments,
            } => self.agent.agent_chat_tool_event(
                self.agent_id,
                grpc::AgentChatToolEvent {
                    command_id: self.command_id.clone(),
                    conversation_id: self.conversation_id.clone(),
                    message_id: self.message_id.clone(),
                    task_id: self.task_id.clone(),
                    kind: "tool_call".to_string(),
                    tool_call_id: call_id,
                    tool_name: name.clone(),
                    status: "running".to_string(),
                    content: format!("调用工具 {name}"),
                    payload_json: json!({ "arguments": arguments }).to_string(),
                },
            ),
            AgentRunEvent::ToolResult {
                call_id,
                name,
                status,
                output,
            } => self.agent.agent_chat_tool_event(
                self.agent_id,
                grpc::AgentChatToolEvent {
                    command_id: self.command_id.clone(),
                    conversation_id: self.conversation_id.clone(),
                    message_id: self.message_id.clone(),
                    task_id: self.task_id.clone(),
                    kind: "tool_result".to_string(),
                    tool_call_id: call_id,
                    tool_name: name.clone(),
                    status: format!("{status:?}").to_lowercase(),
                    content: format!("工具 {name} 已完成"),
                    payload_json: json!({ "output": output }).to_string(),
                },
            ),
        };

        self.sender.send(event).await.map_err(|_| AgentError::Tool {
            name: "chat_stream".to_string(),
            message: "failed to send chat stream event".to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ChatInputMessage {
    role: String,
    content: String,
}

fn chat_prompt_from_messages(messages_json: &str) -> String {
    let messages =
        serde_json::from_str::<Vec<ChatInputMessage>>(messages_json).unwrap_or_else(|_| Vec::new());
    if messages.is_empty() {
        return "Respond to the operator's latest request.".to_string();
    }
    let transcript = messages
        .into_iter()
        .map(|message| {
            let role = match message.role.as_str() {
                "assistant" => "Assistant",
                "tool" => "Tool",
                _ => "User",
            };
            format!("{role}: {}", message.content.trim())
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "Continue this Doro AI chat. Use tools when needed and wait for Doro approvals for risky actions.\n\n{transcript}"
    )
}

async fn execute_vm_command(
    runtime: &VmRuntime,
    envelope: VmCommandEnvelope,
) -> Result<doro_vm::VmCommandResult, VmProviderError> {
    match envelope.command {
        VmCommand::Create { spec } => {
            let state = runtime.provider.create(*spec).await?;
            Ok(doro_vm::VmCommandResult {
                command_id: envelope.command_id,
                vm_id: Some(state.id.clone()),
                status: VmCommandStatus::Succeeded,
                message: "virtual machine created".to_string(),
                details: serde_json::to_value(state)?,
            })
        }
        VmCommand::Start { id } => runtime.provider.start(&id).await,
        VmCommand::Stop { id, mode } => runtime.provider.stop(&id, mode).await,
        VmCommand::Restart { id } => runtime.provider.restart(&id).await,
        VmCommand::Delete { id, mode } => runtime.provider.delete(&id, mode).await,
        VmCommand::Snapshot { id, request } => {
            let snapshot = runtime.provider.snapshot(&id, request).await?;
            Ok(doro_vm::VmCommandResult {
                command_id: envelope.command_id,
                vm_id: Some(id),
                status: VmCommandStatus::Succeeded,
                message: "virtual machine snapshot created".to_string(),
                details: serde_json::to_value(snapshot)?,
            })
        }
        VmCommand::Console { id } => {
            let console = runtime.provider.console(&id).await?;
            Ok(doro_vm::VmCommandResult {
                command_id: envelope.command_id,
                vm_id: Some(id),
                status: VmCommandStatus::Succeeded,
                message: "virtual machine console resolved".to_string(),
                details: serde_json::to_value(console)?,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_agent_config;

    #[tokio::test]
    async fn handle_command_continues_for_ack() {
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(test_agent_config(agent_id));
        let (sender, _receiver) = mpsc::channel(1);
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: None,
            command: Some(grpc::control_plane_command::Command::Ack(
                grpc::AckCommand {
                    message: "connected".to_string(),
                },
            )),
        };

        let terminal = match TerminalManager::new() {
            Ok(terminal) => terminal,
            Err(error) => panic!("terminal should start: {error}"),
        };
        let command_state = AgentCommandState::default();
        let action = handle_command(
            command,
            &agent,
            agent_id,
            &sender,
            &terminal,
            &command_state,
        )
        .await;

        assert_eq!(action, AgentCommandAction::Continue);
    }

    #[tokio::test]
    async fn handle_command_returns_reconnect_for_shutdown_command() {
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(test_agent_config(agent_id));
        let (sender, _receiver) = mpsc::channel(1);
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: None,
            command: Some(grpc::control_plane_command::Command::Shutdown(
                grpc::ShutdownCommand {
                    reason: "control-plane shutting down".to_string(),
                },
            )),
        };

        let terminal = match TerminalManager::new() {
            Ok(terminal) => terminal,
            Err(error) => panic!("terminal should start: {error}"),
        };
        let command_state = AgentCommandState::default();
        let action = handle_command(
            command,
            &agent,
            agent_id,
            &sender,
            &terminal,
            &command_state,
        )
        .await;

        assert_eq!(action, AgentCommandAction::Reconnect);
    }
}
