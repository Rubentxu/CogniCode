// E1.W3 — `cognicode evidence <list|search>` adapter for LadybugDB.
//
// Compiled only when the CLI's `ladybug` feature is active (which in
// turn activates `cognicode-core`'s `evidence-cli-ladybug` feature).
//
// The adapter:
//   1. Provides a factory closure that opens (or creates) the
//      LadybugDB file at the requested path and wraps it as a
//      `LadybugEvidenceBackend`. The factory is registered once at
//      CLI startup via
//      `cognicode_core::interface::cli::evidence_backend::
//      register_evidence_backend`.
//   2. Delegates `list` / `search` to the `LadybugEvidenceStore`
//      instance built around the same store.
//
// We deliberately re-use the LadybugEvidenceStore from cognicode-ladybug
// rather than reimplementing the Cypher here. That keeps the CLI and
// the runtime's `into_api_state` path reading from the same code, so
// `cognicode-cli/tests/evidence_cli_mcp_equivalence.rs` can pin the
// JSON shape without two implementations to drift apart.

use cognicode_core::domain::ports::evidence_store::{EvidenceKind, EvidenceSummary};
use cognicode_core::interface::cli::evidence_backend::{
    register_evidence_backend, EvidenceBackend, EvidenceBackendFactory,
};
use cognicode_ladybug::LadybugStore;
use std::path::PathBuf;
use std::sync::Arc;

/// Adapter that wraps a `LadybugStore` and exposes the two read
/// operations surfaced by the CLI (`list_evidence` and `search_evidence`).
///
/// Cloning is cheap: the inner `Arc<LadybugStore>` is shared.
#[derive(Clone)]
pub struct LadybugEvidenceBackend {
    inner: Arc<LadybugStore>,
}

impl LadybugEvidenceBackend {
    /// Open the LadybugDB at `db_path` and wrap the resulting store
    /// as a backend.
    ///
    /// `LadybugStore::open()` already runs `init_evidence_schema()`
    /// internally (idempotent DDL), so we do NOT call it again here —
    /// doing so would just cost one extra round-trip with no effect.
    pub fn open(db_path: PathBuf) -> Result<Self, String> {
        let store = LadybugStore::open(db_path).map_err(|e| format!("open LadybugStore: {e}"))?;
        Ok(Self {
            inner: Arc::new(store),
        })
    }
}

impl EvidenceBackend for LadybugEvidenceBackend {
    fn list(
        &self,
        workspace: &str,
        kind: Option<EvidenceKind>,
    ) -> Result<Vec<EvidenceSummary>, String> {
        // `LadybugEvidenceStore` impls the domain `EvidenceStore`
        // trait directly; we forward 1:1.
        use cognicode_core::domain::ports::evidence_store::EvidenceStore;
        self.inner
            .list_evidence(workspace, kind)
            .map_err(|e| format!("{e}"))
    }

    fn search(
        &self,
        workspace: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EvidenceSummary>, String> {
        use cognicode_core::domain::ports::evidence_store::EvidenceStore;
        self.inner
            .search_evidence(workspace, query, limit)
            .map_err(|e| format!("{e}"))
    }
}

/// Factory closure that opens a `LadybugEvidenceBackend` for the
/// given path. The CLI registers one of these at startup via
/// `register_evidence_backend`.
///
/// If the caller passes `None` (i.e. no `--db-path` and no default
/// helper available in `cognicode-core`), this falls back to the
/// same default path (`<cwd>/.cognicode/evidence.lbdb`) — but the
/// production `execute_evidence` driver always supplies a concrete
/// path, so the fallback here is only for direct consumers of the
/// factory (e.g. integration tests).
pub fn factory() -> EvidenceBackendFactory {
    Arc::new(|maybe_path: Option<&PathBuf>| -> Result<Arc<dyn EvidenceBackend>, String> {
        let path = match maybe_path {
            Some(p) => p.clone(),
            None => {
                let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                p.push(".cognicode");
                let _ = std::fs::create_dir_all(&p);
                p.push("evidence.lbdb");
                p
            }
        };
        let backend = LadybugEvidenceBackend::open(path)?;
        Ok(Arc::new(backend))
    })
}

/// Convenience: register the Ladybug-backed factory with the global
/// evidence backend registry. Called from `cognicode-cli/src/main.rs`
/// under `#[cfg(feature = "ladybug")]`.
pub fn register() -> Result<(), String> {
    register_evidence_backend(factory())
}
