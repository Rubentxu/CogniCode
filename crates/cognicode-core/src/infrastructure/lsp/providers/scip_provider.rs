//! The S3 tier: code intelligence answered from a caller-supplied SCIP index.
//!
//! The provider reads an index somebody else produced. It never runs an
//! indexer, never writes, and never reaches the network: the only effect
//! it has is answering questions from bytes the caller handed it.
//!
//! The tier answers two questions well — where a symbol is defined and
//! where it is referenced — because those are the two relations a SCIP
//! index states. Every other question is reported as a gap instead of being
//! guessed from a partial index.

use std::path::Path;

use async_trait::async_trait;

use crate::domain::aggregates::Symbol;
use crate::domain::traits::code_intelligence::{
    CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, HoverInfo, PrecisionTier,
    ProviderDiagnostic, ProviderOutcome, Reference, TieredCodeIntelligenceProvider, TieredOutcome,
    TypeHierarchy,
};
use crate::domain::value_objects::Location;
use crate::infrastructure::scip::ScipIndex;

/// Code-intelligence provider backed by a SCIP index.
///
/// The index must be bound to the identity envelope the caller expects
/// (see [`ScipIndex::parse_bound`]); an unbound index answers nothing,
/// because nothing vouches for the context it was produced for.
pub struct ScipIntelligenceProvider {
    index: ScipIndex,
}

impl ScipIntelligenceProvider {
    /// Stable provider identity of the S3 SCIP tier.
    pub const PROVIDER_ID: &'static str = "scip";

    /// Builds a provider over an already-parsed index.
    pub fn new(index: ScipIndex) -> Self {
        Self { index }
    }

    /// The index the provider answers from.
    pub fn index(&self) -> &ScipIndex {
        &self.index
    }

    /// True when the index may serve queries.
    pub fn is_bound(&self) -> bool {
        self.index.is_bound()
    }

    /// The diagnostic an unsupported operation reports.
    fn unsupported(op: &str) -> ProviderDiagnostic {
        ProviderDiagnostic::new(
            Self::PROVIDER_ID,
            PrecisionTier::S3,
            ProviderOutcome::Unavailable,
            format!("{op}: the SCIP tier answers definitions and references only"),
        )
    }

    /// The error an unsupported operation reports.
    fn unsupported_error(op: &str) -> CodeIntelligenceError {
        CodeIntelligenceError::Internal(format!(
            "{op}: the SCIP tier answers definitions and references only"
        ))
    }
}

#[async_trait]
impl CodeIntelligenceProvider for ScipIntelligenceProvider {
    async fn get_symbols(&self, _path: &Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
        Err(Self::unsupported_error("get_symbols"))
    }

    async fn find_references(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Result<Vec<Reference>, CodeIntelligenceError> {
        self.index
            .find_references_at(location, include_declaration)
            .ok_or_else(|| {
                CodeIntelligenceError::Internal(format!(
                    "find_references: the SCIP index does not cover {}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                ))
            })
    }

    async fn get_hierarchy(
        &self,
        _location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError> {
        Err(Self::unsupported_error("get_hierarchy"))
    }

    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        Ok(self.index.find_definition_at(location))
    }

    async fn get_document_symbols(
        &self,
        _path: &Path,
    ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
        Err(Self::unsupported_error("get_document_symbols"))
    }

    async fn hover(
        &self,
        _location: &Location,
    ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
        Err(Self::unsupported_error("hover"))
    }
}

#[async_trait]
impl TieredCodeIntelligenceProvider for ScipIntelligenceProvider {
    async fn get_symbols_tiered(&self, _path: &Path) -> TieredOutcome<Vec<Symbol>> {
        TieredOutcome::Unresolved(vec![Self::unsupported("get_symbols")])
    }

    async fn find_references_tiered(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> TieredOutcome<Vec<Reference>> {
        match self.index.find_references_at(location, include_declaration) {
            Some(references) => TieredOutcome::served(references, PrecisionTier::S3),
            // A miss is not evidence of absence: the index may not cover the
            // position, the symbol may be ambiguous there, or the producer
            // may not have emitted the relation. The tier answers nothing and
            // lets a lower tier answer instead.
            None => TieredOutcome::Unresolved(vec![ProviderDiagnostic::new(
                Self::PROVIDER_ID,
                PrecisionTier::S3,
                ProviderOutcome::Degraded,
                format!(
                    "find_references: the SCIP index does not cover {}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                ),
            )]),
        }
    }

    async fn get_hierarchy_tiered(&self, _location: &Location) -> TieredOutcome<TypeHierarchy> {
        TieredOutcome::Unresolved(vec![Self::unsupported("get_hierarchy")])
    }

    async fn get_definition_tiered(&self, location: &Location) -> TieredOutcome<Option<Location>> {
        match self.index.find_definition_at(location) {
            Some(definition) => TieredOutcome::served(Some(definition), PrecisionTier::S3),
            None => TieredOutcome::Unresolved(vec![ProviderDiagnostic::new(
                Self::PROVIDER_ID,
                PrecisionTier::S3,
                ProviderOutcome::Degraded,
                format!(
                    "get_definition: the SCIP index does not cover {}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                ),
            )]),
        }
    }

    async fn get_document_symbols_tiered(
        &self,
        _path: &Path,
    ) -> TieredOutcome<Vec<DocumentSymbol>> {
        TieredOutcome::Unresolved(vec![Self::unsupported("get_document_symbols")])
    }

    async fn hover_tiered(&self, _location: &Location) -> TieredOutcome<Option<HoverInfo>> {
        TieredOutcome::Unresolved(vec![Self::unsupported("hover")])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::scip::{DEFAULT_MAX_INDEX_BYTES, ScipIndexBinding};
    use sha2::{Digest, Sha256};

    const REAL_INDEX: &[u8] =
        include_bytes!("../../../../tests/data/scip/rust-analyzer-minimal.scip");

    fn bound_index() -> ScipIndex {
        let binding = ScipIndexBinding {
            repository_id: "fixture-repo".to_string(),
            revision: "0123456789abcdef".to_string(),
            build_configuration: "cargo-default".to_string(),
            indexer: "rust-analyzer".to_string(),
            indexer_version: "1.96.0 (ac68faa 2026-05-25)".to_string(),
            project_root: "file:///tmp/cgh096-scip-fixture".to_string(),
            scope: vec!["src/lib.rs".to_string()],
            payload_sha256: hex::encode(Sha256::digest(REAL_INDEX)),
        };
        ScipIndex::parse_bound(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES, &binding, &binding).unwrap()
    }

    #[tokio::test]
    async fn serves_references_from_the_bound_index() {
        let provider = ScipIntelligenceProvider::new(bound_index());
        let location = Location::new("/tmp/cgh096-scip-fixture/src/lib.rs", 1, 8);

        let references = provider.find_references(&location, false).await.unwrap();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].location.line(), 6);
    }

    #[tokio::test]
    async fn reports_a_gap_for_the_operations_the_tier_does_not_answer() {
        let provider = ScipIntelligenceProvider::new(bound_index());
        let location = Location::new("/tmp/cgh096-scip-fixture/src/lib.rs", 1, 8);

        assert!(provider.get_symbols(Path::new("/tmp")).await.is_err());
        assert!(provider.get_hierarchy(&location).await.is_err());
        assert!(
            provider
                .get_document_symbols(Path::new("/tmp"))
                .await
                .is_err()
        );
        assert!(provider.hover(&location).await.is_err());
    }

    #[tokio::test]
    async fn an_unbound_index_answers_no_tiered_question() {
        let index = ScipIndex::parse(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES).unwrap();
        let provider = ScipIntelligenceProvider::new(index);
        let location = Location::new("/tmp/cgh096-scip-fixture/src/lib.rs", 1, 8);

        let outcome = provider.find_references_tiered(&location, true).await;
        assert!(!outcome.is_served());
        assert_eq!(outcome.diagnostics().len(), 1);
        assert_eq!(outcome.diagnostics()[0].attempted_tier, PrecisionTier::S3);
    }
}
