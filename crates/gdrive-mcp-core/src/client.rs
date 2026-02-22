use std::sync::Arc;

use crate::auth::DriveHubType;

/// Wrapper around the Google Drive API hub, shareable across tasks.
#[derive(Clone)]
pub struct DriveClient {
    hub: Arc<DriveHubType>,
}

impl DriveClient {
    pub fn new(hub: DriveHubType) -> Self {
        Self {
            hub: Arc::new(hub),
        }
    }

    /// Access the underlying DriveHub.
    pub fn hub(&self) -> &DriveHubType {
        &self.hub
    }
}
