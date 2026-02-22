use rmcp::{
    ErrorData as McpError,
    model::*,
    prompt, prompt_router,
    handler::server::wrapper::Parameters,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, serde::Serialize, Deserialize, schemars::JsonSchema)]
pub struct SharingGuideArgs {
    /// Describe what you want to share and with whom.
    pub scenario: String,
}

#[prompt_router(router = "sharing_guide_prompt_router", vis = "pub")]
impl GDriveServer {
    /// Get guidance on sharing files and folders in Google Drive, including
    /// permissions, roles, link sharing, and security best practices.
    #[prompt(name = "gdrive_sharing_guide")]
    async fn sharing_guide(
        &self,
        Parameters(args): Parameters<SharingGuideArgs>,
    ) -> Result<GetPromptResult, McpError> {
        let messages = vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    r#"Help me set up sharing for Google Drive. My scenario: "{}"

Available permission roles:
- reader — can view only
- commenter — can view and comment
- writer — can view, comment, and edit
- fileOrganizer — can manage files within shared drive
- organizer — can manage members and settings of shared drive
- owner — full ownership (only for My Drive files)

Permission types:
- user — share with specific user email
- group — share with a Google Group
- domain — share with everyone in a domain
- anyone — share with anyone who has the link

Available tools:
- gdrive_permissions_create — add a new permission
- gdrive_permissions_list — list current permissions
- gdrive_permissions_update — change a permission role
- gdrive_permissions_delete — remove a permission
- gdrive_files_get — check current sharing status

Security best practices:
1. Use the principle of least privilege
2. Prefer sharing with specific users over "anyone with link"
3. Set expiration dates for temporary access
4. Use domain-restricted sharing for sensitive content
5. Regularly audit permissions on important files
6. Use shared drives for team content instead of personal shares

Please provide step-by-step instructions for my scenario."#,
                    args.scenario
                ),
            ),
        ];

        Ok(GetPromptResult {
            description: Some(format!(
                "Sharing guide for: {}",
                args.scenario
            )),
            messages,
        })
    }
}
