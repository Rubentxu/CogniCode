//! Workspace resolution helpers for the ingest pipeline (ADR-017, ADR-025).
//!
//! The PG-backed `IngestController` (async scan job orchestration over
//! `PostgresRepository`) was removed with the full postgres removal
//! (e29-7). What remains is the PG-free workspace resolver used by the
//! explorer's `WorkspaceService` and the runtime's ingest wiring.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

/// Derive a stable workspace id from its root path.
pub fn workspace_id_for_path(root_path: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    root_path.display().to_string().hash(&mut h);
    format!("workspace:{:x}", h.finish())
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
