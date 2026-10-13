//! Regression test: `build_lightweight_index` must not walk VCS-ignored trees.
//!
//! Measured on the release MCP server: indexing this repository took 1813s
//! and returned no result, while `crates/` alone (53,300 symbols) took 2.9s.
//! `sandbox/repos` alone accounted for 796s and produced no symbols at all,
//! and it holds 176,200 files ignored at `.gitignore:87`.
//!
//! `LightweightIndex::build_index` pruned only a fixed list of conventional
//! directory names from `WalkFilter` (`target`, `node_modules`, `.sandbox`,
//! ...). That list cannot know what a project keeps out of its own index, so
//! a `WalkDir` behind a name filter descended into every ignored tree.
//!
//! A user pointing this advertised tool at any repository with large ignored
//! trees (vendored checkouts, caches, build outputs) waited minutes for an
//! answer that never came. The fix is to honour the ignore rules the walk
//! was already conceptually skipping, not to raise the timeout.
//!
//! The assertions read the files the index actually recorded, via the
//! public `entries()` surface, so they test observable behaviour rather
//! than the walker's internal configuration.

use cognicode_core::infrastructure::graph::LightweightIndex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .expect("crate dir must have two ancestors")
}

/// Ask git itself, so this cannot drift from real ignore behaviour.
fn git_ignored_files(root: &Path) -> HashSet<String> {
    let out = std::process::Command::new("git")
        .args(["ls-files", "--others", "--ignored", "--exclude-standard"])
        .current_dir(root)
        .output()
        .expect("git must be available");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.trim().replace('\\', "/"))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Every distinct file path the index recorded, relative to the repo root.
fn indexed_files(index: &LightweightIndex, root: &Path) -> HashSet<String> {
    index
        .entries()
        .flat_map(|(_, locations)| locations.iter())
        .filter_map(|loc| {
            Path::new(loc.file())
                .strip_prefix(root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect()
}

#[test]
fn gitignored_files_are_not_indexed() {
    let root = repo_root();
    assert!(
        root.join(".gitignore").exists(),
        "not a repo root: {root:?}"
    );

    let ignored = git_ignored_files(&root);
    assert!(
        ignored.len() > 1000,
        "expected many ignored files to test against, got {}",
        ignored.len()
    );
    // `sandbox/repos` is the specific tree that made the tool unusable.
    assert!(
        ignored.iter().any(|f| f.starts_with("sandbox/repos/")),
        "this repository is expected to ignore sandbox/repos/"
    );

    let mut index = LightweightIndex::new();
    index.build_index(&root).expect("index build must succeed");

    let indexed = indexed_files(&index, &root);
    let offenders: Vec<&String> = indexed.intersection(&ignored).collect();

    assert!(
        offenders.is_empty(),
        "build_lightweight_index indexed {} git-ignored file(s); the walk \
         must honour .gitignore. First offenders: {:?}",
        offenders.len(),
        &offenders[..offenders.len().min(10)]
    );
}

#[test]
fn tracked_sources_are_still_indexed() {
    // The guard above must not be satisfied by indexing nothing at all.
    let root = repo_root();
    let mut index = LightweightIndex::new();
    index.build_index(&root).expect("index build must succeed");

    let indexed = indexed_files(&index, &root);
    assert!(
        indexed.len() > 100,
        "expected a real index, got {} files",
        indexed.len()
    );
    assert!(
        indexed.iter().any(|p| p.ends_with("lib.rs")),
        "the index must still contain tracked Rust sources; \
         indexed {} files, none of them lib.rs",
        indexed.len()
    );
}
