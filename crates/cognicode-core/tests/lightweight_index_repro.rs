//! Reproduce the `build_lightweight_index` SIGABRT outside the MCP server.
//!
//! Driving the tool over stdio aborts with glibc `corrupted size vs. prev_size`
//! on every call, and the release binary is stripped so gdb only yields bare
//! addresses. Calling `LightweightIndex::build_index` directly, in a test binary
//! that *does* carry debug info, either reproduces the abort with a resolvable
//! backtrace or proves the corruption is specific to the server process.
//!
//! `build_graph` parses the same tree without aborting, so the interesting
//! comparison is whether this path alone corrupts the heap.
//!
//! Ignored by default: it indexes the whole repository and takes minutes.
//! Run explicitly:
//!
//! ```text
//! cargo test -p cognicode-core --test lightweight_index_repro -- --ignored --nocapture
//! ```

use cognicode_core::infrastructure::graph::LightweightIndex;

/// Index the repository root and report what happened.
#[test]
#[ignore = "indexes the whole repository; run explicitly"]
fn lightweight_index_over_repo_root_does_not_corrupt_the_heap() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();

    eprintln!("indexing {}", root.display());
    let mut index = LightweightIndex::new();
    let result = index.build_index(&root);

    match result {
        Ok(()) => {
            let symbols = index.symbol_count();
            eprintln!("OK: {symbols} distinct symbols indexed");
            assert!(
                symbols > 0,
                "build_index reported success but indexed nothing, which would \
                 read as a clean index over zero files"
            );
        }
        Err(e) => panic!("build_index returned Err: {e}"),
    }
}

/// Same tree, but only the files the parser recognises as one language.
///
/// `build_lightweight_index` aborted on the full repo while every
/// single-language directory and every per-directory probe passed, so the
/// trigger involves the combination. This pins one language at scale: if Rust
/// alone over the whole `crates/` tree also aborts, the problem is volume of a
/// single grammar rather than grammar mixing.
#[test]
#[ignore = "indexes a large tree; run explicitly"]
fn lightweight_index_over_one_large_language_tree() {
    // CARGO_MANIFEST_DIR is `crates/cognicode-core`, so the workspace root is
    // two levels up and `crates/` is one level up from there.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("crates")
        .to_path_buf();

    eprintln!("indexing {}", root.display());
    let mut index = LightweightIndex::new();
    index
        .build_index(&root)
        .unwrap_or_else(|e| panic!("build_index returned Err: {e}"));

    eprintln!("OK: {} distinct symbols", index.symbol_count());
}
