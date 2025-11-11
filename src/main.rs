mod browser;
mod server;

use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use server::ProjectionLabServer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const BIND_ADDRESS: &str = "127.0.0.1:8000";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,projectionlab_mcp=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting ProjectionLab MCP Server on {}", BIND_ADDRESS);
    tracing::info!("Server will launch Firefox browser on first client connection");
    tracing::info!("You will need to log in to ProjectionLab manually");

    // Create the streamable HTTP service
    let service = StreamableHttpService::new(
        || Ok(ProjectionLabServer::new()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    // Create the router
    let router = axum::Router::new().nest_service("/mcp", service);

    // Bind to TCP listener
    let tcp_listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;

    tracing::info!("✓ Server ready at http://{}/mcp", BIND_ADDRESS);
    tracing::info!("Configure your MCP client to connect to this URL");

    // Serve with graceful shutdown
    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.unwrap();
            tracing::info!("Shutting down server...");
        })
        .await?;

    Ok(())
}
