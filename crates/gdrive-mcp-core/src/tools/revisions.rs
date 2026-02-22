use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RevisionsListParams {
    /// The file ID.
    pub file_id: String,
    /// Maximum number of revisions to return.
    #[serde(default)]
    pub page_size: Option<i32>,
    /// Page token for pagination.
    #[serde(default)]
    pub page_token: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RevisionsGetParams {
    /// The file ID.
    pub file_id: String,
    /// The revision ID.
    pub revision_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RevisionsUpdateParams {
    /// The file ID.
    pub file_id: String,
    /// The revision ID.
    pub revision_id: String,
    /// Whether to keep this revision forever (prevents auto-purge).
    #[serde(default)]
    pub keep_forever: Option<bool>,
    /// Whether this revision is published (for Google Docs).
    #[serde(default)]
    pub published: Option<bool>,
    /// Whether to publish auto-republish (for Google Docs).
    #[serde(default)]
    pub publish_auto: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RevisionsDeleteParams {
    /// The file ID.
    pub file_id: String,
    /// The revision ID to delete.
    pub revision_id: String,
}

#[tool_router(router = revisions_tool_router, vis = "pub")]
impl GDriveServer {
    /// List revisions of a file.
    #[tool(name = "gdrive_revisions_list")]
    async fn revisions_list(
        &self,
        Parameters(params): Parameters<RevisionsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = self
            .client
            .hub()
            .revisions()
            .list(&params.file_id)
            .param("fields", "revisions(id,modifiedTime,lastModifyingUser,size,keepForever),nextPageToken");

        if let Some(ps) = params.page_size {
            req = req.page_size(ps);
        }
        if let Some(pt) = &params.page_token {
            req = req.page_token(pt);
        }

        let (_, list) = req.doit().await.map_err(drive_err)?;

        let mut json = serde_json::to_string_pretty(&list.revisions)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if let Some(npt) = &list.next_page_token {
            json.push_str(&format!("\n\n[Next page token: {npt}]"));
        }
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get metadata for a specific revision.
    #[tool(name = "gdrive_revisions_get")]
    async fn revisions_get(
        &self,
        Parameters(params): Parameters<RevisionsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let (_, rev) = self
            .client
            .hub()
            .revisions()
            .get(&params.file_id, &params.revision_id)
            .param("fields", "id,mimeType,modifiedTime,lastModifyingUser,size,keepForever,published,publishAuto,exportLinks")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&rev)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Update revision metadata (keepForever, published, publishAuto).
    #[tool(name = "gdrive_revisions_update")]
    async fn revisions_update(
        &self,
        Parameters(params): Parameters<RevisionsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut rev = google_drive3::api::Revision::default();
        rev.keep_forever = params.keep_forever;
        rev.published = params.published;
        rev.publish_auto = params.publish_auto;

        let (_, result) = self
            .client
            .hub()
            .revisions()
            .update(rev, &params.file_id, &params.revision_id)
            .param("fields", "id,modifiedTime,keepForever,published,publishAuto")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Revision updated:\n{json}"
        ))]))
    }

    /// Delete a specific revision (only for files with multiple revisions).
    #[tool(name = "gdrive_revisions_delete")]
    async fn revisions_delete(
        &self,
        Parameters(params): Parameters<RevisionsDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.client
            .hub()
            .revisions()
            .delete(&params.file_id, &params.revision_id)
            .doit()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Revision {} deleted from file {}",
            params.revision_id, params.file_id
        ))]))
    }
}

fn drive_err(e: google_drive3::Error) -> McpError {
    McpError::internal_error(format!("Google Drive API error: {e}"), None)
}
