use rmcp::{
    ErrorData as McpError,
    model::*,
    prompt, prompt_router,
    handler::server::wrapper::Parameters,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, serde::Serialize, Deserialize, schemars::JsonSchema)]
pub struct OrganizeFilesArgs {
    /// Describe your current file organization problem or goal.
    pub situation: String,
}

#[prompt_router(router = "organize_files_prompt_router", vis = "pub")]
impl GDriveServer {
    /// Get guidance on organizing files and folders in Google Drive, including
    /// best practices for folder structure, naming conventions, and using shared drives.
    #[prompt(name = "gdrive_organize_files")]
    async fn organize_files(
        &self,
        Parameters(args): Parameters<OrganizeFilesArgs>,
    ) -> Result<GetPromptResult, McpError> {
        let messages = vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    r#"Help me organize my Google Drive files. My situation: "{}"

Please suggest a plan using these available tools:
- gdrive_files_list — to find and audit existing files
- gdrive_files_create — to create new folders (mimeType: application/vnd.google-apps.folder)
- gdrive_files_move — to move files into organized folders
- gdrive_files_update — to rename files
- gdrive_files_trash — to clean up unneeded files
- gdrive_drives_create — to create shared drives for team content

Best practices to consider:
1. Use a clear folder hierarchy (e.g., Year/Project/Type)
2. Use consistent naming conventions
3. Separate personal and shared content
4. Use shared drives for team collaboration
5. Archive old files rather than deleting them
6. Use starring for frequently accessed files

Please provide step-by-step instructions using the tools above."#,
                    args.situation
                ),
            ),
        ];

        Ok(GetPromptResult {
            description: Some(format!(
                "File organization guidance for: {}",
                args.situation
            )),
            messages,
        })
    }
}
