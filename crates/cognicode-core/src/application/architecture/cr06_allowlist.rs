//! CR-06 — Temporary exceptions for the application_no_infrastructure
//! and application_no_interface rules.
//!
//! Two new canonical constraints were added in CR-06
//! (`architecture.application_no_infrastructure`,
//! `architecture.application_no_interface`). On the current source
//! tree, the first surfaces **historical drift findings** in
//! `cognicode-core` itself (every `use crate::infrastructure::...` from
//! inside `application/`). The remediation is the ST-01..05 program
//! (composition roots + port extraction), which is out of scope for
//! this PR.
//!
//! This module is the **freeze-and-track** mechanism: every drift is
//! listed here as a [`TemporaryException`] with an **owner**
//! (`team:st-XX`), a **rationale** pointing to the remediation slice,
//! and an **expiry** (`2026-12-31`). The registry's `filter_exceptions`
//! consults this list and suppresses matching violations; once an
//! exception expires, the drift surfaces again and the build fails
//! until the slice closes.
//!
//! ## How to retire an exception
//!
//! 1. Refactor the offending `use` statement to go through a port
//!    (`domain::ports` or its successor).
//! 2. Re-run the self-host: the violation no longer fires.
//! 3. Delete the corresponding line from this file.
//! 4. Open the PR. The gate is strict again for that tuple.
//!
//! ## Inventory contract
//!
//! Each line is a single tuple
//! `(constraint_id, file_path, dependency_path)` followed by
//! `owner|rationale|expiry`. The format is greppable
//! (`git grep "architecture.application_no_infrastructure"`) and
//! reviewable: a maintainer can see the full backlog in one screen.
//!
//! `file_path` matches what the architecture evaluator emits for the
//! `architecture_self_host_e2e` test: relative to the crate's `src/`,
//! so it begins with `application/...` (not `src/application/...`).
//!
//! ## Why this lives in the source tree (not in a config file)
//!
//! * **Visibility** — the allowlist is part of the architecture
//!   contract; hiding it in `config/` would make the contract
//!   asymmetric. Source-level visibility is the auditor's friend.
//! * **Versioning** — every PR that retires a slice retires its entry
//!   atomically, with the same code review as the refactor.
//! * **CI-friendly** — `cargo test` already exercises the list; no
//!   extra config-loading path is needed.
//!
//! Last reconciled: 2026-09-26 against `f774b89f`.

use crate::domain::architecture::TemporaryException;

/// The full CR-06 allowlist.
///
/// Every entry is a real drift finding on the current source tree
/// (see `crates/cognicode-core/tests/architecture_self_host_e2e.rs`
/// for the gate that emits them). The list is exhaustive: every
/// drift the evaluator produces against the current source must
/// either be fixed in code or appear here. If a future commit
/// produces a new drift that is not in this list, the self-host
/// test fails — the allowlist is not a permission to drift, it is
/// an inventory of work.
///
/// The `dependency_path` is matched by **prefix** (see
/// [`TemporaryException::matches`]), so an entry whose path ends in
/// `::` covers every leaf under that prefix (e.g. the entry
/// `infrastructure::parser::` covers both
/// `infrastructure::parser::Language` and
/// `infrastructure::parser::TreeSitterParser` from the same `use`
/// statement).
pub fn exceptions() -> Vec<TemporaryException> {
    vec![
        // ====================================================================
        // application_no_infrastructure — ST-01 (FileOperations seams)
        // ====================================================================
        ex(
            "application_no_infrastructure",
            "application/services/file_operations.rs",
            "infrastructure::parser",
            "team:st-01",
            "ST-01: extract PathPolicy; file ops depends on a port, not the concrete tree-sitter adapter.",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/file_operations.rs",
            "infrastructure::vfs::",
            "team:st-01",
            "ST-01: filesystem port (VirtualFileSystem, RealFileSystem).",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/file_operations.rs",
            "infrastructure::verification::",
            "team:st-01",
            "ST-01: verifier port (RustVerifier, RealVerifier).",
        ),
        // ====================================================================
        // application_no_infrastructure — ST-02 (WorkspaceSession composition)
        // ====================================================================
        ex(
            "application_no_infrastructure",
            "application/workspace_session.rs",
            "infrastructure::graph::",
            "team:st-02",
            "ST-02: composition root injects GraphCache via port (covers GraphCache, TraversalDirection).",
        ),
        ex(
            "application_no_infrastructure",
            "application/workspace_session.rs",
            "infrastructure::lsp",
            "team:st-02",
            "ST-02: composition root injects LSP provider via port (CompositeProvider).",
        ),
        ex(
            "application_no_infrastructure",
            "application/workspace_session.rs",
            "infrastructure::parser::",
            "team:st-02",
            "ST-02: parser types via port (Language).",
        ),
        ex(
            "application_no_infrastructure",
            "application/workspace_session.rs",
            "infrastructure::persistence::",
            "team:st-02",
            "ST-02: persistence via port (InMemoryGraphStore).",
        ),
        ex(
            "application_no_infrastructure",
            "application/workspace_session.rs",
            "infrastructure::semantic",
            "team:st-02",
            "ST-02: semantic module as a port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/workspace_session.rs",
            "infrastructure::verification::",
            "team:st-02",
            "ST-02: verifier via port (RustVerifier).",
        ),
        // ====================================================================
        // application_no_infrastructure — ST-03 (AnalysisService mega-split)
        // ====================================================================
        // Bare module path (`use crate::infrastructure::graph;` with
        // no leaf) emits without trailing `::`; prefix-match below
        // covers the module itself plus every leaf under it.
        ex(
            "application_no_infrastructure",
            "application/services/analysis_service.rs",
            "infrastructure::graph",
            "team:st-03",
            "ST-03: graph types behind a port (LightweightIndex, PerFileGraphCache, BuildStatus, PetGraphStore).",
        ),
        // The evaluator emits the bare module path when an import
        // group (`use crate::infrastructure::parser::{Language, ...}`)
        // cannot be resolved to a leaf; prefix-match below covers the
        // group itself plus every leaf under it.
        ex(
            "application_no_infrastructure",
            "application/services/analysis_service.rs",
            "infrastructure::parser",
            "team:st-03",
            "ST-03: parser types behind a port (Language, TreeSitterParser).",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/refactor_service.rs",
            "infrastructure::graph::",
            "team:st-03",
            "ST-03: PetGraphStore behind a port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/refactor_service.rs",
            "infrastructure::parser",
            "team:st-03",
            "ST-03: parser types behind a port (refactor_service).",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/refactor_service.rs",
            "infrastructure::safety",
            "team:st-03",
            "ST-03: safety module behind a port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/refactor_service.rs",
            "infrastructure::refactor::",
            "team:st-03",
            "ST-03: refactor strategies (ExtractStrategy, InlineStrategy) behind a port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/refactor_service.rs",
            "infrastructure::vfs::",
            "team:st-03",
            "ST-03: filesystem port (VirtualFileSystem).",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/graph_insights.rs",
            "infrastructure::graph::analytics::community_detector::",
            "team:st-03",
            "ST-03: CommunityDetector behind a port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/relation_candidates.rs",
            "infrastructure::graph::analytics::community_detector::",
            "team:st-03",
            "ST-03: CommunityDetector behind a port (relation_candidates).",
        ),
        ex(
            "application_no_infrastructure",
            "application/dto/symbol_dto.rs",
            "infrastructure::semantic::symbol_code::",
            "team:st-03",
            "ST-03: extract_docstring is pure-domain; move into domain::semantic.",
        ),
        ex(
            "application_no_infrastructure",
            "application/program_analysis/ast_lift.rs",
            "infrastructure::parser::",
            "team:st-03",
            "ST-03: parser types behind a port (ast_lift).",
        ),
        // ====================================================================
        // application_no_infrastructure — ST-04 (HandlerContext refactor)
        // ====================================================================
        ex(
            "application_no_infrastructure",
            "application/fact_bridge/batch_builder.rs",
            "infrastructure::parser::language_config::",
            "team:st-04",
            "ST-04: parser config injection.",
        ),
        ex(
            "application_no_infrastructure",
            "application/fact_bridge/production_grounding.rs",
            "infrastructure::evidence_kernel",
            "team:st-04",
            "ST-04: kernel via port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/fact_bridge/production_grounding.rs",
            "infrastructure::parser::language_config::",
            "team:st-04",
            "ST-04: parser config injection.",
        ),
        ex(
            "application_no_infrastructure",
            "application/fact_bridge/tree_sitter_facts.rs",
            "infrastructure::parser::language_config::",
            "team:st-04",
            "ST-04: parser config injection.",
        ),
        ex(
            "application_no_infrastructure",
            "application/fact_bridge/lsp_facts.rs",
            "infrastructure::lsp::providers::fallback::",
            "team:st-04",
            "ST-04: TreesitterFallbackProvider via port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/fact_bridge/lsp_facts.rs",
            "infrastructure::lsp::providers::lsp::",
            "team:st-04",
            "ST-04: LspIntelligenceProvider via port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/findings/grounded_finding_flow.rs",
            "infrastructure::evidence_kernel",
            "team:st-04",
            "ST-04: kernel via port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/findings/grounded_finding_flow.rs",
            "infrastructure::parser::language_config::",
            "team:st-04",
            "ST-04: parser config injection.",
        ),
        ex(
            "application_no_infrastructure",
            "application/findings/grounded_ast_projection.rs",
            "infrastructure::parser::language_config::",
            "team:st-04",
            "ST-04: parser config injection.",
        ),
        ex(
            "application_no_infrastructure",
            "application/findings/grounded_ast_projection.rs",
            "infrastructure::evidence_kernel",
            "team:st-04",
            "ST-04: kernel via port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/ingest/scan.rs",
            "infrastructure::parser::",
            "team:st-04",
            "ST-04: parser types via port (LanguageConfig).",
        ),
        ex(
            "application_no_infrastructure",
            "application/ingest/extract_stage.rs",
            "infrastructure::parser::",
            "team:st-04",
            "ST-04: parser types via port (LanguageConfig).",
        ),
        ex(
            "application_no_infrastructure",
            "application/ingest/extractor.rs",
            "infrastructure::parser::",
            "team:st-04",
            "ST-04: parser types via port (LanguageConfig).",
        ),
        ex(
            "application_no_infrastructure",
            "application/ingest/analyzer.rs",
            "infrastructure::graph::graph_cache::",
            "team:st-04",
            "ST-04: graph-cache via port (GraphCache).",
        ),
        ex(
            "application_no_infrastructure",
            "application/ingest/refresh.rs",
            "infrastructure::graph::graph_cache::",
            "team:st-04",
            "ST-04: graph-cache via port (GraphCache).",
        ),
        ex(
            "application_no_infrastructure",
            "application/ingest/refresh.rs",
            "infrastructure::graph::snapshot_provider::",
            "team:st-04",
            "ST-04: snapshot-provider via port (SnapshotProvider).",
        ),
        ex(
            "application_no_infrastructure",
            "application/intelligence_log/recorder.rs",
            "infrastructure::intelligence_log::",
            "team:st-04",
            "ST-04: EventLog via port.",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/lsp_proxy_service.rs",
            "infrastructure::lsp",
            "team:st-04",
            "ST-04: LSP module behind a port (CompositeProvider).",
        ),
        // ====================================================================
        // application_no_interface — current source: zero drifts.
        //
        // application/* does not currently `use crate::bin::...` or
        // `use crate::interface::mcp::...` (the latter lives outside the
        // `application/` tree). The constraint still runs but the
        // allowlist is empty. If a future commit introduces such an
        // import, the gate fires immediately and the entry must be added
        // here with an explicit owner.
        // ====================================================================
    ]
}

fn ex(
    constraint_short: &str,
    file_path: &str,
    dependency_path: &str,
    owner: &str,
    rationale: &str,
) -> TemporaryException {
    TemporaryException {
        constraint_id: format!("architecture.{constraint_short}"),
        file_path: file_path.into(),
        dependency_path: dependency_path.into(),
        owner: owner.into(),
        rationale: rationale.into(),
        // Hard expiry: the gate stops suppressing on 2026-12-31.
        // After that, every entry here becomes a build failure until
        // the corresponding ST-XX slice closes.
        expiry: "2026-12-31".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the inventory size so a future audit can compare
    /// "violations found" against "exceptions declared" without
    /// grepping. The size is recomputed at audit time and asserted
    /// here; if the count drifts, the auditor must update both the
    /// allowlist and this test, in the same commit.
    #[test]
    fn inventory_size_is_pinned_at_current_baseline() {
        let list = exceptions();
        // application_no_infrastructure tuples cover the unique
        // (file, dependency_path) combinations observed by the
        // self-host evaluator against `f774b89f`.
        assert_eq!(
            list.len(),
            38,
            "expected exactly 38 entries; if you removed/added a drift \
             without updating this counter, the allowlist is out of sync \
             with the source. Update both the allowlist and this test in \
             the same commit."
        );
    }

    /// Every entry must point at a real file in the source tree.
    /// If a file is renamed/moved without updating the allowlist, the
    /// exception becomes a silent no-op (it never matches because the
    /// `file_path` no longer exists) — this test surfaces that
    /// situation before the gate does.
    #[test]
    fn every_entry_points_at_an_existing_file() {
        let list = exceptions();
        let cargo_manifest =
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let src_root = std::path::Path::new(&cargo_manifest).join("src");
        for entry in &list {
            let p = src_root.join(&entry.file_path);
            assert!(
                p.exists(),
                "CR-06 allowlist entry points at missing file: {} \
                 (entry: constraint={}, dep={}). Either refactor the file \
                 or rename the entry.",
                p.display(),
                entry.constraint_id,
                entry.dependency_path,
            );
        }
    }

    /// No expiry may be in the past relative to the test's clock
    /// baseline. If an exception has already expired, the test
    /// fails loudly — the drift will surface in production and the
    /// build will break. Better to catch it here.
    #[test]
    fn no_entry_is_already_expired() {
        let list = exceptions();
        for entry in &list {
            // Baseline: 2026-09-26. Tests run on or after this date.
            assert!(
                !entry.is_expired("2026-09-26"),
                "CR-06 allowlist entry has expired: {} @ {} → {} \
                 (expiry={}). Refactor or extend with a documented ADR.",
                entry.constraint_id,
                entry.file_path,
                entry.dependency_path,
                entry.expiry,
            );
        }
    }

    /// Every entry must carry an owner and a rationale (the
    /// contract is owner+rationale+expiry; a missing rationale is a
    /// signal that the entry was added without review).
    #[test]
    fn every_entry_has_owner_rationale_expiry() {
        let list = exceptions();
        for entry in &list {
            assert!(!entry.owner.is_empty(), "owner must be set on every entry");
            assert!(
                !entry.rationale.is_empty(),
                "rationale must be set on every entry: {} @ {}",
                entry.constraint_id,
                entry.file_path
            );
            assert!(
                !entry.expiry.is_empty(),
                "expiry must be set on every entry"
            );
        }
    }
}
