use clap::{Parser, ValueEnum};

/// Transport mode for the MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Transport {
    /// Standard I/O (stdin/stdout) - for Claude Desktop and MCP Inspector
    Stdio,
    /// Streamable HTTP - for web clients and remote access
    Http,
}

/// Google Drive MCP Server configuration.
#[derive(Debug, Clone, Parser)]
#[command(name = "gdrive-mcp-server", about = "Google Drive MCP Server")]
pub struct AppConfig {
    /// Transport mode
    #[arg(long, env = "GDRIVE_MCP_TRANSPORT", default_value = "stdio")]
    pub transport: Transport,

    /// HTTP bind address (only used with --transport http)
    #[arg(long, env = "GDRIVE_MCP_HTTP_ADDR", default_value = "127.0.0.1:3000")]
    pub http_addr: String,

    /// Path to Google OAuth2 client credentials JSON file
    #[arg(long, env = "GDRIVE_MCP_CREDENTIALS", default_value = "client_secret.json")]
    pub credentials_file: String,

    /// Path to persist OAuth2 token cache
    #[arg(long, env = "GDRIVE_MCP_TOKEN_CACHE", default_value = "~/.gdrive-mcp-token.json")]
    pub token_cache_path: String,

    /// Log level
    #[arg(long, env = "GDRIVE_MCP_LOG_LEVEL", default_value = "info")]
    pub log_level: String,
}

impl AppConfig {
    /// Resolve the token cache path, expanding ~ to home directory.
    pub fn resolved_token_cache_path(&self) -> String {
        if self.token_cache_path.starts_with('~') {
            if let Some(home) = dirs_home() {
                return self.token_cache_path.replacen('~', &home, 1);
            }
        }
        self.token_cache_path.clone()
    }
}

fn dirs_home() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
}
