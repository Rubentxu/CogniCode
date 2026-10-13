//! RED regression: the gitignore-blind walk that made three advertised tools
//! answer in over 100 seconds.
//!
//! Found by exercising all 77 advertised tools against the release server.
//! `find_usages`, `nl_to_symbol` and `smart_search` each failed to answer
//! within 100s for a single valid symbol; the same call against a one-file
//! directory answered in 0.0s.
//!
//! It was not a hang and not a dead server: while the slow call was still
//! running, a second unrelated call answered normally. It was slow work, and
//! the cause was the same defect already fixed in
//! `LightweightIndex::build_index`: `walkdir::WalkDir` prunes by directory
//! name alone (`USAGE_SKIP_DIRS`, `WalkFilter`) and never reads `.gitignore`,
//! so it descended into this repo's ignored `sandbox/` tree - 311,779 files,
//! 176,200 of them under `sandbox/repos` - and re-parsed every one with
//! tree-sitter on every call.
//!
//! Measured after the fix, against the rebuilt release server:
//!   find_usages  100s+ -> 4.3s
//!   nl_to_symbol 100s+ -> 17.8s
//!   smart_search 100s+ -> 18.3s

use cognicode_core::infrastructure::graph::LightweightIndex;

/// The walk these three tools share must not descend into an ignored
/// directory, and must still find the tracked sources beside it.
///
/// This uses `LightweightIndex` rather than reaching into the three private
/// search helpers: the failing behaviour is the walk policy, and the index is
/// the public surface where that policy is already pinned. Reaching past it
/// into private helpers would widen production visibility for an assertion.
#[test]
fn the_shared_walk_ignores_gitignored_trees() {
    let root = std::env::temp_dir().join("cc_walk_ignore_probe");
    let _ = std::fs::remove_dir_all(&root);

    // A tracked source, and an ignored tree that would dominate the cost.
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("generated/junk")).unwrap();
    std::fs::write(root.join(".gitignore"), "generated/\n").unwrap();
    // `ignore::WalkBuilder` reads `.gitignore` as git does, and git ignore
    // rules only apply inside a repository. The builder sets
    // `.parents(false)`, so without a `.git` here the rule would never load
    // and the test would assert nothing. The temporary root is disposable, so
    // this only initialises it empty.
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn tracked_symbol() {}\n").unwrap();
    for i in 0..200 {
        std::fs::write(
            root.join(format!("generated/junk/f{i}.rs")),
            "pub fn ignored_symbol() {}\n",
        )
        .unwrap();
    }

    let mut index = LightweightIndex::default();
    index.build_index(&root).expect("build must succeed");

    let names: Vec<String> = index.entries().map(|(name, _)| name.clone()).collect();

    assert!(
        names.iter().any(|n| n == "tracked_symbol"),
        "the tracked source must still be indexed; got {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "ignored_symbol"),
        "an ignored tree must not be indexed; it contributed 200 files of cost \
         and zero results. If this fails, the walk is blind to .gitignore \
         again, which is what made find_usages/nl_to_symbol/smart_search \
         exceed 100s on this repository."
    );
}

/// The gitignore policy must not turn a gitignore'd repository into an empty
/// one. This is the failure mode that matters most: pruning too aggressively
/// would have looked like a fast fix while silently returning no results.
#[test]
fn pruning_still_indexes_everything_that_is_not_ignored() {
    let root = std::env::temp_dir().join("cc_walk_ignore_probe_broad");
    let _ = std::fs::remove_dir_all(&root);

    std::fs::create_dir_all(root.join("a/b/c")).unwrap();
    std::fs::write(root.join(".gitignore"), "build/\n").unwrap();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("a/b/c/deep.rs"), "pub fn deep_symbol() {}\n").unwrap();

    let mut index = LightweightIndex::default();
    index.build_index(&root).expect("build must succeed");

    let names: Vec<String> = index.entries().map(|(name, _)| name.clone()).collect();

    assert!(
        names.iter().any(|n| n == "deep_symbol"),
        "nested non-ignored sources must still be reached; pruning must not \
         be so aggressive that it reports an empty index as success"
    );
}
