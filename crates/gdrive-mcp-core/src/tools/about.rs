use rmcp::{
    ErrorData as McpError,
    model::*,
    tool, tool_router,
};

use crate::server::GDriveServer;

#[tool_router(router = about_tool_router, vis = "pub")]
impl GDriveServer {
    /// Get information about the authenticated user, storage quota, and supported import/export formats.
    #[tool(name = "gdrive_about_get")]
    async fn about_get(&self) -> Result<CallToolResult, McpError> {
        let (_, about) = self
            .client
            .hub()
            .about()
            .get()
            .param("fields", "user,storageQuota,importFormats,exportFormats,maxUploadSize")
            .doit()
            .await
            .map_err(|e| McpError::internal_error(format!("Google Drive API error: {e}"), None))?;

        let json = serde_json::to_string_pretty(&about)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
