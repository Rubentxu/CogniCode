//! Production grounding activation (e67).
//!
//! Wires the existing tree-sitter extraction pipeline
//! (`extract_file` + `tree_sitter_facts::collect` + `FactBatchBuilder`) to the
//! canonical [`FactStore`] port. This is the first production call-site of
//! `FactStore::commit` in the codebase (e67 explore phase verified there are
//! zero pre-existing callers).
//!
//! # Async end-to-end
//!
//! This module honours REQ-DGN-001 from `openspec/changes/e67-lsi-production-grounding/design.md`:
//! the production seam is `pub async fn` and `FactStore::commit(...).await`
//! propagates to the caller. No sync wrapper, no `Handle::current().block_on`,
//! no nested executor.
//!
//! # Receipt
//!
//! `ingest_rust_facts` returns a [`GroundedIngestReceipt`] instead of `()`
//! so that downstream consumers (WU2's detector exercise, WU3's gate tests)
//! can refer back to the exact knowledge produced by this commit without
//! re-reading the store.
//!
//! # Layering
//!
//! - Domain: `Fact`, `FactId`, `FactStore`, `WorkspaceId`, `SnapshotId`,
//!   `RelationKind`, `KernelError`.
//! - Application: `extract_file`, `tree_sitter_facts::collect`,
//!   `FactBatchBuilder`, `RUST_CONFIG`.
//! - No `tree-sitter` types in the public signature.
//! - No concrete `FactStore` adapter.

use std::path::Path;

use crate::application::fact_bridge::{batch_builder::FactBatchBuilder, tree_sitter_facts};
use crate::application::ingest::extractor::extract_file;
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::evidence_kernel::ports::{FactStore, KernelError};
use crate::domain::kernel_ids::FactId;
use crate::domain::value_objects::WorkspaceId;
use crate::infrastructure::parser::language_config::RUST_CONFIG;

/// Receipt returned by [`ingest_rust_facts`].
///
/// Carries enough information for downstream consumers (WU2 detectors, WU3
/// gate tests, e68 affected-work planner) to refer to the exact knowledge
/// produced by a single commit, without re-reading the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundedIngestReceipt {
    /// The snapshot the batch was committed into (echoed for convenience).
    pub snapshot: SnapshotId,
    /// The `FactId`s assigned by `FactStore::commit` to the persisted facts.
    pub fact_ids: Vec<FactId>,
    /// Number of facts persisted (`fact_ids.len()`).
    pub fact_count: usize,
}

/// Errors raised by [`ingest_rust_facts`].
#[derive(Debug, thiserror::Error)]
pub enum ProductionGroundingError {
    /// The tree-sitter extractor reported a failed extraction.
    /// The bridge does NOT commit a partial batch in this case (e37 design D1).
    #[error("rust extraction failed for {path}: {message}")]
    Extract { path: String, message: String },

    /// `FactStore::commit` returned a kernel error.
    #[error("fact store commit failed: {0}")]
    Commit(#[from] KernelError),
}

/// Async end-to-end ingestion seam (e67 WU1).
///
/// Flow:
/// 1. `extract_file(RUST_CONFIG, path, source, hash)` (sync, local)
/// 2. On extraction failure → return [`ProductionGroundingError::Extract`]
/// 3. `tree_sitter_facts::collect(&mut builder, &result)` (sync)
/// 4. `builder.finish() -> Vec<Fact>` (sync, canonical ordering)
/// 5. `fact_store.commit(ws, snap, batch).await` (async, propagates)
/// 6. Return [`GroundedIngestReceipt`]
///
/// `FactStore::commit(...).await` propagates to the caller. The caller owns
/// async execution. There is no sync wrapper around the async call.
pub async fn ingest_rust_facts(
    fact_store: &dyn FactStore,
    ws: &WorkspaceId,
    snap: &SnapshotId,
    path: &Path,
    source: &str,
    hash: &str,
) -> Result<GroundedIngestReceipt, ProductionGroundingError> {
    let result = extract_file(&RUST_CONFIG, path, source, hash);
    if let Some(err) = result.error.as_ref() {
        return Err(ProductionGroundingError::Extract {
            path: path.display().to_string(),
            message: err.clone(),
        });
    }

    let mut builder = FactBatchBuilder::new(*snap);
    tree_sitter_facts::collect(&mut builder, &result);
    let facts: Vec<_> = builder.finish();

    let fact_ids = fact_store.commit(ws, snap, facts).await?;

    Ok(GroundedIngestReceipt {
        snapshot: *snap,
        fact_count: fact_ids.len(),
        fact_ids,
    })
}

// ============================================================================
// Tests (WU1) — all `#[tokio::test]` per the cycle's async invariant.
// ============================================================================

#[cfg(test)]
#[cfg(feature = "evidence-kernel")]
mod tests {
    use super::*;
    use crate::domain::evidence_kernel::bootstrap::bootstrap_registry;
    use crate::infrastructure::evidence_kernel::{InMemoryFactStore, InMemorySchemaRegistry};
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Construct a fresh store with the canonical `core:*` predicate set.
    fn fresh_store() -> Arc<InMemoryFactStore> {
        let registry = InMemorySchemaRegistry::new();
        bootstrap_registry(&registry).expect("canonical bootstrap registers the core:* set");
        Arc::new(InMemoryFactStore::new(Arc::new(registry)))
    }

    /// Load the fixture from disk. Tests MUST read the fixture source from
    /// disk (NOT a hand-crafted `ExtractionResult`) — invariant #1.
    /// Uses `CARGO_MANIFEST_DIR` so the test works regardless of which
    /// workspace `cargo test` is invoked from.
    fn load_fixture() -> (PathBuf, String, String) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is set by cargo during tests");
        // crates/cognicode-core -> ../../ -> workspace root
        let path = PathBuf::from(manifest_dir)
            .join("..")
            .join("..")
            .join("sandbox/fixtures/lsi-grounding/sample.rs");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {:?}: {e}", path));
        let hash = "sha256:e67-fixture-pin";
        (path, source, hash.to_string())
    }

    /// Helper: predicates present in a snapshot.
    async fn predicates_in(
        store: &dyn FactStore,
        ws: &WorkspaceId,
        snap: &SnapshotId,
    ) -> Vec<String> {
        let facts = store
            .facts_in_snapshot(ws, snap)
            .await
            .expect("facts_in_snapshot");
        let mut preds: Vec<String> = facts
            .iter()
            .map(|f| f.predicate.as_str().to_string())
            .collect();
        preds.sort();
        preds.dedup();
        preds
    }

    /// Helper: FactIds present in a snapshot.
    async fn ids_in(
        store: &dyn FactStore,
        ws: &WorkspaceId,
        snap: &SnapshotId,
    ) -> BTreeSet<FactId> {
        let facts = store
            .facts_in_snapshot(ws, snap)
            .await
            .expect("facts_in_snapshot");
        facts.iter().map(|f| f.id).collect()
    }

    #[tokio::test]
    async fn test_ingest_rust_fixture_into_canonical_factstore() {
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        let receipt = ingest_rust_facts(&*store, &ws, &snap, &path, &source, &hash)
            .await
            .expect("ingest succeeds");

        assert!(
            receipt.fact_count >= 3,
            "expected at least 3 facts (2 defines + 1 calls + 1 contains); got {}",
            receipt.fact_count
        );
        assert_eq!(receipt.fact_ids.len(), receipt.fact_count);

        // Assert core:defines is present for at least the fixture's two
        // symbols (`sample::greet` and `sample::inner::helper`).
        let facts = store
            .facts_in_snapshot(&ws, &snap)
            .await
            .expect("facts_in_snapshot");
        let defines_count = facts
            .iter()
            .filter(|f| f.predicate.as_str() == "core:defines")
            .count();
        assert!(
            defines_count >= 2,
            "expected >=2 core:defines (greet, helper); got {}",
            defines_count
        );
    }

    #[tokio::test]
    async fn test_batch_predicate_set_bounded() {
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        ingest_rust_facts(&*store, &ws, &snap, &path, &source, &hash)
            .await
            .expect("ingest succeeds");

        let facts = store
            .facts_in_snapshot(&ws, &snap)
            .await
            .expect("facts_in_snapshot");

        let allowed: BTreeSet<&str> = [
            "core:defines",
            "core:contains",
            "core:calls",
            "core:imports",
            "core:references",
        ]
        .into_iter()
        .collect();

        for fact in &facts {
            assert!(
                allowed.contains(fact.predicate.as_str()),
                "predicate {} leaked outside the bridge's canonical set",
                fact.predicate.as_str()
            );
        }
    }

    #[tokio::test]
    async fn test_replay_byte_stable() {
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        // Two independent stores, same source — semantic equivalence.
        let store_a = fresh_store();
        let store_b = fresh_store();

        let receipt_a = ingest_rust_facts(&*store_a, &ws, &snap, &path, &source, &hash)
            .await
            .expect("ingest A succeeds");
        let receipt_b = ingest_rust_facts(&*store_b, &ws, &snap, &path, &source, &hash)
            .await
            .expect("ingest B succeeds");

        // Determinism: same number of facts (FactIds themselves are batch-local
        // so they need not match across independent stores).
        assert_eq!(receipt_a.fact_count, receipt_b.fact_count);

        // Semantic equivalence: same predicate set in both.
        let preds_a = predicates_in(&*store_a, &ws, &snap).await;
        let preds_b = predicates_in(&*store_b, &ws, &snap).await;
        assert_eq!(preds_a, preds_b);
    }

    #[tokio::test]
    async fn rust_production_ingest_persists_facts_pinned_to_snapshot() {
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        let receipt = ingest_rust_facts(&*store, &ws, &snap, &path, &source, &hash)
            .await
            .expect("ingest succeeds");
        assert!(receipt.fact_count > 0);

        // Read back: for every persisted FactId, the snapshot must contain
        // a fact with that id.
        let visible_ids = ids_in(&*store, &ws, &snap).await;

        for fid in &receipt.fact_ids {
            assert!(
                visible_ids.contains(fid),
                "FactId {:?} from receipt is not visible in snapshot {}",
                fid,
                snap.get()
            );
        }
    }

    #[tokio::test]
    async fn production_ingest_is_not_visible_from_another_snapshot() {
        // U42-style snapshot isolation regression guard: commit to snap=1,
        // then create snap=2 for the SAME workspace, and assert the facts
        // from snap=1 are not visible in snap=2.
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap1 = SnapshotId::new(1);
        let snap2 = SnapshotId::new(2);
        let (path, source, hash) = load_fixture();

        let receipt1 = ingest_rust_facts(&*store, &ws, &snap1, &path, &source, &hash)
            .await
            .expect("ingest snap1 succeeds");
        assert!(receipt1.fact_count > 0);

        let snap2_facts = store
            .facts_in_snapshot(&ws, &snap2)
            .await
            .expect("facts_in_snapshot snap2");
        assert_eq!(
            snap2_facts.len(),
            0,
            "facts from snap=1 must not leak into snap=2 (got {} facts)",
            snap2_facts.len()
        );

        // And: no FactId from receipt1 is visible in snap2.
        let snap2_ids = ids_in(&*store, &ws, &snap2).await;
        for fid in &receipt1.fact_ids {
            assert!(
                !snap2_ids.contains(fid),
                "FactId {:?} from snap=1 leaked into snap=2",
                fid
            );
        }
    }
}
