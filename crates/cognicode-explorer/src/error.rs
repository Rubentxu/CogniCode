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

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}
