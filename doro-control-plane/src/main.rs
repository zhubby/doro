use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "doro_control_plane=info,tower_http=info".into()),
        )
        .init();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8787));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("doro-control-plane listening on http://{addr}");
    axum::serve(listener, doro_control_plane::app()).await?;
    Ok(())
}
