use crate::openai;
use async_trait::async_trait;
use doro_protocol::CapabilityRisk;
use futures_util::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

const DEFAULT_AGENT_INSTRUCTIONS: &str = r#"You are a Doro host operations agent.
Use the available tools to inspect and operate only the current enrolled host.
Prefer read-only inspection before mutation. For risky work, explain the exact action in the tool summary because Doro policy and approval decide execution.
Return a concise operational result with what changed, what failed, and anything the operator must review."#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentRunnerConfig {
    pub max_turns: u32,
    pub max_tool_calls: u32,
}

impl Default for AgentRunnerConfig {
    fn default() -> Self {
        Self {
            max_turns: 12,
            max_tool_calls: 32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRunRequest {
    pub prompt: String,
    pub context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRunOutcome {
    pub status: AgentRunStatus,
    pub summary: String,
    pub transcript: Vec<AgentTranscriptItem>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentTranscriptItem {
    ModelText {
        text: String,
    },
    ToolCall {
        call_id: String,
        name: String,
        arguments: Value,
    },
    ToolResult {
        call_id: String,
        name: String,
        status: AgentToolResultStatus,
        output: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentToolDefinition {
    pub name: String,
    pub description: String,
    pub risk: CapabilityRisk,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentToolResult {
    pub status: AgentToolResultStatus,
    pub output: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolResultStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone)]
pub struct AgentModelRequest {
    pub instructions: String,
    pub input: Vec<Value>,
    pub tools: Vec<AgentToolDefinition>,
}

#[derive(Debug, Clone)]
pub struct AgentModelResponse {
    pub raw_output: Vec<Value>,
    pub tool_calls: Vec<AgentToolCall>,
    pub final_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentRunEvent {
    TextDelta {
        delta: String,
    },
    ToolCall {
        call_id: String,
        name: String,
        arguments: Value,
    },
    ToolResult {
        call_id: String,
        name: String,
        status: AgentToolResultStatus,
        output: Value,
    },
}

#[async_trait]
pub trait AgentRunEventSink: Send + Sync {
    async fn emit(&self, event: AgentRunEvent) -> Result<(), AgentError>;
}

#[async_trait]
pub trait AgentModelProvider: Send + Sync {
    async fn respond(&self, request: AgentModelRequest) -> Result<AgentModelResponse, AgentError>;

    async fn respond_stream(
        &self,
        request: AgentModelRequest,
        sink: &dyn AgentRunEventSink,
    ) -> Result<AgentModelResponse, AgentError> {
        let response = self.respond(request).await?;
        if let Some(text) = response.final_text.as_ref()
            && !text.trim().is_empty()
        {
            sink.emit(AgentRunEvent::TextDelta {
                delta: text.clone(),
            })
            .await?;
        }
        Ok(response)
    }
}

#[async_trait]
pub trait AgentToolExecutor: Send + Sync {
    async fn execute(
        &self,
        call: AgentToolCall,
        definition: &AgentToolDefinition,
    ) -> Result<AgentToolResult, AgentError>;
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("AI provider is disabled")]
    ProviderDisabled,
    #[error("AI model request failed: {0}")]
    Model(String),
    #[error("AI tool {name} failed: {message}")]
    Tool { name: String, message: String },
    #[error("AI tool approval denied for {name}: {message}")]
    ApprovalDenied { name: String, message: String },
    #[error("AI agent exceeded max turns ({0})")]
    MaxTurns(u32),
    #[error("AI agent exceeded max tool calls ({0})")]
    MaxToolCalls(u32),
}

#[derive(Clone)]
pub struct AgentRunner {
    provider: Arc<dyn AgentModelProvider>,
    tools: Vec<AgentToolDefinition>,
    config: AgentRunnerConfig,
    instructions: String,
}

impl std::fmt::Debug for AgentRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentRunner")
            .field("tools", &self.tools)
            .field("config", &self.config)
            .field("instructions", &self.instructions)
            .finish_non_exhaustive()
    }
}

impl AgentRunner {
    pub fn new(
        provider: Arc<dyn AgentModelProvider>,
        tools: Vec<AgentToolDefinition>,
        config: AgentRunnerConfig,
    ) -> Self {
        Self {
            provider,
            tools,
            config,
            instructions: DEFAULT_AGENT_INSTRUCTIONS.to_string(),
        }
    }

    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = instructions.into();
        self
    }

    pub async fn run(
        &self,
        request: AgentRunRequest,
        executor: &dyn AgentToolExecutor,
    ) -> Result<AgentRunOutcome, AgentError> {
        let mut input = vec![user_input_item(&request.prompt, &request.context)];
        let tools_by_name = self
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool))
            .collect::<HashMap<_, _>>();
        let mut transcript = Vec::new();
        let mut tool_call_count = 0_u32;

        for _ in 0..self.config.max_turns {
            let response = self
                .provider
                .respond(AgentModelRequest {
                    instructions: self.instructions.clone(),
                    input: input.clone(),
                    tools: self.tools.clone(),
                })
                .await?;

            if let Some(text) = response.final_text.clone()
                && !text.trim().is_empty()
            {
                transcript.push(AgentTranscriptItem::ModelText { text });
            }
            input.extend(response.raw_output);

            if response.tool_calls.is_empty() {
                let summary = response
                    .final_text
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| "AI agent completed without additional output".to_string());
                return Ok(AgentRunOutcome {
                    status: AgentRunStatus::Succeeded,
                    summary,
                    transcript,
                });
            }

            for call in response.tool_calls {
                tool_call_count += 1;
                if tool_call_count > self.config.max_tool_calls {
                    return Err(AgentError::MaxToolCalls(self.config.max_tool_calls));
                }

                transcript.push(AgentTranscriptItem::ToolCall {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                });

                let Some(definition) = tools_by_name.get(call.name.as_str()).copied() else {
                    let result = AgentToolResult {
                        status: AgentToolResultStatus::Failed,
                        output: json!({
                            "error": format!("unknown Doro agent tool: {}", call.name),
                        }),
                    };
                    input.push(function_call_output(&call.call_id, &result.output));
                    transcript.push(AgentTranscriptItem::ToolResult {
                        call_id: call.call_id,
                        name: call.name,
                        status: result.status,
                        output: result.output,
                    });
                    continue;
                };

                let result = executor.execute(call.clone(), definition).await?;
                input.push(function_call_output(&call.call_id, &result.output));
                transcript.push(AgentTranscriptItem::ToolResult {
                    call_id: call.call_id,
                    name: call.name,
                    status: result.status,
                    output: result.output,
                });
            }
        }

        Err(AgentError::MaxTurns(self.config.max_turns))
    }

    pub async fn run_streaming(
        &self,
        request: AgentRunRequest,
        executor: &dyn AgentToolExecutor,
        sink: &dyn AgentRunEventSink,
    ) -> Result<AgentRunOutcome, AgentError> {
        let mut input = vec![user_input_item(&request.prompt, &request.context)];
        let tools_by_name = self
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool))
            .collect::<HashMap<_, _>>();
        let mut transcript = Vec::new();
        let mut tool_call_count = 0_u32;

        for _ in 0..self.config.max_turns {
            let response = self
                .provider
                .respond_stream(
                    AgentModelRequest {
                        instructions: self.instructions.clone(),
                        input: input.clone(),
                        tools: self.tools.clone(),
                    },
                    sink,
                )
                .await?;

            if let Some(text) = response.final_text.clone()
                && !text.trim().is_empty()
            {
                transcript.push(AgentTranscriptItem::ModelText { text });
            }
            input.extend(response.raw_output);

            if response.tool_calls.is_empty() {
                let summary = response
                    .final_text
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| "AI agent completed without additional output".to_string());
                return Ok(AgentRunOutcome {
                    status: AgentRunStatus::Succeeded,
                    summary,
                    transcript,
                });
            }

            for call in response.tool_calls {
                tool_call_count += 1;
                if tool_call_count > self.config.max_tool_calls {
                    return Err(AgentError::MaxToolCalls(self.config.max_tool_calls));
                }

                sink.emit(AgentRunEvent::ToolCall {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                })
                .await?;
                transcript.push(AgentTranscriptItem::ToolCall {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                });

                let Some(definition) = tools_by_name.get(call.name.as_str()).copied() else {
                    let result = AgentToolResult {
                        status: AgentToolResultStatus::Failed,
                        output: json!({
                            "error": format!("unknown Doro agent tool: {}", call.name),
                        }),
                    };
                    sink.emit(AgentRunEvent::ToolResult {
                        call_id: call.call_id.clone(),
                        name: call.name.clone(),
                        status: result.status,
                        output: result.output.clone(),
                    })
                    .await?;
                    input.push(function_call_output(&call.call_id, &result.output));
                    transcript.push(AgentTranscriptItem::ToolResult {
                        call_id: call.call_id,
                        name: call.name,
                        status: result.status,
                        output: result.output,
                    });
                    continue;
                };

                let result = executor.execute(call.clone(), definition).await?;
                sink.emit(AgentRunEvent::ToolResult {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    status: result.status,
                    output: result.output.clone(),
                })
                .await?;
                input.push(function_call_output(&call.call_id, &result.output));
                transcript.push(AgentTranscriptItem::ToolResult {
                    call_id: call.call_id,
                    name: call.name,
                    status: result.status,
                    output: result.output,
                });
            }
        }

        Err(AgentError::MaxTurns(self.config.max_turns))
    }
}

#[derive(Debug, Clone)]
pub struct DisabledAgentProvider;

#[async_trait]
impl AgentModelProvider for DisabledAgentProvider {
    async fn respond(&self, _request: AgentModelRequest) -> Result<AgentModelResponse, AgentError> {
        Err(AgentError::ProviderDisabled)
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiAgentProvider {
    client: openai::OpenAiClient,
    model: String,
}

impl OpenAiAgentProvider {
    pub fn new(client: openai::OpenAiClient, model: impl Into<String>) -> Self {
        Self {
            client,
            model: model.into(),
        }
    }
}

#[async_trait]
impl AgentModelProvider for OpenAiAgentProvider {
    async fn respond(&self, request: AgentModelRequest) -> Result<AgentModelResponse, AgentError> {
        let mut response_request =
            openai::ResponseRequest::items(self.model.clone(), request.input);
        response_request.instructions = Some(request.instructions);
        response_request
            .extra
            .insert("tools".to_string(), tools_to_openai(&request.tools));
        response_request
            .extra
            .insert("tool_choice".to_string(), Value::String("auto".to_string()));

        let response = self
            .client
            .create_response(response_request)
            .await
            .map_err(|error| AgentError::Model(error.to_string()))?;

        Ok(model_response_from_openai(response))
    }

    async fn respond_stream(
        &self,
        request: AgentModelRequest,
        sink: &dyn AgentRunEventSink,
    ) -> Result<AgentModelResponse, AgentError> {
        let mut response_request =
            openai::ResponseRequest::items(self.model.clone(), request.input);
        response_request.instructions = Some(request.instructions);
        response_request
            .extra
            .insert("tools".to_string(), tools_to_openai(&request.tools));
        response_request
            .extra
            .insert("tool_choice".to_string(), Value::String("auto".to_string()));

        let mut stream = self
            .client
            .stream_response(response_request)
            .await
            .map_err(|error| AgentError::Model(error.to_string()))?;
        let mut raw_output = Vec::new();
        let mut tool_calls = Vec::new();
        let mut final_text = String::new();

        while let Some(event) = stream.next().await {
            match event.map_err(|error| AgentError::Model(error.to_string()))? {
                openai::OpenAiStreamEvent::Response(event) => {
                    if event.event_type == "response.output_text.delta"
                        && let Some(delta) = event.payload.get("delta").and_then(Value::as_str)
                        && !delta.is_empty()
                    {
                        final_text.push_str(delta);
                        sink.emit(AgentRunEvent::TextDelta {
                            delta: delta.to_string(),
                        })
                        .await?;
                    }

                    if event.event_type == "response.output_item.done"
                        && let Some(item) = event.payload.get("item")
                    {
                        raw_output.push(item.clone());
                        if let Some(call) = tool_call_from_openai_value(item) {
                            tool_calls.push(call);
                        }
                    }

                    if event.event_type == "response.completed"
                        && let Some(response) = event.payload.get("response")
                        && let Ok(response) =
                            serde_json::from_value::<openai::ResponseObject>(response.clone())
                    {
                        let parsed = model_response_from_openai(response);
                        if final_text.is_empty()
                            && let Some(text) = parsed.final_text.as_ref()
                        {
                            final_text.push_str(text);
                            if !text.trim().is_empty() {
                                sink.emit(AgentRunEvent::TextDelta {
                                    delta: text.clone(),
                                })
                                .await?;
                            }
                        }
                        if raw_output.is_empty() {
                            raw_output = parsed.raw_output;
                        }
                        if tool_calls.is_empty() {
                            tool_calls = parsed.tool_calls;
                        }
                    }
                }
                openai::OpenAiStreamEvent::Done => break,
                openai::OpenAiStreamEvent::Chat(_) => {}
            }
        }

        Ok(AgentModelResponse {
            raw_output,
            tool_calls,
            final_text: if final_text.trim().is_empty() {
                None
            } else {
                Some(final_text)
            },
        })
    }
}

fn user_input_item(prompt: &str, context: &Value) -> Value {
    json!({
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": format!(
                "{}\n\nDoro context JSON:\n{}",
                prompt.trim(),
                context
            )
        }]
    })
}

fn function_call_output(call_id: &str, output: &Value) -> Value {
    json!({
        "type": "function_call_output",
        "call_id": call_id,
        "output": output.to_string(),
    })
}

fn tools_to_openai(tools: &[AgentToolDefinition]) -> Value {
    Value::Array(
        tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                })
            })
            .collect(),
    )
}

fn model_response_from_openai(response: openai::ResponseObject) -> AgentModelResponse {
    let final_text = response
        .extra
        .get("output_text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| response_output_text(&response.output));
    let raw_output = response
        .output
        .iter()
        .filter_map(|item| serde_json::to_value(item).ok())
        .collect::<Vec<_>>();
    let tool_calls = response
        .output
        .iter()
        .filter_map(tool_call_from_openai_item)
        .collect();

    AgentModelResponse {
        raw_output,
        tool_calls,
        final_text,
    }
}

fn response_output_text(items: &[openai::ResponseOutputItem]) -> Option<String> {
    let text = items
        .iter()
        .flat_map(|item| &item.content)
        .filter_map(|content| content.text.as_deref())
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() { None } else { Some(text) }
}

fn tool_call_from_openai_item(item: &openai::ResponseOutputItem) -> Option<AgentToolCall> {
    if item.item_type != "function_call" {
        return None;
    }

    let name = item.extra.get("name").and_then(Value::as_str)?.to_string();
    let call_id = item
        .extra
        .get("call_id")
        .and_then(Value::as_str)
        .unwrap_or(&item.id)
        .to_string();
    let arguments = match item.extra.get("arguments") {
        Some(Value::String(arguments)) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| json!({ "raw": arguments }))
        }
        Some(value) => value.clone(),
        None => json!({}),
    };

    Some(AgentToolCall {
        call_id,
        name,
        arguments,
    })
}

fn tool_call_from_openai_value(item: &Value) -> Option<AgentToolCall> {
    if item.get("type").and_then(Value::as_str) != Some("function_call") {
        return None;
    }
    let name = item.get("name").and_then(Value::as_str)?.to_string();
    let call_id = item
        .get("call_id")
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)?
        .to_string();
    let arguments = match item.get("arguments") {
        Some(Value::String(arguments)) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| json!({ "raw": arguments }))
        }
        Some(value) => value.clone(),
        None => json!({}),
    };
    Some(AgentToolCall {
        call_id,
        name,
        arguments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct ScriptedProvider {
        responses: Mutex<Vec<AgentModelResponse>>,
    }

    #[async_trait]
    impl AgentModelProvider for ScriptedProvider {
        async fn respond(
            &self,
            _request: AgentModelRequest,
        ) -> Result<AgentModelResponse, AgentError> {
            let mut responses = self
                .responses
                .lock()
                .map_err(|_| AgentError::Model("lock".into()))?;
            if responses.is_empty() {
                return Err(AgentError::Model("no scripted response".to_string()));
            }
            Ok(responses.remove(0))
        }
    }

    #[derive(Debug)]
    struct EchoExecutor;

    #[async_trait]
    impl AgentToolExecutor for EchoExecutor {
        async fn execute(
            &self,
            call: AgentToolCall,
            _definition: &AgentToolDefinition,
        ) -> Result<AgentToolResult, AgentError> {
            Ok(AgentToolResult {
                status: AgentToolResultStatus::Succeeded,
                output: json!({ "tool": call.name, "arguments": call.arguments }),
            })
        }
    }

    #[derive(Debug, Default)]
    struct RecordingSink {
        events: Mutex<Vec<AgentRunEvent>>,
    }

    #[async_trait]
    impl AgentRunEventSink for RecordingSink {
        async fn emit(&self, event: AgentRunEvent) -> Result<(), AgentError> {
            self.events
                .lock()
                .map_err(|_| AgentError::Model("lock".into()))?
                .push(event);
            Ok(())
        }
    }

    #[tokio::test]
    async fn runner_returns_final_text_without_tools() -> Result<(), Box<dyn std::error::Error>> {
        let provider = ScriptedProvider {
            responses: Mutex::new(vec![AgentModelResponse {
                raw_output: Vec::new(),
                tool_calls: Vec::new(),
                final_text: Some("done".to_string()),
            }]),
        };
        let runner = AgentRunner::new(Arc::new(provider), Vec::new(), AgentRunnerConfig::default());

        let result = runner
            .run(
                AgentRunRequest {
                    prompt: "check host".to_string(),
                    context: json!({}),
                },
                &EchoExecutor,
            )
            .await?;

        assert_eq!(result.status, AgentRunStatus::Succeeded);
        assert_eq!(result.summary, "done");
        Ok(())
    }

    #[tokio::test]
    async fn runner_executes_tool_calls_and_continues() -> Result<(), Box<dyn std::error::Error>> {
        let provider = ScriptedProvider {
            responses: Mutex::new(vec![
                AgentModelResponse {
                    raw_output: vec![json!({"type": "function_call", "name": "host_metrics"})],
                    tool_calls: vec![AgentToolCall {
                        call_id: "call-1".to_string(),
                        name: "host_metrics".to_string(),
                        arguments: json!({}),
                    }],
                    final_text: None,
                },
                AgentModelResponse {
                    raw_output: Vec::new(),
                    tool_calls: Vec::new(),
                    final_text: Some("metrics collected".to_string()),
                },
            ]),
        };
        let tools = vec![AgentToolDefinition {
            name: "host_metrics".to_string(),
            description: "Read metrics".to_string(),
            risk: CapabilityRisk::Low,
            parameters: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }];
        let runner = AgentRunner::new(Arc::new(provider), tools, AgentRunnerConfig::default());

        let result = runner
            .run(
                AgentRunRequest {
                    prompt: "check host".to_string(),
                    context: json!({}),
                },
                &EchoExecutor,
            )
            .await?;

        assert_eq!(result.summary, "metrics collected");
        assert!(matches!(
            result.transcript.first(),
            Some(AgentTranscriptItem::ToolCall { name, .. }) if name == "host_metrics"
        ));
        Ok(())
    }

    #[tokio::test]
    async fn streaming_runner_emits_tool_and_text_events() -> Result<(), Box<dyn std::error::Error>>
    {
        let provider = ScriptedProvider {
            responses: Mutex::new(vec![
                AgentModelResponse {
                    raw_output: vec![json!({"type": "function_call", "name": "host_metrics"})],
                    tool_calls: vec![AgentToolCall {
                        call_id: "call-1".to_string(),
                        name: "host_metrics".to_string(),
                        arguments: json!({}),
                    }],
                    final_text: None,
                },
                AgentModelResponse {
                    raw_output: Vec::new(),
                    tool_calls: Vec::new(),
                    final_text: Some("metrics collected".to_string()),
                },
            ]),
        };
        let tools = vec![AgentToolDefinition {
            name: "host_metrics".to_string(),
            description: "Read metrics".to_string(),
            risk: CapabilityRisk::Low,
            parameters: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        }];
        let runner = AgentRunner::new(Arc::new(provider), tools, AgentRunnerConfig::default());
        let sink = RecordingSink::default();

        let result = runner
            .run_streaming(
                AgentRunRequest {
                    prompt: "check host".to_string(),
                    context: json!({}),
                },
                &EchoExecutor,
                &sink,
            )
            .await?;

        let events = sink.events.lock().map_err(|_| "lock")?;
        assert_eq!(result.summary, "metrics collected");
        assert!(matches!(
            events.first(),
            Some(AgentRunEvent::ToolCall { name, .. }) if name == "host_metrics"
        ));
        assert!(events.iter().any(|event| matches!(
            event,
            AgentRunEvent::ToolResult { name, status, .. }
                if name == "host_metrics" && *status == AgentToolResultStatus::Succeeded
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            AgentRunEvent::TextDelta { delta } if delta == "metrics collected"
        )));
        Ok(())
    }
}
