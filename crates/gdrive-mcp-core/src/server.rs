use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::router::{prompt::PromptRouter, tool::ToolRouter},
    model::*,
    prompt_handler, tool_handler,
    service::RequestContext,
};

use crate::client::DriveClient;
use crate::resources;

/// The main MCP server for Google Drive.
#[derive(Clone)]
pub struct GDriveServer {
    pub(crate) client: DriveClient,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl GDriveServer {
    pub fn new(client: DriveClient) -> Self {
        Self {
            client,
            tool_router: crate::tools::build_tool_router(),
            prompt_router: crate::prompts::build_prompt_router(),
        }
    }

    /// Create a new server instance with a different DriveClient but reusing
    /// the same tool and prompt routers. Used in HTTP mode to create per-session
    /// servers with per-user Google tokens.
    pub fn with_client(&self, client: DriveClient) -> Self {
        Self {
            client,
            tool_router: self.tool_router.clone(),
            prompt_router: self.prompt_router.clone(),
        }
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for GDriveServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            server_info: Implementation {
                name: "gdrive-mcp-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Google Drive MCP Server".to_string()),
                description: Some("Comprehensive Google Drive access via MCP".to_string()),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Google Drive MCP Server - provides comprehensive access to Google Drive \
                 including file operations, permissions, comments, revisions, shared drives, \
                 and more. Use gdrive_files_list to search for files, gdrive_files_get to \
                 read file content, and gdrive_files_create to create new files."
                    .to_string(),
            ),
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![],
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: resources::resource_templates(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        resources::read_resource(&self.client, &request.uri).await
    }
}
