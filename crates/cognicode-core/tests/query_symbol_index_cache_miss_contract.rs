//! RED characterization: `query_symbol_index` returns `internal: timeout` for a
//! real repository, while succeeding on a one-file directory.
//!
//! Measured against the release server over stdio JSON-RPC, both with the
//! exact same required input (`symbol_name`):
//!
//!   directory=<tempdir with lib.rs>  -> {"locations":[{...}],"total":1}  0.0s
//!   directory=<this repo>            -> "internal: timeout"                 0.5s
//!   directory omitted                -> "internal: timeout"                 0.5s
//!
//! 0.5s is not a coincidence: it is the `search` category budget in
//! `timeout_for_category` (rmcp_adapter.rs:269). The handler's cache-miss
//! path calls `LightweightStrategy::build_index(&dir)` and discards its
//! result with `.ok()`. On this repo that build takes ~4.8s (measured
//! separately for `build_lightweight_index`), so the call is cancelled at
//! 500ms, the error is swallowed, and the server reports a bare
//! "internal: timeout" with no indication that a cache miss was involved.
//!
//! Two distinct defects, both measured:
//!
//!   1. The advertised schema has an optional `directory`, but omitting it
//!      makes the tool unusable rather than defaulting to the working dir.
//!   2. `.ok()` on a fallible build turns any failure - timeout included -
//!      into an empty index that is then cached, so the failure outlives the
//!      call that caused it.

use cognicode_core::infrastructure::graph::LightweightIndex;

#[test]
fn a_fresh_index_build_reports_failure_instead_of_an_empty_index() {
    // The core invariant the handler's `.ok()` breaks: a failed build must
    // not look identical to a successful build that found no symbols.
    let dir = std::env::temp_dir().join("cc_query_symbol_index_probe");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lib.rs"), "pub fn foo(){}\n").unwrap();

    let mut index = LightweightIndex::default();
    let result = index.build_index(&dir);

    // A real directory must build successfully. If this ever returns Err for
    // a readable directory, the sweep's "timeout" readings need re-reading.
    assert!(
        result.is_ok(),
        "build_index on a readable directory must succeed, got {:?}",
        result.err()
    );

    let found: usize = index.entries().map(|(_, locs)| locs.len()).sum();
    assert_eq!(
        found, 1,
        "the one-file directory must index exactly one symbol; an empty index \
         here is the same silent-failure shape as the handler's `.ok()`"
    );
}

#[test]
fn a_nonexistent_directory_is_reported_rather_than_swallowed() {
    // The contrast case. A missing directory cannot succeed, and that must be
    // visible as an Err so a caller can distinguish it from "no symbols".
    let mut index = LightweightIndex::default();
    let result = index.build_index(std::path::Path::new("/nonexistent/cc/probe"));
    assert!(
        result.is_err(),
        "a nonexistent directory must surface an error; if this is Ok, the \
         handler's `.ok()` is silently manufacturing an empty index"
    );
}
