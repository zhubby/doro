use futures_util::Stream;
use futures_util::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::env;
use std::pin::Pin;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_TIMEOUT_SECONDS: u64 = 60;

pub type OpenAiStream =
    Pin<Box<dyn Stream<Item = Result<OpenAiStreamEvent, OpenAiError>> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiConfig {
    pub api_key_env: String,
    pub base_url: String,
    pub timeout_seconds: u64,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key_env: "OPENAI_API_KEY".to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl OpenAiClient {
    pub fn new(config: OpenAiConfig) -> Result<Self, OpenAiError> {
        let api_key = env::var(&config.api_key_env)
            .map_err(|_| OpenAiError::MissingApiKeyEnv(config.api_key_env.clone()))?;
        Self::with_api_key(config, api_key)
    }

    pub fn with_api_key(
        config: OpenAiConfig,
        api_key: impl Into<String>,
    ) -> Result<Self, OpenAiError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()?;
        Ok(Self {
            http,
            api_key: api_key.into(),
            base_url: trim_base_url(config.base_url),
        })
    }

    pub async fn create_chat_completion(
        &self,
        mut request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, OpenAiError> {
        request.stream = None;
        self.post_json("chat/completions", &request).await
    }

    pub async fn stream_chat_completion(
        &self,
        mut request: ChatCompletionRequest,
    ) -> Result<OpenAiStream, OpenAiError> {
        request.stream = Some(true);
        self.post_stream("chat/completions", &request, StreamKind::Chat)
            .await
    }

    pub async fn create_response(
        &self,
        mut request: ResponseRequest,
    ) -> Result<ResponseObject, OpenAiError> {
        request.stream = None;
        self.post_json("responses", &request).await
    }

    pub async fn stream_response(
        &self,
        mut request: ResponseRequest,
    ) -> Result<OpenAiStream, OpenAiError> {
        request.stream = Some(true);
        self.post_stream("responses", &request, StreamKind::Response)
            .await
    }

    async fn post_json<T, R>(&self, path: &str, request: &T) -> Result<R, OpenAiError>
    where
        T: Serialize + ?Sized,
        R: for<'de> Deserialize<'de>,
    {
        let response = self
            .http
            .post(self.url(path))
            .bearer_auth(&self.api_key)
            .json(request)
            .send()
            .await?;
        parse_json_response(response).await
    }

    async fn post_stream<T>(
        &self,
        path: &str,
        request: &T,
        kind: StreamKind,
    ) -> Result<OpenAiStream, OpenAiError>
    where
        T: Serialize + ?Sized,
    {
        let response = self
            .http
            .post(self.url(path))
            .bearer_auth(&self.api_key)
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(api_error_from_response(response).await);
        }

        let mut parser = SseParser::new(kind);
        Ok(Box::pin(response.bytes_stream().flat_map(move |chunk| {
            let events = match chunk {
                Ok(bytes) => parser.push(&String::from_utf8_lossy(&bytes)),
                Err(error) => vec![Err(OpenAiError::Http(error))],
            };
            futures_util::stream::iter(events)
        })))
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }
}

#[derive(Debug, Error)]
pub enum OpenAiError {
    #[error("OpenAI API key environment variable {0} is not set")]
    MissingApiKeyEnv(String),
    #[error("OpenAI HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("OpenAI API request failed with status {status}: {message}")]
    Api { status: u16, message: String },
    #[error("failed to parse OpenAI response JSON")]
    Json(#[source] serde_json::Error),
    #[error("failed to parse OpenAI stream event: {0}")]
    Stream(String),
    #[error("OpenAI stream ended with incomplete event data")]
    IncompleteStreamEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ChatCompletionRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_tokens: None,
            stream: None,
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    #[serde(default)]
    pub usage: Option<OpenAiUsage>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCompletionChoice {
    pub index: u32,
    pub message: ChatMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChunkChoice>,
    #[serde(default)]
    pub usage: Option<OpenAiUsage>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCompletionChunkChoice {
    pub index: u32,
    pub delta: ChatCompletionDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatCompletionDelta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseRequest {
    pub model: String,
    pub input: ResponseInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ResponseRequest {
    pub fn text(model: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            input: ResponseInput::Text(input.into()),
            instructions: None,
            temperature: None,
            max_output_tokens: None,
            stream: None,
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ResponseInput {
    Text(String),
    Items(Vec<ResponseInputItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseInputItem {
    pub role: String,
    pub content: ResponseInputContent,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ResponseInputContent {
    Text(String),
    Items(Vec<Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseObject {
    pub id: String,
    pub object: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub output: Vec<ResponseOutputItem>,
    #[serde(default)]
    pub usage: Option<OpenAiUsage>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseOutputItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Vec<ResponseOutputContent>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseOutputContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(flatten)]
    pub payload: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenAiUsage {
    #[serde(default)]
    pub prompt_tokens: Option<u32>,
    #[serde(default)]
    pub completion_tokens: Option<u32>,
    #[serde(default)]
    pub total_tokens: Option<u32>,
    #[serde(default)]
    pub input_tokens: Option<u32>,
    #[serde(default)]
    pub output_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpenAiStreamEvent {
    Chat(ChatCompletionChunk),
    Response(ResponseStreamEvent),
    Done,
}

#[derive(Debug, Clone, Copy)]
enum StreamKind {
    Chat,
    Response,
}

struct SseParser {
    kind: StreamKind,
    buffer: String,
}

impl SseParser {
    fn new(kind: StreamKind) -> Self {
        Self {
            kind,
            buffer: String::new(),
        }
    }

    fn push(&mut self, chunk: &str) -> Vec<Result<OpenAiStreamEvent, OpenAiError>> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();

        while let Some(index) = self.buffer.find("\n\n") {
            let frame = self.buffer[..index].to_string();
            self.buffer.drain(..index + 2);
            if let Some(event) = parse_sse_frame(&frame, self.kind) {
                events.push(event);
            }
        }

        events
    }

    #[cfg(test)]
    fn finish(self) -> Result<(), OpenAiError> {
        if self.buffer.trim().is_empty() {
            Ok(())
        } else {
            Err(OpenAiError::IncompleteStreamEvent)
        }
    }
}

fn parse_sse_frame(
    frame: &str,
    kind: StreamKind,
) -> Option<Result<OpenAiStreamEvent, OpenAiError>> {
    let data_lines = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
        .collect::<Vec<_>>();

    if data_lines.is_empty() {
        return None;
    }

    let data = data_lines.join("\n");
    if data.trim() == "[DONE]" {
        return Some(Ok(OpenAiStreamEvent::Done));
    }

    Some(parse_stream_event(&data, kind))
}

fn parse_stream_event(data: &str, kind: StreamKind) -> Result<OpenAiStreamEvent, OpenAiError> {
    match kind {
        StreamKind::Chat => serde_json::from_str::<ChatCompletionChunk>(data)
            .map(OpenAiStreamEvent::Chat)
            .map_err(|source| OpenAiError::Stream(source.to_string())),
        StreamKind::Response => serde_json::from_str::<ResponseStreamEvent>(data)
            .map(OpenAiStreamEvent::Response)
            .map_err(|source| OpenAiError::Stream(source.to_string())),
    }
}

async fn parse_json_response<R>(response: reqwest::Response) -> Result<R, OpenAiError>
where
    R: for<'de> Deserialize<'de>,
{
    if !response.status().is_success() {
        return Err(api_error_from_response(response).await);
    }

    let body = response.text().await?;
    serde_json::from_str::<R>(&body).map_err(OpenAiError::Json)
}

async fn api_error_from_response(response: reqwest::Response) -> OpenAiError {
    let status = response.status().as_u16();
    let message = match response.text().await {
        Ok(body) => extract_error_message(&body),
        Err(error) => error.to_string(),
    };
    OpenAiError::Api { status, message }
}

fn extract_error_message(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return body.to_string();
    };

    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn trim_base_url(base_url: String) -> String {
    base_url.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_chat_completion_stream_request() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = ChatCompletionRequest::new(
            "gpt-4.1-mini",
            vec![
                ChatMessage::system("be direct"),
                ChatMessage::user("status"),
            ],
        );
        request.stream = Some(true);

        let value = serde_json::to_value(request)?;

        assert_eq!(value["model"], "gpt-4.1-mini");
        assert_eq!(value["stream"], true);
        assert_eq!(value["messages"][0]["role"], "system");
        assert_eq!(value["messages"][1]["content"], "status");
        Ok(())
    }

    #[test]
    fn serializes_response_stream_request() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = ResponseRequest::text("gpt-4.1-mini", "summarize");
        request.stream = Some(true);

        let value = serde_json::to_value(request)?;

        assert_eq!(value["model"], "gpt-4.1-mini");
        assert_eq!(value["input"], "summarize");
        assert_eq!(value["stream"], true);
        Ok(())
    }

    #[test]
    fn parses_chat_stream_chunk_and_done() {
        let mut parser = SseParser::new(StreamKind::Chat);
        let events = parser.push(
            r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","created":1,"model":"gpt-4.1-mini","choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}

data: [DONE]

"#,
        );

        assert_eq!(events.len(), 2);
        match &events[0] {
            Ok(OpenAiStreamEvent::Chat(chunk)) => {
                assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("hi"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(matches!(events[1], Ok(OpenAiStreamEvent::Done)));
    }

    #[test]
    fn parses_response_output_text_delta_event() {
        let mut parser = SseParser::new(StreamKind::Response);
        let events = parser.push(
            r#"event: response.output_text.delta
data: {"type":"response.output_text.delta","response_id":"resp_1","item_id":"msg_1","output_index":0,"content_index":0,"delta":"hello"}

"#,
        );

        assert_eq!(events.len(), 1);
        match &events[0] {
            Ok(OpenAiStreamEvent::Response(event)) => {
                assert_eq!(event.event_type, "response.output_text.delta");
                assert_eq!(event.payload["delta"], "hello");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn reports_malformed_stream_json() {
        let mut parser = SseParser::new(StreamKind::Response);
        let events = parser.push("data: {not json}\n\n");

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Err(OpenAiError::Stream(_))));
    }

    #[test]
    fn reports_incomplete_stream_event() {
        let mut parser = SseParser::new(StreamKind::Chat);
        let events = parser.push("data: {");

        assert!(events.is_empty());
        assert!(matches!(
            parser.finish(),
            Err(OpenAiError::IncompleteStreamEvent)
        ));
    }

    #[test]
    fn extracts_api_error_message() {
        let message = extract_error_message(
            &json!({
                "error": {
                    "message": "invalid model"
                }
            })
            .to_string(),
        );

        assert_eq!(message, "invalid model");
    }
}
