//! `GraphError` — domain error type for the Generic Graph Layer.
//!
//! Used by the `GraphRepository` port (and the `FederatedGraphService`
//! that fans out over multiple repositories). Lives in core so the
//! port trait can return it without taking a dependency on
//! `cognicode-explorer`'s `ExplorerError`.
//!
//! Kept deliberately small — just the four variants the generic
//! graph layer needs. Adapters can wrap upstream errors (sqlx,
//! serde, etc.) into `Storage(String)` for transport.
//!
//! Gated behind the `multimodal` Cargo feature. Default builds do
//! not include this module and `GraphError` is not exported from
//! `cognicode_core`.

use thiserror::Error;

/// Domain error type for graph repository operations.
///
/// Adapters that need to surface richer error chains (e.g. the
/// PostgreSQL adapter) wrap the upstream error in [`GraphError::Storage`]
/// and propagate. Consumers that need a stable error type for
/// cross-crate propagation should match on this enum.
#[derive(Debug, Error)]
pub enum GraphError {
    /// A lookup returned no row. Adapters return this instead of
    /// `Option::None` when the missing entry is unexpected (e.g.
    /// a referential-integrity violation in a write path).
    #[error("not found: {0}")]
    NotFound(String),

    /// Caller supplied an invalid input. Maps to HTTP 400 in API
    /// layers. Used for empty ids, out-of-range confidence, etc.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A storage backend (PostgreSQL, in-memory mock, …) reported
    /// a failure. The wrapped message is the human-readable
    /// upstream error; structured causes are deliberately not
    /// preserved (the adapters map them onto this variant).
    #[error("storage error: {0}")]
    Storage(String),

    /// The query itself was malformed (bad pagination cursor,
    /// unsupported direction, etc.). Distinct from
    /// [`GraphError::InvalidInput`] so the API layer can map
    /// them to different HTTP statuses (`400` for `InvalidInput`,
    /// `400 invalid_query` for `InvalidQuery` — the explorer
    /// keeps the existing two-tier distinction).
    #[error("invalid query: {0}")]
    InvalidQuery(String),
}

/// Convenience alias: `Result<T, GraphError>`. Mirrors the
/// `ExplorerResult` / `WorkspaceResult` pattern used elsewhere in
/// the workspace.
pub type GraphResult<T> = Result<T, GraphError>;

#[cfg(test)]
mod tests {
    use super::*;

    /// `Display` for `NotFound` includes the id.
    #[test]
    fn graph_error_not_found_display_includes_id() {
        let e = GraphError::NotFound("user:42".into());
        assert_eq!(e.to_string(), "not found: user:42");
    }

    /// `Display` for `InvalidInput` includes the reason.
    #[test]
    fn graph_error_invalid_input_display_includes_reason() {
        let e = GraphError::InvalidInput("id is empty".into());
        assert_eq!(e.to_string(), "invalid input: id is empty");
    }

    /// `Display` for `Storage` includes the upstream message.
    #[test]
    fn graph_error_storage_display_includes_upstream() {
        let e = GraphError::Storage("connection refused".into());
        assert_eq!(e.to_string(), "storage error: connection refused");
    }

    /// `Display` for `InvalidQuery` includes the reason.
    #[test]
    fn graph_error_invalid_query_display_includes_reason() {
        let e = GraphError::InvalidQuery("cursor is not base64".into());
        assert_eq!(e.to_string(), "invalid query: cursor is not base64");
    }

    /// `GraphResult<T>` is a `Result<T, GraphError>`.
    #[test]
    fn graph_result_is_result_over_graph_error() {
        let ok: GraphResult<u32> = Ok(42);
        let err: GraphResult<u32> = Err(GraphError::NotFound("x".into()));
        assert_eq!(ok.unwrap(), 42);
        assert!(err.is_err());
    }
}
