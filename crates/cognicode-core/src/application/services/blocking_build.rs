use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::analysis_service::AnalysisService;
use crate::application::error::{AppError, AppResult};

impl AnalysisService {
    /// Runs [`Self::build_project_graph`] on a blocking thread so the async
    /// runtime keeps serving other requests while the tree is parsed.
    /// Setting `cancel` makes the build stop and publish nothing.
    pub async fn build_project_graph_blocking(
        self: &Arc<Self>,
        project_dir: &Path,
        cancel: Arc<AtomicBool>,
    ) -> AppResult<()> {
        let service = Arc::clone(self);
        let dir = project_dir.to_path_buf();
        tokio::task::spawn_blocking(move || service.build_project_graph_cancellable(&dir, &cancel))
            .await
            .map_err(|e| AppError::AnalysisError(format!("graph build task failed: {e}")))?
    }
}
