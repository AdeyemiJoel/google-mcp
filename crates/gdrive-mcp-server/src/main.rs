use clap::Parser;
use gdrive_mcp_core::config::AppConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install rustls crypto provider before any TLS usage
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let config = AppConfig::parse();

    // Initialize tracing to stderr (stdout is reserved for MCP stdio transport)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!(
        transport = ?config.transport,
        "Starting gdrive-mcp-server v{}",
        env!("CARGO_PKG_VERSION")
    );

    gdrive_mcp_core::run_server(config).await?;

    Ok(())
}
