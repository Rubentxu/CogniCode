//! Runtime composition seam for production grounding (e67).
//!
//! Re-exports the async ingestion function from `cognicode-core` so the
//! runtime can offer it as a first-class composition primitive, and so
//! downstream callers (CLI, MCP, tests) have a single, stable surface to
//! depend on.
//!
//! # Async end-to-end
//!
//! `ingest_rust_source` is `pub async fn` and propagates
//! `FactStore::commit(...).await` to its caller. There is no sync bridge.
//! The runtime crate already depends on `tokio` (see Cargo.toml) so the
//! async caller can be `await`-ed from any tokio context.
//!
//! # Layering
//!
//! The runtime is the composition root: it OWNS the seam between the
//! `FactStore` port and the application-level grounding function. The
//! `Explorer` crate does NOT depend on this module directly; it must not
//! construct Facts itself (per e67 design layering rules).

use std::path::Path;
use std::sync::Arc;

use cognicode_core::application::fact_bridge::production_grounding::{
    GroundedIngestReceipt, ProductionGroundingError, ingest_rust_facts,
};
use cognicode_core::domain::evidence_kernel::ids::SnapshotId;
use cognicode_core::domain::evidence_kernel::ports::FactStore;
use cognicode_core::domain::value_objects::WorkspaceId;

/// Async end-to-end ingestion of a Rust source file into the canonical
/// [`FactStore`] (e67 WU1).
///
/// This is a thin composition seam over
/// `cognicode_core::application::fact_bridge::production_grounding::ingest_rust_facts`.
///
/// # Async
///
/// `FactStore::commit(...).await` propagates to the caller. The caller owns
/// async execution. No `block_on`, no nested executor.
///
/// # Errors
///
/// Returns [`ProductionGroundingError`] on extraction failure or kernel
/// commit failure.
pub async fn ingest_rust_source(
    fact_store: Arc<dyn FactStore>,
    ws: WorkspaceId,
    snap: SnapshotId,
    path: &Path,
    source: &str,
    hash: &str,
) -> Result<GroundedIngestReceipt, ProductionGroundingError> {
    ingest_rust_facts(fact_store.as_ref(), &ws, &snap, path, source, hash).await
}

// ============================================================================
// Tests (WU1) — `#[tokio::test]`, mirroring the cognicode-core suite.
// ============================================================================

#[cfg(test)]
#[cfg(feature = "ladybug")] // runtime default feature; tests run with default features
mod tests {
    use super::*;
    use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
    use cognicode_core::infrastructure::evidence_kernel::{
        InMemoryFactStore, InMemorySchemaRegistry,
    };
    use std::path::PathBuf;

    fn fresh_store() -> Arc<InMemoryFactStore> {
        let registry = InMemorySchemaRegistry::new();
        bootstrap_registry(&registry).expect("canonical bootstrap registers the core:* set");
        Arc::new(InMemoryFactStore::new(Arc::new(registry)))
    }

    fn load_fixture() -> (PathBuf, String, String) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is set by cargo during tests");
        // crates/cognicode-runtime -> ../../ -> workspace root
        let path = PathBuf::from(manifest_dir)
            .join("..")
            .join("..")
            .join("sandbox/fixtures/lsi-grounding/sample.rs");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read fixture {:?}: {e}", path));
        let hash = "sha256:e67-fixture-pin";
        (path, source, hash.to_string())
    }

    #[tokio::test]
    async fn test_runtime_grounding_ingest_rust_source() {
        let store = fresh_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        let receipt = ingest_rust_source(store, ws, snap, &path, &source, &hash)
            .await
            .expect("runtime ingest succeeds");

        assert!(receipt.fact_count > 0);
        assert_eq!(receipt.fact_ids.len(), receipt.fact_count);
    }
}
