use rmcp::{
    ErrorData as McpError,
    model::*,
    prompt, prompt_router,
    handler::server::wrapper::Parameters,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, serde::Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchHelpArgs {
    /// What kind of files are you looking for? (e.g. "PDFs modified last week", "shared spreadsheets")
    pub description: String,
}

#[prompt_router(router = "search_help_prompt_router", vis = "pub")]
impl GDriveServer {
    /// Get help building Google Drive search queries. Describe what you're looking for
    /// and get the correct Drive query syntax.
    #[prompt(name = "gdrive_search_help")]
    async fn search_help(
        &self,
        Parameters(args): Parameters<SearchHelpArgs>,
    ) -> Result<GetPromptResult, McpError> {
        let messages = vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    r#"Help me build a Google Drive search query for: "{}"

Google Drive query syntax reference:
- name contains 'keyword' — file name contains keyword
- fullText contains 'keyword' — full text search
- mimeType = 'application/pdf' — filter by MIME type
- 'folderId' in parents — files in a specific folder
- modifiedTime > '2024-01-01T00:00:00' — modified after date
- trashed = false — exclude trashed files
- starred = true — starred files only
- sharedWithMe = true — files shared with me
- owners = 'email@example.com' — files owned by user
- visibility = 'anyoneCanFind' — publicly visible files

Common MIME types:
- application/vnd.google-apps.document (Google Docs)
- application/vnd.google-apps.spreadsheet (Google Sheets)
- application/vnd.google-apps.presentation (Google Slides)
- application/vnd.google-apps.folder (folders)
- application/pdf (PDF files)
- text/plain (text files)

Operators: and, or, not
Example: name contains 'report' and mimeType = 'application/pdf' and modifiedTime > '2024-06-01T00:00:00'

Please generate the appropriate query and explain how to use it with gdrive_files_list."#,
                    args.description
                ),
            ),
        ];

        Ok(GetPromptResult {
            description: Some(format!(
                "Search help for: {}",
                args.description
            )),
            messages,
        })
    }
}
