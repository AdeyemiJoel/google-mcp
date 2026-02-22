mod about;
mod changes;
mod comments;
mod drives;
mod files;
mod labels;
mod permissions;
mod replies;
mod revisions;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::server::GDriveServer;

/// Build the merged tool router containing all 41 tools across 9 domains.
pub fn build_tool_router() -> ToolRouter<GDriveServer> {
    let mut router = GDriveServer::files_tool_router();
    router.merge(GDriveServer::permissions_tool_router());
    router.merge(GDriveServer::comments_tool_router());
    router.merge(GDriveServer::replies_tool_router());
    router.merge(GDriveServer::revisions_tool_router());
    router.merge(GDriveServer::drives_tool_router());
    router.merge(GDriveServer::changes_tool_router());
    router.merge(GDriveServer::about_tool_router());
    router.merge(GDriveServer::labels_tool_router());
    router
}
