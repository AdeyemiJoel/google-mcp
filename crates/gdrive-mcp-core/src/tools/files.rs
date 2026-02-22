use base64::Engine;
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router,
};
use serde::Deserialize;

use crate::convert;
use crate::server::GDriveServer;

// ── Parameter types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesListParams {
    /// Google Drive search query (e.g. "name contains 'report'"). See Drive query syntax.
    #[serde(default)]
    pub query: Option<String>,
    /// Maximum number of files to return (1-1000, default 100).
    #[serde(default)]
    pub page_size: Option<i32>,
    /// Page token for pagination.
    #[serde(default)]
    pub page_token: Option<String>,
    /// Sort order (e.g. "modifiedTime desc", "name").
    #[serde(default)]
    pub order_by: Option<String>,
    /// Shared drive ID to search in.
    #[serde(default)]
    pub drive_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesGetParams {
    /// The ID of the file to retrieve.
    pub file_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesCreateParams {
    /// Name for the new file.
    pub name: String,
    /// MIME type of the file (e.g. "text/plain", "application/vnd.google-apps.document").
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Parent folder ID. Defaults to root.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Text content for the file.
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesUpdateParams {
    /// The ID of the file to update.
    pub file_id: String,
    /// New name for the file.
    #[serde(default)]
    pub name: Option<String>,
    /// New text content for the file.
    #[serde(default)]
    pub content: Option<String>,
    /// New MIME type.
    #[serde(default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesDeleteParams {
    /// The ID of the file to permanently delete.
    pub file_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesCopyParams {
    /// The ID of the file to copy.
    pub file_id: String,
    /// Name for the copy.
    #[serde(default)]
    pub name: Option<String>,
    /// Parent folder ID for the copy.
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesMoveParams {
    /// The ID of the file to move.
    pub file_id: String,
    /// The ID of the new parent folder.
    pub new_parent_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesTrashParams {
    /// The ID of the file to trash.
    pub file_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesExportParams {
    /// The ID of the Google Workspace file to export.
    pub file_id: String,
    /// Export MIME type (e.g. "text/markdown", "text/csv", "application/pdf").
    pub export_mime_type: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FilesDownloadParams {
    /// The ID of the file to download.
    pub file_id: String,
}

// ── Tool implementations ────────────────────────────────────────────

#[tool_router(router = files_tool_router, vis = "pub")]
impl GDriveServer {
    /// Search and list files in Google Drive. Supports Drive query syntax for filtering,
    /// pagination, and sorting. Returns file names, IDs, and types.
    #[tool(name = "gdrive_files_list")]
    async fn files_list(
        &self,
        Parameters(params): Parameters<FilesListParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = self.client.hub().files().list();
        req = req.param("fields", "nextPageToken,files(id,name,mimeType,modifiedTime,size,parents,webViewLink)");

        if let Some(q) = &params.query {
            req = req.q(q);
        }
        if let Some(ps) = params.page_size {
            req = req.page_size(ps);
        }
        if let Some(pt) = &params.page_token {
            req = req.page_token(pt);
        }
        if let Some(ob) = &params.order_by {
            req = req.order_by(ob);
        }
        if let Some(drive_id) = &params.drive_id {
            req = req.drive_id(drive_id);
            req = req.include_items_from_all_drives(true);
            req = req.supports_all_drives(true);
            req = req.corpora("drive");
        }

        let (_, file_list) = req.doit().await.map_err(drive_err)?;

        let files = file_list.files.unwrap_or_default();
        let mut result = convert::files_summary(&files);

        if let Some(npt) = file_list.next_page_token {
            result.push_str(&format!("\n\n[Next page token: {npt}]"));
        }

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    /// Get detailed metadata for a specific file by its ID.
    #[tool(name = "gdrive_files_get")]
    async fn files_get(
        &self,
        Parameters(params): Parameters<FilesGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let (_, file) = self
            .client
            .hub()
            .files()
            .get(&params.file_id)
            .param("fields", "id,name,mimeType,modifiedTime,createdTime,size,parents,webViewLink,description,starred,trashed,shared,owners,permissions")
            .supports_all_drives(true)
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Create a new file in Google Drive with optional content.
    #[tool(name = "gdrive_files_create")]
    async fn files_create(
        &self,
        Parameters(params): Parameters<FilesCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut file_meta = google_drive3::api::File::default();
        file_meta.name = Some(params.name);

        if let Some(mime) = &params.mime_type {
            file_meta.mime_type = Some(mime.clone());
        }
        if let Some(parent) = &params.parent_id {
            file_meta.parents = Some(vec![parent.clone()]);
        }

        let content_bytes = params.content.as_deref().unwrap_or("").as_bytes().to_vec();
        let upload_mime: mime::Mime = file_meta
            .mime_type
            .as_deref()
            .unwrap_or("text/plain")
            .parse()
            .unwrap_or(mime::TEXT_PLAIN);
        let cursor = std::io::Cursor::new(content_bytes);

        let (_, file) = self
            .client
            .hub()
            .files()
            .create(file_meta)
            .upload(cursor, upload_mime)
            .await
            .map_err(drive_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Created file: {}",
            convert::file_summary(&file)
        ))]))
    }

    /// Update a file's metadata and/or content.
    #[tool(name = "gdrive_files_update")]
    async fn files_update(
        &self,
        Parameters(params): Parameters<FilesUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut file_meta = google_drive3::api::File::default();

        if let Some(name) = &params.name {
            file_meta.name = Some(name.clone());
        }
        if let Some(mime) = &params.mime_type {
            file_meta.mime_type = Some(mime.clone());
        }

        let result = if let Some(content) = &params.content {
            let mime: mime::Mime = params
                .mime_type
                .as_deref()
                .unwrap_or("text/plain")
                .parse()
                .unwrap_or(mime::TEXT_PLAIN);
            let cursor = std::io::Cursor::new(content.as_bytes().to_vec());
            self.client
                .hub()
                .files()
                .update(file_meta, &params.file_id)
                .upload(cursor, mime)
                .await
                .map_err(drive_err)?
        } else {
            self.client
                .hub()
                .files()
                .update(file_meta, &params.file_id)
                .doit_without_upload()
                .await
                .map_err(drive_err)?
        };

        let file = result.1;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Updated file: {}",
            convert::file_summary(&file)
        ))]))
    }

    /// Permanently delete a file from Google Drive. This cannot be undone.
    #[tool(name = "gdrive_files_delete")]
    async fn files_delete(
        &self,
        Parameters(params): Parameters<FilesDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.client
            .hub()
            .files()
            .delete(&params.file_id)
            .supports_all_drives(true)
            .doit()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Permanently deleted file: {}",
            params.file_id
        ))]))
    }

    /// Copy a file, optionally with a new name or parent folder.
    #[tool(name = "gdrive_files_copy")]
    async fn files_copy(
        &self,
        Parameters(params): Parameters<FilesCopyParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut file_meta = google_drive3::api::File::default();

        if let Some(name) = &params.name {
            file_meta.name = Some(name.clone());
        }
        if let Some(parent) = &params.parent_id {
            file_meta.parents = Some(vec![parent.clone()]);
        }

        let (_, file) = self
            .client
            .hub()
            .files()
            .copy(file_meta, &params.file_id)
            .supports_all_drives(true)
            .doit()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Copied file: {}",
            convert::file_summary(&file)
        ))]))
    }

    /// Move a file to a different folder.
    #[tool(name = "gdrive_files_move")]
    async fn files_move(
        &self,
        Parameters(params): Parameters<FilesMoveParams>,
    ) -> Result<CallToolResult, McpError> {
        // First get current parents
        let (_, current) = self
            .client
            .hub()
            .files()
            .get(&params.file_id)
            .param("fields", "parents")
            .supports_all_drives(true)
            .doit()
            .await
            .map_err(drive_err)?;

        let remove_parents = current
            .parents
            .as_ref()
            .map(|p| p.join(","))
            .unwrap_or_default();

        let file_meta = google_drive3::api::File::default();
        let (_, file) = self
            .client
            .hub()
            .files()
            .update(file_meta, &params.file_id)
            .add_parents(&params.new_parent_id)
            .remove_parents(&remove_parents)
            .supports_all_drives(true)
            .doit_without_upload()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Moved file: {}",
            convert::file_summary(&file)
        ))]))
    }

    /// Move a file to the trash.
    #[tool(name = "gdrive_files_trash")]
    async fn files_trash(
        &self,
        Parameters(params): Parameters<FilesTrashParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut file_meta = google_drive3::api::File::default();
        file_meta.trashed = Some(true);

        let (_, file) = self
            .client
            .hub()
            .files()
            .update(file_meta, &params.file_id)
            .supports_all_drives(true)
            .doit_without_upload()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Trashed file: {}",
            convert::file_summary(&file)
        ))]))
    }

    /// Restore a file from the trash.
    #[tool(name = "gdrive_files_untrash")]
    async fn files_untrash(
        &self,
        Parameters(params): Parameters<FilesTrashParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut file_meta = google_drive3::api::File::default();
        file_meta.trashed = Some(false);

        let (_, file) = self
            .client
            .hub()
            .files()
            .update(file_meta, &params.file_id)
            .supports_all_drives(true)
            .doit_without_upload()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Restored file: {}",
            convert::file_summary(&file)
        ))]))
    }

    /// Permanently delete all files in the trash.
    #[tool(name = "gdrive_files_empty_trash")]
    async fn files_empty_trash(&self) -> Result<CallToolResult, McpError> {
        self.client
            .hub()
            .files()
            .empty_trash()
            .doit()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(
            "Trash emptied successfully.",
        )]))
    }

    /// Export a Google Workspace document to a specified format (e.g. Docs→Markdown, Sheets→CSV).
    #[tool(name = "gdrive_files_export")]
    async fn files_export(
        &self,
        Parameters(params): Parameters<FilesExportParams>,
    ) -> Result<CallToolResult, McpError> {
        // Check if it's a binary export
        if params.export_mime_type.starts_with("image/")
            || params.export_mime_type == "application/pdf"
        {
            let bytes = convert::export_as_bytes(&self.client, &params.file_id, &params.export_mime_type)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Exported as {} ({} bytes, base64-encoded):\n{encoded}",
                params.export_mime_type,
                bytes.len()
            ))]))
        } else {
            let text = convert::export_as_text(&self.client, &params.file_id, &params.export_mime_type)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text(text)]))
        }
    }

    /// Download a non-Google-Workspace file's content.
    #[tool(name = "gdrive_files_download")]
    async fn files_download(
        &self,
        Parameters(params): Parameters<FilesDownloadParams>,
    ) -> Result<CallToolResult, McpError> {
        // First get file metadata to check type
        let (_, file) = self
            .client
            .hub()
            .files()
            .get(&params.file_id)
            .param("fields", "id,name,mimeType,size")
            .supports_all_drives(true)
            .doit()
            .await
            .map_err(drive_err)?;

        let mime_type = file.mime_type.as_deref().unwrap_or("application/octet-stream");

        // If it's a Google Workspace type, export with default format
        if convert::is_google_workspace_type(mime_type) {
            let export_mime = convert::default_export_mime(mime_type)
                .ok_or_else(|| McpError::internal_error(
                    format!("Cannot determine export format for {mime_type}"),
                    None,
                ))?;

            if export_mime.starts_with("image/") {
                let bytes = convert::export_as_bytes(&self.client, &params.file_id, export_mime)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "[Exported as {export_mime}, {len} bytes, base64]\n{encoded}",
                    len = bytes.len()
                ))]));
            }

            let text = convert::export_as_text(&self.client, &params.file_id, export_mime)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return Ok(CallToolResult::success(vec![Content::text(text)]));
        }

        // Regular file: check if text-like
        if is_text_mime(mime_type) {
            let text = convert::download_as_text(&self.client, &params.file_id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text(text)]))
        } else {
            let bytes = convert::download_as_bytes(&self.client, &params.file_id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Ok(CallToolResult::success(vec![Content::text(format!(
                "[Binary file {mime_type}, {len} bytes, base64]\n{encoded}",
                len = bytes.len()
            ))]))
        }
    }
}

fn is_text_mime(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
        || mime_type == "application/json"
        || mime_type == "application/xml"
        || mime_type == "application/javascript"
        || mime_type == "application/typescript"
        || mime_type.ends_with("+xml")
        || mime_type.ends_with("+json")
}

fn drive_err(e: google_drive3::Error) -> McpError {
    McpError::internal_error(format!("Google Drive API error: {e}"), None)
}
