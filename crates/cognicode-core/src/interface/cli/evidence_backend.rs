// E1.W3 — Evidence CLI backend registry.
//
// The `cognicode evidence <list|search>` subcommand is wired into the
// `CliCommand` enum inside `interface::cli::commands` (gated behind the
// `evidence-cli-ladybug` feature). To keep `cognicode-core` free of the
// `lbug` native dependency, the actual query implementation is provided
// by an `EvidenceBackend` registered at startup by `cognicode-cli`
// when it boots with `--features ladybug`.
//
// Rationale (PRF / Hexagonal):
//   * `cognicode-core` defines the *port* (`EvidenceBackend` trait +
//     `register_evidence_backend` hook + `execute_evidence` driver).
//   * `cognicode-cli` provides the *adapter* (the `LadybugEvidenceBackend`
//     in `cognicode-cli/src/cmd/evidence.rs`, registered in `main.rs`
//     under the same cfg gate).
//   * This mirrors how the rest of the workspace already splits domain
//     ports from infrastructure adapters.
//
// The CLI subcommand can carry a per-invocation `--db-path` override,
// so the registry exposes a factory closure rather than a single
// `Arc<dyn EvidenceBackend>`. The factory is invoked once per
// `execute_evidence` call with the requested path (or `None` for
// "use the backend's default").
//
// Default-build behaviour:
//   * When `cognicode-core` is compiled without `evidence-cli-ladybug`,
//     the entire module is `cfg`-stripped — including the trait, the
//     registry, and the `execute_evidence` driver. No dead code, no
//     dangling symbols.
//   * When compiled with `evidence-cli-ladybug` but no backend registered
//     (e.g. a downstream consumer forgets to call `register_evidence_backend`),
//     `execute_evidence` returns an error pointing at the missing
//     registration. This is the failure mode documented in ADR-009 §3.

use crate::domain::ports::evidence_store::{EvidenceKind, EvidenceSummary};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// E1.W3 — Backend trait for the `cognicode evidence` family.
///
/// Implementations live in downstream crates (e.g. `cognicode-cli`'s
/// `LadybugEvidenceBackend`) and are registered once at startup via
/// [`register_evidence_backend`].
///
/// The trait is intentionally narrow: only the two read operations
/// surfaced in E1.W3. Writing evidence rows stays out of scope (the
/// writer port is the next E1 slice — E1.W4 — and will use the same
/// factory pattern when it lands).
pub trait EvidenceBackend: Send + Sync {
    /// List evidence rows for a workspace, optionally filtered by kind.
    fn list(
        &self,
        workspace: &str,
        kind: Option<EvidenceKind>,
    ) -> Result<Vec<EvidenceSummary>, String>;

    /// Full-text search across evidence titles and excerpts.
    fn search(
        &self,
        workspace: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EvidenceSummary>, String>;
}

/// Factory that opens a backend for a specific DB path. The CLI
/// registers one factory at startup; `execute_evidence` calls it on
/// every invocation with the per-command `--db-path` (or `None` if
/// the user did not override the default).
///
/// The returned backend is wrapped in an `Arc` so the executor can
/// keep it alive for the duration of the query without re-opening.
pub type EvidenceBackendFactory =
    Arc<dyn Fn(Option<&PathBuf>) -> Result<Arc<dyn EvidenceBackend>, String> + Send + Sync>;

static EVIDENCE_BACKEND_FACTORY: OnceLock<EvidenceBackendFactory> = OnceLock::new();

/// Register the global evidence backend factory. Called from
/// `cognicode-cli/src/main.rs` under
/// `#[cfg(feature = "evidence-cli-ladybug")]`.
///
/// Returns `Ok(())` on first successful registration. Subsequent calls
/// are ignored (logged to stderr) — this prevents accidental
/// double-registration from blowing up the CLI.
pub fn register_evidence_backend(factory: EvidenceBackendFactory) -> Result<(), String> {
    match EVIDENCE_BACKEND_FACTORY.set(factory) {
        Ok(()) => Ok(()),
        Err(_) => {
            eprintln!(
                "cognicode-core: evidence backend factory already registered; \
                 second register_evidence_backend call ignored"
            );
            Ok(())
        }
    }
}

/// Look up the registered factory, if any.
pub fn evidence_backend_factory() -> Option<EvidenceBackendFactory> {
    EVIDENCE_BACKEND_FACTORY.get().cloned()
}
