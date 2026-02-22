mod organize_files;
mod search_help;
mod sharing_guide;

use rmcp::handler::server::router::prompt::PromptRouter;

use crate::server::GDriveServer;

/// Build the merged prompt router containing all prompts.
pub fn build_prompt_router() -> PromptRouter<GDriveServer> {
    let mut router = GDriveServer::search_help_prompt_router();
    router.merge(GDriveServer::organize_files_prompt_router());
    router.merge(GDriveServer::sharing_guide_prompt_router());
    router
}
