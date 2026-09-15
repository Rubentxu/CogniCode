//! Analysis scope — the workspace/snapshot a detector run is pinned to
//! (M6, cycle e62.2).
//!
//! `FactId` and `EvidenceId` are canonical **per snapshot**, so the numeric ids
//! alone do not identify a fact: `(workspace, snapshot, id)` does. The scope is
//! therefore part of a detector execution's identity, not a free parameter of
//! verification:
//!
//! ```text
//! AnalysisInput.scope ──captured by──► DetectorExecutionRef.scope
//!                                             │
//!                                             ▼
//!                        FindingVerifier: finding.scope == lookup.scope
//! ```
//!
//! Without this, a caller could hydrate a read model from snapshot B for a
//! finding produced in snapshot A and every id would still resolve.
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};

use crate::domain::kernel_ids::SnapshotId;
use crate::domain::value_objects::WorkspaceId;

/// The exact `(workspace, snapshot)` an analysis run was performed against.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnalysisScope {
    /// Workspace the run belongs to.
    pub workspace: WorkspaceId,
    /// Snapshot the run was pinned to.
    pub snapshot: SnapshotId,
}

impl AnalysisScope {
    /// Construct a scope.
    pub fn new(workspace: WorkspaceId, snapshot: SnapshotId) -> Self {
        Self {
            workspace,
            snapshot,
        }
    }
}

impl std::fmt::Display for AnalysisScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.workspace.as_str(), self.snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_round_trips_and_displays() {
        let scope = AnalysisScope::new(WorkspaceId::try_new("w").unwrap(), SnapshotId::new(3));
        assert_eq!(scope.snapshot, SnapshotId::new(3));
        assert_eq!(scope.to_string(), "w@snap:3");
        let json = serde_json::to_string(&scope).unwrap();
        let parsed: AnalysisScope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, scope);
    }

    #[test]
    fn scopes_differ_by_snapshot() {
        let workspace = WorkspaceId::try_new("w").unwrap();
        assert_ne!(
            AnalysisScope::new(workspace.clone(), SnapshotId::new(1)),
            AnalysisScope::new(workspace, SnapshotId::new(2))
        );
    }
}
