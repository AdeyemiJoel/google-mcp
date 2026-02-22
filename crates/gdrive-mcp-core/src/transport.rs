use rmcp::ServiceExt;

use crate::auth::HyperClient;
use crate::config::AppConfig;
use crate::error::GDriveError;
use crate::server::GDriveServer;

/// Serve via stdio transport (for Claude Desktop, MCP Inspector, etc.).
pub async fn serve_stdio(server: GDriveServer) -> crate::error::Result<()> {
    tracing::info!("Starting MCP server on stdio transport");

    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| GDriveError::Other(format!("Failed to start stdio server: {e}")))?;

    service
        .waiting()
        .await
        .map_err(|e| GDriveError::Other(format!("Stdio server error: {e}")))?;

    Ok(())
}

/// Serve via Streamable HTTP transport with MCP OAuth proxy to Google.
///
/// Each MCP session gets its own `DriveHub` with the authenticated user's Google
/// access token. The session factory reads the token from `CURRENT_GOOGLE_TOKEN`
/// task-local (set by auth middleware) and creates a per-user DriveHub.
pub async fn serve_http(
    template_server: GDriveServer,
    hyper_client: HyperClient,
    oauth: crate::oauth::OAuthServer,
    config: &AppConfig,
) -> crate::error::Result<()> {
    use axum::routing::{get, post};
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService,
        session::local::LocalSessionManager,
    };

    let addr = &config.http_addr;
    tracing::info!("Starting MCP server on HTTP transport at {addr}");

    let ct = tokio_util::sync::CancellationToken::new();

    // Session factory: creates a per-user GDriveServer for each MCP session.
    // The auth middleware sets CURRENT_GOOGLE_TOKEN before this factory runs.
    // The factory creates a new DriveHub with the user's Google token (String
    // implements GetToken), reusing the shared hyper client and tool/prompt routers.
    let mcp_service = StreamableHttpService::new(
        move || {
            let google_token = crate::oauth::CURRENT_GOOGLE_TOKEN
                .try_with(|t| t.clone())
                .unwrap_or_default();

            if google_token.is_empty() {
                tracing::warn!("Session created without Google token (initialization request)");
            } else {
                tracing::info!("Creating per-user MCP session with Google token");
            }

            let hub = google_drive3::DriveHub::new(hyper_client.clone(), google_token);
            let drive_client = crate::client::DriveClient::new(hub);
            Ok(template_server.with_client(drive_client))
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );

    let cors = tower_http::cors::CorsLayer::permissive();

    // MCP route with Bearer token auth middleware
    let mcp_router = axum::Router::new()
        .fallback_service(mcp_service)
        .layer(axum::middleware::from_fn_with_state(
            oauth.clone(),
            crate::oauth::auth_middleware,
        ));

    let router = axum::Router::new()
        // OAuth discovery endpoints (RFC 9728 + RFC 8414)
        // Both root and /mcp sub-path variants (spec says client tries sub-path first)
        .route(
            "/.well-known/oauth-protected-resource",
            get(crate::oauth::protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(crate::oauth::protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(crate::oauth::authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server/mcp",
            get(crate::oauth::authorization_server_metadata),
        )
        // OAuth endpoints
        .route("/oauth/register", post(crate::oauth::register_client))
        .route("/oauth/authorize", get(crate::oauth::authorize_get))
        .route("/oauth/callback", get(crate::oauth::google_callback))
        .route("/oauth/token", post(crate::oauth::token_exchange))
        .with_state(oauth)
        // MCP endpoint (protected by auth middleware)
        .nest("/mcp", mcp_router)
        .layer(cors);

    let tcp_listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| GDriveError::Other(format!("Failed to bind to {addr}: {e}")))?;

    tracing::info!("MCP HTTP server listening on {addr}");
    tracing::info!("OAuth flow: MCP client → Google OAuth → /oauth/callback → MCP token");

    axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("Received shutdown signal");
            ct.cancel();
        })
        .await
        .map_err(|e| GDriveError::Other(format!("HTTP server error: {e}")))?;

    Ok(())
}
