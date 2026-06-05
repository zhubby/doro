mod agent_events;
mod agent_grpc;
mod agent_streams;
mod agent_tools;
mod alerts;
mod auth;
mod chat_streams;
mod constants;
mod error;
mod logs;
mod notifications;
mod prelude;
mod routes;
mod server;
mod startup;
mod state;

pub use agent_grpc::GrpcAgentService;
pub use agent_streams::AgentStreamRegistry;
pub use auth::{AuthService, CurrentUser};
pub use error::AppError;
pub use logs::{LogHub, publish_control_plane_runtime_log};
pub use routes::{
    app, app_with_auth, app_with_auth_and_streams, app_with_auth_streams_and_websites,
};
pub use server::{run, run_loaded};
pub use state::AppState;

use chrono::Utc;
use doro_protocol::MetricSnapshot;
use uuid::Uuid;

pub fn example_metric(host_id: Uuid) -> MetricSnapshot {
    MetricSnapshot {
        host_id,
        captured_at: Utc::now(),
        cpu_percent: 0.0,
        memory_percent: 0.0,
        disk_percent: 0.0,
        load_average: 0.0,
        extra: serde_json::json!({}),
    }
}
