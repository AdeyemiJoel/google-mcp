#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]

pub mod auth;
pub mod client;
pub mod config;
pub mod convert;
pub mod error;
pub mod oauth;
pub mod prompts;
pub mod resources;
pub mod server;
pub mod tools;
pub mod transport;

use config::{AppConfig, Transport};

/// Main entry point: build auth → client → server, then run on selected transport.
pub async fn run_server(config: AppConfig) -> error::Result<()> {
    match config.transport {
        Transport::Stdio => {
            // Stdio: single-user mode with eager Google auth
            tracing::info!("Building Google Drive authenticator...");
            let hub = auth::build_drive_hub(&config, true).await?;
            let drive_client = client::DriveClient::new(hub);
            let server = server::GDriveServer::new(drive_client);
            transport::serve_stdio(server).await
        }
        Transport::Http => {
            // HTTP: multi-user mode - each user authenticates via MCP OAuth → Google.
            // Each MCP session gets its own DriveHub with the user's Google access token.
            tracing::info!("Building Google Drive MCP server (multi-user mode)...");

            // Parse Google OAuth credentials to create the OAuth proxy server
            let creds_content = tokio::fs::read_to_string(&config.credentials_file)
                .await
                .map_err(|e| {
                    error::GDriveError::OAuth2(format!("Cannot read credentials file: {e}"))
                })?;
            let google_config = oauth::parse_google_oauth_config(&creds_content).map_err(
                |e| {
                    error::GDriveError::OAuth2(format!(
                        "Failed to parse Google OAuth config: {e}"
                    ))
                },
            )?;
            let base_url = format!("http://{}", config.http_addr);
            let oauth_server = oauth::OAuthServer::new(&base_url, google_config);

            // Build a shared hyper client (cloned into per-session DriveHubs)
            let hyper_client = auth::build_shared_hyper_client()?;

            // Build a template server (routers are reused, client is replaced per-session)
            let placeholder_hub =
                google_drive3::DriveHub::new(hyper_client.clone(), String::new());
            let template_server =
                server::GDriveServer::new(client::DriveClient::new(placeholder_hub));

            transport::serve_http(template_server, hyper_client, oauth_server, &config)
                .await
        }
    }
}
