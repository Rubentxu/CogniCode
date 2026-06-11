use thiserror::Error;

pub type ExplorerResult<T> = Result<T, ExplorerError>;

#[derive(Debug, Error)]
pub enum ExplorerError {
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(String),

    #[error("inspectable object not found: {0}")]
    ObjectNotFound(String),

    #[error("view not available for object {object_id}: {view_id}")]
    ViewNotAvailable { object_id: String, view_id: String },

    #[error("resolution failed for MVP id: {0}")]
    ResolutionFailed(String),

    #[error("source unavailable for object {object_id}: {file}")]
    SourceUnavailable { file: String, object_id: String },

    #[error("call graph is not loaded yet — index the workspace first")]
    GraphNotReady,

    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("feature disabled: {0}")]
    FeatureDisabled(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Subgraph request failed query validation (depth/max_nodes out of
    /// range, unknown `direction`, …). Maps to HTTP 400.
    #[error("invalid_query: {0}")]
    InvalidQuery(String),

    /// Subgraph request asked for a symbol id that is not in the
    /// current graph. Maps to HTTP 404.
    #[error("symbol_not_found: {0}")]
    SymbolNotFound(String),

    /// Subgraph request could not be served because the underlying
    /// call graph is not loaded / not ready. Maps to HTTP 503.
    #[error("graph_unavailable: {0}")]
    GraphUnavailable(String),

    /// Subgraph request had a malformed `:id` (empty, too long, …).
    /// Maps to HTTP 400 with a distinct `invalid_id` body so clients
    /// can tell validation errors apart from query-param errors.
    #[error("invalid_id: {0}")]
    InvalidId(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

// Map the canonical `cognicode_core::domain::ports::GraphError`
// onto `ExplorerError` so the `?` operator works on adapter
// returns inside the explorer. Phase 1 of the Graph Intelligence
// v2 roadmap moved `GraphRepository` to `cognicode-core`; this
// `From` impl keeps the existing `ExplorerError`-shaped call
// sites working without per-site conversion code.
#[cfg(feature = "multimodal")]
impl From<cognicode_core::domain::ports::GraphError> for ExplorerError {
    fn from(err: cognicode_core::domain::ports::GraphError) -> Self {
        use cognicode_core::domain::ports::GraphError as Core;
        match err {
            Core::NotFound(s) => ExplorerError::NotFound(s),
            Core::InvalidInput(s) => ExplorerError::InvalidInput(s),
            Core::Storage(s) => {
                ExplorerError::Anyhow(anyhow::anyhow!("graph repository storage error: {s}"))
            }
            Core::InvalidQuery(s) => ExplorerError::InvalidQuery(s),
        }
    }
}
