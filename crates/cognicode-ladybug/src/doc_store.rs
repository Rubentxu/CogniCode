//! `DocRepository` adapter implementation for LadybugDB.
//!
//! Stub — per-method SQL impl pending (e13-wave2 Phase 3).
//! Mirrors the `QualityStore`/`NarrativeStore` port-impl pattern: the
//! trait is implemented on [`LadybugStore`], and every method returns
//! `Err(Error::Stub(...))` until the lbug 0.19 DDL lands.

use cognicode_core::domain::ports::doc_repository::{DocError, DocRepository, DocSummary};

use crate::LadybugStore;

impl DocRepository for LadybugStore {
    fn list_docs(
        &self,
        _workspace: &str,
        _section: Option<&str>,
    ) -> Result<Vec<DocSummary>, DocError> {
        Err(DocError::Store(
            "LadybugDocRepository not yet implemented".into(),
        ))
    }

    fn search_docs(
        &self,
        _workspace: &str,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<DocSummary>, DocError> {
        Err(DocError::Store(
            "LadybugDocRepository not yet implemented".into(),
        ))
    }
}
