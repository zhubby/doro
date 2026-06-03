use crate::collectors::MetricsCapture;
use crate::filesystem;
use crate::logs::AgentRuntimeLog;
use crate::runtime::Agent;
use crate::terminal;
use doro_ai::{AgentRunOutcome, AgentRunStatus};
use doro_container::{ContainerRuntimeSnapshot, ContainerSummary};
use doro_protocol::{PROTOCOL_VERSION, grpc, protobuf_timestamp_from_utc, protobuf_timestamp_now};
use doro_vm::{VmCommandStatus, VmRuntimeState, VmStatus};
use uuid::Uuid;

impl Agent {
    fn grpc_event(&self, agent_id: Uuid, event: grpc::agent_event::Event) -> grpc::AgentEvent {
        grpc::AgentEvent {
            event_id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            host_id: self.config.host_id.to_string(),
            recorded_at: Some(protobuf_timestamp_now()),
            event: Some(event),
        }
    }

    pub fn connected_event(&self, agent_id: Uuid) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::Connected(grpc::ConnectedEvent {
                protocol_version: PROTOCOL_VERSION.to_string(),
                hostname: self.config.hostname.clone(),
            }),
        )
    }

    pub fn heartbeat_event(&self, agent_id: Uuid) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::Heartbeat(grpc::HeartbeatEvent {
                protocol_version: PROTOCOL_VERSION.to_string(),
            }),
        )
    }

    pub fn metrics_snapshot_event(
        &self,
        agent_id: Uuid,
        metrics: MetricsCapture,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::MetricsSnapshot(grpc::MetricsSnapshotEvent {
                host_id: metrics.snapshot.host_id.to_string(),
                captured_at: Some(protobuf_timestamp_from_utc(metrics.snapshot.captured_at)),
                cpu_percent: metrics.snapshot.cpu_percent,
                memory_percent: metrics.snapshot.memory_percent,
                disk_percent: metrics.snapshot.disk_percent,
                load_average: metrics.snapshot.load_average,
                extra_json: metrics.extra.to_string(),
            }),
        )
    }

    pub fn container_snapshot_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        snapshot: ContainerRuntimeSnapshot,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::ContainerSnapshot(container_snapshot_from_runtime(
                command_id, snapshot,
            )),
        )
    }

    pub fn virtual_machine_snapshot_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        states: Vec<VmRuntimeState>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::VirtualMachineSnapshot(grpc::VirtualMachineSnapshotEvent {
                command_id,
                provider: "qemu".to_string(),
                virtual_machines: states
                    .into_iter()
                    .map(virtual_machine_observation_from_state)
                    .collect(),
                extra_json: serde_json::json!({}).to_string(),
            }),
        )
    }

    pub fn virtual_machine_command_result_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        result: doro_vm::VmCommandResult,
    ) -> grpc::AgentEvent {
        let status = match result.status {
            VmCommandStatus::Succeeded => grpc::CommandStatus::Succeeded,
            VmCommandStatus::Failed => grpc::CommandStatus::Failed,
        };
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::VirtualMachineCommandResult(
                grpc::VirtualMachineCommandResultEvent {
                    command_id,
                    status: status as i32,
                    message: result.message,
                    details_json: result.details.to_string(),
                },
            ),
        )
    }

    pub fn file_command_result_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        output: filesystem::FileCommandOutput,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::FileCommandResult(grpc::FileCommandResultEvent {
                command_id,
                status: grpc::CommandStatus::Succeeded as i32,
                message: output.message,
                result_json: output.result_json,
                content: output.content,
            }),
        )
    }

    pub fn website_routes_applied_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        result: Result<usize, String>,
        website_ids: Vec<String>,
    ) -> grpc::AgentEvent {
        let (status, message, route_count) = match result {
            Ok(route_count) => (
                grpc::CommandStatus::Succeeded,
                "website routes applied".to_string(),
                route_count as u32,
            ),
            Err(message) => (grpc::CommandStatus::Failed, message, 0),
        };
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::WebsiteRoutesApplied(grpc::WebsiteRoutesAppliedEvent {
                command_id,
                status: status as i32,
                message,
                route_count,
                website_ids,
            }),
        )
    }

    pub fn agent_task_progress_event(
        &self,
        agent_id: Uuid,
        progress: grpc::AgentTaskProgressEvent,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::AgentTaskProgress(progress),
        )
    }

    pub fn agent_task_result_event(
        &self,
        agent_id: Uuid,
        command_id: impl Into<String>,
        task_id: impl Into<String>,
        outcome: &AgentRunOutcome,
    ) -> grpc::AgentEvent {
        let status = match outcome.status {
            AgentRunStatus::Succeeded => grpc::CommandStatus::Succeeded,
            AgentRunStatus::Failed => grpc::CommandStatus::Failed,
        };
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::AgentTaskResult(grpc::AgentTaskResultEvent {
                command_id: command_id.into(),
                task_id: task_id.into(),
                status: status as i32,
                summary: outcome.summary.clone(),
                result_json: serde_json::json!({
                    "transcript": outcome.transcript,
                })
                .to_string(),
            }),
        )
    }

    pub fn agent_tool_approval_request_event(
        &self,
        agent_id: Uuid,
        request: grpc::AgentToolApprovalRequestEvent,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::AgentToolApprovalRequest(request),
        )
    }

    pub fn agent_chat_text_delta_event(
        &self,
        agent_id: Uuid,
        delta: grpc::AgentChatTextDeltaEvent,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::AgentChatTextDelta(delta),
        )
    }

    pub fn agent_chat_tool_event(
        &self,
        agent_id: Uuid,
        tool: grpc::AgentChatToolEvent,
    ) -> grpc::AgentEvent {
        self.grpc_event(agent_id, grpc::agent_event::Event::AgentChatTool(tool))
    }

    pub fn agent_chat_turn_result_event(
        &self,
        agent_id: Uuid,
        result: grpc::AgentChatTurnResultEvent,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::AgentChatTurnResult(result),
        )
    }

    pub fn collector_error_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        collector: impl Into<String>,
        message: impl Into<String>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::CollectorError(grpc::CollectorErrorEvent {
                command_id,
                collector: collector.into(),
                message: message.into(),
            }),
        )
    }

    pub fn command_result_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        status: grpc::CommandStatus,
        message: impl Into<String>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::CommandResult(grpc::CommandResultEvent {
                command_id,
                status: status as i32,
                message: message.into(),
            }),
        )
    }

    pub fn terminal_command_result_event(
        &self,
        agent_id: Uuid,
        command_id: String,
        output: terminal::TerminalCommandOutput,
    ) -> grpc::AgentEvent {
        let status = if output.exit_code == Some(0) && !output.timed_out {
            grpc::CommandStatus::Succeeded
        } else {
            grpc::CommandStatus::Failed
        };
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::TerminalCommandResult(grpc::TerminalCommandResultEvent {
                command_id,
                status: status as i32,
                output: output.output,
                exit_code: output.exit_code.unwrap_or(-1),
                started_at: Some(protobuf_timestamp_from_utc(output.started_at)),
                finished_at: Some(protobuf_timestamp_from_utc(output.finished_at)),
            }),
        )
    }

    pub fn terminal_output_event(
        &self,
        agent_id: Uuid,
        session_id: String,
        data: Vec<u8>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::TerminalOutput(grpc::TerminalOutputEvent {
                session_id,
                data: String::from_utf8_lossy(&data).into_owned(),
            }),
        )
    }

    pub fn terminal_session_closed_event(
        &self,
        agent_id: Uuid,
        session_id: String,
        reason: impl Into<String>,
    ) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::TerminalSessionClosed(grpc::TerminalSessionClosedEvent {
                session_id,
                reason: reason.into(),
            }),
        )
    }

    pub fn log_line_event(&self, agent_id: Uuid, log: AgentRuntimeLog) -> grpc::AgentEvent {
        self.grpc_event(
            agent_id,
            grpc::agent_event::Event::LogLine(grpc::LogLineEvent {
                log_id: log.id.to_string(),
                level: log.level,
                target: log.target,
                message: log.message,
                fields_json: log.fields.to_string(),
            }),
        )
    }
}

fn container_snapshot_from_runtime(
    command_id: String,
    snapshot: ContainerRuntimeSnapshot,
) -> grpc::ContainerSnapshotEvent {
    let extra_json = serde_json::json!({
        "daemon": snapshot.daemon,
        "networks": snapshot.networks,
        "volumes": snapshot.volumes,
    })
    .to_string();

    grpc::ContainerSnapshotEvent {
        command_id,
        runtime: snapshot.runtime,
        containers: snapshot
            .containers
            .into_iter()
            .map(container_observation_from_summary)
            .collect(),
        extra_json,
    }
}

fn container_observation_from_summary(container: ContainerSummary) -> grpc::ContainerObservation {
    grpc::ContainerObservation {
        id: container.id.unwrap_or_default(),
        names: container.names,
        image: container.image.unwrap_or_default(),
        image_id: container.image_id.unwrap_or_default(),
        command: container.command.unwrap_or_default(),
        created: container.created.unwrap_or_default(),
        ports_json: container.ports.to_string(),
        labels_json: container.labels.to_string(),
        state: container.state.unwrap_or_default(),
        status: container.status.unwrap_or_default(),
    }
}

fn virtual_machine_observation_from_state(
    state: VmRuntimeState,
) -> grpc::VirtualMachineObservation {
    grpc::VirtualMachineObservation {
        vm_ref: state.id.to_string(),
        name: state.name,
        status: serialize_vm_status(state.status).to_string(),
        cpu_cores: u32::from(state.cpu_cores),
        memory_mib: state.memory_mib,
        disk_gb: state.disk_gb,
        image: state
            .metadata
            .get("image")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        networks_json: serde_json::to_string(&state.networks).unwrap_or_else(|_| "[]".to_string()),
        console_json: state
            .console
            .and_then(|console| serde_json::to_string(&console).ok())
            .unwrap_or_else(|| "null".to_string()),
        metadata_json: state.metadata.to_string(),
        created_at: state.created_at.map(protobuf_timestamp_from_utc),
        observed_at: Some(protobuf_timestamp_from_utc(state.observed_at)),
    }
}

fn serialize_vm_status(status: VmStatus) -> &'static str {
    match status {
        VmStatus::Unknown => "unknown",
        VmStatus::Stopped => "stopped",
        VmStatus::Starting => "starting",
        VmStatus::Running => "running",
        VmStatus::Paused => "paused",
        VmStatus::Stopping => "stopping",
        VmStatus::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfig;
    use crate::test_support::test_agent_config;
    use doro_protocol::PROTOCOL_VERSION;

    #[test]
    fn grpc_event_includes_durable_identity_and_payload() {
        let agent_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let agent = Agent::new(AgentConfig {
            host_id,
            ..test_agent_config(agent_id)
        });

        let event = agent.connected_event(agent_id);

        assert_eq!(event.agent_id, agent_id.to_string());
        assert_eq!(event.host_id, host_id.to_string());
        let Some(grpc::agent_event::Event::Connected(connected)) = event.event else {
            panic!("connected event should use typed payload");
        };
        assert_eq!(connected.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn container_snapshot_event_preserves_command_id() {
        let agent_id = Uuid::new_v4();
        let command_id = Uuid::new_v4().to_string();
        let agent = Agent::new(test_agent_config(agent_id));

        let event = agent.container_snapshot_event(
            agent_id,
            command_id.clone(),
            ContainerRuntimeSnapshot {
                runtime: "docker".to_string(),
                daemon: None,
                containers: vec![ContainerSummary {
                    id: Some("abc".to_string()),
                    names: vec!["/db".to_string()],
                    image: Some("postgres".to_string()),
                    image_id: None,
                    command: None,
                    created: None,
                    ports: serde_json::json!([]),
                    labels: serde_json::json!({}),
                    state: None,
                    status: None,
                }],
                networks: Vec::new(),
                volumes: Vec::new(),
            },
        );

        let Some(grpc::agent_event::Event::ContainerSnapshot(snapshot)) = event.event else {
            panic!("container event should use typed payload");
        };
        assert_eq!(snapshot.command_id, command_id);
        assert_eq!(snapshot.containers.len(), 1);
        assert_eq!(snapshot.containers[0].id, "abc");
    }

    #[test]
    fn log_line_event_preserves_runtime_log_fields() {
        let agent_id = Uuid::new_v4();
        let agent = Agent::new(test_agent_config(agent_id));
        let log_id = Uuid::new_v4();

        let event = agent.log_line_event(
            agent_id,
            AgentRuntimeLog {
                id: log_id,
                level: "INFO".to_string(),
                target: "doro_agent".to_string(),
                message: "agent connected".to_string(),
                fields: serde_json::json!({"message": "agent connected"}),
            },
        );

        let Some(grpc::agent_event::Event::LogLine(log)) = event.event else {
            panic!("log line event should use typed payload");
        };
        assert_eq!(log.log_id, log_id.to_string());
        assert_eq!(log.level, "INFO");
        assert_eq!(log.message, "agent connected");
    }
}
