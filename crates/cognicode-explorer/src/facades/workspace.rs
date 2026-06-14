//! [`WorkspaceService`] implementation.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::dto::{OpenWorkspaceRequest, WorkspaceSummary};
use crate::error::ExplorerResult;
use crate::facades::WorkspaceService;
use crate::ports::symbol_repository::SymbolRepository;

/// Cap on the number of Spotter results returned per query.
const SPOTTER_RESULT_LIMIT: usize = 20;

/// Concrete implementation of [`WorkspaceService`].
///
/// Holds the same ports that `ExplorerService` uses for workspace operations.
pub struct WorkspaceServiceImpl {
    repo: Arc<dyn SymbolRepository>,
    root_path: PathBuf,
}

impl WorkspaceServiceImpl {
    pub fn new(repo: Arc<dyn SymbolRepository>, root_path: PathBuf) -> Self {
        Self { repo, root_path }
    }
}

#[async_trait]
impl WorkspaceService for WorkspaceServiceImpl {
    async fn open_workspace(
        &self,
        request: OpenWorkspaceRequest,
    ) -> ExplorerResult<WorkspaceSummary> {
        let root_path = PathBuf::from(&request.root_path);
        if !root_path.exists() {
            return Err(crate::error::ExplorerError::WorkspaceNotFound(
                request.root_path,
            ));
        }

        let db_path = root_path.join(".cognicode/cognicode.db");
        let graph_status = if db_path.exists() {
            crate::dto::GraphStatus::Ready
        } else {
            crate::dto::GraphStatus::Missing
        };

        // Spec Req 4: only populate real stats when the graph is ready.
        let (symbol_count, relation_count) = if db_path.exists() {
            let stats = self.repo.graph_stats();
            (stats.symbol_count, stats.relation_count)
        } else {
            (0, 0)
        };

        Ok(WorkspaceSummary {
            id: workspace_id(&root_path),
            root_path: root_path.display().to_string(),
            graph_status,
            indexed_at: None,
            symbol_count,
            relation_count,
        })
    }

    fn current_workspace(&self) -> ExplorerResult<WorkspaceSummary> {
        // Called from async handlers, so a Tokio runtime is available.
        tokio::runtime::Handle::current().block_on(self.open_workspace(OpenWorkspaceRequest {
            root_path: self.root_path.display().to_string(),
        }))
    }
}

/// Derive a stable workspace id from its root path.
fn workspace_id(root_path: &PathBuf) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    root_path.display().to_string().hash(&mut h);
    format!("workspace:{:x}", h.finish())
}
