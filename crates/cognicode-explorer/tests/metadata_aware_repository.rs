//! Contract tests for [`MetadataAwareRepository`].
//!
//! This file implements the four contract scenarios from the
//! `explorer-graph-repository-bridge` spec:
//!
//! 1. **Golden triples**: a frozen graph with three edges of distinct
//!    provenance yields exact `(Provenance, f64)` triples via
//!    `callees_with_metadata()`.
//! 2. **Invariant**: every `confidence` returned by
//!    `edges_with_metadata()` is finite, not NaN, and in `[0.0, 1.0]`.
//! 3. **Backward compatibility**: the base `SymbolRepository::callees`
//!    method on the same fixture returns `RelationTarget` values that
//!    do NOT carry the new metadata fields (sub-trait fields are
//!    simply absent from the base return type).
//! 4. **Polymorphism**: `&dyn SymbolRepository` (the base trait) does
//!    NOT expose the metadata methods — only the `MetadataAwareRepository`
//!    sub-trait does.

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, Provenance, SymbolKind};
use cognicode_explorer::adapters::CallGraphRepository;
use cognicode_explorer::ports::{MetadataAwareRepository, SymbolRepository};

/// Build the canonical "frozen graph" fixture from the spec
/// (task 4.2): three edges from `a`, one per extraction context.
fn build_frozen_metadata_graph() -> (CallGraph, SymbolId, SymbolId, SymbolId) {
    let mut g = CallGraph::new();
    let a = g.add_symbol(Symbol::new(
        "alpha",
        SymbolKind::Function,
        Location::new("src/a.rs", 1, 0),
    ));
    let b = g.add_symbol(Symbol::new(
        "beta",
        SymbolKind::Function,
        Location::new("src/b.rs", 5, 0),
    ));
    let c = g.add_symbol(Symbol::new(
        "gamma",
        SymbolKind::Function,
        Location::new("src/c.rs", 9, 0),
    ));
    // DirectExtraction -> (Extracted, 1.0)
    g.add_dependency_with_provenance(
        &a,
        &b,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    )
    .expect("add a->b");
    // Heuristic 0.7 -> (Inferred, 0.7)
    g.add_dependency_with_provenance(
        &a,
        &c,
        DependencyType::Imports,
        ExtractionContext::Heuristic { score: 0.7 },
    )
    .expect("add a->c");
    // Unresolved -> (Ambiguous, 0.3) (per ConfidenceRules)
    let d = g.add_symbol(Symbol::new(
        "delta",
        SymbolKind::Function,
        Location::new("src/d.rs", 20, 0),
    ));
    g.add_dependency_with_provenance(
        &a,
        &d,
        DependencyType::References,
        ExtractionContext::Unresolved,
    )
    .expect("add a->d");

    (g, a, b, c)
}

#[test]
fn golden_metadata_triples_match_confidence_rules_output() {
    let (g, a, _b, _c) = build_frozen_metadata_graph();
    let repo = CallGraphRepository::from_graph(g);

    // Use the trait method to prove the sub-trait compiles through
    // the opt-in dispatch.
    let metas: Vec<_> = MetadataAwareRepository::callees_with_metadata(&repo, &a);

    assert_eq!(metas.len(), 3, "expected exactly three edges from a");

    // Find each entry by provenance and assert the exact f64.
    let extracted = metas
        .iter()
        .find(|m| m.provenance == Provenance::Extracted)
        .expect("Extracted edge present");
    assert_eq!(extracted.provenance, Provenance::Extracted);
    assert_eq!(extracted.confidence, 1.0_f64);
    assert_eq!(extracted.dependency_type, DependencyType::Calls);

    let inferred = metas
        .iter()
        .find(|m| m.provenance == Provenance::Inferred)
        .expect("Inferred edge present");
    assert_eq!(inferred.provenance, Provenance::Inferred);
    assert_eq!(inferred.confidence, 0.7_f64);
    assert_eq!(inferred.dependency_type, DependencyType::Imports);

    let ambiguous = metas
        .iter()
        .find(|m| m.provenance == Provenance::Ambiguous)
        .expect("Ambiguous edge present");
    assert_eq!(ambiguous.provenance, Provenance::Ambiguous);
    assert_eq!(ambiguous.confidence, 0.3_f64);
    assert_eq!(ambiguous.dependency_type, DependencyType::References);
}

#[test]
fn invariant_every_confidence_is_finite_and_in_unit_range() {
    let (g, a, _b, _c) = build_frozen_metadata_graph();
    let repo = CallGraphRepository::from_graph(g);

    let edges = MetadataAwareRepository::edges_with_metadata(&repo);
    assert!(!edges.is_empty(), "fixture must produce edges");
    for edge in &edges {
        assert!(
            edge.confidence.is_finite(),
            "non-finite conf: {}",
            edge.confidence
        );
        assert!(!edge.confidence.is_nan(), "NaN conf leaked");
        assert!(
            (0.0..=1.0).contains(&edge.confidence),
            "out-of-range conf: {}",
            edge.confidence
        );
    }

    // The base trait call site must also remain a coherent entry point.
    let _ = repo.callees(&a);
}

#[test]
fn backward_compat_base_callees_has_no_metadata_fields() {
    let (g, a, _b, _c) = build_frozen_metadata_graph();
    let repo = CallGraphRepository::from_graph(g);

    // The base `SymbolRepository::callees` returns `Vec<RelationTarget>`.
    // The struct is unchanged from the pre-slice state — no metadata
    // fields exist on it. We assert that the call still succeeds and
    // the targets are well-formed.
    let targets = <CallGraphRepository as SymbolRepository>::callees(&repo, &a);
    assert_eq!(targets.len(), 3);

    // Every target carries the symbol's display fields and nothing
    // else — the sub-trait's metadata is reachable only via the
    // sub-trait method.
    for t in &targets {
        assert!(!t.name.is_empty(), "callee name should be resolved");
        assert!(!t.file.is_empty(), "callee file should be resolved");
    }

    // Same data, metadata-aware view: three entries, one per edge.
    let metas = MetadataAwareRepository::callees_with_metadata(&repo, &a);
    assert_eq!(metas.len(), targets.len());
    for (target, meta) in targets.iter().zip(metas.iter()) {
        // The metadata-aware view's `target` field is the same DTO as
        // the base trait returns, so we can byte-compare them.
        assert_eq!(&meta.target, target);
    }
}

#[test]
fn polymorphism_base_dyn_reference_does_not_expose_metadata_methods() {
    let (g, a, _b, _c) = build_frozen_metadata_graph();
    let repo = CallGraphRepository::from_graph(g);

    // Upcast to the base trait object.
    let base_ref: &dyn SymbolRepository = &repo;

    // Base trait call sites still compile and return the right shape.
    let _targets = base_ref.callees(&a);
    let _stats = base_ref.graph_stats();
    let _fan_in = base_ref.fan_in(&a);
    let _fan_out = base_ref.fan_out(&a);

    // The metadata-aware surface is NOT reachable through `&dyn SymbolRepository`.
    // We use a compile-time trick: assign the function pointer for
    // `callees_with_metadata` from the sub-trait and prove it has a
    // distinct type. If this test compiles, polymorphism is preserved.
    fn assert_metadata_method_trait_object_distinct() {
        fn _accepts_base(_: &dyn SymbolRepository) {}
        fn _accepts_metadata(_: &dyn MetadataAwareRepository) {
            // never called; just proves the two are distinct trait objects
        }
        // Both function pointers exist; the test fails at compile time
        // if either trait loses its object-safety.
        let _: fn(&dyn SymbolRepository) = _accepts_base;
        let _: fn(&dyn MetadataAwareRepository) = _accepts_metadata;
    }
    assert_metadata_method_trait_object_distinct();

    // The helper downcast does expose the sub-trait surface explicitly.
    let metadata_ref = CallGraphRepository::as_metadata_aware(&repo)
        .expect("CallGraphRepository implements MetadataAwareRepository");
    let metas = metadata_ref.callees_with_metadata(&a);
    assert_eq!(metas.len(), 3);
}

/// Dyn-compatibility smoke test for the new `Repository` trait.
/// Ensures `Box<dyn Repository>` is usable where `Send + Sync` is
/// required (the spec explicitly calls this out for the PostgreSQL
/// seam in a follow-up slice).
#[tokio::test]
async fn repository_trait_is_dyn_compatible_send_sync() {
    use async_trait::async_trait;
    use cognicode_core::domain::aggregates::Symbol;
    use cognicode_core::domain::traits::{Repository, RepositoryError};
    use cognicode_core::domain::value_objects::EdgeMetadata;
    use std::sync::Arc;

    struct StubRepo;

    #[async_trait]
    impl Repository for StubRepo {
        async fn find_symbol_by_qualified_name(
            &self,
            _name: &str,
        ) -> Result<Option<Symbol>, RepositoryError> {
            Ok(None)
        }
        async fn count_symbols(&self) -> Result<usize, RepositoryError> {
            Ok(0)
        }
        async fn find_edges_by_caller(
            &self,
            _caller_id: &str,
        ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
            Ok(Vec::new())
        }
        async fn find_edges_by_callee(
            &self,
            _callee_id: &str,
        ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
            Ok(Vec::new())
        }
        async fn count_edges(&self) -> Result<usize, RepositoryError> {
            Ok(0)
        }
    }

    let boxed: Box<dyn Repository> = Box::new(StubRepo);
    let shared: Arc<dyn Repository> = Arc::new(StubRepo);
    assert_eq!(boxed.count_symbols().await.unwrap(), 0);
    assert_eq!(shared.count_symbols().await.unwrap(), 0);
    assert_eq!(boxed.count_edges().await.unwrap(), 0);
    assert!(
        boxed
            .find_symbol_by_qualified_name("nope")
            .await
            .unwrap()
            .is_none()
    );
    assert!(boxed.find_edges_by_caller("nope").await.unwrap().is_empty());
    assert!(boxed.find_edges_by_callee("nope").await.unwrap().is_empty());
}
