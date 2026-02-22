use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RepliesCreateParams {
    /// The file ID.
    pub file_id: String,
    /// The comment ID to reply to.
    pub comment_id: String,
    /// The reply text content.
    pub content: String,
    /// Action: "resolve" or "reopen" (optional, to change comment resolved state).
    #[serde(default)]
    pub action: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RepliesListParams {
    /// The file ID.
    pub file_id: String,
    /// The comment ID.
    pub comment_id: String,
    /// Maximum number of replies to return.
    #[serde(default)]
    pub page_size: Option<i32>,
    /// Page token for pagination.
    #[serde(default)]
    pub page_token: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RepliesGetParams {
    /// The file ID.
    pub file_id: String,
    /// The comment ID.
    pub comment_id: String,
    /// The reply ID.
    pub reply_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RepliesUpdateParams {
    /// The file ID.
    pub file_id: String,
    /// The comment ID.
    pub comment_id: String,
    /// The reply ID.
    pub reply_id: String,
    /// New content for the reply.
    pub content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RepliesDeleteParams {
    /// The file ID.
    pub file_id: String,
    /// The comment ID.
    pub comment_id: String,
    /// The reply ID to delete.
    pub reply_id: String,
}

#[tool_router(router = replies_tool_router, vis = "pub")]
impl GDriveServer {
    /// Create a reply to a comment.
    #[tool(name = "gdrive_replies_create")]
    async fn replies_create(
        &self,
        Parameters(params): Parameters<RepliesCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut reply = google_drive3::api::Reply::default();
        reply.content = Some(params.content);
        reply.action = params.action;

        let (_, result) = self
            .client
            .hub()
            .replies()
            .create(reply, &params.file_id, &params.comment_id)
            .param("fields", "id,content,author,createdTime,modifiedTime,action")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Reply created:\n{json}"
        ))]))
    }

    /// List replies to a comment.
    #[tool(name = "gdrive_replies_list")]
    async fn replies_list(
        &self,
        Parameters(params): Parameters<RepliesListParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = self
            .client
            .hub()
            .replies()
            .list(&params.file_id, &params.comment_id)
            .param("fields", "replies(id,content,author,createdTime,modifiedTime,action),nextPageToken");

        if let Some(ps) = params.page_size {
            req = req.page_size(ps);
        }
        if let Some(pt) = &params.page_token {
            req = req.page_token(pt);
        }

        let (_, list) = req.doit().await.map_err(drive_err)?;

        let mut json = serde_json::to_string_pretty(&list.replies)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if let Some(npt) = &list.next_page_token {
            json.push_str(&format!("\n\n[Next page token: {npt}]"));
        }
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get a specific reply by ID.
    #[tool(name = "gdrive_replies_get")]
    async fn replies_get(
        &self,
        Parameters(params): Parameters<RepliesGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let (_, reply) = self
            .client
            .hub()
            .replies()
            .get(&params.file_id, &params.comment_id, &params.reply_id)
            .param("fields", "id,content,author,createdTime,modifiedTime,action")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&reply)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Update a reply's content.
    #[tool(name = "gdrive_replies_update")]
    async fn replies_update(
        &self,
        Parameters(params): Parameters<RepliesUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut reply = google_drive3::api::Reply::default();
        reply.content = Some(params.content);

        let (_, result) = self
            .client
            .hub()
            .replies()
            .update(reply, &params.file_id, &params.comment_id, &params.reply_id)
            .param("fields", "id,content,author,modifiedTime")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Reply updated:\n{json}"
        ))]))
    }

    /// Delete a reply from a comment.
    #[tool(name = "gdrive_replies_delete")]
    async fn replies_delete(
        &self,
        Parameters(params): Parameters<RepliesDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.client
            .hub()
            .replies()
            .delete(&params.file_id, &params.comment_id, &params.reply_id)
            .doit()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Reply {} deleted from comment {} on file {}",
            params.reply_id, params.comment_id, params.file_id
        ))]))
    }
}

fn drive_err(e: google_drive3::Error) -> McpError {
    McpError::internal_error(format!("Google Drive API error: {e}"), None)
}
