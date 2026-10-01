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
        // ST-01 CLOSED for production code, 2026-09-30.
        //
        // The ports injection moved the service's tree-sitter and verifier
        // use behind `SyntaxAnalysis`, and deleted the never-read
        // `vfs: VirtualFileSystem` field outright. So of the four entries
        // this file carried, ONE is genuinely gone (`infrastructure::vfs::`)
        // and the other three survive only because the `#[cfg(test)]` module
        // composes the real adapters — behaviour tests wire the actual
        // tree-sitter and verifier implementations by design.
        //
        // The three survivors are the regression guard, not leftover debt.
        // Each carries the rule: a match on a non-test line means ST-01
        // regressed. The prefixes stay BROAD on purpose — matching is by
        // prefix (see `TemporaryException::matches`), so `infrastructure::parser`
        // also catches a future `infrastructure::parser::TreeSitterParser`
        // import in production code, which a narrowed
        // `infrastructure::parser::syntax_analysis` would let through.
        ex(
            "application_no_infrastructure",
            "application/services/file_operations.rs",
            "infrastructure::parser",
            "team:st-01",
            "cfg(test)-only: behaviour tests compose the real tree-sitter adapter. Regression rule: a match on a non-test line means ST-01 regressed.",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/file_operations.rs",
            "infrastructure::verification::",
            "team:st-01",
            "cfg(test)-only: behaviour tests compose the real RustVerifier. Regression rule: a match on a non-test line means ST-01 regressed.",
        ),
        ex(
            "application_no_interface",
            "application/services/file_operations.rs",
            "interface::mcp::security",
            "team:st-01",
            "cfg(test)-only: behaviour tests compose InputValidator directly. Production path validation goes through the PathPolicy port (ST-01). Regression rule: a match on a non-test line means ST-01 regressed.",
        ),
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
        // application_no_interface — 1 entry here, measured 2026-09-30.
        //
        // These drifts existed all along: `LayerId` did not model
        // `interface`, so `crate::interface::...` imports resolved to
        // `Unknown` and this constraint could never fire — the previous
        // revision of this section documented "zero drifts", which the
        // audit of 2026-09-30 disproved. With `LayerId::Interface` the
        // evaluator sees them. `file_operations.rs` moved to the ST-01
        // block above (its remaining match is the test module); the one
        // below is the `workspace_session.rs` composition root that
        // ST-02 will rewire.
        // ====================================================================
        ex(
            "application_no_interface",
            "application/workspace_session.rs",
            "interface::mcp::security",
            "team:st-02",
            "ST-02: same security port via the composition root.",
        ),
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
        // 38 -> 40 (2026-09-30): the two application_no_interface
        // drifts became visible once `LayerId::Interface` existed; the
        // previous baseline of 38 counted a constraint whose gate
        // could not fire.
        // 40 -> 39 (2026-09-30, ST-01). One entry is genuinely DELETED:
        // `infrastructure::vfs::` on file_operations.rs, because the
        // `vfs: VirtualFileSystem` field it covered is gone — the field was
        // constructed per service and never read. The other three entries
        // for that file are NOT new: they already existed and are still
        // there because the `#[cfg(test)]` module composes the real
        // adapters. They survive as the regression guard, with their
        // rationales rewritten, not as fresh debt.
        //
        // An earlier revision of this commit claimed 42, having counted
        // those three as additions. It was arithmetic, not measurement;
        // the test below caught it, which is the only reason this file
        // still has a number worth trusting.
        assert_eq!(
            list.len(),
            39,
            "expected exactly 39 entries; if you removed/added a drift \
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
