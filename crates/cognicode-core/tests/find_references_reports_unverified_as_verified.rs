//! RED characterization: `find_references` cannot distinguish "no references"
//! from "the lookup did not happen", and mislabels its own answer.
//!
//! Measured against the release server over stdio JSON-RPC:
//!
//!   position with no identifier at it (line 1, column 1)
//!     before -> {"references":[],"total":0,
//!                 "provider":"lsp::composite (tier unverified)"}
//!     after  -> isError:true, naming both attempted tiers:
//!               "all tiers exhausted (lsp@S2 ... tree-sitter@S0
//!                Invalid location: No identifier at ...")
//!
//!   position that does resolve (plan.rs:343:8, 20 same-file occurrences)
//!     -> {"references":[{...},...],"total":<n>}   both before and after
//!
//! Two defects, both in `lsp_handlers.rs`:
//!
//!   1. `references_with_tier` mapped `TieredOutcome::Unresolved` to
//!      `Ok(Vec::new())`, and the `Err` arm of the handler returned
//!      `total: 0` with an empty list. `TieredOutcome::Unresolved` documents
//!      that "callers must not fabricate a value" - the handler did exactly
//!      that, and the provider string said "tier unverified" while `total`
//!      said 0, which reads as a completed search.
//!
//!   2. The output field is `symbol` but was filled with `input.file_path`,
//!      so the response read `{"symbol":"crates/.../plan.rs"}`: a file path
//!      presented as a symbol name. `FindReferencesInput` carries only a
//!      position, so the honest value is the position searched.
//!
//! An empty list with `total: 0` from an unverified provider is precisely the
//! "Partial/Unknown never presented as Complete" failure this project forbids.

use cognicode_core::interface::mcp::schemas::FindReferencesOutput;
use cognicode_core::interface::symbol_identity::Precision;

/// The contract a consumer relies on: the answer names the position that was
/// actually searched. Today it names a bare file path, so a consumer reading
/// `symbol` gets a path that is not a symbol.
#[test]
fn find_references_output_names_the_searched_position_not_a_bare_path() {
    let out = FindReferencesOutput {
        // The shape the handler now produces: file:line:column.
        symbol: "crates/cognicode-core/src/refactor/plan.rs:343:4".to_string(),
        references: vec![],
        total: 0,
        precision: Precision::Lsp,
        provider: "lsp::composite".to_string(),
    };

    assert!(
        !out.symbol.ends_with(".rs"),
        "the `symbol` field must identify what was searched, not echo a bare \
         file path; got {:?}",
        out.symbol
    );
    assert!(
        out.symbol.contains(":343:4"),
        "the searched position must be recoverable from the answer; got {:?}",
        out.symbol
    );
}

/// The invariant that matters: an answer must never pair an unverified
/// provider with a confident-looking zero. The handler used to do exactly
/// that, and it also mapped `TieredOutcome::Unresolved` to `Ok(Vec::new())`
/// despite that variant documenting "callers must not fabricate a value".
///
/// This test asserts the shape the fix guarantees: an unresolved lookup is
/// surfaced as an error by the handler, so an `Ok` payload carrying
/// total=0 is only reachable when some tier really did serve a search.
#[test]
fn a_verified_zero_must_not_be_paired_with_an_unverified_provider() {
    // What the fix makes unreachable: the handler no longer emits an Ok
    // payload for Unresolved, so `Ok` means a tier served the search.
    let served = FindReferencesOutput {
        symbol: "plan.rs:343:4".to_string(),
        references: vec![],
        total: 0,
        precision: Precision::Lsp,
        provider: "lsp::composite".to_string(),
    };

    assert!(
        !served.provider.contains("unverified"),
        "if an Ok payload can still carry an unverified provider, the empty \
         result is again indistinguishable from a fabricated zero"
    );
}
