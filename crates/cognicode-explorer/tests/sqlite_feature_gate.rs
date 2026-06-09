//! RED-gate test for the `sqlite-feature-gate` spec, applied to
//! `cognicode-explorer`. Mirrors the gate in
//! `cognicode-core/tests/sqlite_feature_gate.rs` and
//! `cognicode-db/tests/sqlite_feature_gate.rs`.
//!
//! This test asserts the `sqlite` feature is declared on
//! `cognicode-explorer` AND that the crate forwards it to both
//! `cognicode-core` and `cognicode-db` (Cargo `crate/feature` syntax).

use std::fs;
use std::path::PathBuf;

fn crate_manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn crate_manifest() -> String {
    fs::read_to_string(crate_manifest_dir().join("Cargo.toml"))
        .expect("crate Cargo.toml must be readable from CARGO_MANIFEST_DIR")
}

#[test]
fn cognicode_explorer_declares_and_forwards_sqlite_feature() {
    let manifest = crate_manifest();

    let features_idx = manifest
        .find("[features]")
        .expect("cognicode-explorer Cargo.toml must contain [features] section");

    let after_features = &manifest[features_idx..];
    let section_end = after_features[1..]
        .find("\n[")
        .map(|i| i + 1)
        .unwrap_or(after_features.len());
    let features_block = &after_features[..section_end];

    // 1. The `sqlite` feature must exist.
    let sqlite_line = features_block
        .lines()
        .find(|l| l.trim_start().starts_with("sqlite"))
        .unwrap_or_else(|| {
            panic!(
                "FAIL: cognicode-explorer MUST define a `sqlite` feature forwarding to \
                 cognicode-core and cognicode-db. Add to [features]:\n\
                 sqlite = [\"cognicode-core/sqlite\", \"cognicode-db/sqlite\"]\n\
                 Current [features] block:\n{}",
                features_block
            )
        });

    // 2. It must forward to BOTH `cognicode-core/sqlite` and
    //    `cognicode-db/sqlite`.
    assert!(
        sqlite_line.contains("cognicode-core/sqlite"),
        "FAIL: `sqlite` feature must forward to `cognicode-core/sqlite`. Got: `{}`",
        sqlite_line.trim()
    );
    assert!(
        sqlite_line.contains("cognicode-db/sqlite"),
        "FAIL: `sqlite` feature must forward to `cognicode-db/sqlite`. Got: `{}`",
        sqlite_line.trim()
    );
}
