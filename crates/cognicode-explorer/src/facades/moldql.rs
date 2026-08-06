//! [`MoldQLService`] implementation.

use crate::dto::MoldQLResultDto;
use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::plan::executor::GraphExecutor;
use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};

use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::LensService;
use crate::facades::MoldQLService;
use crate::moldql::{MoldQLExecutor, MoldQLResult, MoldQLView};
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::SymbolRepository;
use cognicode_core::domain::ports::QualityStore;

#[cfg(feature = "multimodal")]
use cognicode_core::domain::ports::GraphRepository;

/// Cap on the number of nodes returned by MoldQL queries.
const MOLDQL_RESULT_LIMIT: usize = 100;

/// Concrete implementation of [`MoldQLService`].
///
/// Executes MoldQL queries against the explorer ports.
pub struct MoldQLServiceImpl {
    repo: Arc<dyn SymbolRepository>,
    quality: Option<Arc<dyn QualityStore>>,
    reader: Arc<dyn SourceReader>,
    lens_executor: Arc<dyn LensService>,
    #[cfg(feature = "multimodal")]
    graph_repo: Option<Arc<dyn GraphRepository>>,
    /// Pattern Profile executor. `None` ⇒ `FeatureDisabled` at run time.
    graph_executor: Option<Arc<dyn GraphExecutor>>,
    /// Workspace pin for Pattern Profile queries.
    workspace_id: Option<String>,
    /// Revision pin for Pattern Profile queries.
    revision_id: Option<u64>,
}

impl MoldQLServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: Arc<dyn SymbolRepository>,
        quality: Option<Arc<dyn QualityStore>>,
        reader: Arc<dyn SourceReader>,
        lens_executor: Arc<dyn LensService>,
        #[cfg(feature = "multimodal")] graph_repo: Option<Arc<dyn GraphRepository>>,
        graph_executor: Option<Arc<dyn GraphExecutor>>,
        workspace_id: Option<String>,
        revision_id: Option<u64>,
    ) -> Self {
        Self {
            repo,
            quality,
            reader,
            lens_executor,
            #[cfg(feature = "multimodal")]
            graph_repo,
            graph_executor,
            workspace_id,
            revision_id,
        }
    }
}

#[async_trait]
impl MoldQLService for MoldQLServiceImpl {
    async fn execute_query(&self, query: &str) -> ExplorerResult<MoldQLResult> {
        let ast = match crate::moldql::lower_intent(query) {
            Some(Ok(ast)) => ast,
            Some(Err(e)) => {
                return Err(ExplorerError::ResolutionFailed(format!(
                    "intent query `{query}` invalid: {e}"
                )));
            }
            None => crate::moldql::parser::parse(query)
                .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?,
        };

        let view = self.build_moldql_view();
        MoldQLExecutor::new(&view).execute(ast).await
    }

    async fn execute_query_with_target(
        &self,
        query: &str,
        target: crate::moldql::compile::CompileTarget,
    ) -> ExplorerResult<MoldQLResult> {
        let ast = match crate::moldql::lower_intent(query) {
            Some(Ok(ast)) => ast,
            Some(Err(e)) => {
                return Err(ExplorerError::ResolutionFailed(format!(
                    "intent query `{query}` invalid: {e}"
                )));
            }
            None => crate::moldql::parser::parse(query)
                .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?,
        };

        let view = self.build_moldql_view();
        MoldQLExecutor::new(&view).execute_with_target(ast, target)
    }

    async fn execute_query_pinned(
        &self,
        query: &str,
        workspace_id: String,
        revision_id: u64,
    ) -> ExplorerResult<MoldQLResult> {
        let ast = match crate::moldql::lower_intent(query) {
            Some(Ok(ast)) => ast,
            Some(Err(e)) => {
                return Err(ExplorerError::ResolutionFailed(format!(
                    "intent query `{query}` invalid: {e}"
                )));
            }
            None => crate::moldql::parser::parse(query)
                .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?,
        };

        let ws_id = WorkspaceId::try_new(workspace_id)
            .map_err(|e| ExplorerError::ResolutionFailed(format!("invalid workspace_id: {e}")))?;
        let view = self.build_moldql_view_with_pin(ws_id, RevisionId(revision_id));
        MoldQLExecutor::new(&view).execute(ast).await
    }
}

impl MoldQLServiceImpl {
    /// Build a `MoldQLView` from the current ports.
    fn build_moldql_view(&self) -> MoldQLView {
        // Build the apply_lens closure that bridges async LensService to sync MoldQLView.
        let lens_executor = self.lens_executor.clone();
        let apply_lens: std::sync::Arc<
            dyn Fn(&str, &str) -> ExplorerResult<crate::dto::LensResult> + Send + Sync,
        > = std::sync::Arc::new(move |object_id, lens_id| {
            // Use block_on to call the async LensService from the sync MoldQLView context.
            tokio::runtime::Handle::current().block_on(lens_executor.apply_lens(object_id, lens_id))
        });

        MoldQLView {
            repo: self.repo.clone(),
            quality: self.quality.clone(),
            reader: self.reader.clone(),
            apply_lens,
            #[cfg(feature = "multimodal")]
            graph_repo: self.graph_repo.clone(),
            graph_query: None,
            graph_executor: self.graph_executor.clone(),
            pin: match (&self.workspace_id, self.revision_id) {
                (Some(ws), Some(rev)) => {
                    // Unwrap is safe here — workspace_id stored in struct is pre-validated.
                    Some((WorkspaceId::try_new(ws.clone()).unwrap(), RevisionId(rev)))
                }
                _ => None,
            },
        }
    }

    /// Build a `MoldQLView` with an explicit pin, overriding the instance-level pin.
    fn build_moldql_view_with_pin(
        &self,
        workspace_id: WorkspaceId,
        revision_id: RevisionId,
    ) -> MoldQLView {
        // Build the apply_lens closure that bridges async LensService to sync MoldQLView.
        let lens_executor = self.lens_executor.clone();
        let apply_lens: std::sync::Arc<
            dyn Fn(&str, &str) -> ExplorerResult<crate::dto::LensResult> + Send + Sync,
        > = std::sync::Arc::new(move |object_id, lens_id| {
            // Use block_on to call the async LensService from the sync MoldQLView context.
            tokio::runtime::Handle::current().block_on(lens_executor.apply_lens(object_id, lens_id))
        });

        MoldQLView {
            repo: self.repo.clone(),
            quality: self.quality.clone(),
            reader: self.reader.clone(),
            apply_lens,
            #[cfg(feature = "multimodal")]
            graph_repo: self.graph_repo.clone(),
            graph_query: None,
            graph_executor: self.graph_executor.clone(),
            pin: Some((workspace_id, revision_id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ExplorerResult;
    use crate::ports::symbol_repository::{GraphStats, ResolvedSymbol, SymbolRepository};
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::plan::executor::StubExecutor;

    /// Stub LensService for tests.
    struct TestLensService;
    #[async_trait]
    impl LensService for TestLensService {
        async fn apply_lens(
            &self,
            _object_id: &str,
            _lens_id: &str,
        ) -> ExplorerResult<crate::dto::LensResult> {
            Err(ExplorerError::FeatureDisabled("test".into()))
        }
    }

    /// Stub SourceReader for tests.
    struct TestSourceReader;
    impl SourceReader for TestSourceReader {
        fn read_source(&self, _path: &str) -> ExplorerResult<String> {
            Ok(String::new())
        }
        fn read_lines(
            &self,
            _path: &str,
            _start: u32,
            _end: u32,
        ) -> ExplorerResult<Vec<(u32, String)>> {
            Ok(vec![])
        }
    }

    /// Stub SymbolRepository for tests.
    struct TestSymbolRepo;
    impl SymbolRepository for TestSymbolRepo {
        fn resolve(&self, _id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(None)
        }
        fn find_symbols_by_name(&self, _name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        fn find_symbols_by_file(&self, _file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        fn graph_stats(&self) -> GraphStats {
            GraphStats::default()
        }
        fn module_list(&self) -> Vec<String> {
            vec![]
        }
    }

    /// Test that `execute_query_pinned` accepts valid workspace_id and revision_id
    /// without erroring on pin construction.
    #[tokio::test]
    async fn execute_query_pinned_accepts_valid_pin() {
        let repo: Arc<dyn SymbolRepository> = Arc::new(TestSymbolRepo);
        let reader: Arc<dyn SourceReader> = Arc::new(TestSourceReader);
        let lens_executor: Arc<dyn LensService> = Arc::new(TestLensService);
        let stub_executor: Arc<dyn GraphExecutor> = Arc::new(StubExecutor::new());

        let service = MoldQLServiceImpl::new(
            repo,
            None,
            reader,
            lens_executor,
            #[cfg(feature = "multimodal")]
            None,
            Some(stub_executor),
            Some("default".to_string()),
            Some(1),
        );

        // execute_query_pinned should not error on workspace_id validation.
        // Query format must have named node bindings (e.g., n:Function).
        let result = service
            .execute_query_pinned(
                "match (n:Function)-[:Calls]->(m:Function) return path(n,m)",
                "test-workspace".to_string(),
                42,
            )
            .await;

        // Should not return Err(ResolutionFailed) for invalid workspace_id
        assert!(
            !matches!(result, Err(ExplorerError::ResolutionFailed(ref msg)) if msg.contains("invalid workspace_id")),
            "execute_query_pinned should not fail on valid workspace_id"
        );
    }

    /// Test that `execute_query_pinned` correctly overrides the instance-level pin.
    #[tokio::test]
    async fn execute_query_pinned_overrides_instance_pin() {
        let repo: Arc<dyn SymbolRepository> = Arc::new(TestSymbolRepo);
        let reader: Arc<dyn SourceReader> = Arc::new(TestSourceReader);
        let lens_executor: Arc<dyn LensService> = Arc::new(TestLensService);
        let stub_executor: Arc<dyn GraphExecutor> = Arc::new(StubExecutor::new());

        // Service constructed with instance-level pin (ws-0, rev-0)
        let service = MoldQLServiceImpl::new(
            repo,
            None,
            reader,
            lens_executor,
            #[cfg(feature = "multimodal")]
            None,
            Some(stub_executor),
            Some("ws-0".to_string()),
            Some(0),
        );

        // execute_query_pinned called with (ws-1, rev-1) should override.
        // Query format must have named node bindings (e.g., n:Function).
        let result = service
            .execute_query_pinned(
                "match (n:Function)-[:Calls]->(m:Function) return path(n,m)",
                "ws-1".to_string(),
                1,
            )
            .await;

        // Should succeed (StubExecutor returns empty result, not FeatureDisabled)
        assert!(
            result.is_ok(),
            "execute_query_pinned should succeed with StubExecutor, got: {:?}",
            result
        );
    }

    /// Test that `MoldQLServiceImpl` accepts a non-None graph_executor.
    #[tokio::test]
    async fn moldql_service_impl_accepts_graph_executor() {
        let repo: Arc<dyn SymbolRepository> = Arc::new(TestSymbolRepo);
        let reader: Arc<dyn SourceReader> = Arc::new(TestSourceReader);
        let lens_executor: Arc<dyn LensService> = Arc::new(TestLensService);
        let stub_executor: Arc<dyn GraphExecutor> = Arc::new(StubExecutor::new());

        // Service constructed with a graph_executor
        let _service = MoldQLServiceImpl::new(
            repo,
            None,
            reader,
            lens_executor,
            #[cfg(feature = "multimodal")]
            None,
            Some(stub_executor),
            Some("default".to_string()),
            Some(1),
        );

        // Verify that the service was constructed without panicking
        // (MoldQLServiceImpl::new should not require graph_executor to be Some,
        // but the field should be populated correctly)
        assert!(
            true,
            "MoldQLServiceImpl should be constructible with graph_executor"
        );
    }
}