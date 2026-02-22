use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DrivesCreateParams {
    /// Name of the shared drive.
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DrivesListParams {
    /// Maximum number of shared drives to return.
    #[serde(default)]
    pub page_size: Option<i32>,
    /// Page token for pagination.
    #[serde(default)]
    pub page_token: Option<String>,
    /// Search query for filtering shared drives.
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DrivesGetParams {
    /// The shared drive ID.
    pub drive_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DrivesUpdateParams {
    /// The shared drive ID.
    pub drive_id: String,
    /// New name for the shared drive.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DrivesDeleteParams {
    /// The shared drive ID to delete.
    pub drive_id: String,
}

#[tool_router(router = drives_tool_router, vis = "pub")]
impl GDriveServer {
    /// Create a new shared drive.
    #[tool(name = "gdrive_drives_create")]
    async fn drives_create(
        &self,
        Parameters(params): Parameters<DrivesCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut drive = google_drive3::api::Drive::default();
        drive.name = Some(params.name);

        // requestId is required for idempotency
        let request_id = format!("mcp-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis());

        let (_, result) = self
            .client
            .hub()
            .drives()
            .create(drive, &request_id)
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Shared drive created:\n{json}"
        ))]))
    }

    /// List shared drives the user has access to.
    #[tool(name = "gdrive_drives_list")]
    async fn drives_list(
        &self,
        Parameters(params): Parameters<DrivesListParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = self.client.hub().drives().list();

        if let Some(ps) = params.page_size {
            req = req.page_size(ps);
        }
        if let Some(pt) = &params.page_token {
            req = req.page_token(pt);
        }
        if let Some(q) = &params.query {
            req = req.q(q);
        }

        let (_, list) = req.doit().await.map_err(drive_err)?;

        let drives = list.drives.unwrap_or_default();
        let mut result = if drives.is_empty() {
            "No shared drives found.".to_string()
        } else {
            drives
                .iter()
                .map(|d| {
                    let name = d.name.as_deref().unwrap_or("(unnamed)");
                    let id = d.id.as_deref().unwrap_or("(no id)");
                    format!("{name} (id: {id})")
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        if let Some(npt) = &list.next_page_token {
            result.push_str(&format!("\n\n[Next page token: {npt}]"));
        }
        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Get metadata for a specific shared drive.
    #[tool(name = "gdrive_drives_get")]
    async fn drives_get(
        &self,
        Parameters(params): Parameters<DrivesGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let (_, drive) = self
            .client
            .hub()
            .drives()
            .get(&params.drive_id)
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&drive)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Update a shared drive's metadata.
    #[tool(name = "gdrive_drives_update")]
    async fn drives_update(
        &self,
        Parameters(params): Parameters<DrivesUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut drive = google_drive3::api::Drive::default();
        if let Some(name) = &params.name {
            drive.name = Some(name.clone());
        }

        let (_, result) = self
            .client
            .hub()
            .drives()
            .update(drive, &params.drive_id)
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Shared drive updated:\n{json}"
        ))]))
    }

    /// Delete a shared drive (must be empty).
    #[tool(name = "gdrive_drives_delete")]
    async fn drives_delete(
        &self,
        Parameters(params): Parameters<DrivesDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.client
            .hub()
            .drives()
            .delete(&params.drive_id)
            .doit()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Shared drive {} deleted",
            params.drive_id
        ))]))
    }
}

fn drive_err(e: google_drive3::Error) -> McpError {
    McpError::internal_error(format!("Google Drive API error: {e}"), None)
}
