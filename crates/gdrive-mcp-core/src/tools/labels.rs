use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_router,
};
use serde::Deserialize;

use crate::server::GDriveServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LabelsListParams {
    /// The file ID.
    pub file_id: String,
    /// Maximum number of labels to return.
    #[serde(default)]
    pub max_results: Option<i32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LabelsModifyParams {
    /// The file ID.
    pub file_id: String,
    /// JSON body for the modifyLabels request. See Google Drive API docs for the format.
    /// Example: {"labelModifications": [{"labelId": "...", "fieldModifications": [...]}]}
    pub modifications: serde_json::Value,
}

#[tool_router(router = labels_tool_router, vis = "pub")]
impl GDriveServer {
    /// List labels applied to a file.
    #[tool(name = "gdrive_labels_list")]
    async fn labels_list(
        &self,
        Parameters(params): Parameters<LabelsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut req = self.client.hub().files().list_labels(&params.file_id);

        if let Some(max) = params.max_results {
            req = req.max_results(max);
        }

        let (_, list) = req.doit().await.map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&list)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Modify labels on a file (add, update, or remove label field values).
    #[tool(name = "gdrive_labels_modify")]
    async fn labels_modify(
        &self,
        Parameters(params): Parameters<LabelsModifyParams>,
    ) -> Result<CallToolResult, McpError> {
        let modify_req: google_drive3::api::ModifyLabelsRequest =
            serde_json::from_value(params.modifications)
                .map_err(|e| McpError::invalid_params(format!("Invalid modifications: {e}"), None))?;

        let (_, result) = self
            .client
            .hub()
            .files()
            .modify_labels(modify_req, &params.file_id)
            .doit()
            .await
            .map_err(drive_err)?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Labels modified:\n{json}"
        ))]))
    }
}

fn drive_err(e: google_drive3::Error) -> McpError {
    McpError::internal_error(format!("Google Drive API error: {e}"), None)
}
