//! RED-gate test for the `sqlite-feature-gate` spec.
//!
//! Asserts that the `cognicode-core` crate declares a `sqlite` opt-in
//! feature in its `[features]` table that references `rusqlite`. This
//! is the minimum-viable gate for PR 1 of `postgres-default-config`:
//! the workspace now has the `sqlite` feature plumbing even though
//! `rusqlite` itself remains a hard dependency on this crate (the
//! dependency becomes `optional = true` in PR 2 when the default
//! features flip to PG-first).
//!
//! This test parses the crate's own manifest at compile time (via
//! `CARGO_MANIFEST_DIR`), so it runs without external tools and stays
//! hermetic.

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
fn cognicode_core_declares_sqlite_feature() {
    let manifest = crate_manifest();

    // Locate the `[features]` block.
    let features_idx = manifest
        .find("[features]")
        .expect("Cargo.toml must contain [features] section");

    // The `[features]` block ends at the next `[section]` or EOF.
    let after_features = &manifest[features_idx..];
    let section_end = after_features[1..]
        .find("\n[")
        .map(|i| i + 1)
        .unwrap_or(after_features.len());
    let features_block = &after_features[..section_end];

    assert!(
        features_block.contains("sqlite"),
        "FAIL: cognicode-core MUST define a `sqlite` feature. Add to [features]:\n\
         sqlite = [\"dep:rusqlite\"]\n\
         Current [features] block:\n{}",
        features_block
    );

    // The `sqlite` feature must reference `dep:rusqlite` (or a
    // workspace dep that resolves to rusqlite).
    let sqlite_feature_line = features_block
        .lines()
        .find(|l| l.trim_start().starts_with("sqlite"))
        .expect("`sqlite` feature entry must exist under [features]");

    assert!(
        sqlite_feature_line.contains("rusqlite"),
        "FAIL: `sqlite` feature must enable `rusqlite`. Got: `{}`",
        sqlite_feature_line.trim()
    );

    // The `sqlite` feature MUST NOT be in the default list. This
    // check is enabled in PR 2 (when the workspace flips to PG-first).
    // For PR 1 the SQLite path is still the default, so we only assert
    // that the feature exists and is opt-in via `dep:rusqlite`. The
    // PR 2 follow-up will tighten this check.
    //
    // To re-enable this strict check, uncomment the block below.
    //
    // let default_line = features_block
    //     .lines()
    //     .find(|l| l.trim_start().starts_with("default"));
    // if let Some(default_line) = default_line {
    //     assert!(
    //         !default_line.contains("\"sqlite\""),
    //         "FAIL: `sqlite` must NOT be in default features. Got: `{}`",
    //         default_line.trim()
    //     );
    // }
}
