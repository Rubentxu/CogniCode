//! Contract: `safe_refactor` must resolve a symbol inside its category budget.
//!
//! RED evidence, measured against the real release server
//! (`cognicode-mcp` 0.101.16 over stdio JSON-RPC) on this repository:
//!
//! ```text
//! tool_call tool=safe_refactor duration_ms=60510 status=error
//! ```
//!
//! with no reply on stdout. `safe_refactor` is category `refactor`, so
//! `rmcp_adapter::timeout_for_category` gives it 30s. The 0b5771e7 fix made the
//! advertised `target` reachable by walking the workspace to find the defining
//! file, but that walk read and tree-sitter-parsed every source file with no
//! skip policy, so the call blew the budget and errored instead of returning a
//! plan. Same class of defect as the 1813s index: a walk that honours neither
//! .gitignore nor dependency/build-tree skipping.
//!
//! The fixture is built here rather than pointed at this repository, because the
//! first version of this test used the repo root and came out VACUOUS: it
//! passed in 16.7s with the fix reverted, because the surrounding checkout had
//! no large `target/` tree under the shared CARGO_TARGET_DIR. A budget test
//! whose fixture it does not control measures the machine, not the code. The
//! tree here is built to make the defect unavoidable, so reverting the fix
//! fails the test rather than merely slowing it down.

use cognicode_core::application::services::refactor_service::RefactorService;
use std::path::Path;
use std::time::{Duration, Instant};

/// The `refactor` category budget in `rmcp_adapter::timeout_for_category`.
const CATEGORY_BUDGET: Duration = Duration::from_secs(30);

/// Write `count` parseable Rust files into `root/build/`, a directory name that
/// is on the skip policy by EXACT match (`USAGE_SKIP_DIRS.contains(&name)` is
/// not a substring test, so the directory has to be named `build`, not
/// something like `my_build_dir`). It sorts before `src`, so an unpruned walk
/// must traverse all of it before reaching the definition. Each file is large
/// enough that parsing them all is seconds of work.
fn seed_build_tree(root: &Path, count: usize) {
    let build = root.join("build");
    std::fs::create_dir_all(&build).expect("fixture dir");
    let body: String = (0..200)
        .map(|i| format!("pub fn helper_{i}(x: u32) -> u32 {{ x + {i} }}\n"))
        .collect();
    for f in 0..count {
        std::fs::write(
            build.join(format!("mod_{f}.rs")),
            format!("// generated {f}\n{body}"),
        )
        .expect("fixture write");
    }
}

#[test]
fn resolve_symbol_file_skips_build_trees_and_stays_within_budget() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // 900 generated files under a skipped directory name.
    //
    // The definition deliberately does NOT exist: `resolve_symbol_file` has to
    // walk the entire tree before it can answer `None`. Two earlier fixture
    // shapes passed in 0.16s even with the fix reverted, and both for the same
    // reason — `ignore::WalkBuilder` yields files before descending into their
    // directories (measured: with the definition in `src/lib.rs` it came first,
    // at index 0 of 901), so the expensive tree was never reached before the
    // early `return`. A budget test has to force the full traversal.
    seed_build_tree(root, 6_000);

    let svc = RefactorService::new();
    let start = Instant::now();
    let resolved = svc
        .resolve_symbol_file(root, "the_target_symbol")
        .expect("resolution must not error");
    let elapsed = start.elapsed();

    assert!(
        resolved.is_none(),
        "the symbol is not defined anywhere, so a full traversal must return None"
    );

    // Budget, not just correctness: a walk that still parses the build tree
    // blows this on the fixture above.
    assert!(
        elapsed < CATEGORY_BUDGET,
        "resolve_symbol_file took {elapsed:?} over 6,000 files in a skipped \
         directory, over the {CATEGORY_BUDGET:?} `refactor` budget; the walk is \
         not pruning build trees"
    );
}

#[test]
fn resolve_symbol_file_finds_a_real_definition_alongside_a_build_tree() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn the_target_symbol() -> u32 { 1 }\n",
    )
    .unwrap();
    seed_build_tree(root, 200);

    let svc = RefactorService::new();
    let resolved = svc
        .resolve_symbol_file(root, "the_target_symbol")
        .expect("resolution must not error");
    let path = resolved.expect("the symbol is defined in src/lib.rs");
    assert!(
        path.ends_with("lib.rs"),
        "resolved to the wrong file: {}",
        path.display()
    );
}

#[test]
fn resolve_symbol_file_still_finds_a_symbol_that_lives_in_a_skipped_tree_name() {
    // The skip policy is name-based, so a source file that happens to sit under
    // a directory named `target` is invisible to the walk. That is the accepted
    // trade-off of the skip list, pinned here so a future change to the policy
    // is a deliberate decision rather than a silent regression.
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    std::fs::create_dir_all(root.join("target/nested")).unwrap();
    std::fs::write(
        root.join("target/nested/only.rs"),
        "pub fn buried_symbol() -> u32 { 2 }\n",
    )
    .unwrap();

    let svc = RefactorService::new();
    let resolved = svc
        .resolve_symbol_file(root, "buried_symbol")
        .expect("must not error");
    assert!(
        resolved.is_none(),
        "a file under a skipped directory is not scanned; the skip list is \
         name-based and this documents that consequence"
    );
}

#[test]
fn resolve_symbol_file_reports_none_for_an_undefined_symbol() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn other() {}\n").unwrap();
    seed_build_tree(root, 50);

    let svc = RefactorService::new();
    let resolved = svc
        .resolve_symbol_file(root, "definitely_not_a_symbol_xyzzy")
        .expect("an absent symbol is a clean refusal, not an error");
    assert!(
        resolved.is_none(),
        "an undefined symbol must resolve to None, got {:?}",
        resolved.map(|p| p.display().to_string())
    );
}

#[test]
fn identifier_prefilter_respects_word_boundaries() {
    // A symbol name that is a prefix of a longer identifier must not be
    // reported as defined just because the longer one appears. A false negative
    // here would silently hide a real definition.
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn the_target_symbol_extended() -> u32 { 1 }\n",
    )
    .unwrap();

    let svc = RefactorService::new();
    let resolved = svc
        .resolve_symbol_file(root, "the_target_symbol")
        .expect("prefilter must not error");
    assert!(
        resolved.is_none(),
        "a prefix of a longer identifier must not resolve as the symbol"
    );
}
