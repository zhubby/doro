use crate::error::AppError;
use crate::prelude::*;

pub(crate) fn with_file_command_id(
    command: grpc::control_plane_command::Command,
    command_id: &str,
) -> grpc::control_plane_command::Command {
    match command {
        grpc::control_plane_command::Command::ListDirectory(mut command) => {
            command.command_id = command_id.to_string();
            grpc::control_plane_command::Command::ListDirectory(command)
        }
        grpc::control_plane_command::Command::ReadFile(mut command) => {
            command.command_id = command_id.to_string();
            grpc::control_plane_command::Command::ReadFile(command)
        }
        grpc::control_plane_command::Command::SearchFiles(mut command) => {
            command.command_id = command_id.to_string();
            grpc::control_plane_command::Command::SearchFiles(command)
        }
        grpc::control_plane_command::Command::RunFileOperation(mut command) => {
            command.command_id = command_id.to_string();
            grpc::control_plane_command::Command::RunFileOperation(command)
        }
        other => other,
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentStreamRegistry {
    streams: Arc<Mutex<HashMap<Uuid, AgentStreamHandle>>>,
    terminal_sessions: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<String>>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentStreamHandle {
    agent_id: Uuid,
    sender: mpsc::Sender<Result<grpc::ControlPlaneCommand, Status>>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<AgentCommandReply>>>>,
}

#[derive(Debug)]
pub(crate) enum AgentCommandReply {
    ContainerSnapshot(grpc::ContainerSnapshotEvent),
    VirtualMachineSnapshot(grpc::VirtualMachineSnapshotEvent),
    VirtualMachineCommandResult,
    TerminalCommandResult(grpc::TerminalCommandResultEvent),
    FileCommandResult(grpc::FileCommandResultEvent),
    WebsiteRoutesApplied(grpc::WebsiteRoutesAppliedEvent),
    CommandResult(grpc::CommandResultEvent),
    Failed(String),
}

impl AgentStreamRegistry {
    pub(crate) async fn register(
        &self,
        host_id: Uuid,
        agent_id: Uuid,
        sender: mpsc::Sender<Result<grpc::ControlPlaneCommand, Status>>,
    ) -> Arc<Mutex<HashMap<String, oneshot::Sender<AgentCommandReply>>>> {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        self.streams.lock().await.insert(
            host_id,
            AgentStreamHandle {
                agent_id,
                sender,
                pending: pending.clone(),
            },
        );
        pending
    }

    pub(crate) async fn unregister(&self, host_id: Uuid, agent_id: Uuid) {
        let mut streams = self.streams.lock().await;
        if streams
            .get(&host_id)
            .is_some_and(|handle| handle.agent_id == agent_id)
        {
            streams.remove(&host_id);
        }
    }

    pub(crate) async fn shutdown_all(&self, reason: impl Into<String>) {
        let reason = reason.into();
        let handles = self
            .streams
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for handle in handles {
            let command = grpc::ControlPlaneCommand {
                command_id: Uuid::new_v4().to_string(),
                issued_at: Some(protobuf_timestamp_now()),
                command: Some(grpc::control_plane_command::Command::Shutdown(
                    grpc::ShutdownCommand {
                        reason: reason.clone(),
                    },
                )),
            };
            if handle.sender.send(Ok(command)).await.is_err() {
                tracing::debug!(
                    agent_id = %handle.agent_id,
                    "failed to enqueue agent stream shutdown command"
                );
            }
        }
    }

    pub(crate) async fn agent_id_for_host(&self, host_id: Uuid) -> Option<Uuid> {
        self.streams
            .lock()
            .await
            .get(&host_id)
            .map(|handle| handle.agent_id)
    }

    pub(crate) async fn collect_containers(
        &self,
        host_id: Uuid,
    ) -> Result<grpc::ContainerSnapshotEvent, ContainerRefreshError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(ContainerRefreshError::NoStream)?;
        let command_id = Uuid::new_v4().to_string();
        let (reply_sender, reply_receiver) = oneshot::channel();
        handle
            .pending
            .lock()
            .await
            .insert(command_id.clone(), reply_sender);

        let command = grpc::ControlPlaneCommand {
            command_id: command_id.clone(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::CollectContainers(
                grpc::CollectContainersCommand {
                    runtime: "docker".to_string(),
                },
            )),
        };

        if handle.sender.send(Ok(command)).await.is_err() {
            handle.pending.lock().await.remove(&command_id);
            return Err(ContainerRefreshError::NoStream);
        }

        match tokio::time::timeout(CONTAINER_REFRESH_TIMEOUT, reply_receiver).await {
            Ok(Ok(AgentCommandReply::ContainerSnapshot(snapshot))) => Ok(snapshot),
            Ok(Ok(AgentCommandReply::Failed(message))) => {
                Err(ContainerRefreshError::AgentFailed(message))
            }
            Ok(Ok(AgentCommandReply::TerminalCommandResult(_))) => Err(
                ContainerRefreshError::AgentFailed("unexpected terminal response".to_string()),
            ),
            Ok(Ok(AgentCommandReply::FileCommandResult(_))) => Err(
                ContainerRefreshError::AgentFailed("unexpected file response".to_string()),
            ),
            Ok(Ok(AgentCommandReply::WebsiteRoutesApplied(_))) => Err(
                ContainerRefreshError::AgentFailed("unexpected website response".to_string()),
            ),
            Ok(Ok(AgentCommandReply::CommandResult(result))) => {
                Err(ContainerRefreshError::AgentFailed(result.message))
            }
            Ok(Ok(AgentCommandReply::VirtualMachineSnapshot(_)))
            | Ok(Ok(AgentCommandReply::VirtualMachineCommandResult)) => {
                Err(ContainerRefreshError::AgentFailed(
                    "unexpected virtual machine response".to_string(),
                ))
            }
            Ok(Err(_)) => Err(ContainerRefreshError::NoStream),
            Err(_) => {
                handle.pending.lock().await.remove(&command_id);
                Err(ContainerRefreshError::Timeout)
            }
        }
    }

    pub(crate) async fn collect_virtual_machines(
        &self,
        host_id: Uuid,
    ) -> Result<grpc::VirtualMachineSnapshotEvent, ContainerRefreshError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(ContainerRefreshError::NoStream)?;
        let command_id = Uuid::new_v4().to_string();
        let (reply_sender, reply_receiver) = oneshot::channel();
        handle
            .pending
            .lock()
            .await
            .insert(command_id.clone(), reply_sender);

        let command = grpc::ControlPlaneCommand {
            command_id: command_id.clone(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(
                grpc::control_plane_command::Command::CollectVirtualMachines(
                    grpc::CollectVirtualMachinesCommand {
                        provider: "qemu".to_string(),
                    },
                ),
            ),
        };

        if handle.sender.send(Ok(command)).await.is_err() {
            handle.pending.lock().await.remove(&command_id);
            return Err(ContainerRefreshError::NoStream);
        }

        match tokio::time::timeout(CONTAINER_REFRESH_TIMEOUT, reply_receiver).await {
            Ok(Ok(AgentCommandReply::VirtualMachineSnapshot(snapshot))) => Ok(snapshot),
            Ok(Ok(AgentCommandReply::Failed(message))) => {
                Err(ContainerRefreshError::AgentFailed(message))
            }
            Ok(Ok(AgentCommandReply::WebsiteRoutesApplied(_))) => Err(
                ContainerRefreshError::AgentFailed("unexpected website response".to_string()),
            ),
            Ok(Ok(_)) => Err(ContainerRefreshError::AgentFailed(
                "unexpected agent response".to_string(),
            )),
            Ok(Err(_)) => Err(ContainerRefreshError::NoStream),
            Err(_) => {
                handle.pending.lock().await.remove(&command_id);
                Err(ContainerRefreshError::Timeout)
            }
        }
    }

    pub(crate) async fn run_terminal_command(
        &self,
        request: &TerminalCommandRequest,
    ) -> Result<grpc::TerminalCommandResultEvent, TerminalCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&request.host_id)
            .cloned()
            .ok_or(TerminalCommandError::NoStream)?;
        let command_id = Uuid::new_v4().to_string();
        let timeout_seconds = request
            .timeout_seconds
            .unwrap_or(DEFAULT_TERMINAL_TIMEOUT_SECONDS)
            .clamp(1, MAX_TERMINAL_TIMEOUT_SECONDS);
        let (reply_sender, reply_receiver) = oneshot::channel();
        handle
            .pending
            .lock()
            .await
            .insert(command_id.clone(), reply_sender);

        let command = grpc::ControlPlaneCommand {
            command_id: command_id.clone(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::RunTerminalCommand(
                grpc::RunTerminalCommandCommand {
                    command_id: command_id.clone(),
                    input: request.input.clone(),
                    cols: request.cols.unwrap_or(80).clamp(20, 300),
                    rows: request.rows.unwrap_or(24).clamp(5, 120),
                    timeout_seconds,
                },
            )),
        };

        if handle.sender.send(Ok(command)).await.is_err() {
            handle.pending.lock().await.remove(&command_id);
            return Err(TerminalCommandError::NoStream);
        }

        let wait = Duration::from_secs(timeout_seconds as u64 + 2);
        match tokio::time::timeout(wait, reply_receiver).await {
            Ok(Ok(AgentCommandReply::TerminalCommandResult(result))) => Ok(result),
            Ok(Ok(AgentCommandReply::Failed(message))) => {
                Err(TerminalCommandError::AgentFailed(message))
            }
            Ok(Ok(AgentCommandReply::FileCommandResult(_))) => Err(
                TerminalCommandError::AgentFailed("unexpected file response".to_string()),
            ),
            Ok(Ok(AgentCommandReply::ContainerSnapshot(_))) => Err(
                TerminalCommandError::AgentFailed("unexpected container response".to_string()),
            ),
            Ok(Ok(AgentCommandReply::WebsiteRoutesApplied(_))) => Err(
                TerminalCommandError::AgentFailed("unexpected website response".to_string()),
            ),
            Ok(Ok(AgentCommandReply::CommandResult(result))) => {
                Err(TerminalCommandError::AgentFailed(result.message))
            }
            Ok(Ok(AgentCommandReply::VirtualMachineSnapshot(_)))
            | Ok(Ok(AgentCommandReply::VirtualMachineCommandResult)) => {
                Err(TerminalCommandError::AgentFailed(
                    "unexpected virtual machine response".to_string(),
                ))
            }
            Ok(Err(_)) => Err(TerminalCommandError::NoStream),
            Err(_) => {
                handle.pending.lock().await.remove(&command_id);
                Err(TerminalCommandError::Timeout)
            }
        }
    }

    pub(crate) async fn run_agent_task(
        &self,
        host_id: Uuid,
        mut command: grpc::RunAgentTaskCommand,
    ) -> Result<grpc::CommandResultEvent, AgentTaskCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(AgentTaskCommandError::NoStream)?;
        let command_id = Uuid::new_v4().to_string();
        command.command_id = command_id.clone();
        let (reply_sender, reply_receiver) = oneshot::channel();
        handle
            .pending
            .lock()
            .await
            .insert(command_id.clone(), reply_sender);

        let command = grpc::ControlPlaneCommand {
            command_id: command_id.clone(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::RunAgentTask(command)),
        };

        if handle.sender.send(Ok(command)).await.is_err() {
            handle.pending.lock().await.remove(&command_id);
            return Err(AgentTaskCommandError::NoStream);
        }

        match tokio::time::timeout(AGENT_TASK_TIMEOUT, reply_receiver).await {
            Ok(Ok(AgentCommandReply::CommandResult(result))) => Ok(result),
            Ok(Ok(AgentCommandReply::Failed(message))) => {
                Err(AgentTaskCommandError::AgentFailed(message))
            }
            Ok(Ok(AgentCommandReply::WebsiteRoutesApplied(_))) => Err(
                AgentTaskCommandError::AgentFailed("unexpected website response".to_string()),
            ),
            Ok(Ok(_)) => Err(AgentTaskCommandError::AgentFailed(
                "unexpected agent response".to_string(),
            )),
            Ok(Err(_)) => Err(AgentTaskCommandError::NoStream),
            Err(_) => {
                handle.pending.lock().await.remove(&command_id);
                Err(AgentTaskCommandError::Timeout)
            }
        }
    }

    pub(crate) async fn start_agent_chat_turn(
        &self,
        host_id: Uuid,
        mut command: grpc::RunAgentChatTurnCommand,
    ) -> Result<String, AgentTaskCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(AgentTaskCommandError::NoStream)?;
        let command_id = Uuid::new_v4().to_string();
        command.command_id = command_id.clone();
        let command = grpc::ControlPlaneCommand {
            command_id: command_id.clone(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::RunAgentChatTurn(
                command,
            )),
        };
        handle
            .sender
            .send(Ok(command))
            .await
            .map_err(|_| AgentTaskCommandError::NoStream)?;
        Ok(command_id)
    }

    pub(crate) async fn send_agent_tool_approval_decision(
        &self,
        host_id: Uuid,
        decision: grpc::AgentToolApprovalDecisionCommand,
    ) -> Result<(), AgentTaskCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(AgentTaskCommandError::NoStream)?;
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(
                grpc::control_plane_command::Command::AgentToolApprovalDecision(decision),
            ),
        };
        handle
            .sender
            .send(Ok(command))
            .await
            .map_err(|_| AgentTaskCommandError::NoStream)
    }

    pub(crate) async fn apply_website_routes(
        &self,
        host_id: Uuid,
        mut command: grpc::ApplyWebsiteRoutesCommand,
    ) -> Result<grpc::WebsiteRoutesAppliedEvent, AgentTaskCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(AgentTaskCommandError::NoStream)?;
        let command_id = Uuid::new_v4().to_string();
        command.command_id = command_id.clone();
        let (reply_sender, reply_receiver) = oneshot::channel();
        handle
            .pending
            .lock()
            .await
            .insert(command_id.clone(), reply_sender);

        let command = grpc::ControlPlaneCommand {
            command_id: command_id.clone(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::ApplyWebsiteRoutes(
                command,
            )),
        };

        if handle.sender.send(Ok(command)).await.is_err() {
            handle.pending.lock().await.remove(&command_id);
            return Err(AgentTaskCommandError::NoStream);
        }

        match tokio::time::timeout(CONTAINER_REFRESH_TIMEOUT, reply_receiver).await {
            Ok(Ok(AgentCommandReply::WebsiteRoutesApplied(result))) => {
                if result.status == grpc::CommandStatus::Failed as i32 {
                    Err(AgentTaskCommandError::AgentFailed(result.message))
                } else {
                    Ok(result)
                }
            }
            Ok(Ok(AgentCommandReply::Failed(message))) => {
                Err(AgentTaskCommandError::AgentFailed(message))
            }
            Ok(Ok(_)) => Err(AgentTaskCommandError::AgentFailed(
                "unexpected agent response".to_string(),
            )),
            Ok(Err(_)) => Err(AgentTaskCommandError::NoStream),
            Err(_) => {
                handle.pending.lock().await.remove(&command_id);
                Err(AgentTaskCommandError::Timeout)
            }
        }
    }

    pub(crate) async fn list_directory(
        &self,
        host_id: Uuid,
        path: String,
    ) -> Result<grpc::FileCommandResultEvent, FileCommandError> {
        self.run_file_command(
            host_id,
            grpc::control_plane_command::Command::ListDirectory(grpc::ListDirectoryCommand {
                command_id: String::new(),
                path,
            }),
        )
        .await
    }

    pub(crate) async fn read_file(
        &self,
        host_id: Uuid,
        path: String,
    ) -> Result<grpc::FileCommandResultEvent, FileCommandError> {
        self.run_file_command(
            host_id,
            grpc::control_plane_command::Command::ReadFile(grpc::ReadFileCommand {
                command_id: String::new(),
                path,
            }),
        )
        .await
    }

    pub(crate) async fn search_files(
        &self,
        host_id: Uuid,
        path: String,
        query: String,
        limit: u32,
    ) -> Result<grpc::FileCommandResultEvent, FileCommandError> {
        self.run_file_command(
            host_id,
            grpc::control_plane_command::Command::SearchFiles(grpc::SearchFilesCommand {
                command_id: String::new(),
                path,
                query,
                limit,
            }),
        )
        .await
    }

    pub(crate) async fn run_file_operation(
        &self,
        host_id: Uuid,
        command: grpc::RunFileOperationCommand,
    ) -> Result<grpc::FileCommandResultEvent, FileCommandError> {
        self.run_file_command(
            host_id,
            grpc::control_plane_command::Command::RunFileOperation(command),
        )
        .await
    }

    pub(crate) async fn run_file_command(
        &self,
        host_id: Uuid,
        command: grpc::control_plane_command::Command,
    ) -> Result<grpc::FileCommandResultEvent, FileCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(FileCommandError::NoStream)?;
        let command_id = Uuid::new_v4().to_string();
        let (reply_sender, reply_receiver) = oneshot::channel();
        handle
            .pending
            .lock()
            .await
            .insert(command_id.clone(), reply_sender);

        let command = grpc::ControlPlaneCommand {
            command_id: command_id.clone(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(with_file_command_id(command, &command_id)),
        };

        if handle.sender.send(Ok(command)).await.is_err() {
            handle.pending.lock().await.remove(&command_id);
            return Err(FileCommandError::NoStream);
        }

        match tokio::time::timeout(FILE_COMMAND_TIMEOUT, reply_receiver).await {
            Ok(Ok(AgentCommandReply::FileCommandResult(result))) => Ok(result),
            Ok(Ok(AgentCommandReply::Failed(message))) => {
                Err(FileCommandError::AgentFailed(message))
            }
            Ok(Ok(_)) => Err(FileCommandError::AgentFailed(
                "unexpected agent response".to_string(),
            )),
            Ok(Err(_)) => Err(FileCommandError::NoStream),
            Err(_) => {
                handle.pending.lock().await.remove(&command_id);
                Err(FileCommandError::Timeout)
            }
        }
    }

    pub(crate) async fn open_terminal_session(
        &self,
        host_id: Uuid,
        session_id: String,
        cols: u32,
        rows: u32,
        output_sender: mpsc::UnboundedSender<String>,
    ) -> Result<(), TerminalCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(TerminalCommandError::NoStream)?;
        self.terminal_sessions
            .lock()
            .await
            .insert(session_id.clone(), output_sender);
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::OpenTerminalSession(
                grpc::OpenTerminalSessionCommand {
                    session_id: session_id.clone(),
                    cols: cols.clamp(20, 300),
                    rows: rows.clamp(5, 120),
                },
            )),
        };
        if handle.sender.send(Ok(command)).await.is_err() {
            self.terminal_sessions.lock().await.remove(&session_id);
            return Err(TerminalCommandError::NoStream);
        }
        Ok(())
    }

    pub(crate) async fn send_terminal_input(
        &self,
        host_id: Uuid,
        session_id: String,
        data: String,
    ) -> Result<(), TerminalCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(TerminalCommandError::NoStream)?;
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::TerminalInput(
                grpc::TerminalInputCommand { session_id, data },
            )),
        };
        handle
            .sender
            .send(Ok(command))
            .await
            .map_err(|_| TerminalCommandError::NoStream)
    }

    pub(crate) async fn resize_terminal_session(
        &self,
        host_id: Uuid,
        session_id: String,
        cols: u32,
        rows: u32,
    ) -> Result<(), TerminalCommandError> {
        let handle = self
            .streams
            .lock()
            .await
            .get(&host_id)
            .cloned()
            .ok_or(TerminalCommandError::NoStream)?;
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::ResizeTerminalSession(
                grpc::ResizeTerminalSessionCommand {
                    session_id,
                    cols: cols.clamp(20, 300),
                    rows: rows.clamp(5, 120),
                },
            )),
        };
        handle
            .sender
            .send(Ok(command))
            .await
            .map_err(|_| TerminalCommandError::NoStream)
    }

    pub(crate) async fn close_terminal_session(
        &self,
        host_id: Uuid,
        session_id: String,
        reason: String,
    ) {
        self.terminal_sessions.lock().await.remove(&session_id);
        let Some(handle) = self.streams.lock().await.get(&host_id).cloned() else {
            return;
        };
        let command = grpc::ControlPlaneCommand {
            command_id: Uuid::new_v4().to_string(),
            issued_at: Some(protobuf_timestamp_now()),
            command: Some(grpc::control_plane_command::Command::CloseTerminalSession(
                grpc::CloseTerminalSessionCommand { session_id, reason },
            )),
        };
        let _ = handle.sender.send(Ok(command)).await;
    }

    pub(crate) async fn publish_terminal_output(&self, session_id: &str, data: String) {
        let sender = self.terminal_sessions.lock().await.get(session_id).cloned();
        if let Some(sender) = sender
            && sender.send(data).is_err()
        {
            self.terminal_sessions.lock().await.remove(session_id);
        }
    }

    pub(crate) async fn close_terminal_output(&self, session_id: &str, reason: String) {
        if let Some(sender) = self.terminal_sessions.lock().await.remove(session_id) {
            let _ = sender.send(format!("\r\n[terminal closed: {reason}]\r\n"));
        }
    }
}

#[derive(Debug)]
pub(crate) enum ContainerRefreshError {
    NoStream,
    Timeout,
    AgentFailed(String),
}

#[derive(Debug)]
pub(crate) enum TerminalCommandError {
    NoStream,
    Timeout,
    AgentFailed(String),
}

#[derive(Debug)]
pub(crate) enum FileCommandError {
    NoStream,
    Timeout,
    AgentFailed(String),
}

#[derive(Debug)]
pub(crate) enum AgentTaskCommandError {
    NoStream,
    Timeout,
    AgentFailed(String),
}

pub(crate) fn terminal_command_error_message(error: TerminalCommandError) -> String {
    match error {
        TerminalCommandError::NoStream => "agent stream is not connected".to_string(),
        TerminalCommandError::Timeout => "agent terminal command timed out".to_string(),
        TerminalCommandError::AgentFailed(message) => {
            format!("agent terminal command failed: {message}")
        }
    }
}

pub(crate) fn agent_task_error_message(error: AgentTaskCommandError) -> String {
    match error {
        AgentTaskCommandError::NoStream => "agent stream is not connected".to_string(),
        AgentTaskCommandError::Timeout => "agent task command timed out".to_string(),
        AgentTaskCommandError::AgentFailed(message) => {
            format!("agent task command failed: {message}")
        }
    }
}

pub(crate) fn container_refresh_app_error(error: ContainerRefreshError) -> AppError {
    match error {
        ContainerRefreshError::NoStream => AppError::status(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent stream is not connected",
        ),
        ContainerRefreshError::Timeout => AppError::status(
            StatusCode::GATEWAY_TIMEOUT,
            "agent container refresh timed out",
        ),
        ContainerRefreshError::AgentFailed(message) => AppError::status(
            StatusCode::BAD_GATEWAY,
            format!("agent container refresh failed: {message}"),
        ),
    }
}

pub(crate) fn terminal_command_app_error(error: TerminalCommandError) -> AppError {
    match error {
        TerminalCommandError::NoStream => AppError::status(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent stream is not connected",
        ),
        TerminalCommandError::Timeout => AppError::status(
            StatusCode::GATEWAY_TIMEOUT,
            "agent terminal command timed out",
        ),
        TerminalCommandError::AgentFailed(message) => AppError::status(
            StatusCode::BAD_GATEWAY,
            format!("agent terminal command failed: {message}"),
        ),
    }
}

pub(crate) fn file_command_app_error(error: FileCommandError) -> AppError {
    match error {
        FileCommandError::NoStream => AppError::status(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent stream is not connected",
        ),
        FileCommandError::Timeout => {
            AppError::status(StatusCode::GATEWAY_TIMEOUT, "agent file command timed out")
        }
        FileCommandError::AgentFailed(message) => AppError::status(
            StatusCode::BAD_GATEWAY,
            format!("agent file command failed: {message}"),
        ),
    }
}

pub(crate) fn command_status_label(status: i32) -> &'static str {
    if status == grpc::CommandStatus::Succeeded as i32 {
        "succeeded"
    } else {
        "failed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stream_registry_dispatches_container_command_and_receives_snapshot() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel(1);
        let pending = registry.register(host_id, agent_id, sender).await;

        let collect = tokio::spawn({
            let registry = registry.clone();
            async move { registry.collect_containers(host_id).await }
        });
        let command = match receiver.recv().await {
            Some(Ok(command)) => command,
            Some(Err(error)) => panic!("command stream item should be ok: {error}"),
            None => panic!("registry should send collect command"),
        };
        assert!(matches!(
            command.command,
            Some(grpc::control_plane_command::Command::CollectContainers(_))
        ));
        let reply_sender = match pending.lock().await.remove(&command.command_id) {
            Some(reply_sender) => reply_sender,
            None => panic!("command should have pending waiter"),
        };
        if reply_sender
            .send(AgentCommandReply::ContainerSnapshot(
                grpc::ContainerSnapshotEvent {
                    command_id: command.command_id,
                    runtime: "docker".to_string(),
                    containers: Vec::new(),
                    extra_json: "{}".to_string(),
                },
            ))
            .is_err()
        {
            panic!("waiter should receive snapshot");
        }

        let snapshot = match collect.await {
            Ok(Ok(snapshot)) => snapshot,
            Ok(Err(error)) => panic!("snapshot should succeed: {error:?}"),
            Err(error) => panic!("collect task should complete: {error}"),
        };
        assert_eq!(snapshot.runtime, "docker");
    }

    #[tokio::test]
    async fn stream_registry_dispatches_terminal_command_and_receives_result() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel(1);
        let pending = registry.register(host_id, agent_id, sender).await;

        let execute = tokio::spawn({
            let registry = registry.clone();
            async move {
                registry
                    .run_terminal_command(&TerminalCommandRequest {
                        host_id,
                        input: "pwd".to_string(),
                        cols: Some(100),
                        rows: Some(30),
                        timeout_seconds: Some(10),
                    })
                    .await
            }
        });
        let command = match receiver.recv().await {
            Some(Ok(command)) => command,
            Some(Err(error)) => panic!("command stream item should be ok: {error}"),
            None => panic!("registry should send terminal command"),
        };
        assert!(matches!(
            command.command,
            Some(grpc::control_plane_command::Command::RunTerminalCommand(_))
        ));
        let reply_sender = match pending.lock().await.remove(&command.command_id) {
            Some(reply_sender) => reply_sender,
            None => panic!("command should have pending waiter"),
        };
        if reply_sender
            .send(AgentCommandReply::TerminalCommandResult(
                grpc::TerminalCommandResultEvent {
                    command_id: command.command_id,
                    status: grpc::CommandStatus::Succeeded as i32,
                    output: "/tmp".to_string(),
                    exit_code: 0,
                    started_at: Some(protobuf_timestamp_now()),
                    finished_at: Some(protobuf_timestamp_now()),
                },
            ))
            .is_err()
        {
            panic!("waiter should receive terminal result");
        }

        let result = match execute.await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => panic!("terminal command should succeed: {error:?}"),
            Err(error) => panic!("execute task should complete: {error}"),
        };
        assert_eq!(result.output, "/tmp");
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn stream_registry_dispatches_agent_task_and_receives_command_result() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel(1);
        let pending = registry.register(host_id, agent_id, sender).await;

        let execute = tokio::spawn({
            let registry = registry.clone();
            async move {
                registry
                    .run_agent_task(
                        host_id,
                        grpc::RunAgentTaskCommand {
                            command_id: String::new(),
                            task_id: Uuid::new_v4().to_string(),
                            scheduled_task_id: Uuid::new_v4().to_string(),
                            prompt: "inspect host".to_string(),
                            template_json: "{}".to_string(),
                            ai_provider: None,
                        },
                    )
                    .await
            }
        });
        let command = match receiver.recv().await {
            Some(Ok(command)) => command,
            Some(Err(error)) => panic!("command stream item should be ok: {error}"),
            None => panic!("registry should send agent task command"),
        };
        assert!(matches!(
            command.command,
            Some(grpc::control_plane_command::Command::RunAgentTask(_))
        ));
        let reply_sender = match pending.lock().await.remove(&command.command_id) {
            Some(reply_sender) => reply_sender,
            None => panic!("command should have pending waiter"),
        };
        if reply_sender
            .send(AgentCommandReply::CommandResult(grpc::CommandResultEvent {
                command_id: command.command_id,
                status: grpc::CommandStatus::Succeeded as i32,
                message: "agent core placeholder accepted".to_string(),
            }))
            .is_err()
        {
            panic!("waiter should receive command result");
        }

        let result = match execute.await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => panic!("agent task command should succeed: {error:?}"),
            Err(error) => panic!("execute task should complete: {error}"),
        };
        assert_eq!(result.message, "agent core placeholder accepted");
    }

    #[tokio::test]
    async fn stream_registry_dispatches_website_routes_and_receives_apply_result() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let website_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel(1);
        let pending = registry.register(host_id, agent_id, sender).await;

        let apply = tokio::spawn({
            let registry = registry.clone();
            async move {
                registry
                    .apply_website_routes(
                        host_id,
                        grpc::ApplyWebsiteRoutesCommand {
                            command_id: String::new(),
                            routes: vec![grpc::WebsiteRoute {
                                website_id: website_id.to_string(),
                                primary_domain: "example.com".to_string(),
                                aliases: Vec::new(),
                                status: "running".to_string(),
                                kind: "reverse_proxy".to_string(),
                                protocol: "http".to_string(),
                                listen_port: 8080,
                                upstream_url: "http://127.0.0.1:8787".to_string(),
                                config_json: "{}".to_string(),
                            }],
                        },
                    )
                    .await
            }
        });
        let command = match receiver.recv().await {
            Some(Ok(command)) => command,
            Some(Err(error)) => panic!("command stream item should be ok: {error}"),
            None => panic!("registry should send website apply command"),
        };
        assert!(matches!(
            command.command,
            Some(grpc::control_plane_command::Command::ApplyWebsiteRoutes(_))
        ));
        let reply_sender = match pending.lock().await.remove(&command.command_id) {
            Some(reply_sender) => reply_sender,
            None => panic!("command should have pending waiter"),
        };
        if reply_sender
            .send(AgentCommandReply::WebsiteRoutesApplied(
                grpc::WebsiteRoutesAppliedEvent {
                    command_id: command.command_id,
                    status: grpc::CommandStatus::Succeeded as i32,
                    message: "routes applied".to_string(),
                    route_count: 1,
                    website_ids: vec![website_id.to_string()],
                },
            ))
            .is_err()
        {
            panic!("waiter should receive website apply result");
        }

        let result = match apply.await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => panic!("website routes should apply: {error:?}"),
            Err(error) => panic!("apply task should complete: {error}"),
        };
        assert_eq!(result.route_count, 1);
        assert_eq!(result.website_ids, vec![website_id.to_string()]);
    }

    #[tokio::test]
    async fn stream_registry_dispatches_file_command_and_receives_result() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel(1);
        let pending = registry.register(host_id, agent_id, sender).await;

        let list = tokio::spawn({
            let registry = registry.clone();
            async move { registry.list_directory(host_id, "/".to_string()).await }
        });
        let command = match receiver.recv().await {
            Some(Ok(command)) => command,
            Some(Err(error)) => panic!("command stream item should be ok: {error}"),
            None => panic!("registry should send file command"),
        };
        assert!(matches!(
            command.command,
            Some(grpc::control_plane_command::Command::ListDirectory(_))
        ));
        let reply_sender = match pending.lock().await.remove(&command.command_id) {
            Some(reply_sender) => reply_sender,
            None => panic!("command should have pending waiter"),
        };
        if reply_sender
            .send(AgentCommandReply::FileCommandResult(
                grpc::FileCommandResultEvent {
                    command_id: command.command_id,
                    status: grpc::CommandStatus::Succeeded as i32,
                    message: "directory listed".to_string(),
                    result_json: serde_json::json!({
                        "path": "/",
                        "parent_path": null,
                        "items": []
                    })
                    .to_string(),
                    content: Vec::new(),
                },
            ))
            .is_err()
        {
            panic!("waiter should receive file result");
        }

        let result = match list.await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => panic!("file command should succeed: {error:?}"),
            Err(error) => panic!("list task should complete: {error}"),
        };
        assert_eq!(result.message, "directory listed");
    }

    #[tokio::test]
    async fn stream_registry_bridges_interactive_terminal_output() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let session_id = Uuid::new_v4().to_string();
        let (sender, mut receiver) = mpsc::channel(1);
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel();
        registry.register(host_id, agent_id, sender).await;

        registry
            .open_terminal_session(host_id, session_id.clone(), 100, 30, output_sender)
            .await
            .unwrap_or_else(|error| panic!("terminal session should open: {error:?}"));
        let command = match receiver.recv().await {
            Some(Ok(command)) => command,
            Some(Err(error)) => panic!("command stream item should be ok: {error}"),
            None => panic!("registry should send open terminal command"),
        };
        assert!(matches!(
            command.command,
            Some(grpc::control_plane_command::Command::OpenTerminalSession(_))
        ));

        registry
            .publish_terminal_output(&session_id, "hello".to_string())
            .await;
        let output = match output_receiver.recv().await {
            Some(output) => output,
            None => panic!("websocket output channel should receive terminal output"),
        };
        assert_eq!(output, "hello");

        registry
            .close_terminal_session(host_id, session_id, "test complete".to_string())
            .await;
    }

    #[tokio::test]
    async fn stream_registry_sends_shutdown_to_registered_streams() {
        let registry = AgentStreamRegistry::default();
        let host_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let (sender, mut receiver) = mpsc::channel(1);
        registry.register(host_id, agent_id, sender).await;

        registry.shutdown_all("control-plane shutting down").await;

        let command = match receiver.recv().await {
            Some(Ok(command)) => command,
            Some(Err(error)) => panic!("command stream item should be ok: {error}"),
            None => panic!("registry should send shutdown command"),
        };
        let Some(grpc::control_plane_command::Command::Shutdown(shutdown)) = command.command else {
            panic!("shutdown_all should send shutdown command");
        };
        assert_eq!(shutdown.reason, "control-plane shutting down");
    }
}
