use rmcp::ErrorData as McpError;

#[derive(Debug, thiserror::Error)]
pub enum GDriveError {
    #[error("Google Drive API error: {0}")]
    DriveApi(#[from] google_drive3::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("OAuth2 error: {0}")]
    OAuth2(String),

    #[error("HTTP error: {0}")]
    Http(#[from] hyper::Error),

    #[error("HTTP body error: {0}")]
    HttpBody(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid parameter: {0}")]
    InvalidParam(String),

    #[error("Export not supported: {0}")]
    ExportNotSupported(String),

    #[error("{0}")]
    Other(String),
}

impl From<GDriveError> for McpError {
    fn from(err: GDriveError) -> Self {
        match &err {
            GDriveError::NotFound(_) => McpError::resource_not_found(err.to_string(), None),
            GDriveError::InvalidParam(_) => McpError::invalid_params(err.to_string(), None),
            _ => McpError::internal_error(err.to_string(), None),
        }
    }
}

/// Convenience type alias for results returning GDriveError.
pub type Result<T> = std::result::Result<T, GDriveError>;
