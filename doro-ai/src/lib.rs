use doro_protocol::CapabilityName;
use doro_protocol::CapabilityRisk;
use doro_protocol::TaskStep;
use doro_protocol::TaskStepStatus;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

pub mod agent;
pub mod openai;

pub use agent::AgentError;
pub use agent::AgentModelProvider;
pub use agent::AgentRunEvent;
pub use agent::AgentRunEventSink;
pub use agent::AgentRunOutcome;
pub use agent::AgentRunRequest;
pub use agent::AgentRunStatus;
pub use agent::AgentRunner;
pub use agent::AgentRunnerConfig;
pub use agent::AgentToolCall;
pub use agent::AgentToolDefinition;
pub use agent::AgentToolExecutor;
pub use agent::AgentToolResult;
pub use agent::AgentToolResultStatus;
pub use agent::AgentTranscriptItem;
pub use agent::DisabledAgentProvider;
pub use agent::OpenAiAgentProvider;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiPlanRequest {
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiPlan {
    pub summary: String,
    pub steps: Vec<TaskStep>,
    pub requires_approval: bool,
}

pub trait PlanProvider {
    fn plan(&self, request: AiPlanRequest) -> anyhow::Result<AiPlan>;
}

#[derive(Debug, Default)]
pub struct DeterministicPlanner;

impl PlanProvider for DeterministicPlanner {
    fn plan(&self, request: AiPlanRequest) -> anyhow::Result<AiPlan> {
        let step = TaskStep {
            id: Uuid::new_v4(),
            capability: CapabilityName::MetricsRead,
            risk: CapabilityRisk::Low,
            summary: "Collect host status before proposing an operation".to_string(),
            status: TaskStepStatus::Pending,
            payload: json!({ "source_prompt": request.prompt }),
        };

        Ok(AiPlan {
            summary: "Create a low-risk inspection task. Execution policy still applies."
                .to_string(),
            steps: vec![step],
            requires_approval: false,
        })
    }
}
