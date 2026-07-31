//! [`WorkspaceService`] implementation.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::application::ingest::{StaticWorkspaceResolver, workspace_id_for_path};

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
    workspace_resolver: Option<Arc<StaticWorkspaceResolver>>,
}

impl WorkspaceServiceImpl {
    pub fn new(
        repo: Arc<dyn SymbolRepository>,
        root_path: PathBuf,
        workspace_resolver: Option<Arc<StaticWorkspaceResolver>>,
    ) -> Self {
        Self {
            repo,
            root_path,
            workspace_resolver,
        }
    }

    fn summarize_workspace(&self, root_path: PathBuf) -> ExplorerResult<WorkspaceSummary> {
        if !root_path.exists() {
            return Err(crate::error::ExplorerError::WorkspaceNotFound(
                root_path.display().to_string(),
            ));
        }

        let stats = self.repo.graph_stats();
        let symbol_count = stats.symbol_count;
        let relation_count = stats.relation_count;
        let graph_status = if symbol_count > 0 || relation_count > 0 {
            crate::dto::GraphStatus::Ready
        } else {
            crate::dto::GraphStatus::Missing
        };

        let id = workspace_id_for_path(&root_path);
        if let Some(resolver) = &self.workspace_resolver {
            resolver.register(id.clone(), root_path.clone());
        }

        Ok(WorkspaceSummary {
            id,
            root_path: root_path.display().to_string(),
            graph_status,
            indexed_at: None,
            symbol_count,
            relation_count,
        })
    }
}

#[async_trait]
impl WorkspaceService for WorkspaceServiceImpl {
    async fn open_workspace(
        &self,
        request: OpenWorkspaceRequest,
    ) -> ExplorerResult<WorkspaceSummary> {
        self.summarize_workspace(PathBuf::from(request.root_path))
    }

    fn current_workspace(&self) -> ExplorerResult<WorkspaceSummary> {
        self.summarize_workspace(self.root_path.clone())
    }
}
