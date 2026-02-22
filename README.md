# google-mcp

A comprehensive Google MCP (Model Context Protocol) server written in Rust. Currently provides **41 tools**, resource templates, and prompts for full Google Drive access, with Gmail and other Google services planned.

Supports both **stdio** and **Streamable HTTP** transports with multi-user OAuth.

## Features

### Tools (41 total)

| Domain | Count | Tools |
|--------|-------|-------|
| Files | 12 | list, get, create, update, delete, copy, move, trash, untrash, empty_trash, export, download |
| Permissions | 5 | create, list, get, update, delete |
| Comments | 5 | create, list, get, update, delete |
| Replies | 5 | create, list, get, update, delete |
| Revisions | 4 | list, get, update, delete |
| Shared Drives | 5 | create, list, get, update, delete |
| Changes | 2 | get_start_page_token, list |
| About | 1 | get |
| Labels | 2 | list, modify |

### Resources

- `gdrive:///{file_id}` -- Read file content with auto-conversion (Docs → Markdown, Sheets → CSV, Slides → text, Drawings → PNG)
- `gdrive:///folder/{folder_id}` -- List folder contents

### Prompts

- `gdrive_search_help` -- Help building Google Drive search queries
- `gdrive_organize_files` -- File/folder organization guidance
- `gdrive_sharing_guide` -- Sharing and permissions guidance

## Prerequisites

- **Rust** 1.80+ (stable toolchain)
- **Google Cloud Project** with the Drive API enabled
- **OAuth2 credentials** (`client_secret.json`)

## Setup

### 1. Google Cloud Console

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project (or select an existing one)
3. Enable the **Google Drive API** (APIs & Services → Library → search "Google Drive API")
4. Go to **APIs & Services → Credentials → Create Credentials → OAuth client ID**
5. Select **Desktop app** as application type
6. Download the JSON file and save it as `client_secret.json` in the project root

> **Note:** For HTTP transport (multi-user), you may use **Web application** type instead, but **Desktop app** works for both modes.

### 2. Build

```bash
git clone https://github.com/cafercangundogdu/google-mcp.git
cd google-mcp
cargo build --release
```

The binary will be at `target/release/gdrive-mcp-server`.

## Usage

### Stdio Transport (default)

Single-user mode for Claude Desktop, MCP Inspector, or any stdio-based MCP client. On first run, a browser window opens for Google OAuth2 authorization. The token is cached for subsequent runs.

```bash
gdrive-mcp-server --credentials-file client_secret.json
```

### HTTP Transport (multi-user)

Multi-user mode with full MCP OAuth 2.1 support. Each user authenticates with their own Google account via the MCP OAuth flow (RFC 9728). The server acts as an OAuth proxy to Google.

```bash
gdrive-mcp-server --transport http --http-addr 0.0.0.0:3000 --credentials-file client_secret.json
```

The MCP endpoint will be available at `http://localhost:3000/mcp`.

**OAuth flow:**
1. MCP client discovers OAuth metadata via `/.well-known/oauth-protected-resource`
2. Client registers dynamically via `/oauth/register` (RFC 7591)
3. Authorization redirects to Google OAuth consent screen
4. User authenticates with their Google account
5. Server issues per-user MCP tokens bound to Google tokens
6. Each user gets isolated Google Drive access

### Environment Variables

All CLI options can be set via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `GDRIVE_MCP_TRANSPORT` | `stdio` | Transport mode: `stdio` or `http` |
| `GDRIVE_MCP_HTTP_ADDR` | `127.0.0.1:3000` | HTTP bind address |
| `GDRIVE_MCP_CREDENTIALS` | -- | Path to OAuth2 credentials JSON |
| `GDRIVE_MCP_TOKEN_CACHE` | `~/.gdrive-mcp-token.json` | Token cache path (stdio mode) |
| `GDRIVE_MCP_LOG_LEVEL` | `info` | Log level: `error`, `warn`, `info`, `debug`, `trace` |

### Claude Desktop

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "gdrive": {
      "command": "/path/to/gdrive-mcp-server",
      "args": ["--credentials-file", "/path/to/client_secret.json"]
    }
  }
}
```

### MCP Inspector

```bash
# Stdio
npx @modelcontextprotocol/inspector ./target/release/gdrive-mcp-server -- --credentials-file client_secret.json

# HTTP
npx @modelcontextprotocol/inspector http://localhost:3000/mcp
```

### Service Account

For server-to-server usage without user interaction, use a service account key:

```bash
gdrive-mcp-server --credentials-file service-account-key.json
```

The server auto-detects the credential type from the JSON file.

## Architecture

```
crates/
  gdrive-mcp-server/        # Binary crate (thin main.rs)
  gdrive-mcp-core/          # Library crate (all logic)
    src/
      auth.rs                # OAuth2 setup, token persistence, service account
      client.rs              # DriveClient wrapper
      config.rs              # CLI args + env vars (clap)
      convert.rs             # Google Doc format conversions
      error.rs               # Error types (thiserror)
      oauth.rs               # MCP OAuth 2.1 proxy to Google (HTTP mode)
      server.rs              # GDriveServer, ServerHandler impl
      transport.rs           # stdio / Streamable HTTP switching
      tools/                 # 41 MCP tools across 9 domain modules
      resources/             # Resource templates (file, folder)
      prompts/               # Prompts (search, organize, sharing)
```

## Tech Stack

| Component | Choice |
|-----------|--------|
| MCP SDK | [rmcp](https://github.com/anthropics/rust-sdk) v0.16 |
| Google Drive API | [google-drive3](https://docs.rs/google-drive3) v7.0 |
| OAuth2 | [yup-oauth2](https://docs.rs/yup-oauth2) v12 |
| Async Runtime | [tokio](https://tokio.rs) |
| HTTP Framework | [axum](https://docs.rs/axum) v0.8 |
| CLI | [clap](https://docs.rs/clap) v4 |

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes
4. Push to the branch
5. Open a Pull Request

## License

This project is licensed under the [MIT License](LICENSE).

---

**Roadmap:** Gmail tools, Google Calendar, and more Google services are planned. Contributions welcome!
