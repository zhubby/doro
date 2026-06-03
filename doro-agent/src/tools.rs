use crate::constants::MAX_FILE_TRANSFER_BYTES;
use crate::filesystem;
use crate::runtime::Agent;
use crate::terminal::{TerminalCommand, TerminalManager};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use doro_ai::{
    AgentError, AgentToolCall, AgentToolDefinition, AgentToolExecutor, AgentToolResult,
    AgentToolResultStatus,
};
use doro_protocol::{CapabilityRisk, grpc};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub(crate) struct AgentCommandState {
    pending_tool_approvals:
        Arc<Mutex<HashMap<String, oneshot::Sender<grpc::AgentToolApprovalDecisionCommand>>>>,
}

impl AgentCommandState {
    async fn wait_for_tool_approval(
        &self,
        request_id: String,
        timeout: Duration,
    ) -> Result<grpc::AgentToolApprovalDecisionCommand, AgentError> {
        let (sender, receiver) = oneshot::channel();
        self.pending_tool_approvals
            .lock()
            .await
            .insert(request_id.clone(), sender);

        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(decision)) => Ok(decision),
            Ok(Err(_)) => Err(AgentError::Tool {
                name: "approval".to_string(),
                message: "approval channel closed".to_string(),
            }),
            Err(_) => {
                self.pending_tool_approvals.lock().await.remove(&request_id);
                Err(AgentError::Tool {
                    name: "approval".to_string(),
                    message: "approval timed out".to_string(),
                })
            }
        }
    }

    pub(crate) async fn resolve_tool_approval(
        &self,
        decision: grpc::AgentToolApprovalDecisionCommand,
    ) {
        let sender = self
            .pending_tool_approvals
            .lock()
            .await
            .remove(&decision.request_id);
        if let Some(sender) = sender {
            let _ = sender.send(decision);
        }
    }
}

impl Agent {
    pub(crate) fn ai_tool_definitions(&self) -> Vec<AgentToolDefinition> {
        let mut tools = vec![
            AgentToolDefinition {
                name: "host_metrics".to_string(),
                description: "Read current host metrics and basic resource status".to_string(),
                risk: CapabilityRisk::Low,
                parameters: empty_schema(),
            },
            AgentToolDefinition {
                name: "list_directory".to_string(),
                description: "List files in a directory as the agent OS user".to_string(),
                risk: CapabilityRisk::Low,
                parameters: object_schema(vec![("path", "Directory path")], &["path"]),
            },
            AgentToolDefinition {
                name: "read_file".to_string(),
                description: "Read a file as the agent OS user within the transfer limit"
                    .to_string(),
                risk: CapabilityRisk::Low,
                parameters: object_schema(vec![("path", "File path")], &["path"]),
            },
            AgentToolDefinition {
                name: "search_files".to_string(),
                description: "Search file and directory names below a path".to_string(),
                risk: CapabilityRisk::Low,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Root directory path" },
                        "query": { "type": "string", "description": "Case-insensitive name query" },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 500 }
                    },
                    "required": ["path", "query"],
                    "additionalProperties": false
                }),
            },
            AgentToolDefinition {
                name: "run_shell".to_string(),
                description:
                    "Run a shell command through the Doro terminal path after approval"
                        .to_string(),
                risk: CapabilityRisk::High,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Shell command or script" },
                        "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 120 }
                    },
                    "required": ["input"],
                    "additionalProperties": false
                }),
            },
            AgentToolDefinition {
                name: "write_file".to_string(),
                description: "Write UTF-8 text to a file after approval".to_string(),
                risk: CapabilityRisk::High,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Target file path" },
                        "content": { "type": "string", "description": "UTF-8 file content" },
                        "overwrite": { "type": "boolean" }
                    },
                    "required": ["path", "content"],
                    "additionalProperties": false
                }),
            },
            AgentToolDefinition {
                name: "file_operation".to_string(),
                description:
                    "Create directory, rename, move, copy, or delete a filesystem path after approval"
                        .to_string(),
                risk: CapabilityRisk::High,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["create_directory", "rename", "move", "copy", "delete"]
                        },
                        "path": { "type": "string" },
                        "target_path": { "type": "string" },
                        "name": { "type": "string" },
                        "overwrite": { "type": "boolean" }
                    },
                    "required": ["operation", "path"],
                    "additionalProperties": false
                }),
            },
        ];

        if self.container_runtime.is_some() {
            tools.push(AgentToolDefinition {
                name: "container_snapshot".to_string(),
                description: "Read current Docker runtime, container, network, and volume state"
                    .to_string(),
                risk: CapabilityRisk::Low,
                parameters: empty_schema(),
            });
        }
        if self.vm_runtime.is_some() {
            tools.push(AgentToolDefinition {
                name: "virtual_machine_snapshot".to_string(),
                description: "Read current QEMU virtual machine state".to_string(),
                risk: CapabilityRisk::Low,
                parameters: empty_schema(),
            });
        }

        tools
    }
}

fn empty_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

fn object_schema(properties: Vec<(&str, &str)>, required: &[&str]) -> Value {
    let properties = properties
        .into_iter()
        .map(|(name, description)| {
            (
                name.to_string(),
                serde_json::json!({
                    "type": "string",
                    "description": description,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

#[derive(Clone)]
pub(crate) struct LocalAgentToolExecutor {
    pub(crate) agent: Agent,
    pub(crate) agent_id: Uuid,
    pub(crate) command_id: String,
    pub(crate) task_id: String,
    pub(crate) sender: mpsc::Sender<grpc::AgentEvent>,
    pub(crate) terminal: TerminalManager,
    pub(crate) command_state: AgentCommandState,
    pub(crate) tool_timeout: Duration,
    pub(crate) shell_timeout: Duration,
    pub(crate) approval_timeout: Duration,
}

#[async_trait]
impl AgentToolExecutor for LocalAgentToolExecutor {
    async fn execute(
        &self,
        call: AgentToolCall,
        definition: &AgentToolDefinition,
    ) -> Result<AgentToolResult, AgentError> {
        let step_id = if definition.risk >= CapabilityRisk::High {
            let request_id = Uuid::new_v4().to_string();
            let request = grpc::AgentToolApprovalRequestEvent {
                request_id: request_id.clone(),
                command_id: self.command_id.clone(),
                task_id: self.task_id.clone(),
                tool_call_id: call.call_id.clone(),
                tool_name: call.name.clone(),
                risk: format!("{:?}", definition.risk),
                summary: tool_approval_summary(&call),
                arguments_json: call.arguments.to_string(),
            };
            if self
                .sender
                .send(
                    self.agent
                        .agent_tool_approval_request_event(self.agent_id, request),
                )
                .await
                .is_err()
            {
                return Err(AgentError::Tool {
                    name: call.name,
                    message: "failed to send tool approval request".to_string(),
                });
            }
            let decision = self
                .command_state
                .wait_for_tool_approval(request_id, self.approval_timeout)
                .await?;
            if !decision.approved {
                return Err(AgentError::ApprovalDenied {
                    name: call.name,
                    message: if decision.message.trim().is_empty() {
                        "approval denied".to_string()
                    } else {
                        decision.message
                    },
                });
            }
            decision.step_id
        } else {
            String::new()
        };

        if !step_id.is_empty() {
            self.send_tool_progress(&step_id, "running", "tool execution started", json!({}))
                .await;
        }

        let execution_timeout = if call.name == "run_shell" {
            self.shell_timeout + Duration::from_secs(2)
        } else {
            self.tool_timeout
        };
        let execution = tokio::time::timeout(
            execution_timeout,
            self.execute_approved_tool(call.clone(), definition),
        )
        .await;
        let result = match execution {
            Ok(result) => result,
            Err(_) => AgentToolResult {
                status: AgentToolResultStatus::Failed,
                output: json!({
                    "error": "tool execution timed out",
                    "timeout_seconds": execution_timeout.as_secs(),
                }),
            },
        };

        if !step_id.is_empty() {
            let status = match result.status {
                AgentToolResultStatus::Succeeded => "succeeded",
                AgentToolResultStatus::Failed => "failed",
            };
            self.send_tool_progress(
                &step_id,
                status,
                "tool execution finished",
                result.output.clone(),
            )
            .await;
        }

        Ok(result)
    }
}

impl LocalAgentToolExecutor {
    async fn execute_approved_tool(
        &self,
        call: AgentToolCall,
        _definition: &AgentToolDefinition,
    ) -> AgentToolResult {
        match call.name.as_str() {
            "host_metrics" => value_tool_result(
                serde_json::to_value(self.agent.metrics()).map_err(anyhow::Error::from),
            ),
            "list_directory" => {
                let path = required_argument(&call.arguments, "path");
                file_output_tool_result(path.and_then(|path| filesystem::list_directory(&path)))
            }
            "read_file" => {
                let path = required_argument(&call.arguments, "path");
                match path.and_then(|path| filesystem::read_file(&path, MAX_FILE_TRANSFER_BYTES)) {
                    Ok(output) => {
                        let content = String::from_utf8_lossy(&output.content).into_owned();
                        AgentToolResult {
                            status: AgentToolResultStatus::Succeeded,
                            output: json!({
                                "message": output.message,
                                "metadata": parse_json_value(&output.result_json),
                                "content": content,
                            }),
                        }
                    }
                    Err(error) => failed_tool_result(error),
                }
            }
            "search_files" => {
                let path = required_argument(&call.arguments, "path");
                let query = required_argument(&call.arguments, "query");
                let limit = call
                    .arguments
                    .get("limit")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .unwrap_or(500)
                    .min(500);
                file_output_tool_result(
                    path.and_then(|path| query.map(|query| (path, query)))
                        .and_then(|(path, query)| filesystem::search_files(&path, &query, limit)),
                )
            }
            "run_shell" => self.run_shell_tool(&call).await,
            "write_file" => self.write_file_tool(&call),
            "file_operation" => self.file_operation_tool(&call),
            "container_snapshot" => self.container_snapshot_tool().await,
            "virtual_machine_snapshot" => self.virtual_machine_snapshot_tool().await,
            other => AgentToolResult {
                status: AgentToolResultStatus::Failed,
                output: json!({ "error": format!("unsupported tool: {other}") }),
            },
        }
    }

    async fn run_shell_tool(&self, call: &AgentToolCall) -> AgentToolResult {
        let input = match required_argument(&call.arguments, "input") {
            Ok(input) => input,
            Err(error) => return failed_tool_result(error),
        };
        let timeout = call
            .arguments
            .get("timeout_seconds")
            .and_then(Value::as_u64)
            .map(Duration::from_secs)
            .unwrap_or(self.shell_timeout)
            .min(self.shell_timeout);
        match self
            .terminal
            .execute(TerminalCommand {
                command_id: call.call_id.clone(),
                input,
                cols: 100,
                rows: 30,
                timeout,
            })
            .await
        {
            Ok(output) => AgentToolResult {
                status: if output.exit_code == Some(0) && !output.timed_out {
                    AgentToolResultStatus::Succeeded
                } else {
                    AgentToolResultStatus::Failed
                },
                output: json!({
                    "output": output.output,
                    "exit_code": output.exit_code,
                    "timed_out": output.timed_out,
                    "started_at": output.started_at,
                    "finished_at": output.finished_at,
                }),
            },
            Err(error) => failed_tool_result(error),
        }
    }

    fn write_file_tool(&self, call: &AgentToolCall) -> AgentToolResult {
        let path = match required_argument(&call.arguments, "path") {
            Ok(path) => path,
            Err(error) => return failed_tool_result(error),
        };
        let content = match required_argument(&call.arguments, "content") {
            Ok(content) => content,
            Err(error) => return failed_tool_result(error),
        };
        let overwrite = call
            .arguments
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let command = grpc::RunFileOperationCommand {
            command_id: call.call_id.clone(),
            operation: "upload".to_string(),
            path,
            target_path: String::new(),
            name: String::new(),
            content: content.into_bytes(),
            overwrite,
        };
        file_output_tool_result(filesystem::run_operation(command, MAX_FILE_TRANSFER_BYTES))
    }

    fn file_operation_tool(&self, call: &AgentToolCall) -> AgentToolResult {
        let operation = match required_argument(&call.arguments, "operation") {
            Ok(operation) => operation,
            Err(error) => return failed_tool_result(error),
        };
        let path = match required_argument(&call.arguments, "path") {
            Ok(path) => path,
            Err(error) => return failed_tool_result(error),
        };
        let command = grpc::RunFileOperationCommand {
            command_id: call.call_id.clone(),
            operation,
            path,
            target_path: call
                .arguments
                .get("target_path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            name: call
                .arguments
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            content: call
                .arguments
                .get("content_base64")
                .and_then(Value::as_str)
                .and_then(|content| STANDARD.decode(content.as_bytes()).ok())
                .unwrap_or_default(),
            overwrite: call
                .arguments
                .get("overwrite")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        file_output_tool_result(filesystem::run_operation(command, MAX_FILE_TRANSFER_BYTES))
    }

    async fn container_snapshot_tool(&self) -> AgentToolResult {
        let Some(runtime) = &self.agent.container_runtime else {
            return AgentToolResult {
                status: AgentToolResultStatus::Failed,
                output: json!({ "error": "container runtime is not enabled" }),
            };
        };
        value_tool_result(
            runtime
                .snapshot()
                .await
                .map_err(anyhow::Error::from)
                .and_then(|snapshot| serde_json::to_value(snapshot).map_err(anyhow::Error::from)),
        )
    }

    async fn virtual_machine_snapshot_tool(&self) -> AgentToolResult {
        let Some(runtime) = &self.agent.vm_runtime else {
            return AgentToolResult {
                status: AgentToolResultStatus::Failed,
                output: json!({ "error": "virtual machine provider is not enabled" }),
            };
        };
        value_tool_result(
            runtime
                .provider
                .list()
                .await
                .map_err(anyhow::Error::from)
                .and_then(|states| serde_json::to_value(states).map_err(anyhow::Error::from)),
        )
    }

    async fn send_tool_progress(&self, step_id: &str, status: &str, message: &str, details: Value) {
        let event = self.agent.agent_task_progress_event(
            self.agent_id,
            grpc::AgentTaskProgressEvent {
                command_id: self.command_id.clone(),
                task_id: self.task_id.clone(),
                step_id: step_id.to_string(),
                status: status.to_string(),
                message: message.to_string(),
                details_json: details.to_string(),
            },
        );
        if self.sender.send(event).await.is_err() {
            tracing::warn!("failed to enqueue agent task progress event");
        }
    }
}

fn tool_approval_summary(call: &AgentToolCall) -> String {
    match call.name.as_str() {
        "run_shell" => call
            .arguments
            .get("input")
            .and_then(Value::as_str)
            .map(|input| format!("Run shell command: {input}"))
            .unwrap_or_else(|| "Run shell command".to_string()),
        "write_file" => call
            .arguments
            .get("path")
            .and_then(Value::as_str)
            .map(|path| format!("Write file {path}"))
            .unwrap_or_else(|| "Write file".to_string()),
        "file_operation" => {
            let operation = call
                .arguments
                .get("operation")
                .and_then(Value::as_str)
                .unwrap_or("file_operation");
            let path = call
                .arguments
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!("Run {operation} on {path}")
        }
        other => format!("Run high-risk AI tool {other}"),
    }
}

fn required_argument(arguments: &Value, name: &str) -> anyhow::Result<String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("tool argument {name} is required"))
}

fn file_output_tool_result(
    output: anyhow::Result<filesystem::FileCommandOutput>,
) -> AgentToolResult {
    match output {
        Ok(output) => AgentToolResult {
            status: AgentToolResultStatus::Succeeded,
            output: json!({
                "message": output.message,
                "result": parse_json_value(&output.result_json),
                "content_bytes": output.content.len(),
            }),
        },
        Err(error) => failed_tool_result(error),
    }
}

fn value_tool_result(output: anyhow::Result<Value>) -> AgentToolResult {
    match output {
        Ok(output) => AgentToolResult {
            status: AgentToolResultStatus::Succeeded,
            output,
        },
        Err(error) => failed_tool_result(error),
    }
}

fn failed_tool_result(error: impl std::fmt::Display) -> AgentToolResult {
    AgentToolResult {
        status: AgentToolResultStatus::Failed,
        output: json!({ "error": error.to_string() }),
    }
}

pub(crate) fn parse_json_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| json!({ "raw": value }))
}
