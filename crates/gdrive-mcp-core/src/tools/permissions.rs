use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PermissionsCreateParams {
    /// The file or folder ID to share.
    pub file_id: String,
    /// The role: "owner", "organizer", "fileOrganizer", "writer", "commenter", "reader".
    pub role: String,
    /// The type: "user", "group", "domain", "anyone".
    #[serde(rename = "type")]
    pub perm_type: String,
    /// Email address (required for user/group types).
    #[serde(default)]
    pub email_address: Option<String>,
    /// Domain (required for domain type).
    #[serde(default)]
    pub domain: Option<String>,
    /// Send notification email (default true).
    #[serde(default)]
    pub send_notification: Option<bool>,
    /// Custom message for the notification email.
    #[serde(default)]
    pub email_message: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PermissionsListParams {
    /// The file or folder ID.
    pub file_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PermissionsGetParams {
    /// The file or folder ID.
    pub file_id: String,
    /// The permission ID.
    pub permission_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PermissionsUpdateParams {
    /// The file or folder ID.
    pub file_id: String,
    /// The permission ID.
    pub permission_id: String,
    /// New role for the permission.
    pub role: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PermissionsDeleteParams {
    /// The file or folder ID.
    pub file_id: String,
    /// The permission ID to remove.
    pub permission_id: String,
}

#[tool_router(router = permissions_tool_router, vis = "pub")]
impl GDriveServer {
    /// Create a new permission (share) on a file or folder. Supports user, group, domain, and anyone access.
    #[tool(name = "gdrive_permissions_create")]
    async fn permissions_create(
        &self,
        Parameters(params): Parameters<PermissionsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut perm = google_drive3::api::Permission::default();
        perm.role = Some(params.role);
        perm.type_ = Some(params.perm_type);
        perm.email_address = params.email_address;
        perm.domain = params.domain;

        let mut req = self.client.hub().permissions().create(perm, &params.file_id);
        req = req.supports_all_drives(true);

        if let Some(send) = params.send_notification {
            req = req.send_notification_email(send);
        }
        if let Some(msg) = &params.email_message {
            req = req.email_message(msg);
        }

        let (_, result) = req.doit().await.map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Permission created:\n{json}"
        ))]))
    }

    /// List all permissions on a file or folder.
    #[tool(name = "gdrive_permissions_list")]
    async fn permissions_list(
        &self,
        Parameters(params): Parameters<PermissionsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let (_, list) = self
            .client
            .hub()
            .permissions()
            .list(&params.file_id)
            .supports_all_drives(true)
            .param("fields", "permissions(id,type,role,emailAddress,domain,displayName)")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&list.permissions)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get a specific permission by ID.
    #[tool(name = "gdrive_permissions_get")]
    async fn permissions_get(
        &self,
        Parameters(params): Parameters<PermissionsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let (_, perm) = self
            .client
            .hub()
            .permissions()
            .get(&params.file_id, &params.permission_id)
            .supports_all_drives(true)
            .param("fields", "id,type,role,emailAddress,domain,displayName,expirationTime")
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&perm)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Update the role of an existing permission.
    #[tool(name = "gdrive_permissions_update")]
    async fn permissions_update(
        &self,
        Parameters(params): Parameters<PermissionsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut perm = google_drive3::api::Permission::default();
        perm.role = Some(params.role);

        let (_, result) = self
            .client
            .hub()
            .permissions()
            .update(perm, &params.file_id, &params.permission_id)
            .supports_all_drives(true)
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Permission updated:\n{json}"
        ))]))
    }

    /// Remove a permission (unshare) from a file or folder.
    #[tool(name = "gdrive_permissions_delete")]
    async fn permissions_delete(
        &self,
        Parameters(params): Parameters<PermissionsDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.client
            .hub()
            .permissions()
            .delete(&params.file_id, &params.permission_id)
            .supports_all_drives(true)
            .doit()
            .await
            .map_err(drive_err)?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Permission {} removed from file {}",
            params.permission_id, params.file_id
        ))]))
    }
}

fn drive_err(e: google_drive3::Error) -> McpError {
    McpError::internal_error(format!("Google Drive API error: {e}"), None)
}
