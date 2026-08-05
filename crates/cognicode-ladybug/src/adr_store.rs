//! `AdrRepository` adapter implementation for LadybugDB.
//!
//! Stub — per-method SQL impl pending (e13-wave2 Phase 3).
//! Mirrors the `QualityStore`/`NarrativeStore` port-impl pattern: the
//! trait is implemented on [`LadybugStore`], and every method returns
//! `Err(Error::Stub(...))` until the lbug 0.19 DDL lands.

use cognicode_core::domain::ports::adr_repository::{
    AdrError, AdrRepository, AdrStatus, AdrSummary,
};

use crate::LadybugStore;

impl AdrRepository for LadybugStore {
    fn list_adrs(
        &self,
        _workspace: &str,
        _status: Option<AdrStatus>,
    ) -> Result<Vec<AdrSummary>, AdrError> {
        Err(AdrError::Store(
            "LadybugAdrRepository not yet implemented".into(),
        ))
    }

    fn search_adrs(
        &self,
        _workspace: &str,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<AdrSummary>, AdrError> {
        Err(AdrError::Store(
            "LadybugAdrRepository not yet implemented".into(),
        ))
    }

    fn get_adr(&self, _id: &str) -> Result<String, AdrError> {
        Err(AdrError::Store(
            "LadybugAdrRepository not yet implemented".into(),
        ))
    }
}
