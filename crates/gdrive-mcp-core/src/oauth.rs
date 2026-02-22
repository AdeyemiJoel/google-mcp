//! MCP OAuth 2.1 Authorization with Google OAuth Proxy.
//!
//! Implements the MCP Authorization spec as an OAuth Proxy:
//! - MCP client registers via Dynamic Client Registration (RFC 7591)
//! - Authorization endpoint redirects to Google OAuth
//! - Google callback exchanges code for Google tokens
//! - Server issues its own MCP tokens bound to the user's Google tokens
//! - Each user gets isolated Google Drive access via their own Google tokens
//!
//! Flow:
//!   MCP Client → /oauth/authorize → Google OAuth consent → /oauth/callback
//!   → MCP auth code issued → /oauth/token → MCP access token issued
//!   → MCP requests use per-user Google tokens for Drive API calls

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

// ── Per-request token context ───────────────────────────────────────

tokio::task_local! {
    /// The current user's Google access token for this request.
    /// Set by auth_middleware, read by the session factory to create per-user DriveHubs.
    pub static CURRENT_GOOGLE_TOKEN: String;
}

// ── Google OAuth Config ─────────────────────────────────────────────

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_SCOPES: &str = "https://www.googleapis.com/auth/drive";

/// Google OAuth client credentials (from client_secret.json).
#[derive(Clone, Debug)]
pub struct GoogleOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
}

// ── OAuth Server State ──────────────────────────────────────────────

/// Shared OAuth state for the authorization server.
#[derive(Clone)]
pub struct OAuthServer {
    state: Arc<RwLock<OAuthState>>,
    pub base_url: String,
    pub google: GoogleOAuthConfig,
}

struct OAuthState {
    /// Registered MCP clients (Dynamic Client Registration).
    clients: HashMap<String, ClientInfo>,
    /// Pending authorization flows (keyed by our internal state token).
    pending_flows: HashMap<String, PendingFlow>,
    /// MCP auth codes ready for token exchange.
    auth_codes: HashMap<String, AuthCodeInfo>,
    /// Issued MCP access tokens → per-user Google tokens.
    tokens: HashMap<String, TokenInfo>,
}

#[derive(Clone, Serialize)]
struct ClientInfo {
    client_id: String,
    client_name: Option<String>,
    redirect_uris: Vec<String>,
}

/// Tracks state between our /oauth/authorize → Google → /oauth/callback.
struct PendingFlow {
    mcp_client_id: String,
    mcp_redirect_uri: String,
    mcp_state: String,
    mcp_code_challenge: String,
    mcp_scope: String,
    created_at: Instant,
}

struct AuthCodeInfo {
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    scope: String,
    /// The user's Google access & refresh tokens.
    google_access_token: String,
    google_refresh_token: Option<String>,
    created_at: Instant,
}

/// MCP token → Google tokens mapping.
pub struct TokenInfo {
    #[allow(dead_code)]
    pub client_id: String,
    pub google_access_token: String,
    pub google_refresh_token: Option<String>,
    pub created_at: Instant,
}

const AUTH_CODE_LIFETIME: Duration = Duration::from_secs(600);
const TOKEN_LIFETIME: Duration = Duration::from_secs(3600);
const PENDING_FLOW_LIFETIME: Duration = Duration::from_secs(600);

impl OAuthServer {
    pub fn new(base_url: &str, google: GoogleOAuthConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(OAuthState {
                clients: HashMap::new(),
                pending_flows: HashMap::new(),
                auth_codes: HashMap::new(),
                tokens: HashMap::new(),
            })),
            base_url: base_url.trim_end_matches('/').to_string(),
            google,
        }
    }

    /// Validate a Bearer token. Returns the Google access token if valid.
    pub async fn validate_token(&self, token: &str) -> Option<String> {
        let state = self.state.read().await;
        state.tokens.get(token).and_then(|info| {
            if info.created_at.elapsed() < TOKEN_LIFETIME {
                Some(info.google_access_token.clone())
            } else {
                None
            }
        })
    }

    /// Get the Google access token for a given MCP bearer token.
    pub async fn get_google_token(&self, mcp_token: &str) -> Option<String> {
        self.validate_token(mcp_token).await
    }
}

/// Parse Google OAuth credentials from client_secret.json content.
pub fn parse_google_oauth_config(content: &str) -> Result<GoogleOAuthConfig, String> {
    let json: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("Invalid JSON: {e}"))?;

    let inner = json
        .get("installed")
        .or_else(|| json.get("web"))
        .ok_or("Credentials file must contain 'installed' or 'web' key")?;

    let client_id = inner
        .get("client_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing client_id")?
        .to_string();

    let client_secret = inner
        .get("client_secret")
        .and_then(|v| v.as_str())
        .ok_or("Missing client_secret")?
        .to_string();

    Ok(GoogleOAuthConfig {
        client_id,
        client_secret,
    })
}

// ── Metadata Endpoints ──────────────────────────────────────────────

/// GET /.well-known/oauth-protected-resource
pub async fn protected_resource_metadata(
    State(oauth): State<OAuthServer>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "resource": oauth.base_url,
        "authorization_servers": [oauth.base_url],
        "scopes_supported": ["gdrive"],
        "bearer_methods_supported": ["header"]
    }))
}

/// GET /.well-known/oauth-authorization-server
pub async fn authorization_server_metadata(
    State(oauth): State<OAuthServer>,
) -> Json<serde_json::Value> {
    let base = &oauth.base_url;
    Json(serde_json::json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "registration_endpoint": format!("{base}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": ["gdrive"],
        "token_endpoint_auth_methods_supported": ["none"],
        "client_id_metadata_document_supported": false
    }))
}

// ── Dynamic Client Registration (RFC 7591) ──────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    client_name: Option<String>,
    redirect_uris: Vec<String>,
    #[allow(dead_code)]
    grant_types: Option<Vec<String>>,
    #[allow(dead_code)]
    response_types: Option<Vec<String>>,
    #[allow(dead_code)]
    token_endpoint_auth_method: Option<String>,
}

/// POST /oauth/register
pub async fn register_client(
    State(oauth): State<OAuthServer>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let client_id = uuid::Uuid::new_v4().to_string();

    let client = ClientInfo {
        client_id: client_id.clone(),
        client_name: req.client_name.clone(),
        redirect_uris: req.redirect_uris.clone(),
    };

    oauth
        .state
        .write()
        .await
        .clients
        .insert(client_id.clone(), client);

    tracing::info!(client_id = %client_id, "Registered new OAuth client");

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "client_id": client_id,
            "client_name": req.client_name,
            "redirect_uris": req.redirect_uris,
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none"
        })),
    )
}

// ── Authorization Endpoint (Proxy to Google) ────────────────────────

#[derive(Deserialize)]
pub struct AuthorizeParams {
    response_type: String,
    client_id: String,
    redirect_uri: String,
    scope: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    #[allow(dead_code)]
    code_challenge_method: Option<String>,
    #[allow(dead_code)]
    resource: Option<String>,
}

/// GET /oauth/authorize
///
/// Instead of showing our own consent page, we redirect to Google OAuth.
/// The user authenticates with Google, and Google redirects back to our
/// /oauth/callback endpoint with a Google auth code.
pub async fn authorize_get(
    State(oauth): State<OAuthServer>,
    Query(params): Query<AuthorizeParams>,
) -> impl IntoResponse {
    if params.response_type != "code" {
        return Html(error_html("Unsupported response_type. Only 'code' is supported."))
            .into_response();
    }

    // Generate internal state token to track this flow
    let internal_state = uuid::Uuid::new_v4().to_string();

    // Save MCP client's flow details
    let flow = PendingFlow {
        mcp_client_id: params.client_id,
        mcp_redirect_uri: params.redirect_uri,
        mcp_state: params.state.unwrap_or_default(),
        mcp_code_challenge: params.code_challenge.unwrap_or_default(),
        mcp_scope: params.scope.unwrap_or_else(|| "gdrive".to_string()),
        created_at: Instant::now(),
    };

    oauth
        .state
        .write()
        .await
        .pending_flows
        .insert(internal_state.clone(), flow);

    // Build Google OAuth URL
    let callback_url = format!("{}/oauth/callback", oauth.base_url);
    let google_auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&access_type=offline&prompt=consent",
        GOOGLE_AUTH_URL,
        urlencoded(&oauth.google.client_id),
        urlencoded(&callback_url),
        urlencoded(GOOGLE_SCOPES),
        urlencoded(&internal_state),
    );

    tracing::info!("Redirecting to Google OAuth");
    Redirect::to(&google_auth_url).into_response()
}

// ── Google OAuth Callback ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// GET /oauth/callback
///
/// Google redirects here after user consents. We:
/// 1. Exchange the Google auth code for Google tokens
/// 2. Generate an MCP auth code
/// 3. Redirect back to the MCP client with the MCP auth code
pub async fn google_callback(
    State(oauth): State<OAuthServer>,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    // Handle Google errors
    if let Some(err) = &params.error {
        return Html(error_html(&format!("Google OAuth error: {err}"))).into_response();
    }

    let google_code = match &params.code {
        Some(c) => c.clone(),
        None => return Html(error_html("Missing authorization code from Google")).into_response(),
    };

    let internal_state = match &params.state {
        Some(s) => s.clone(),
        None => return Html(error_html("Missing state parameter")).into_response(),
    };

    // Look up the pending flow
    let flow = {
        let mut state = oauth.state.write().await;
        match state.pending_flows.remove(&internal_state) {
            Some(f) => f,
            None => {
                return Html(error_html("Invalid or expired authorization flow")).into_response()
            }
        }
    };

    // Check flow expiration
    if flow.created_at.elapsed() > PENDING_FLOW_LIFETIME {
        return Html(error_html("Authorization flow expired")).into_response();
    }

    // Exchange Google auth code for Google tokens
    let callback_url = format!("{}/oauth/callback", oauth.base_url);
    let google_tokens = match exchange_google_code(
        &google_code,
        &callback_url,
        &oauth.google.client_id,
        &oauth.google.client_secret,
    )
    .await
    {
        Ok(tokens) => tokens,
        Err(err) => {
            tracing::error!("Google token exchange failed: {err}");
            return Html(error_html(&format!("Google token exchange failed: {err}")))
                .into_response();
        }
    };

    tracing::info!("Google tokens obtained for user");

    // Generate MCP auth code
    let mcp_code = uuid::Uuid::new_v4().to_string();

    let code_info = AuthCodeInfo {
        client_id: flow.mcp_client_id,
        redirect_uri: flow.mcp_redirect_uri.clone(),
        code_challenge: flow.mcp_code_challenge,
        scope: flow.mcp_scope,
        google_access_token: google_tokens.access_token,
        google_refresh_token: google_tokens.refresh_token,
        created_at: Instant::now(),
    };

    oauth
        .state
        .write()
        .await
        .auth_codes
        .insert(mcp_code.clone(), code_info);

    // Redirect back to MCP client
    let sep = if flow.mcp_redirect_uri.contains('?') {
        "&"
    } else {
        "?"
    };
    let redirect = format!(
        "{}{}code={}&state={}",
        flow.mcp_redirect_uri, sep, mcp_code, flow.mcp_state
    );

    tracing::info!("MCP authorization code issued, redirecting to client");
    Redirect::to(&redirect).into_response()
}

// ── Token Endpoint ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TokenRequest {
    grant_type: String,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    code_verifier: Option<String>,
    #[allow(dead_code)]
    resource: Option<String>,
}

/// POST /oauth/token
pub async fn token_exchange(
    State(oauth): State<OAuthServer>,
    axum::Form(req): axum::Form<TokenRequest>,
) -> impl IntoResponse {
    if req.grant_type != "authorization_code" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "unsupported_grant_type" })),
        )
            .into_response();
    }

    let code = match &req.code {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "error_description": "missing code"
                })),
            )
                .into_response();
        }
    };

    let mut state = oauth.state.write().await;

    let code_info = match state.auth_codes.remove(&code) {
        Some(info) => info,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_grant",
                    "error_description": "invalid or expired code"
                })),
            )
                .into_response();
        }
    };

    if code_info.created_at.elapsed() > AUTH_CODE_LIFETIME {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "code expired"
            })),
        )
            .into_response();
    }

    // Validate redirect_uri
    if let Some(ref uri) = req.redirect_uri {
        if *uri != code_info.redirect_uri {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_grant",
                    "error_description": "redirect_uri mismatch"
                })),
            )
                .into_response();
        }
    }

    // PKCE validation
    if !code_info.code_challenge.is_empty() {
        let verifier = match &req.code_verifier {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "invalid_request",
                        "error_description": "missing code_verifier"
                    })),
                )
                    .into_response();
            }
        };

        let computed = {
            let hash = Sha256::digest(verifier.as_bytes());
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
        };

        if computed != code_info.code_challenge {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_grant",
                    "error_description": "PKCE verification failed"
                })),
            )
                .into_response();
        }
    }

    // Issue MCP tokens (bound to user's Google tokens)
    let access_token = uuid::Uuid::new_v4().to_string();
    let refresh_token = uuid::Uuid::new_v4().to_string();

    state.tokens.insert(
        access_token.clone(),
        TokenInfo {
            client_id: req.client_id.unwrap_or(code_info.client_id),
            google_access_token: code_info.google_access_token,
            google_refresh_token: code_info.google_refresh_token,
            created_at: Instant::now(),
        },
    );

    tracing::info!("MCP access token issued (bound to user's Google tokens)");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "expires_in": TOKEN_LIFETIME.as_secs(),
            "refresh_token": refresh_token,
            "scope": code_info.scope
        })),
    )
        .into_response()
}

// ── Auth Middleware ──────────────────────────────────────────────────

/// Middleware to validate Bearer token on MCP requests.
///
/// Sets the user's Google access token in task-local storage so that
/// the session factory can capture it when creating per-user DriveHubs.
pub async fn auth_middleware(
    State(oauth): State<OAuthServer>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            match oauth.validate_token(token).await {
                Some(google_token) => {
                    // Set the Google access token in task-local storage.
                    // The session factory reads this to create per-user DriveHubs.
                    CURRENT_GOOGLE_TOKEN
                        .scope(google_token, async move { next.run(req).await })
                        .await
                        .into_response()
                }
                None => unauthorized_response(&oauth.base_url),
            }
        }
        _ => unauthorized_response(&oauth.base_url),
    }
}

fn unauthorized_response(base_url: &str) -> axum::response::Response {
    let body = serde_json::json!({ "error": "unauthorized" });
    (
        StatusCode::UNAUTHORIZED,
        [(
            axum::http::header::WWW_AUTHENTICATE,
            format!(
                "Bearer resource_metadata=\"{base_url}/.well-known/oauth-protected-resource\", scope=\"gdrive\""
            ),
        )],
        Json(body),
    )
        .into_response()
}

fn error_html(msg: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>GDrive MCP - Error</title>
<style>body {{ font-family: -apple-system, sans-serif; max-width: 480px; margin: 80px auto; padding: 20px; }}</style>
</head><body><h1>Error</h1><p>{msg}</p></body></html>"#
    )
}

// ── Google Token Exchange ───────────────────────────────────────────

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    #[allow(dead_code)]
    token_type: Option<String>,
    #[allow(dead_code)]
    expires_in: Option<u64>,
}

/// Exchange a Google authorization code for access + refresh tokens.
async fn exchange_google_code(
    code: &str,
    redirect_uri: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<GoogleTokenResponse, String> {
    let client = reqwest::Client::new();

    let resp = client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Google token endpoint returned {status}: {body}"));
    }

    resp.json::<GoogleTokenResponse>()
        .await
        .map_err(|e| format!("Failed to parse Google token response: {e}"))
}

/// Simple URL encoding helper.
fn urlencoded(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}
