use std::path::Path;

use google_drive3::DriveHub;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::client::legacy::connect::HttpConnector;
use yup_oauth2::{
    ApplicationSecret, InstalledFlowAuthenticator, InstalledFlowReturnMethod,
    ServiceAccountAuthenticator,
};

use crate::config::AppConfig;
use crate::error::{GDriveError, Result};

/// The hyper client type used by google-drive3 (uses BoxBody, not Full<Bytes>).
pub type HyperClient = google_drive3::common::Client<hyper_rustls::HttpsConnector<HttpConnector>>;

/// The authenticator type for OAuth2 flows.
pub type Authenticator = yup_oauth2::authenticator::Authenticator<hyper_rustls::HttpsConnector<HttpConnector>>;

/// The concrete DriveHub type.
pub type DriveHubType = DriveHub<hyper_rustls::HttpsConnector<HttpConnector>>;

/// Google Drive OAuth2 scopes.
#[allow(dead_code)]
const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/drive",
    "https://www.googleapis.com/auth/drive.file",
    "https://www.googleapis.com/auth/drive.readonly",
    "https://www.googleapis.com/auth/drive.metadata.readonly",
];

/// Build an authenticated DriveHub from the app configuration.
/// If `eager_token` is true, acquires the token at startup (for stdio transport).
/// If false, the token is acquired lazily on the first API call (for HTTP transport).
pub async fn build_drive_hub(config: &AppConfig, eager_token: bool) -> Result<DriveHubType> {
    let auth = build_authenticator(config).await?;

    if eager_token {
        // Eagerly acquire token at startup so OAuth browser flow completes
        // before the MCP transport starts. Without this, the token is acquired
        // lazily on the first API call, which can timeout in stdio/MCP mode.
        tracing::info!("Acquiring OAuth2 token (browser may open on first run)...");
        let _token = auth
            .token(SCOPES)
            .await
            .map_err(|e| GDriveError::OAuth2(format!("Failed to acquire token: {e}")))?;
        tracing::info!("OAuth2 token acquired successfully.");
    } else {
        tracing::info!("Google OAuth2 token will be acquired on first API call.");
    }

    let connector = HttpsConnectorBuilder::new()
        .with_native_roots()
        .map_err(|e| GDriveError::Other(format!("Failed to build HTTPS connector: {e}")))?
        .https_or_http()
        .enable_http2()
        .build();

    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(connector);

    Ok(DriveHub::new(client, auth))
}

/// Build an OAuth2 authenticator, detecting service account vs installed app.
async fn build_authenticator(config: &AppConfig) -> Result<Authenticator> {
    let creds_path = &config.credentials_file;

    let creds_content = tokio::fs::read_to_string(creds_path)
        .await
        .map_err(|e| GDriveError::OAuth2(format!("Cannot read credentials file '{creds_path}': {e}")))?;

    // Try service account first
    if creds_content.contains("\"type\": \"service_account\"")
        || creds_content.contains("\"type\":\"service_account\"")
    {
        return build_service_account_auth(creds_path).await;
    }

    // Installed application flow
    build_installed_flow_auth(&creds_content, config).await
}

async fn build_service_account_auth(creds_path: &str) -> Result<Authenticator> {
    let sa_key = yup_oauth2::read_service_account_key(creds_path)
        .await
        .map_err(|e| GDriveError::OAuth2(format!("Invalid service account key: {e}")))?;

    ServiceAccountAuthenticator::builder(sa_key)
        .build()
        .await
        .map_err(|e| GDriveError::OAuth2(format!("Failed to build service account authenticator: {e}")))
}

async fn build_installed_flow_auth(creds_content: &str, config: &AppConfig) -> Result<Authenticator> {
    let secret: ApplicationSecret = parse_application_secret(creds_content)?;
    let token_cache = config.resolved_token_cache_path();

    let mut builder = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect);

    if Path::new(&token_cache).exists() || !token_cache.is_empty() {
        builder = builder.persist_tokens_to_disk(&token_cache);
    }

    builder
        .build()
        .await
        .map_err(|e| GDriveError::OAuth2(format!("Failed to build installed flow authenticator: {e}")))
}

/// Build a shared hyper HTTPS client for HTTP transport (multi-user mode).
///
/// In HTTP mode, each MCP session gets its own `DriveHub` with the user's Google
/// access token (a plain `String` implementing `GetToken`). This shared hyper client
/// is cloned into each per-session `DriveHub` to avoid recreating TLS connectors.
pub fn build_shared_hyper_client() -> Result<HyperClient> {
    let connector = HttpsConnectorBuilder::new()
        .with_native_roots()
        .map_err(|e| GDriveError::Other(format!("Failed to build HTTPS connector: {e}")))?
        .https_or_http()
        .enable_http2()
        .build();

    Ok(
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build(connector),
    )
}

/// Parse the application secret from a Google OAuth2 credentials JSON file.
fn parse_application_secret(content: &str) -> Result<ApplicationSecret> {
    let json: serde_json::Value =
        serde_json::from_str(content).map_err(|e| GDriveError::OAuth2(format!("Invalid JSON: {e}")))?;

    // Google credentials files wrap the secret in "installed" or "web"
    let inner = json
        .get("installed")
        .or_else(|| json.get("web"))
        .ok_or_else(|| {
            GDriveError::OAuth2(
                "Credentials file must contain an 'installed' or 'web' key".to_string(),
            )
        })?;

    serde_json::from_value(inner.clone())
        .map_err(|e| GDriveError::OAuth2(format!("Failed to parse application secret: {e}")))
}
