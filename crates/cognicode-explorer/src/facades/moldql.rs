//! [`MoldQLService`] implementation.

use std::sync::Arc;

use async_trait::async_trait;

use crate::dto::MoldQLResultDto;
use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::LensExecutor;
use crate::facades::MoldQLService;
use crate::moldql::{MoldQLExecutor, MoldQLResult, MoldQLView};
use crate::ports::quality_repository::QualityRepository;
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::SymbolRepository;

#[cfg(feature = "multimodal")]
use crate::ports::GraphRepository;

/// Cap on the number of nodes returned by MoldQL queries.
const MOLDQL_RESULT_LIMIT: usize = 100;

/// Concrete implementation of [`MoldQLService`].
///
/// Executes MoldQL queries against the explorer ports.
pub struct MoldQLServiceImpl {
    repo: Arc<dyn SymbolRepository>,
    quality: Option<Arc<dyn QualityRepository>>,
    reader: Arc<dyn SourceReader>,
    lens_executor: Arc<dyn LensExecutor>,
    #[cfg(feature = "multimodal")]
    graph_repo: Option<Arc<dyn GraphRepository>>,
}

impl MoldQLServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: Arc<dyn SymbolRepository>,
        quality: Option<Arc<dyn QualityRepository>>,
        reader: Arc<dyn SourceReader>,
        lens_executor: Arc<dyn LensExecutor>,
        #[cfg(feature = "multimodal")] graph_repo: Option<Arc<dyn GraphRepository>>,
    ) -> Self {
        Self {
            repo,
            quality,
            reader,
            lens_executor,
            #[cfg(feature = "multimodal")]
            graph_repo,
        }
    }
}

#[async_trait]
impl MoldQLService for MoldQLServiceImpl {
    async fn execute_query(&self, query: &str) -> ExplorerResult<MoldQLResult> {
        let ast = crate::moldql::parser::parse(query)
            .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?;

        let view = self.build_moldql_view();
        MoldQLExecutor::new(&view).execute(ast)
    }

    async fn execute_query_with_target(
        &self,
        query: &str,
        target: crate::moldql::compile::CompileTarget,
    ) -> ExplorerResult<MoldQLResult> {
        let ast = crate::moldql::parser::parse(query)
            .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?;

        let view = self.build_moldql_view();
        MoldQLExecutor::new(&view).execute_with_target(ast, target)
    }
}

impl MoldQLServiceImpl {
    /// Build a `MoldQLView` from the current ports.
    fn build_moldql_view(&self) -> MoldQLView {
        // Build the apply_lens closure that bridges async LensExecutor to sync MoldQLView.
        let lens_executor = self.lens_executor.clone();
        let apply_lens: std::sync::Arc<
            dyn Fn(&str, &str) -> ExplorerResult<crate::dto::LensResult> + Send + Sync,
        > = std::sync::Arc::new(move |object_id, lens_id| {
            // Use block_on to call the async LensExecutor from the sync MoldQLView context.
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
        }
    }
}
