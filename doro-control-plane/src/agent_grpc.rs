use crate::agent_events::*;
use crate::agent_streams::{AgentCommandReply, AgentStreamRegistry};
use crate::agent_tools::{create_agent_tool_approval, normalize_task_step_status};
use crate::chat_streams::ChatStreamHub;
use crate::error::{enrollment_status, store_status};
use crate::logs::LogHub;
use crate::prelude::*;
use crate::routes::websites::sync_running_websites_for_connected_host;
use crate::server::{shutdown_requested, wait_for_shutdown};

pub struct GrpcAgentService {
    pub(crate) store: Store,
    pub(crate) agent_streams: AgentStreamRegistry,
    pub(crate) chat_streams: ChatStreamHub,
    pub(crate) logs: LogHub,
    pub(crate) shutdown_rx: watch::Receiver<bool>,
}

#[tonic::async_trait]
impl AgentControlPlane for GrpcAgentService {
    type OpenAgentStreamStream = ReceiverStream<Result<grpc::ControlPlaneCommand, Status>>;

    async fn enroll(
        &self,
        request: Request<grpc::EnrollRequest>,
    ) -> Result<Response<grpc::EnrollResponse>, Status> {
        let request = request.into_inner();
        if request.enrollment_token.trim().is_empty() {
            return Err(Status::invalid_argument("enrollment token is required"));
        }

        let agent_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let observed_at = Utc::now();
        let hostname = if request.hostname.trim().is_empty() {
            format!("doro-agent-{host_id}")
        } else {
            request.hostname
        };
        let capabilities = request
            .capabilities
            .into_iter()
            .filter_map(grpc_capability_to_protocol)
            .collect();
        let system_profile = parse_event_payload(&request.system_profile_json);

        self.store
            .agents()
            .register(AgentRegistration {
                agent_id,
                host_id,
                enrollment_token: request.enrollment_token,
                hostname,
                system_profile,
                capabilities,
                observed_at,
            })
            .await
            .map_err(enrollment_status)?;
        Ok(Response::new(grpc::EnrollResponse {
            agent_id: agent_id.to_string(),
            host_id: host_id.to_string(),
            control_plane_id: "doro-control-plane-local".to_string(),
        }))
    }

    async fn report_heartbeat(
        &self,
        request: Request<grpc::HeartbeatRequest>,
    ) -> Result<Response<grpc::HeartbeatResponse>, Status> {
        let request = request.into_inner();
        if request.agent_id.trim().is_empty() || request.host_id.trim().is_empty() {
            return Err(Status::invalid_argument(
                "agent_id and host_id are required",
            ));
        }

        let agent_id = doro_store::parse_uuid(&request.agent_id)
            .map_err(|_| Status::invalid_argument("agent_id must be a uuid"))?;
        let host_id = doro_store::parse_uuid(&request.host_id)
            .map_err(|_| Status::invalid_argument("host_id must be a uuid"))?;
        let observed_at = request
            .observed_at
            .as_ref()
            .and_then(timestamp_to_utc)
            .unwrap_or_else(Utc::now);
        let capabilities = request
            .capabilities
            .into_iter()
            .filter_map(grpc_capability_to_protocol)
            .collect();

        self.store
            .agents()
            .heartbeat(AgentHeartbeat {
                agent_id,
                host_id,
                capabilities,
                observed_at,
            })
            .await
            .map_err(store_status)?;
        self.store
            .events()
            .record(NewAgentEvent {
                agent_id: Some(agent_id),
                host_id: Some(host_id),
                event_type: "heartbeat".to_string(),
                event_json: serde_json::json!({
                    "agent_id": agent_id,
                    "host_id": host_id,
                    "observed_at": observed_at
                }),
                recorded_at: observed_at,
            })
            .await
            .map_err(store_status)?;

        Ok(Response::new(grpc::HeartbeatResponse {
            accepted: true,
            message: "heartbeat accepted".to_string(),
        }))
    }

    async fn open_agent_stream(
        &self,
        request: Request<Streaming<grpc::AgentEvent>>,
    ) -> Result<Response<Self::OpenAgentStreamStream>, Status> {
        let store = self.store.clone();
        let agent_streams = self.agent_streams.clone();
        let chat_streams = self.chat_streams.clone();
        let logs = self.logs.clone();
        let shutdown_rx = self.shutdown_rx.clone();
        let mut inbound = request.into_inner();
        tracing::debug!("agent opened grpc stream");
        let (sender, receiver) = mpsc::channel(8);
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::Ack(
                grpc::AckCommand {
                    message: "grpc agent stream connected".to_string(),
                },
            )),
        };
        if sender.send(Ok(command)).await.is_err() {
            tracing::warn!("failed to enqueue initial grpc command");
        }

        tokio::spawn(async move {
            let command_sender = sender;
            let mut pending_commands: Option<
                Arc<Mutex<HashMap<String, oneshot::Sender<AgentCommandReply>>>>,
            > = None;
            let mut connected_agent: Option<(Uuid, Uuid)> = None;
            loop {
                let event = tokio::select! {
                    event = inbound.message() => {
                        match event {
                            Ok(Some(event)) => event,
                            Ok(None) => break,
                            Err(error) => {
                                if shutdown_requested(&shutdown_rx) {
                                    tracing::debug!(%error, "agent stream receive stopped during shutdown");
                                } else {
                                    tracing::warn!(%error, "agent stream receive failed");
                                }
                                break;
                            }
                        }
                    }
                    () = wait_for_shutdown(shutdown_rx.clone()) => break,
                };
                let recorded_at = event
                    .recorded_at
                    .as_ref()
                    .and_then(timestamp_to_utc)
                    .unwrap_or_else(Utc::now);
                let agent_id = parse_optional_uuid(&event.agent_id);
                let host_id = parse_optional_uuid(&event.host_id);
                let Some((event_type, payload)) = typed_agent_event_payload(&event) else {
                    tracing::warn!("agent stream event missing typed payload");
                    continue;
                };
                match event.event.clone() {
                    Some(grpc::agent_event::Event::Connected(_))
                    | Some(grpc::agent_event::Event::Heartbeat(_)) => {
                        if let (Some(agent_id), Some(host_id)) = (agent_id, host_id) {
                            if let Some((old_agent_id, old_host_id)) = connected_agent
                                && old_host_id != host_id
                            {
                                agent_streams.unregister(old_host_id, old_agent_id).await;
                            }
                            connected_agent = Some((agent_id, host_id));
                            pending_commands = Some(
                                agent_streams
                                    .register(host_id, agent_id, command_sender.clone())
                                    .await,
                            );
                            tracing::debug!(
                                agent_id = %agent_id,
                                host_id = %host_id,
                                event_type,
                                "agent stream registered"
                            );
                            if let Err(error) = store
                                .agents()
                                .mark_online(agent_id, host_id, recorded_at)
                                .await
                            {
                                tracing::warn!(%error, "failed to refresh streamed agent heartbeat");
                            }
                            if event_type == "connected" {
                                let sync_store = store.clone();
                                let sync_streams = agent_streams.clone();
                                tokio::spawn(async move {
                                    sync_running_websites_for_connected_host(
                                        sync_store,
                                        sync_streams,
                                        host_id,
                                    )
                                    .await;
                                });
                            }
                        }
                    }
                    Some(grpc::agent_event::Event::ContainerSnapshot(snapshot)) => {
                        if let Some(pending_commands) = &pending_commands
                            && !snapshot.command_id.is_empty()
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&snapshot.command_id)
                        {
                            let _ = reply_sender
                                .send(AgentCommandReply::ContainerSnapshot(snapshot.clone()));
                        }
                    }
                    Some(grpc::agent_event::Event::VirtualMachineSnapshot(snapshot)) => {
                        if let Some(pending_commands) = &pending_commands
                            && !snapshot.command_id.is_empty()
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&snapshot.command_id)
                        {
                            let _ = reply_sender
                                .send(AgentCommandReply::VirtualMachineSnapshot(snapshot.clone()));
                        }
                    }
                    Some(grpc::agent_event::Event::VirtualMachineCommandResult(result)) => {
                        if let Some(pending_commands) = &pending_commands
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&result.command_id)
                        {
                            let _ = reply_sender.send(
                                AgentCommandReply::VirtualMachineCommandResult(result.clone()),
                            );
                        }
                    }
                    Some(grpc::agent_event::Event::DockerCommandResult(result)) => {
                        if let Some(pending_commands) = &pending_commands
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&result.command_id)
                        {
                            let _ = reply_sender
                                .send(AgentCommandReply::DockerCommandResult(result.clone()));
                        }
                    }
                    Some(grpc::agent_event::Event::CommandResult(result)) => {
                        if let Some(pending_commands) = &pending_commands
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&result.command_id)
                        {
                            if result.status == grpc::CommandStatus::Failed as i32 {
                                let _ =
                                    reply_sender.send(AgentCommandReply::Failed(result.message));
                            } else {
                                let _ = reply_sender
                                    .send(AgentCommandReply::CommandResult(result.clone()));
                            }
                        }
                    }
                    Some(grpc::agent_event::Event::TerminalCommandResult(result)) => {
                        if let Some(pending_commands) = &pending_commands
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&result.command_id)
                        {
                            let _ = reply_sender
                                .send(AgentCommandReply::TerminalCommandResult(result.clone()));
                        }
                    }
                    Some(grpc::agent_event::Event::FileCommandResult(result)) => {
                        if let Some(pending_commands) = &pending_commands
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&result.command_id)
                        {
                            let _ = reply_sender
                                .send(AgentCommandReply::FileCommandResult(result.clone()));
                        }
                    }
                    Some(grpc::agent_event::Event::WebsiteRoutesApplied(result)) => {
                        if let Some(pending_commands) = &pending_commands
                            && let Some(reply_sender) =
                                pending_commands.lock().await.remove(&result.command_id)
                        {
                            let _ = reply_sender
                                .send(AgentCommandReply::WebsiteRoutesApplied(result.clone()));
                        }
                    }
                    Some(grpc::agent_event::Event::AgentTaskProgress(progress)) => {
                        if let Some(step_id) = parse_optional_uuid(&progress.step_id)
                            && let Some(status) = normalize_task_step_status(&progress.status)
                            && let Err(error) =
                                store.tasks().update_step_status(step_id, status).await
                        {
                            tracing::warn!(
                                %error,
                                step_id = %step_id,
                                "failed to update agent task step status"
                            );
                        }
                    }
                    Some(grpc::agent_event::Event::AgentToolApprovalRequest(request)) => {
                        if let Some(host_id) = host_id {
                            match create_agent_tool_approval(
                                &store,
                                host_id,
                                request.clone(),
                                recorded_at,
                            )
                            .await
                            {
                                Ok(approval) => {
                                    if let Some(task_id) = parse_optional_uuid(&request.task_id)
                                        && let Err(error) = publish_chat_approval_required(
                                            &store,
                                            &chat_streams,
                                            task_id,
                                            &request,
                                            &approval,
                                            recorded_at,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            %error,
                                            "failed to publish chat approval request"
                                        );
                                    }
                                }
                                Err(error) => {
                                    tracing::warn!(
                                        %error,
                                        "failed to create agent tool approval request"
                                    );
                                }
                            }
                        }
                    }
                    Some(grpc::agent_event::Event::AgentChatTextDelta(delta)) => {
                        if let Err(error) =
                            ingest_chat_text_delta(&store, &chat_streams, delta, recorded_at).await
                        {
                            tracing::warn!(%error, "failed to ingest chat text delta");
                        }
                    }
                    Some(grpc::agent_event::Event::AgentChatTool(tool)) => {
                        if let Err(error) =
                            ingest_chat_tool_event(&store, &chat_streams, tool, recorded_at).await
                        {
                            tracing::warn!(%error, "failed to ingest chat tool event");
                        }
                    }
                    Some(grpc::agent_event::Event::AgentChatTurnResult(result)) => {
                        if let Err(error) =
                            ingest_chat_turn_result(&store, &chat_streams, result, recorded_at)
                                .await
                        {
                            tracing::warn!(%error, "failed to ingest chat turn result");
                        }
                    }
                    Some(grpc::agent_event::Event::TerminalOutput(output)) => {
                        agent_streams
                            .publish_terminal_output(&output.session_id, output.data)
                            .await;
                    }
                    Some(grpc::agent_event::Event::TerminalSessionClosed(closed)) => {
                        agent_streams
                            .close_terminal_output(&closed.session_id, closed.reason)
                            .await;
                    }
                    Some(grpc::agent_event::Event::LogLine(line)) => {
                        if let (Some(agent_id), Some(host_id)) = (agent_id, host_id) {
                            logs.push(runtime_log_from_agent_line(
                                line,
                                agent_id,
                                host_id,
                                recorded_at,
                            ));
                        }
                    }
                    _ => {}
                };

                if event_type != "log.line" && event_type != "agent_chat.text_delta" {
                    if let Err(error) = store
                        .events()
                        .record(NewAgentEvent {
                            agent_id,
                            host_id,
                            event_type: event_type.clone(),
                            event_json: serde_json::json!({
                                "event_id": event.event_id,
                                "kind": event_type,
                                "payload": payload
                            }),
                            recorded_at,
                        })
                        .await
                    {
                        tracing::warn!(%error, "failed to persist agent stream event");
                    }

                    if let Err(error) =
                        ingest_agent_event(&store, host_id, &event_type, &payload, recorded_at)
                            .await
                    {
                        tracing::warn!(%error, event_type, "failed to ingest agent stream event");
                    }
                }
            }

            if let Some((agent_id, host_id)) = connected_agent {
                agent_streams.unregister(host_id, agent_id).await;
                let recorded_at = Utc::now();
                if let Err(error) = store
                    .agents()
                    .mark_offline(agent_id, host_id, recorded_at)
                    .await
                {
                    tracing::warn!(%error, "failed to mark disconnected agent offline");
                }
                if let Err(error) = store
                    .events()
                    .record(NewAgentEvent {
                        agent_id: Some(agent_id),
                        host_id: Some(host_id),
                        event_type: "agent_disconnected".to_string(),
                        event_json: serde_json::json!({
                            "agent_id": agent_id,
                            "host_id": host_id
                        }),
                        recorded_at,
                    })
                    .await
                {
                    tracing::warn!(%error, "failed to persist agent disconnect event");
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(receiver)))
    }
}

async fn ingest_chat_text_delta(
    store: &Store,
    chat_streams: &ChatStreamHub,
    delta: grpc::AgentChatTextDeltaEvent,
    recorded_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    let conversation_id = Uuid::parse_str(&delta.conversation_id)?;
    let message_id = Uuid::parse_str(&delta.message_id)?;
    store
        .ai_chats()
        .append_message_content(message_id, &delta.delta, recorded_at)
        .await?;
    chat_streams
        .publish(AiChatStreamEvent {
            event_id: Uuid::new_v4(),
            conversation_id,
            message_id,
            kind: AiChatEventKind::TextDelta,
            content: Some(delta.delta),
            payload: serde_json::json!({}),
            created_at: recorded_at,
        })
        .await;
    Ok(())
}

async fn ingest_chat_tool_event(
    store: &Store,
    chat_streams: &ChatStreamHub,
    tool: grpc::AgentChatToolEvent,
    recorded_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    let conversation_id = Uuid::parse_str(&tool.conversation_id)?;
    let message_id = Uuid::parse_str(&tool.message_id)?;
    let kind = match tool.kind.as_str() {
        "tool_result" => AiChatEventKind::ToolResult,
        "approval_required" => AiChatEventKind::ApprovalRequired,
        "error" => AiChatEventKind::Error,
        _ => AiChatEventKind::ToolCall,
    };
    let event = store
        .ai_chats()
        .record_event(NewAiChatEvent {
            id: Uuid::new_v4(),
            conversation_id,
            message_id,
            kind,
            content: normalize_chat_content(tool.content),
            payload: serde_json::json!({
                "command_id": tool.command_id,
                "task_id": tool.task_id,
                "tool_call_id": tool.tool_call_id,
                "tool_name": tool.tool_name,
                "status": tool.status,
                "payload": parse_event_payload(&tool.payload_json),
            }),
            created_at: recorded_at,
        })
        .await?;
    chat_streams
        .publish(stream_event_from_chat_event(event))
        .await;
    Ok(())
}

async fn publish_chat_approval_required(
    store: &Store,
    chat_streams: &ChatStreamHub,
    task_id: Uuid,
    request: &grpc::AgentToolApprovalRequestEvent,
    approval: &ApprovalRequest,
    recorded_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    let Some(message) = store.ai_chats().message_for_task(task_id).await? else {
        return Ok(());
    };
    let _ = store
        .ai_chats()
        .update_message(
            message.id,
            AiChatMessageChanges {
                status: Some(AiChatMessageStatus::WaitingApproval),
                updated_at: Some(recorded_at),
                ..AiChatMessageChanges::default()
            },
        )
        .await?;
    let event = store
        .ai_chats()
        .record_event(NewAiChatEvent {
            id: Uuid::new_v4(),
            conversation_id: message.conversation_id,
            message_id: message.id,
            kind: AiChatEventKind::ApprovalRequired,
            content: Some(request.summary.clone()),
            payload: serde_json::json!({
                "approval": approval,
                "request_id": request.request_id,
                "command_id": request.command_id,
                "tool_call_id": request.tool_call_id,
                "tool_name": request.tool_name,
                "risk": request.risk,
                "arguments": parse_event_payload(&request.arguments_json),
            }),
            created_at: recorded_at,
        })
        .await?;
    chat_streams
        .publish(stream_event_from_chat_event(event))
        .await;
    Ok(())
}

async fn ingest_chat_turn_result(
    store: &Store,
    chat_streams: &ChatStreamHub,
    result: grpc::AgentChatTurnResultEvent,
    recorded_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    let conversation_id = Uuid::parse_str(&result.conversation_id)?;
    let message_id = Uuid::parse_str(&result.message_id)?;
    let task_id = Uuid::parse_str(&result.task_id)?;
    let succeeded = result.status == grpc::CommandStatus::Succeeded as i32;
    let message_status = if succeeded {
        AiChatMessageStatus::Succeeded
    } else {
        AiChatMessageStatus::Failed
    };
    let task_status = if succeeded {
        TaskStatus::Succeeded
    } else {
        TaskStatus::Failed
    };
    let event_kind = if succeeded {
        AiChatEventKind::Done
    } else {
        AiChatEventKind::Error
    };
    let error_message = (!succeeded).then_some(result.message.clone());
    store
        .ai_chats()
        .update_message(
            message_id,
            AiChatMessageChanges {
                status: Some(message_status),
                updated_at: Some(recorded_at),
                metadata: Some(parse_event_payload(&result.result_json)),
                ..AiChatMessageChanges::default()
            },
        )
        .await?;
    store
        .tasks()
        .update_status(
            task_id,
            task_status,
            Some(recorded_at),
            error_message.clone(),
        )
        .await?;
    store
        .tasks()
        .update_first_step_status_for_task(task_id, if succeeded { "succeeded" } else { "failed" })
        .await?;
    store
        .tasks()
        .finish_latest_run_for_task(
            task_id,
            if succeeded { "succeeded" } else { "failed" }.to_string(),
            Some(result.command_id.clone()),
            recorded_at,
            parse_event_payload(&result.result_json),
            error_message,
        )
        .await?;
    let event = store
        .ai_chats()
        .record_event(NewAiChatEvent {
            id: Uuid::new_v4(),
            conversation_id,
            message_id,
            kind: event_kind,
            content: Some(result.message),
            payload: parse_event_payload(&result.result_json),
            created_at: recorded_at,
        })
        .await?;
    chat_streams
        .publish(stream_event_from_chat_event(event))
        .await;
    Ok(())
}

fn normalize_chat_content(content: String) -> Option<String> {
    let content = content.trim().to_string();
    (!content.is_empty()).then_some(content)
}

fn stream_event_from_chat_event(event: AiChatEvent) -> AiChatStreamEvent {
    AiChatStreamEvent {
        event_id: event.id,
        conversation_id: event.conversation_id,
        message_id: event.message_id,
        kind: event.kind,
        content: event.content,
        payload: event.payload,
        created_at: event.created_at,
    }
}
