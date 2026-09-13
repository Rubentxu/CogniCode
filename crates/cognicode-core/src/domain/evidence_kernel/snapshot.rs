//! Snapshot descriptor — the M1 facade over the canonical-graph revision
//! model (design D4 / ADR-039).
//!
//! A [`SnapshotDescriptor`] is a stable, addressable view of one workspace
//! revision. `SnapshotId` is bijective with `RevisionId` per workspace; the
//! existing revision timeline remains the source of truth — no parallel
//! snapshot system is introduced.

use serde::{Deserialize, Serialize};

use crate::domain::value_objects::{RevisionId, WorkspaceId};

use super::ids::SnapshotId;

/// Addressable descriptor of one workspace snapshot.
///
/// `source_state` and `config_digest` are reserved (design D4): they will
/// carry the source reference (e.g. git SHA or content digest) and the
/// extractor/pack configuration so one revision may yield several
/// descriptors later. In M1 both may stay empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotDescriptor {
    /// Snapshot id (bijective with `revision` in this workspace).
    pub id: SnapshotId,
    /// Owning workspace.
    pub workspace: WorkspaceId,
    /// The underlying graph revision.
    pub revision: RevisionId,
    /// Reserved: source state reference (git SHA or content digest).
    pub source_state: String,
    /// Reserved: extractor/pack configuration digest.
    pub config_digest: String,
}

impl SnapshotDescriptor {
    /// Builds the descriptor for a revision — the pure half of the
    /// `SnapshotStore::from_revision` port (design D4).
    pub fn from_revision(
        workspace: WorkspaceId,
        revision: RevisionId,
        source_state: impl Into<String>,
        config_digest: impl Into<String>,
    ) -> Self {
        Self {
            id: SnapshotId::from_revision(revision),
            workspace,
            revision,
            source_state: source_state.into(),
            config_digest: config_digest.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::value_objects::{RevisionId, WorkspaceId};

    use super::super::ids::SnapshotId;
    use super::*;

    /// `SnapshotDescriptor::from_revision` must map the revision bijectively:
    /// `id == SnapshotId(rev)`, same workspace, same revision (design D4).
    #[test]
    fn snapshot_descriptor_from_revision_maps_bijectively() {
        let ws = WorkspaceId::try_new("ws-a").expect("valid workspace");
        let rev = RevisionId::new(3);
        let descriptor = SnapshotDescriptor::from_revision(ws.clone(), rev, "", "");

        assert_eq!(descriptor.id, SnapshotId::new(3));
        assert_eq!(descriptor.id.to_revision(), rev);
        assert_eq!(descriptor.workspace, ws);
        assert_eq!(descriptor.revision, rev);
    }

    /// Reserved fields (`source_state`, `config_digest`) are carried through
    /// untouched; in M1 they may stay empty (design D4 open question).
    #[test]
    fn snapshot_descriptor_reserved_fields_are_carried() {
        let ws = WorkspaceId::try_new("ws-a").expect("valid workspace");
        let empty = SnapshotDescriptor::from_revision(ws.clone(), RevisionId::new(1), "", "");
        assert_eq!(empty.source_state, "");
        assert_eq!(empty.config_digest, "");

        let populated = SnapshotDescriptor::from_revision(
            ws,
            RevisionId::new(2),
            "git:abc123",
            "blake3:deadbeef",
        );
        assert_eq!(populated.source_state, "git:abc123");
        assert_eq!(populated.config_digest, "blake3:deadbeef");
    }

    /// `SnapshotDescriptor` must serialize and deserialize losslessly.
    #[test]
    fn snapshot_descriptor_serde_round_trip() {
        let ws = WorkspaceId::try_new("ws-a").expect("valid workspace");
        let descriptor =
            SnapshotDescriptor::from_revision(ws, RevisionId::new(5), "git:abc123", "blake3:ff");
        let json = serde_json::to_string(&descriptor).expect("serialize");
        let parsed: SnapshotDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, descriptor);
    }
}
