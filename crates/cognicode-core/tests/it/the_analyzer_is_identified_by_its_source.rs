//! The analyzer is identified by the source it was built from, not by the
//! commit git points at.
//!
//! Two builds of different code can sit on the same commit —a working tree
//! with edits that nobody committed yet— and a snapshot written by one of
//! them must not be served by the other. The identity that tells them apart
//! is a digest of the analyzer's own inputs, computed by the build script
//! with the same module this test includes.

use cognicode_core::domain::value_objects::{AnalyzerBuild, SnapshotKey};

#[path = "../../analyzer_source.rs"]
mod analyzer_source;

use analyzer_source::{Entry, collect, digest};

fn file(path: &str, bytes: &[u8]) -> Entry {
    Entry {
        path: path.to_string(),
        bytes: Some(bytes.to_vec()),
    }
}

fn absent(path: &str) -> Entry {
    Entry {
        path: path.to_string(),
        bytes: None,
    }
}

#[test]
fn the_same_source_gives_the_same_digest() {
    let a = digest(vec![
        file("src/lib.rs", b"fn a() {}"),
        file("Cargo.toml", b"[package]"),
    ]);
    let b = digest(vec![
        file("Cargo.toml", b"[package]"),
        file("src/lib.rs", b"fn a() {}"),
    ]);

    assert_eq!(
        a, b,
        "the digest is a function of the files, not of the order they were read in"
    );
    assert_eq!(a.len(), 64, "the digest is a hex sha256: {a}");
    assert!(a.bytes().all(|b| b.is_ascii_hexdigit()), "hex only: {a}");
}

#[test]
fn one_byte_of_source_is_another_digest() {
    let a = digest(vec![file("src/lib.rs", b"fn a() {}")]);
    let b = digest(vec![file("src/lib.rs", b"fn b() {}")]);

    assert_ne!(a, b, "two different sources cannot share an identity");
}

#[test]
fn the_path_of_a_file_is_part_of_the_source() {
    let a = digest(vec![file("src/a.rs", b"fn a() {}")]);
    let b = digest(vec![file("src/b.rs", b"fn a() {}")]);

    assert_ne!(a, b, "the same bytes under another name are another tree");
}

#[test]
fn the_boundary_between_two_files_is_part_of_the_source() {
    let a = digest(vec![file("a", b"xy"), file("b", b"z")]);
    let b = digest(vec![file("a", b"x"), file("b", b"yz")]);

    assert_ne!(
        a, b,
        "moving a byte from one file to the next is another tree, so the \
         digest cannot be a plain concatenation"
    );
}

#[test]
fn a_missing_input_is_not_an_empty_one() {
    let missing = digest(vec![absent("../cognicode-graph-algos/src")]);
    let empty = digest(vec![file("../cognicode-graph-algos/src", b"")]);

    assert_ne!(
        missing, empty,
        "an input that is not there is a fact the identity records, not an \
         empty file"
    );
}

#[test]
fn two_builds_of_different_code_on_the_same_commit_are_two_producers() {
    let built = |source: &[u8]| AnalyzerBuild {
        version: "1.0.0".to_string(),
        commit: Some("abc1234".to_string()),
        features: vec!["persistence".to_string()],
        source_digest: digest(vec![file("src/lib.rs", source)]),
    };

    let before = SnapshotKey::new("/ws", built(b"fn a() {}"), "schema");
    let after = SnapshotKey::new("/ws", built(b"fn a() { edited() }"), "schema");

    assert!(
        !before.same_producer_as(&after),
        "the same commit with other code is another analyzer: a snapshot \
         written by one cannot be served by the other"
    );
}

/// The build script and this test share `collect`, so the comparison below
/// cannot see a `collect` that reads the wrong tree. This one can: the tree
/// that is hashed holds the analyzer and the path crates it is built with.
#[test]
fn the_hashed_tree_holds_the_analyzer_and_its_path_dependencies() {
    let tree = collect(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("the analyzer's inputs can be read");

    for expected in [
        "src/lib.rs",
        "product/tools.json",
        "../../Cargo.lock",
        "../cognicode-graph-algos/src/lib.rs",
        "../cognicode-macros/src/lib.rs",
    ] {
        assert!(
            tree.iter()
                .any(|entry| entry.path == expected && entry.bytes.is_some()),
            "{expected} is part of what the analyzer is built from, so it is \
             part of the digest"
        );
    }
    assert!(
        tree.iter().all(|entry| entry.bytes.is_some()),
        "in the workspace every input exists; an absent one leaves its source \
         out of the identity"
    );
}

#[test]
fn the_running_analyzer_declares_the_digest_of_its_own_source() {
    let tree = collect(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("the analyzer's inputs can be read");

    assert_eq!(
        AnalyzerBuild::current().source_digest,
        digest(tree),
        "the identity baked into the analyzer is the digest of the tree it \
         was built from; if this fails, the build script hashes something \
         other than what this test reads"
    );
}
