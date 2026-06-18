use crate::agent_grpc::GrpcAgentService;
use crate::agent_streams::AgentStreamRegistry;
use crate::auth::AuthService;
use crate::chat_streams::ChatStreamHub;
use crate::logs::LogHub;
use crate::prelude::*;
use crate::routes::app_with_auth_streams_logs_and_chat;
use crate::routes::scheduled_tasks::run_scheduled_task_scheduler;
use crate::startup::print_control_plane_startup_summary;
use std::path::Path;

pub async fn run(config: doro_config::ControlPlaneConfig) -> anyhow::Result<()> {
    run_inner(None, false, config).await
}

pub async fn run_loaded(
    loaded_config: doro_config::LoadedControlPlaneConfig,
) -> anyhow::Result<()> {
    run_inner(
        loaded_config.path.as_deref(),
        loaded_config.created,
        loaded_config.config,
    )
    .await
}

async fn run_inner(
    config_path: Option<&Path>,
    config_created: bool,
    config: doro_config::ControlPlaneConfig,
) -> anyhow::Result<()> {
    let console_addr: SocketAddr = config.server.console_bind.parse()?;
    let agent_addr: SocketAddr = config.server.agent_bind.parse()?;
    print_control_plane_startup_summary(config_path, config_created, &config);
    let store = Store::connect_with_config(&config.store).await?;
    store.migrate().await?;
    let auth = AuthService::load_or_create(&store, config.security.jwt_secret.as_deref()).await?;
    let logs = LogHub::default();
    logs.register_control_plane_global();

    let console_listener = tokio::net::TcpListener::bind(console_addr).await?;
    tracing::info!("doro control-plane console listening on http://{console_addr}");
    tracing::info!("doro control-plane agent listening on http://{agent_addr}");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!("shutdown signal received, stopping control-plane services");
        let _ = shutdown_tx.send(true);
    });

    let agent_streams = AgentStreamRegistry::default();
    let chat_streams = ChatStreamHub::default();
    let shutdown_streams = agent_streams.clone();
    let console_store = store.clone();
    let console_streams = agent_streams.clone();
    let console_chat_streams = chat_streams.clone();
    let console_logs = logs.clone();
    let agent_store = store.clone();
    let grpc_streams = agent_streams.clone();
    let grpc_chat_streams = chat_streams.clone();
    let agent_logs = logs.clone();
    let console_shutdown = shutdown_rx.clone();
    let stream_shutdown = shutdown_rx.clone();
    let scheduler_shutdown = shutdown_rx.clone();
    let agent_shutdown = shutdown_rx;
    tokio::spawn(run_scheduled_task_scheduler(
        store.clone(),
        agent_streams.clone(),
        scheduler_shutdown,
    ));
    tokio::spawn(async move {
        wait_for_shutdown(stream_shutdown).await;
        shutdown_streams
            .shutdown_all("control-plane shutting down")
            .await;
    });
    let console_server = async move {
        axum::serve(
            console_listener,
            app_with_auth_streams_logs_and_chat(
                console_store,
                auth,
                console_streams,
                console_logs,
                console_chat_streams,
            ),
        )
        .with_graceful_shutdown(wait_for_shutdown(console_shutdown))
        .await
        .map_err(anyhow::Error::from)
    };
    let agent_server = async move {
        Server::builder()
            .add_service(AgentControlPlaneServer::new(GrpcAgentService {
                store: agent_store,
                agent_streams: grpc_streams,
                chat_streams: grpc_chat_streams,
                logs: agent_logs,
                shutdown_rx: agent_shutdown.clone(),
            }))
            .serve_with_shutdown(agent_addr, wait_for_shutdown(agent_shutdown))
            .await
            .map_err(anyhow::Error::from)
    };

    tokio::try_join!(console_server, agent_server)?;
    Ok(())
}

pub(crate) async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    while !*shutdown_rx.borrow_and_update() {
        if shutdown_rx.changed().await.is_err() {
            break;
        }
    }
}

pub(crate) fn shutdown_requested(shutdown_rx: &watch::Receiver<bool>) -> bool {
    *shutdown_rx.borrow()
}

pub(crate) async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "failed to listen for ctrl-c shutdown signal");
        }
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::SignalKind;

        let terminate = async {
            match tokio::signal::unix::signal(SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to listen for terminate shutdown signal");
                    std::future::pending::<()>().await;
                }
            }
        };

        tokio::select! {
            () = ctrl_c => {}
            () = terminate => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}
