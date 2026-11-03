use std::path::Path;

use super::{HandlerContext, HandlerError, HandlerResult};

impl HandlerContext {
    /// Builds the project graph off the async runtime. The build stops, and
    /// publishes nothing, as soon as this request is cancelled.
    pub(super) async fn build_graph(&self, dir: &Path) -> HandlerResult<()> {
        self.analysis_service
            .build_project_graph_blocking(dir, self.cancellation_token().clone())
            .await
            .map_err(HandlerError::App)
    }
}
