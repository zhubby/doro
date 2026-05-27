use std::net::SocketAddr;

use doro_control_plane::GrpcAgentService;
use doro_protocol::grpc::agent_control_plane_server::AgentControlPlaneServer;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "doro_control_plane=info,tower_http=info".into()),
        )
        .init();

    let http_addr = SocketAddr::from(([127, 0, 0, 1], 8787));
    let grpc_addr = SocketAddr::from(([127, 0, 0, 1], 8788));

    let http_listener = tokio::net::TcpListener::bind(http_addr).await?;
    tracing::info!("doro-control-plane http listening on http://{http_addr}");
    tracing::info!("doro-control-plane grpc listening on http://{grpc_addr}");

    let http_server = async move {
        axum::serve(http_listener, doro_control_plane::app())
            .await
            .map_err(anyhow::Error::from)
    };
    let grpc_server = async move {
        Server::builder()
            .add_service(AgentControlPlaneServer::new(GrpcAgentService))
            .serve(grpc_addr)
            .await
            .map_err(anyhow::Error::from)
    };

    tokio::try_join!(http_server, grpc_server)?;
    Ok(())
}
