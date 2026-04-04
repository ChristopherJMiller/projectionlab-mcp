mod browser;
mod models;
mod resources;
mod server;
mod sync;
mod tools;

use rmcp::ServiceExt;
use server::ProjectionLabServer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing — all output to stderr so stdout stays clean for MCP protocol
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,projectionlab_mcp=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting ProjectionLab MCP Server (stdio)");

    let server = ProjectionLabServer::new();
    let browser = server.browser_handle();

    // Clone browser handle for the shutdown hook
    let browser_for_shutdown = browser.clone();

    // Register shutdown handler for SIGTERM/SIGINT so browser gets cleaned up
    // even if the process is killed by the MCP client
    tokio::spawn(async move {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to register SIGTERM handler");
        let mut sigint =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("failed to register SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
            _ = sigint.recv() => tracing::info!("Received SIGINT"),
        }

        tracing::info!("Signal received, cleaning up browser...");
        let mut guard = browser_for_shutdown.lock().await;
        if let Some(ref mut session) = *guard {
            session.shutdown().await;
        }
        *guard = None;
        tracing::info!("Cleanup complete, exiting");
        std::process::exit(0);
    });

    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    // Normal MCP session end (stdin closed) — clean up browser
    tracing::info!("MCP session ended, cleaning up...");
    let mut guard = browser.lock().await;
    if let Some(ref mut session) = *guard {
        session.shutdown().await;
    }
    *guard = None;

    Ok(())
}
