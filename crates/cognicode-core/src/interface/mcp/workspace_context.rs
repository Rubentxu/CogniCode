//! The one way to build the context of a workspace.
//!
//! Every server that serves a workspace needs the same pieces: a root that
//! has been canonicalised, a validator confined to it, the service that
//! reads and writes files through that validator, a cancellation flag and a
//! read-only posture chosen at construction. Building them in each binary
//! let the HTTP server ship without the file service, without a posture and
//! without the flag that selects it.

use std::path::PathBuf;
use std::sync::Arc;

use super::handlers::{HandlerContext, HandlerContextBuilder};
use crate::application::services::file_operations::FileOperationsService;
use crate::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis;
use crate::infrastructure::verification::RustVerifier;
use crate::interface::mcp::security::InputValidator;

impl HandlerContext {
    /// A context for the workspace at `root`.
    ///
    /// With `read_only`, the tools that change the workspace are hidden from
    /// `tools/list` and refused at dispatch. The posture is fixed here: the
    /// finished context has no setter for it, so a live handler cannot be
    /// flipped in or out of read-only after a client connected.
    pub fn for_workspace(root: PathBuf, read_only: bool) -> Self {
        Self::workspace_builder(root, read_only).build()
    }

    /// The builder behind [`HandlerContext::for_workspace`], for the callers
    /// that bring their own graph store or IaC repository on top of it.
    pub(crate) fn workspace_builder(root: PathBuf, read_only: bool) -> HandlerContextBuilder {
        let canonical_root = std::fs::canonicalize(&root).unwrap_or(root);

        let validator =
            Arc::new(InputValidator::new().with_workspace(vec![canonical_root.clone()]));
        let file_ops_service = Arc::new(FileOperationsService::new(
            canonical_root.to_string_lossy().as_ref(),
            validator,
            Arc::new(RustVerifier::new()),
            Arc::new(TreeSitterSyntaxAnalysis::new()),
        ));

        HandlerContext::builder()
            .with_working_dir(canonical_root)
            .with_file_ops_service(file_ops_service)
            .with_read_only(read_only)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_context_carries_the_file_service_and_the_posture_asked_for() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canonical");

        let writable = HandlerContext::for_workspace(dir.path().to_path_buf(), false);
        assert!(!writable.is_read_only());
        assert!(writable.file_ops_service().is_some());
        assert_eq!(writable.working_dir(), root);

        let read_only = HandlerContext::for_workspace(dir.path().to_path_buf(), true);
        assert!(read_only.is_read_only());
        assert!(read_only.file_ops_service().is_some());
    }

    #[test]
    fn a_workspace_context_starts_without_a_pending_cancellation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ctx = HandlerContext::for_workspace(dir.path().to_path_buf(), false);
        assert!(!ctx.is_cancelled());
    }
}
