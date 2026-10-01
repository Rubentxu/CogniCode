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
            "cfg(test)-only: behaviour tests compose the real tree-sitter adapter. This gate CANNOT see line numbers, so it suppresses any import in this file; the real guard is `cfg_test_only_entries_really_have_no_production_import`.",
        ),
        ex(
            "application_no_infrastructure",
            "application/services/file_operations.rs",
            "infrastructure::verification::",
            "team:st-01",
            "cfg(test)-only: behaviour tests compose the real RustVerifier. This gate CANNOT see line numbers, so it suppresses any import in this file; the real guard is `cfg_test_only_entries_really_have_no_production_import`.",
        ),
        ex(
            "application_no_interface",
            "application/services/file_operations.rs",
            "interface::mcp::security",
            "team:st-01",
            "cfg(test)-only: behaviour tests compose InputValidator directly. Production path validation goes through the PathPolicy port (ST-01). This gate CANNOT see line numbers, so it suppresses any import in this file; the real guard is `cfg_test_only_entries_really_have_no_production_import`.",
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
        // application_no_interface — 1 entry here, re-measured 2026-10-01.
        //
        // These drifts existed all along: `LayerId` did not model
        // `interface`, so `crate::interface::...` imports resolved to
        // `Unknown` and this constraint could never fire — the previous
        // revision of this section documented "zero drifts", which the
        // audit of 2026-09-30 disproved. With `LayerId::Interface` the
        // evaluator sees them.
        //
        // ST-02 slice 1 rewired the composition: `WorkspaceSession::new`
        // became `with_path_policy`, and the code that names
        // `InputValidator` moved to the two `interface/cli/commands.rs`
        // call sites, which may legally name an interface type. The only
        // remaining import in this crate's `application` layer is the
        // `#[cfg(test)]` convenience constructor that composes the REAL
        // validator for the 75 behavioural tests in that module.
        //
        // So this entry is no longer production debt: it is a regression
        // guard, exactly like the three `file_operations.rs` entries above.
        // It does NOT disappear, and that is the measured fact, not an
        // oversight: the evaluator reads source lines, so a `#[cfg(test)]`
        // import still matches. N+69.3 predicted 39 -> 38 entries; the
        // self-host run says otherwise and the count stays 39.
        //
        // With both entries being test-only guards, no PRODUCTION
        // `application -> interface` coupling remains in the codebase.
        // ====================================================================
        ex(
            "application_no_interface",
            "application/workspace_session.rs",
            "interface::mcp::security",
            "team:st-02",
            "cfg(test)-only after ST-02 slice 1: the 75 behavioural tests in this module compose the real InputValidator by design. Production path validation goes through the PathPolicy port, wired by interface/cli/commands.rs. This gate CANNOT see line numbers, so it suppresses any import in this file; the real guard is `cfg_test_only_entries_really_have_no_production_import`.",
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

/// Classify every line containing `needle` as inside or outside a
/// `#[cfg(test)]` scope.
///
/// Returns `(lineno, trimmed_line, is_test_only)` for each hit.
///
/// A `#[cfg(test)]` attribute is only test-gating the *item it annotates*,
/// not the rest of the file, so the scope is the brace-balanced span that
/// follows the attribute. Concretely:
///
/// - `#[cfg(test)] mod tests { … }` — the span is the whole module.
/// - `#[cfg(test)] fn helper() { … }` — the span is that function's body.
///
/// The scan records the brace depth at which each `#[cfg(test)]` attribute
/// appears, and treats every subsequent line as test-only until the depth
/// returns below that point. Lines that are not inside any such span are
/// production, which is the case this guard exists to catch.
///
/// A `#[cfg(test)]` with no braces (a `use` or a `const`) applies to that one
/// item only, and the depth scan terminates on the first newline that closes
/// it. This is a source scanner, not a parser: it does not attempt to be
/// Rust-aware, so string literals containing braces would skew it. The two
/// files it is pointed at are not written that way, and the guard fails
/// loudly on a violation rather than silently, which is the property that
/// matters here.
#[cfg(test)]
fn classify_imports<'a>(source: &'a str, needle: &str) -> Vec<(usize, &'a str, bool)> {
    let mut hits = Vec::new();
    // Brace depth at which each open `#[cfg(test)]` scope started. Signed,
    // because a closing `}` moves the depth down and a `u32` would wrap.
    let mut test_scopes: Vec<i32> = Vec::new();
    let mut depth: i32 = 0;
    let mut pending_cfg_test = false;

    for (idx, raw) in source.lines().enumerate() {
        let lineno = idx + 1;
        let trimmed = raw.trim();

        // The attribute gates the item that comes after it, so it only
        // opens a scope on the following line.
        if trimmed == "#[cfg(test)]" {
            pending_cfg_test = true;
            continue;
        }

        if trimmed.starts_with("use ") && trimmed.contains(needle) {
            hits.push((lineno, trimmed, test_scopes.iter().any(|d| *d < depth)));
        }

        if pending_cfg_test {
            test_scopes.push(depth);
            pending_cfg_test = false;
        }

        // Count this line's braces BEFORE closing scopes. The line that opens
        // a `#[cfg(test)] mod tests {` is where the scope becomes real, so
        // filtering first would drop the scope on the same line it was added
        // (its opening depth equals the current depth, not less than it).
        depth += count_braces(trimmed);

        // A scope stays open while the depth is strictly greater than the
        // depth at which it opened, and closes once we return to it.
        test_scopes.retain(|d| *d < depth);
    }

    hits
}

/// Net brace delta for a line, ignoring braces inside string literals and
/// character literals so that a `"{"` in a message does not open a scope.
#[cfg(test)]
fn count_braces(line: &str) -> i32 {
    let mut delta = 0i32;
    let mut in_str = false;
    let mut in_char = false;
    let mut escaped = false;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if escaped {
            escaped = false;
        } else if c == b'\\' && (in_str || in_char) {
            escaped = true;
        } else if in_str {
            if c == b'"' {
                in_str = false;
            }
        } else if in_char {
            if c == b'\'' {
                in_char = false;
            }
        } else {
            match c {
                b'"' => in_str = true,
                b'\'' => in_char = true,
                b'{' => delta += 1,
                b'}' => delta -= 1,
                _ => {}
            }
        }
        i += 1;
    }
    delta
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

    /// Makes the "cfg(test)-only" rationale actually enforceable.
    ///
    /// Four allowlist entries claim they exist only because the test module
    /// composes real adapters, and that a production regression would be
    /// caught. **Measured 2026-10-01: the gate does not catch it.**
    /// Reintroducing `use crate::interface::mcp::security::InputValidator;`
    /// at the top of `workspace_session.rs` left `architecture_self_host_e2e`
    /// at 6/6 green.
    ///
    /// The reason is `TemporaryException::matches`: it compares
    /// `constraint_id`, `file_path` and a `dependency_path` prefix, and has
    /// no notion of line numbers. One entry therefore suppresses every
    /// occurrence in that file — the test import we mean, and a production
    /// import we would not. That is the N+66 ghost-filter shape: an entry
    /// promising to protect something the filter cannot see.
    ///
    /// So the claim gets a test instead of staying prose.
    ///
    /// Scope: this asserts that every forbidden import in the file is inside
    /// a `#[cfg(test)]` scope, which is what the rationale claims and is the
    /// only sound reading of it.
    ///
    /// Earlier this test cut the scan at the FIRST `#[cfg(test)]` marker and
    /// stopped there, on the argument that "everything before the first marker
    /// is unconditionally compiled into production". That argument was sound
    /// for the region it covered and silent about the rest, and the silence
    /// was load-bearing: measured 2026-10-01, planting a production
    /// `use crate::interface::mcp::security::InputValidator` at line 2174 of
    /// `workspace_session.rs` — after that marker, before `mod tests`, 1983
    /// lines of real production code — left this test green. The CR-06 gate
    /// does not catch it either, for the reason above: the allowlist entry
    /// suppresses the whole file.
    ///
    /// So a "cfg(test)-only" entry could be made a lie anywhere past the first
    /// marker and the only mechanism meant to catch it would agree. That is
    /// the N+66 ghost-filter shape again, one level down, in the test that
    /// exists to resolve it.
    ///
    /// The fix is to classify each import by whether it sits inside a
    /// `#[cfg(test)]` scope, rather than by whether it sits before the first
    /// marker: walk the file tracking brace depth, and treat the span from a
    /// `#[cfg(test)]` attribute to the item it applies to as test-only. An
    /// import outside every such span is a production import.
    #[test]
    fn cfg_test_only_entries_really_have_no_production_import() {
        /// (file_path, dependency_path) for every entry whose rationale says
        /// "cfg(test)-only". Keep in step with `exceptions()`.
        const GUARDED: &[(&str, &str)] = &[
            (
                "application/services/file_operations.rs",
                "infrastructure::parser",
            ),
            (
                "application/services/file_operations.rs",
                "infrastructure::verification::",
            ),
            (
                "application/services/file_operations.rs",
                "interface::mcp::security",
            ),
            (
                "application/workspace_session.rs",
                "interface::mcp::security",
            ),
        ];

        let cargo_manifest =
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let src_root = std::path::Path::new(&cargo_manifest).join("src");

        for (file_path, dep) in GUARDED {
            let path = src_root.join(file_path);
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

            assert!(
                source.lines().any(|l| l.trim() == "#[cfg(test)]"),
                "{file_path} declares a cfg(test)-only allowlist entry \
                 but has no `#[cfg(test)]` marker, so the rationale \
                 cannot mean anything"
            );

            let forbidden = format!("crate::{dep}");
            for (lineno, trimmed, in_test_scope) in classify_imports(&source, &forbidden) {
                assert!(
                    in_test_scope,
                    "production import of a cfg(test)-only dependency at \
                     {file_path}:{lineno} — `{trimmed}`. The CR-06 gate will \
                     NOT catch this: TemporaryException::matches ignores line \
                     numbers, so the allowlist entry suppresses it silently. \
                     Move the import into test-only code or put it behind a port.",
                );
            }
        }
    }
}
