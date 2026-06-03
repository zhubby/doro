use crate::agent_streams::AgentStreamRegistry;
use crate::auth::AuthService;
use crate::chat_streams::ChatStreamHub;
use crate::logs::LogHub;
use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct AppState {
    pub(crate) store: Store,
    pub(crate) auth: AuthService,
    pub(crate) agent_streams: AgentStreamRegistry,
    pub(crate) chat_streams: ChatStreamHub,
    pub(crate) logs: LogHub,
    pub(crate) control_plane_environment: ControlPlaneEnvironment,
}
