use crate::agent_streams::AgentStreamRegistry;
use crate::auth::{
    AuthService, auth_middleware, auth_status, change_password, login, logout, me, refresh,
    register, update_me,
};
use crate::chat_streams::ChatStreamHub;
use crate::logs;
use crate::prelude::*;
use crate::state::AppState;

pub(crate) mod ai;
pub(crate) mod approvals;
pub(crate) mod files;
pub(crate) mod hosts;
pub(crate) mod scheduled_tasks;
pub(crate) mod system;
pub(crate) mod tasks;
pub(crate) mod terminal;
pub(crate) mod virtual_machines;
pub(crate) mod websites;

use ai::*;
use approvals::*;
use files::*;
use hosts::*;
use logs::*;
use scheduled_tasks::*;
use system::*;
use tasks::*;
use terminal::*;
use virtual_machines::*;
use websites::*;

pub fn app(store: Store) -> Router {
    app_with_auth(store, AuthService::development())
}

pub fn app_with_auth(store: Store, auth: AuthService) -> Router {
    app_with_auth_and_streams(
        store,
        auth,
        AgentStreamRegistry::default(),
        LogHub::default(),
    )
}

pub fn app_with_auth_and_streams(
    store: Store,
    auth: AuthService,
    agent_streams: AgentStreamRegistry,
    logs: LogHub,
) -> Router {
    app_with_auth_streams_and_websites(store, auth, agent_streams, logs)
}

pub fn app_with_auth_streams_and_websites(
    store: Store,
    auth: AuthService,
    agent_streams: AgentStreamRegistry,
    logs: LogHub,
) -> Router {
    app_with_auth_streams_logs_and_chat(store, auth, agent_streams, logs, ChatStreamHub::default())
}

pub(crate) fn app_with_auth_streams_logs_and_chat(
    store: Store,
    auth: AuthService,
    agent_streams: AgentStreamRegistry,
    logs: LogHub,
    chat_streams: ChatStreamHub,
) -> Router {
    let state = AppState {
        store,
        auth,
        agent_streams,
        chat_streams,
        logs,
        control_plane_environment: collect_control_plane_environment(),
    };

    let protected_routes = Router::new()
        .route("/api/v1/hosts", get(list_hosts))
        .route(
            "/api/v1/hosts/enrollment-token",
            axum::routing::post(create_enrollment_token),
        )
        .route(
            "/api/v1/hosts/:host_id",
            axum::routing::delete(delete_host).patch(update_host),
        )
        .route(
            "/api/v1/hosts/:host_id/metrics/latest",
            get(latest_host_metric),
        )
        .route("/api/v1/hosts/:host_id/metrics", get(list_host_metrics))
        .route(
            "/api/v1/hosts/:host_id/containers",
            get(list_host_containers),
        )
        .route("/api/v1/containers", get(refresh_containers))
        .route(
            "/api/v1/virtual-machines",
            get(refresh_virtual_machines).post(create_virtual_machine),
        )
        .route(
            "/api/v1/hosts/:host_id/virtual-machines",
            get(list_host_virtual_machines),
        )
        .route(
            "/api/v1/virtual-machines/images",
            get(list_virtual_machine_images),
        )
        .route(
            "/api/v1/virtual-machines/templates",
            get(list_virtual_machine_templates),
        )
        .route(
            "/api/v1/virtual-machines/:vm_id/start",
            axum::routing::post(start_virtual_machine),
        )
        .route(
            "/api/v1/virtual-machines/:vm_id/stop",
            axum::routing::post(stop_virtual_machine),
        )
        .route(
            "/api/v1/virtual-machines/:vm_id/restart",
            axum::routing::post(restart_virtual_machine),
        )
        .route(
            "/api/v1/virtual-machines/:vm_id/delete",
            axum::routing::post(delete_virtual_machine),
        )
        .route(
            "/api/v1/virtual-machines/:vm_id/snapshots",
            get(list_virtual_machine_snapshots).post(create_virtual_machine_snapshot),
        )
        .route(
            "/api/v1/virtual-machines/:vm_id/console",
            get(virtual_machine_console),
        )
        .route(
            "/api/v1/control-plane/environment",
            get(control_plane_environment),
        )
        .route("/api/v1/websites", get(list_websites).post(create_website))
        .route(
            "/api/v1/websites/:website_id",
            get(get_website)
                .patch(update_website)
                .delete(delete_website),
        )
        .route(
            "/api/v1/websites/:website_id/start",
            axum::routing::post(start_website),
        )
        .route(
            "/api/v1/websites/:website_id/stop",
            axum::routing::post(stop_website),
        )
        .route(
            "/api/v1/websites/:website_id/restart",
            axum::routing::post(restart_website),
        )
        .route("/api/v1/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/v1/ai/model-providers",
            get(list_ai_model_providers).post(create_ai_model_provider),
        )
        .route(
            "/api/v1/ai/model-providers/:provider_id",
            get(get_ai_model_provider)
                .patch(update_ai_model_provider)
                .delete(delete_ai_model_provider),
        )
        .route(
            "/api/v1/ai/conversations",
            get(list_ai_conversations).post(create_ai_conversation),
        )
        .route(
            "/api/v1/ai/conversations/:conversation_id",
            get(get_ai_conversation).delete(delete_ai_conversation),
        )
        .route(
            "/api/v1/ai/conversations/:conversation_id/messages",
            axum::routing::post(create_ai_chat_turn),
        )
        .route(
            "/api/v1/scheduled-tasks",
            get(list_scheduled_tasks).post(create_scheduled_task),
        )
        .route(
            "/api/v1/scheduled-tasks/:scheduled_task_id",
            axum::routing::delete(delete_scheduled_task).patch(update_scheduled_task),
        )
        .route(
            "/api/v1/scheduled-tasks/:scheduled_task_id/enable",
            axum::routing::post(enable_scheduled_task),
        )
        .route(
            "/api/v1/scheduled-tasks/:scheduled_task_id/disable",
            axum::routing::post(disable_scheduled_task),
        )
        .route(
            "/api/v1/scheduled-tasks/:scheduled_task_id/run",
            axum::routing::post(run_scheduled_task_now),
        )
        .route(
            "/api/v1/scheduled-tasks/:scheduled_task_id/runs",
            get(list_scheduled_task_runs),
        )
        .route(
            "/api/v1/terminal/commands",
            axum::routing::post(run_terminal_command),
        )
        .route("/api/v1/files/:host_id/list", get(list_files))
        .route("/api/v1/files/:host_id/search", get(search_files))
        .route("/api/v1/files/:host_id/download", get(download_file))
        .route(
            "/api/v1/files/:host_id/upload",
            axum::routing::post(upload_file),
        )
        .route(
            "/api/v1/files/:host_id/operations",
            axum::routing::post(run_file_operation),
        )
        .route(
            "/api/v1/approvals",
            get(list_approvals).post(create_approval),
        )
        .route(
            "/api/v1/approvals/:approval_id",
            axum::routing::delete(delete_approval),
        )
        .route(
            "/api/v1/approvals/:approval_id/approve",
            axum::routing::post(approve_approval),
        )
        .route(
            "/api/v1/approvals/:approval_id/deny",
            axum::routing::post(deny_approval),
        )
        .route("/api/v1/apps", get(list_apps))
        .route("/api/v1/settings", get(settings))
        .route("/api/v1/logs/control-plane", get(list_control_plane_logs))
        .route("/api/v1/logs/agents/:host_id", get(list_agent_logs))
        .route("/api/v1/events", get(events))
        .route("/api/v1/auth/me", get(me).patch(update_me))
        .route(
            "/api/v1/auth/me/password",
            axum::routing::post(change_password),
        )
        .route("/api/v1/auth/logout", axum::routing::post(logout))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/health", get(health))
        .route("/api/v1/terminal/:host_id/ws", get(terminal_session_ws))
        .route("/api/v1/logs/stream", get(runtime_log_stream))
        .route(
            "/api/v1/ai/conversations/:conversation_id/stream",
            get(ai_chat_stream),
        )
        .route("/api/v1/auth/status", get(auth_status))
        .route("/api/v1/auth/register", axum::routing::post(register))
        .route("/api/v1/auth/login", axum::routing::post(login))
        .route("/api/v1/auth/refresh", axum::routing::post(refresh))
        .merge(protected_routes)
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseBackend;
    use sea_orm::MockDatabase;

    #[tokio::test]
    async fn router_builds() {
        let connection = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let store = Store::from_connection(connection, DatabaseBackend::Postgres);
        let _router = app(store);
    }
}
