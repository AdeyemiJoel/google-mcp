pub mod file_resource;
pub mod folder_resource;

use rmcp::{ErrorData as McpError, model::*};

use crate::client::DriveClient;

/// Return resource templates for Google Drive resources.
pub fn resource_templates() -> Vec<ResourceTemplate> {
    vec![
        RawResourceTemplate {
            uri_template: "gdrive:///{file_id}".to_string(),
            name: "Google Drive File".to_string(),
            title: None,
            description: Some(
                "Read the content of a Google Drive file. Google Workspace files are \
                 auto-converted: Docs to Markdown, Sheets to CSV, Slides to text, Drawings to PNG."
                    .to_string(),
            ),
            mime_type: None,
            icons: None,
        }
        .no_annotation(),
        RawResourceTemplate {
            uri_template: "gdrive:///folder/{folder_id}".to_string(),
            name: "Google Drive Folder".to_string(),
            title: None,
            description: Some(
                "List the contents of a Google Drive folder.".to_string(),
            ),
            mime_type: None,
            icons: None,
        }
        .no_annotation(),
    ]
}

/// Dispatch a resource read request based on URI.
pub async fn read_resource(
    client: &DriveClient,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    // Parse URI: gdrive:///folder/{id} or gdrive:///{file_id}
    let path = uri
        .strip_prefix("gdrive:///")
        .ok_or_else(|| McpError::resource_not_found(format!("Unknown URI scheme: {uri}"), None))?;

    if let Some(folder_id) = path.strip_prefix("folder/") {
        folder_resource::read_folder(client, folder_id, uri).await
    } else {
        // The rest is a file_id
        file_resource::read_file(client, path, uri).await
    }
}
