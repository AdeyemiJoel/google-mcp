use rmcp::{ErrorData as McpError, model::*};

use crate::client::DriveClient;
use crate::convert;

/// Read a folder resource - lists its contents.
pub async fn read_folder(
    client: &DriveClient,
    folder_id: &str,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    let query = format!("'{folder_id}' in parents and trashed = false");

    let (_, file_list) = client
        .hub()
        .files()
        .list()
        .q(&query)
        .param("fields", "files(id,name,mimeType,modifiedTime,size)")
        .page_size(100)
        .order_by("name")
        .supports_all_drives(true)
        .include_items_from_all_drives(true)
        .doit()
        .await
        .map_err(|e| McpError::internal_error(format!("Drive API error: {e}"), None))?;

    let files = file_list.files.unwrap_or_default();
    let text = convert::files_summary(&files);

    Ok(ReadResourceResult {
        contents: vec![ResourceContents::text(text, uri)],
    })
}
