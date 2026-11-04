//! H0.3 (delivery-first-radical, package integration) — workspace identity.
//!
//! Claim: `AnalysisBasis::for_workspace` is the single source of truth
//! for path normalization. Two paths that resolve to the same
//! directory MUST produce `AnalysisBasis` objects with the same
//! `workspace_root()`. Downstream code (`graph_full` with
//! `rebuild=false`, `PerFileGraphCache`, MCP dispatch) assumes this
//! property: if it fails, the same workspace analyzed twice via
//! different path spellings gives two different cache slots, and
//! `rebuild=false` could legitimately serve a graph built for a
//! sibling workspace.
//!
//! H0.5 acceptance ("otro workspace ⇒ error/identidad distinta")
//! depends on this property holding for the SAME workspace with
//! different path shapes. The tests below cover the four shape
//! equivalences that the normalization pipeline in
//! `analysis_basis.rs` claims to support:
//!
//!   1. The same canonical path twice        → same basis
//!   2. A relative path (".", "")              → absolute workspace_root
//!   3. An existing path                       → canonical (realpath)
//!   4. A syntactically different path ("..")  → same workspace_root

use cognicode_core::application::analysis_basis::AnalysisBasis;
use std::path::PathBuf;

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "h03-workspace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn for_workspace_is_deterministic_for_the_same_canonical_path() {
    let dir = tempdir();
    let canonical = dir.canonicalize().expect("tempdir canonicalize");

    let a = AnalysisBasis::for_workspace(canonical.clone());
    let b = AnalysisBasis::for_workspace(canonical);

    assert_eq!(
        a.workspace_root(),
        b.workspace_root(),
        "for_workspace must be deterministic for the same input"
    );
}

#[test]
fn for_workspace_returns_absolute_workspace_root_for_a_relative_path() {
    // Relative inputs (".", "", "./foo") must be absolutized against
    // the cwd; the workspace_root is then an absolute PathBuf. If
    // the basis carried a relative root, the cache and the
    // per_file_graph would key on cwd-relative paths and behave
    // non-deterministically across shells.
    let basis = AnalysisBasis::for_workspace(PathBuf::from("."));

    assert!(
        basis.workspace_root().is_absolute(),
        "for_workspace(\".\") must return an absolute workspace_root (got {:?})",
        basis.workspace_root()
    );
}

#[test]
fn for_workspace_canonicalizes_an_existing_path() {
    // Step 3 of the pipeline: when the path exists, canonicalize
    // resolves symlinks. This is what makes
    // `for_workspace(symlink) == for_workspace(realpath)`.
    let dir = tempdir();
    let basis = AnalysisBasis::for_workspace(dir.clone());

    let expected = dir.canonicalize().expect("tempdir canonicalize");
    assert_eq!(
        basis.workspace_root(),
        &expected,
        "for_workspace must canonicalize an existing path (got {:?}, expected {:?})",
        basis.workspace_root(),
        expected
    );
}

#[test]
fn for_workspace_via_dotdot_resolves_to_the_same_basis_as_the_canonical_path() {
    // The core H0.3 property: syntactic . / .. must NOT produce a
    // different workspace_root. Build `<parent>/<marker>/../<basename>`,
    // which is the same directory as `canonical` after lexical
    // normalization. Both basis objects must end up with the same
    // workspace_root.
    let dir = tempdir();
    let canonical = dir.canonicalize().expect("tempdir canonicalize");
    let via_dotdot = canonical
        .parent()
        .expect("tempdir has parent")
        .join("__h03_marker__")
        .join("..")
        .join(canonical.file_name().expect("dir has filename"));

    let a = AnalysisBasis::for_workspace(canonical.clone());
    let b = AnalysisBasis::for_workspace(via_dotdot);

    assert_eq!(
        a.workspace_root(),
        b.workspace_root(),
        "for_workspace must normalize . / .. syntactically (got {:?} vs {:?})",
        a.workspace_root(),
        b.workspace_root()
    );
}
