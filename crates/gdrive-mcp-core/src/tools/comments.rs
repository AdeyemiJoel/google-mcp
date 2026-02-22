use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CommentsCreateParams {
    /// The file ID to comment on.
    pub file_id: String,
    /// The comment text content.
    pub content: String,
    /// Optional anchor text (the quoted text the comment refers to).
    #[serde(default)]
    pub anchor: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CommentsListParams {
    /// The file ID.
    pub file_id: String,
    /// Maximum number of comments to return.
    #[serde(default)]
    pub page_size: Option<i32>,
    /// Page token for pagination.
    #[serde(default)]
    pub page_token: Option<String>,
    /// Include deleted comments (default false).
    #[serde(default)]
    pub include_deleted: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CommentsGetParams {
    /// The file ID.
    pub file_id: String,
    /// The comment ID.
    pub comment_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CommentsUpdateParams {
    /// The file ID.
    pub file_id: String,
    /// The comment ID.
    pub comment_id: String,
    /// New content for the comment.
    pub content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CommentsDeleteParams {
    /// The file ID.
    pub file_id: String,
    /// The comment ID to delete.
    pub comment_id: String,
}

#[tool_router(router = comments_tool_router, vis = "pub")]
impl GDriveServer {
    /// Create a new comment on a file.
    #[tool(name = "gdrive_comments_create")]
    async fn comments_create(
        &self,
        Parameters(params): Parameters<CommentsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut comment = google_drive3::api::Comment::default();
        comment.content = Some(params.content);
        comment.anchor = params.anchor;

        let (_, result) = self
            .client
            .hub()
            .comments()
            .create(comment, &params.file_id)
            .param("fields", "id,content,author,createdTime,modifiedTime,resolved,anchor,quotedFileContent")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Comment created:\n{json}"
        ))]))
    }

    /// List comments on a file.
    #[tool(name = "gdrive_comments_list")]
    async fn comments_list(
        &self,
        Parameters(params): Parameters<CommentsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = self
            .client
            .hub()
            .comments()
            .list(&params.file_id)
            .param("fields", "comments(id,content,author,createdTime,modifiedTime,resolved),nextPageToken");

        if let Some(ps) = params.page_size {
            req = req.page_size(ps);
        }
        if let Some(pt) = &params.page_token {
            req = req.page_token(pt);
        }
        if let Some(del) = params.include_deleted {
            req = req.include_deleted(del);
        }

        let (_, list) = req.doit().await.map_err(drive_err)?;

        let mut json = serde_json::to_string_pretty(&list.comments)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if let Some(npt) = &list.next_page_token {
            json.push_str(&format!("\n\n[Next page token: {npt}]"));
        }
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get a specific comment by ID.
    #[tool(name = "gdrive_comments_get")]
    async fn comments_get(
        &self,
        Parameters(params): Parameters<CommentsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let (_, comment) = self
            .client
            .hub()
            .comments()
            .get(&params.file_id, &params.comment_id)
            .param("fields", "id,content,author,createdTime,modifiedTime,resolved,anchor,quotedFileContent,replies")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&comment)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Update the content of a comment.
    #[tool(name = "gdrive_comments_update")]
    async fn comments_update(
        &self,
        Parameters(params): Parameters<CommentsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut comment = google_drive3::api::Comment::default();
        comment.content = Some(params.content);

        let (_, result) = self
            .client
            .hub()
            .comments()
            .update(comment, &params.file_id, &params.comment_id)
            .param("fields", "id,content,author,modifiedTime")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Comment updated:\n{json}"
        ))]))
    }

    /// Delete a comment from a file.
    #[tool(name = "gdrive_comments_delete")]
    async fn comments_delete(
        &self,
        Parameters(params): Parameters<CommentsDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.client
            .hub()
            .comments()
            .delete(&params.file_id, &params.comment_id)
            .doit()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Comment {} deleted from file {}",
            params.comment_id, params.file_id
        ))]))
    }
}

fn drive_err(e: google_drive3::Error) -> McpError {
    McpError::internal_error(format!("Google Drive API error: {e}"), None)
}
