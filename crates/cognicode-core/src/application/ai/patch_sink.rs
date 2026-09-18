//! InMemoryPatchArtifactSink — deterministic `PatchArtifactSink` adapter (e80b).
//!
//! Stores validated patches by content address. Identical patches collapse to
//! the same [`PatchRef`], so a deterministic `LlmPort` plus the same trusted
//! envelope yields a reproducible `patch_ref` (WU8-N).
//!
//! Semantics: this is an **inert** store. A stored patch is not canonical
//! truth, is never executed, and grants no authority. e80b deliberately does
//! not introduce a durable/distributed patch store.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::domain::ai::patch::{
    PatchArtifactError, PatchArtifactSink, PatchRef, ValidatedSourcePatch,
};

/// A deterministic, in-process patch store.
#[derive(Debug, Default)]
pub struct InMemoryPatchArtifactSink {
    entries: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemoryPatchArtifactSink {
    /// An empty sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of stored artifacts.
    pub fn len(&self) -> usize {
        self.entries.lock().map(|m| m.len()).unwrap_or_default()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether a reference is present.
    pub fn contains(&self, reference: &PatchRef) -> bool {
        self.entries
            .lock()
            .map(|m| m.contains_key(reference.as_str()))
            .unwrap_or(false)
    }

    /// The stored canonical bytes for a reference, if present.
    pub fn get(&self, reference: &PatchRef) -> Option<Vec<u8>> {
        self.entries
            .lock()
            .ok()
            .and_then(|m| m.get(reference.as_str()).cloned())
    }
}

impl PatchArtifactSink for InMemoryPatchArtifactSink {
    fn store(&self, patch: ValidatedSourcePatch) -> Result<PatchRef, PatchArtifactError> {
        let bytes = patch.canonical_bytes();
        let reference = PatchRef::from_content(&bytes);
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| PatchArtifactError::Rejected("sink lock poisoned".to_string()))?;
        entries.insert(reference.as_str().to_string(), bytes);
        Ok(reference)
    }
}

#[cfg(all(test, feature = "evidence-kernel"))]
mod tests {
    use super::*;
    use crate::domain::ai::patch::{
        PatchBaseScope, SourceEdit, SourcePatchCandidate, validate_source_patch,
    };
    use crate::domain::kernel_ids::SnapshotId;
    use crate::domain::value_objects::WorkspaceId;

    use crate::application::ai::fix_agent_test_support::{
        frame_and_request, passing_response_with_candidate,
    };

    fn candidate(path: &str, content: &str) -> SourcePatchCandidate {
        SourcePatchCandidate {
            base_scope: PatchBaseScope::new(
                WorkspaceId::try_new("ws").unwrap(),
                SnapshotId::new(7),
            ),
            edits: vec![SourceEdit {
                path: path.to_string(),
                new_content: content.to_string(),
            }],
            rationale: None,
        }
    }

    fn validate(cand: &SourcePatchCandidate) -> crate::domain::ai::patch::ValidatedSourcePatch {
        let (frame, request) = frame_and_request();
        let response = passing_response_with_candidate(&frame, &request, cand.clone());
        validate_source_patch(&frame, &request, &response, cand, Default::default()).unwrap()
    }

    #[test]
    fn storing_is_content_addressed_and_idempotent() {
        let sink = InMemoryPatchArtifactSink::new();
        let patch = validate(&candidate("src/a.rs", "fn a() {}\n"));
        let r1 = sink.store(patch.clone()).unwrap();
        let r2 = sink.store(patch).unwrap();
        assert_eq!(
            r1, r2,
            "identical patches must collapse to one content address"
        );
        assert_eq!(sink.len(), 1);
        assert!(sink.contains(&r1));
        assert!(r1.as_str().starts_with("sha256:"));
    }

    #[test]
    fn different_content_yields_different_refs() {
        let sink = InMemoryPatchArtifactSink::new();
        let a = sink
            .store(validate(&candidate("src/a.rs", "fn a() {}\n")))
            .unwrap();
        let b = sink
            .store(validate(&candidate("src/b.rs", "fn b() {}\n")))
            .unwrap();
        assert_ne!(a, b);
        assert_eq!(sink.len(), 2);
        assert_eq!(
            sink.get(&a).unwrap(),
            validate(&candidate("src/a.rs", "fn a() {}\n")).canonical_bytes()
        );
    }
}
