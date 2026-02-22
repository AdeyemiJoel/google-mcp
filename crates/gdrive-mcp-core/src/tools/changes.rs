use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ChangesGetStartPageTokenParams {
    /// Shared drive ID (optional, for shared drive changes).
    #[serde(default)]
    pub drive_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ChangesListParams {
    /// The page token returned by a previous call or getStartPageToken.
    pub page_token: String,
    /// Maximum number of changes to return.
    #[serde(default)]
    pub page_size: Option<i32>,
    /// Shared drive ID (optional).
    #[serde(default)]
    pub drive_id: Option<String>,
    /// Include items from all drives.
    #[serde(default)]
    pub include_items_from_all_drives: Option<bool>,
}

#[tool_router(router = changes_tool_router, vis = "pub")]
impl GDriveServer {
    /// Get the starting page token for listing future changes.
    #[tool(name = "gdrive_changes_get_start_page_token")]
    async fn changes_get_start_page_token(
        &self,
        Parameters(params): Parameters<ChangesGetStartPageTokenParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = self.client.hub().changes().get_start_page_token();

        if let Some(drive_id) = &params.drive_id {
            req = req.drive_id(drive_id);
            req = req.supports_all_drives(true);
        }

        let (_, result) = req.doit().await.map_err(drive_err)?;

        let token = result
            .start_page_token
            .as_deref()
            .unwrap_or("(no token returned)");
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Start page token: {token}"
        ))]))
    }

    /// List changes to files and shared drives since a given page token.
    #[tool(name = "gdrive_changes_list")]
    async fn changes_list(
        &self,
        Parameters(params): Parameters<ChangesListParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = self
            .client
            .hub()
            .changes()
            .list(&params.page_token)
            .param("fields", "nextPageToken,newStartPageToken,changes(fileId,removed,time,file(id,name,mimeType,trashed))");

        if let Some(ps) = params.page_size {
            req = req.page_size(ps);
        }
        if let Some(drive_id) = &params.drive_id {
            req = req.drive_id(drive_id);
            req = req.supports_all_drives(true);
            req = req.include_items_from_all_drives(true);
        }
        if let Some(include_all) = params.include_items_from_all_drives {
            if include_all {
                req = req.include_items_from_all_drives(true);
                req = req.supports_all_drives(true);
            }
        }

        let (_, list) = req.doit().await.map_err(drive_err)?;

        let changes = list.changes.unwrap_or_default();
        let mut result = if changes.is_empty() {
            "No changes found.".to_string()
        } else {
            changes
                .iter()
                .map(|c| {
                    let file_id = c.file_id.as_deref().unwrap_or("?");
                    let removed = c.removed.unwrap_or(false);
                    let time = c.time.as_ref().map(|t| t.to_string()).unwrap_or_default();
                    if removed {
                        format!("[{time}] REMOVED: {file_id}")
                    } else if let Some(f) = &c.file {
                        let name = f.name.as_deref().unwrap_or("?");
                        format!("[{time}] {name} ({file_id})")
                    } else {
                        format!("[{time}] {file_id}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        if let Some(npt) = &list.next_page_token {
            result.push_str(&format!("\n\n[Next page token: {npt}]"));
        }
        if let Some(nspt) = &list.new_start_page_token {
            result.push_str(&format!("\n[New start page token: {nspt}]"));
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

fn drive_err(e: google_drive3::Error) -> McpError {
    McpError::internal_error(format!("Google Drive API error: {e}"), None)
}
