use base64::Engine;
use rmcp::{ErrorData as McpError, model::*};

use crate::client::DriveClient;
use crate::convert;

/// Read a file resource, auto-converting Google Workspace types.
pub async fn read_file(
    client: &DriveClient,
    file_id: &str,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    // Get file metadata first
    let (_, file) = client
        .hub()
        .files()
        .get(file_id)
        .param("fields", "id,name,mimeType,size")
        .supports_all_drives(true)
        .doit()
        .await
        .map_err(|e| McpError::internal_error(format!("Drive API error: {e}"), None))?;

    let mime_type = file.mime_type.as_deref().unwrap_or("application/octet-stream");

    // Google Workspace types: export with default format
    if convert::is_google_workspace_type(mime_type) {
        let export_mime = convert::default_export_mime(mime_type).ok_or_else(|| {
            McpError::internal_error(
                format!("No default export format for {mime_type}"),
                None,
            )
        })?;

        // Binary exports (e.g. Drawings -> PNG)
        if export_mime.starts_with("image/") {
            let bytes = convert::export_as_bytes(client, file_id, export_mime)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            return Ok(ReadResourceResult {
                contents: vec![ResourceContents::BlobResourceContents {
                    uri: uri.to_string(),
                    mime_type: Some(export_mime.to_string()),
                    blob: base64::engine::general_purpose::STANDARD.encode(&bytes),
                    meta: None,
                }],
            });
        }

        // Text exports
        let text = convert::export_as_text(client, file_id, export_mime)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        return Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: uri.to_string(),
                mime_type: Some(export_mime.to_string()),
                text,
                meta: None,
            }],
        });
    }

    // Regular text files
    if is_text_mime(mime_type) {
        let text = convert::download_as_text(client, file_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        return Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: uri.to_string(),
                mime_type: Some(mime_type.to_string()),
                text,
                meta: None,
            }],
        });
    }

    // Binary files
    let bytes = convert::download_as_bytes(client, file_id)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(ReadResourceResult {
        contents: vec![ResourceContents::BlobResourceContents {
            uri: uri.to_string(),
            mime_type: Some(mime_type.to_string()),
            blob: base64::engine::general_purpose::STANDARD.encode(&bytes),
            meta: None,
        }],
    })
}

fn is_text_mime(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
        || mime_type == "application/json"
        || mime_type == "application/xml"
        || mime_type == "application/javascript"
        || mime_type.ends_with("+xml")
        || mime_type.ends_with("+json")
}
