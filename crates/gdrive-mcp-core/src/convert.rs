use google_drive3::api::File;
use http_body_util::BodyExt;

use crate::client::DriveClient;
use crate::error::{GDriveError, Result};

/// Google Workspace MIME types.
pub const MIME_GOOGLE_DOC: &str = "application/vnd.google-apps.document";
pub const MIME_GOOGLE_SHEET: &str = "application/vnd.google-apps.spreadsheet";
pub const MIME_GOOGLE_SLIDES: &str = "application/vnd.google-apps.presentation";
pub const MIME_GOOGLE_DRAWING: &str = "application/vnd.google-apps.drawing";
pub const MIME_GOOGLE_FORM: &str = "application/vnd.google-apps.form";
pub const MIME_GOOGLE_SCRIPT: &str = "application/vnd.google-apps.script";
pub const MIME_FOLDER: &str = "application/vnd.google-apps.folder";

/// Check if a MIME type is a Google Workspace type that requires export.
pub fn is_google_workspace_type(mime_type: &str) -> bool {
    matches!(
        mime_type,
        MIME_GOOGLE_DOC
            | MIME_GOOGLE_SHEET
            | MIME_GOOGLE_SLIDES
            | MIME_GOOGLE_DRAWING
            | MIME_GOOGLE_FORM
            | MIME_GOOGLE_SCRIPT
    )
}

/// Get the default export MIME type for a Google Workspace document.
pub fn default_export_mime(workspace_mime: &str) -> Option<&'static str> {
    match workspace_mime {
        MIME_GOOGLE_DOC => Some("text/markdown"),
        MIME_GOOGLE_SHEET => Some("text/csv"),
        MIME_GOOGLE_SLIDES => Some("text/plain"),
        MIME_GOOGLE_DRAWING => Some("image/png"),
        MIME_GOOGLE_FORM => Some("text/plain"),
        MIME_GOOGLE_SCRIPT => Some("application/vnd.google-apps.script+json"),
        _ => None,
    }
}

/// Export a Google Workspace document as text content.
pub async fn export_as_text(
    client: &DriveClient,
    file_id: &str,
    export_mime: &str,
) -> Result<String> {
    let response = client
        .hub()
        .files()
        .export(file_id, export_mime)
        .doit()
        .await
        .map_err(GDriveError::DriveApi)?;

    read_body_as_text(response).await
}

/// Export a Google Workspace document as raw bytes.
pub async fn export_as_bytes(
    client: &DriveClient,
    file_id: &str,
    export_mime: &str,
) -> Result<Vec<u8>> {
    let response = client
        .hub()
        .files()
        .export(file_id, export_mime)
        .doit()
        .await
        .map_err(GDriveError::DriveApi)?;

    read_body_as_bytes(response).await
}

/// Download a regular (non-Workspace) file's content as text.
pub async fn download_as_text(client: &DriveClient, file_id: &str) -> Result<String> {
    let (response, _) = client
        .hub()
        .files()
        .get(file_id)
        .param("alt", "media")
        .doit()
        .await
        .map_err(GDriveError::DriveApi)?;

    read_body_as_text(response).await
}

/// Download a regular (non-Workspace) file's content as raw bytes.
pub async fn download_as_bytes(client: &DriveClient, file_id: &str) -> Result<Vec<u8>> {
    let (response, _) = client
        .hub()
        .files()
        .get(file_id)
        .param("alt", "media")
        .doit()
        .await
        .map_err(GDriveError::DriveApi)?;

    read_body_as_bytes(response).await
}

/// Read a hyper response body as a UTF-8 string.
async fn read_body_as_text(response: google_drive3::common::Response) -> Result<String> {
    let bytes = read_body_as_bytes(response).await?;
    String::from_utf8(bytes)
        .map_err(|e| GDriveError::Other(format!("Response body is not valid UTF-8: {e}")))
}

/// Read a hyper response body as raw bytes.
async fn read_body_as_bytes(response: google_drive3::common::Response) -> Result<Vec<u8>> {
    let body = response.into_body();
    let bytes = body
        .collect()
        .await
        .map_err(|e| GDriveError::HttpBody(format!("Failed to read response body: {e}")))?
        .to_bytes();
    Ok(bytes.to_vec())
}

/// Format a File metadata as a human-readable summary line.
pub fn file_summary(file: &File) -> String {
    let name = file.name.as_deref().unwrap_or("(unnamed)");
    let id = file.id.as_deref().unwrap_or("(no id)");
    let mime = file.mime_type.as_deref().unwrap_or("unknown");
    format!("{name} (id: {id}, type: {mime})")
}

/// Format a list of File objects as a summary string.
pub fn files_summary(files: &[File]) -> String {
    if files.is_empty() {
        return "No files found.".to_string();
    }
    files
        .iter()
        .map(file_summary)
        .collect::<Vec<_>>()
        .join("\n")
}
