//! RED-gate test for the `sqlite-feature-gate` spec, applied to
//! `cognicode-db`.
//!
//! Asserts that the crate declares `rusqlite` as **optional** and
//! exposes a `sqlite` opt-in feature. Mirrors the gate in
//! `cognicode-core/tests/sqlite_feature_gate.rs`.

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
fn rusqlite_is_optional_in_cognicode_db() {
    let manifest = crate_manifest();

    let deps_block_start = manifest
        .find("[dependencies]")
        .expect("Cargo.toml must contain [dependencies] section");
    let after_deps = &manifest[deps_block_start..];

    let rusqlite_idx = after_deps.find("rusqlite").unwrap_or_else(|| {
        panic!(
            "cognicode-db Cargo.toml must reference rusqlite — gate it behind `sqlite` feature. \
             Manifest:\n{}",
            manifest
        )
    });

    let after_rusqlite = &after_deps[rusqlite_idx..];
    let value_start = after_rusqlite
        .find('=')
        .expect("rusqlite entry must have `=`")
        + 1;
    let value_slice = &after_rusqlite[value_start..];

    // Scope to the single line of the inline-table value.
    let line_end = value_slice.find('\n').unwrap_or(value_slice.len());
    let rusqlite_value_line = &value_slice[..line_end];

    assert!(
        rusqlite_value_line.contains("optional = true"),
        "FAIL: rusqlite MUST be declared `optional = true` in cognicode-db. Manifest line:\n{}",
        rusqlite_value_line
    );
}

#[test]
fn cognicode_db_declares_sqlite_feature() {
    let manifest = crate_manifest();

    let features_idx = manifest
        .find("[features]")
        .unwrap_or_else(|| {
            panic!(
                "FAIL: cognicode-db MUST contain a [features] section declaring `sqlite`. \
                 Manifest:\n{}",
                manifest
            )
        });

    let after_features = &manifest[features_idx..];
    let section_end = after_features[1..]
        .find("\n[")
        .map(|i| i + 1)
        .unwrap_or(after_features.len());
    let features_block = &after_features[..section_end];

    let sqlite_line = features_block
        .lines()
        .find(|l| l.trim_start().starts_with("sqlite"))
        .unwrap_or_else(|| {
            panic!(
                "FAIL: cognicode-db MUST define a `sqlite` feature. Add to [features]:\n\
                 sqlite = [\"dep:rusqlite\"]\n\
                 Current [features] block:\n{}",
                features_block
            )
        });

    assert!(
        sqlite_line.contains("rusqlite"),
        "FAIL: `sqlite` feature must enable `rusqlite`. Got: `{}`",
        sqlite_line.trim()
    );
}
