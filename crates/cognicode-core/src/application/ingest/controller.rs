//! Workspace resolution helpers for the ingest pipeline (ADR-017, ADR-025).
//!
//! The PG-backed `IngestController` (async scan job orchestration over
//! `PostgresRepository`) was removed with the full postgres removal
//! (e29-7). What remains is the PG-free workspace resolver used by the
//! explorer's `WorkspaceService` and the runtime's ingest wiring.
//!
//! This module does not derive workspace identities any more. It used to
//! carry a second derivation — `workspace_id_for_path`, the `DefaultHasher`
//! of the path *as written* — next to `graph_cache::workspace_key`, the
//! hash of the *canonical* path: two criteria for one question, and the
//! same project opened through a symlink got two identities here and one
//! there. The only consumer was the explorer facade retired with TASK-177,
//! so the second derivation went with it. `workspace_key` is the one owner
//! of the workspace's name.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Resolves a workspace_id to its root path on disk.
pub trait WorkspaceResolver: Send + Sync {
    fn resolve(&self, workspace_id: &str) -> Option<PathBuf>;
}

impl std::fmt::Debug for dyn WorkspaceResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WorkspaceResolver(..)")
    }
}

/// A simple in-memory workspace resolver (for tests / standalone mode).
#[derive(Default)]
pub struct StaticWorkspaceResolver {
    paths: Mutex<HashMap<String, PathBuf>>,
}

impl StaticWorkspaceResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a workspace: (id, path) mapping.
    pub fn register(&self, id: impl Into<String>, path: PathBuf) {
        self.paths.lock().unwrap().insert(id.into(), path);
    }
}

impl WorkspaceResolver for StaticWorkspaceResolver {
    fn resolve(&self, workspace_id: &str) -> Option<PathBuf> {
        self.paths.lock().unwrap().get(workspace_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_workspace_resolver() {
        let resolver = StaticWorkspaceResolver::new();
        resolver.register("ws1", PathBuf::from("/tmp/ws1"));
        assert_eq!(resolver.resolve("ws1"), Some(PathBuf::from("/tmp/ws1")));
        assert_eq!(resolver.resolve("nonexistent"), None);
    }
}
