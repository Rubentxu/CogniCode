//! `EvidenceStore` adapter implementation for LadybugDB.
//!
//! Stub — per-method SQL impl pending (e13-wave2 Phase 3).
//! Mirrors the `QualityStore`/`NarrativeStore` port-impl pattern: the
//! trait is implemented on [`LadybugStore`], and every method returns
//! `Err(Error::Stub(...))` until the lbug 0.19 DDL lands.

use cognicode_core::domain::ports::evidence_store::{
    EvidenceError, EvidenceKind, EvidenceStore, EvidenceSummary,
};

use crate::LadybugStore;

impl EvidenceStore for LadybugStore {
    fn list_evidence(
        &self,
        _workspace: &str,
        _kind: Option<EvidenceKind>,
    ) -> Result<Vec<EvidenceSummary>, EvidenceError> {
        Err(EvidenceError::Store(
            "LadybugEvidenceStore not yet implemented".into(),
        ))
    }

    fn search_evidence(
        &self,
        _workspace: &str,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<EvidenceSummary>, EvidenceError> {
        Err(EvidenceError::Store(
            "LadybugEvidenceStore not yet implemented".into(),
        ))
    }
}
