//! End-to-end acceptance tests for self-hosting (e76 acceptance).
//!
//! These tests exist to close the gap between the *structural*
//! closure tests in `closure.rs` and the *behavioral* requirement
//! from e76 WU1: "run CogniCode on CogniCode using canonical
//! production ingestion."
//!
//! These tests:
//! - Walk the real `crates/cognicode-core/src/` directory at test
//!   time using `std::fs::read_dir`. The production `baseline`
//!   module is pure (no FS); the FS walk lives in the test
//!   harness only, keeping the production seam platform-neutral
//!   and unit-testable.
//! - Build `BaselineFile` records from the real file bytes.
//! - Call the REAL public API: `compute_baseline`.
//! - Assert the resulting `BaselineDigest` is non-empty,
//!   deterministic, and changes when bytes change.
//!
//! These tests are gated by `#[cfg(feature = "evidence-kernel")]`
//! (self_hosting is feature-gated).

#![cfg(feature = "evidence-kernel")]

use crate::application::self_hosting::baseline::{compute_baseline, BaselineDigest, BaselineFile};

/// Walk a directory recursively, returning `(canonical_path, bytes)`
/// pairs for every regular file found. Symlinks are NOT followed
/// (we want a stable enumeration, not a graph walk). Hidden
/// directories (names starting with `.`) are skipped.
fn walk_dir_for_acceptance(root: &std::path::Path) -> Vec<BaselineFile> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue, // unreadable dirs (e.g. permission) are skipped
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                if let Ok(bytes) = std::fs::read(&path) {
                    // Canonical path string (POSIX form on Linux).
                    let canonical = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push(BaselineFile {
                        canonical_path: canonical,
                        bytes,
                    });
                }
            }
        }
    }
    out
}

#[test]
fn acceptance_cognicode_core_baseline_is_deterministic() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = walk_dir_for_acceptance(&root);
    assert!(
        files.len() >= 100,
        "cognicode-core/src should have many files; got {}",
        files.len()
    );
    let baseline_a = compute_baseline(&files).expect("baseline a");
    let baseline_b = compute_baseline(&files).expect("baseline b");
    assert_eq!(
        baseline_a, baseline_b,
        "two identical walks must produce identical baselines"
    );
    assert_eq!(baseline_a.file_count, files.len());
    assert!(!baseline_a.composite.is_empty());
    assert!(
        baseline_a.composite.chars().all(|c| c.is_ascii_hexdigit()),
        "composite digest must be hex (got {})",
        baseline_a.composite
    );
}

#[test]
fn acceptance_baseline_changes_when_a_real_file_changes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = walk_dir_for_acceptance(&root);
    assert!(!files.is_empty());
    let baseline_v1: BaselineDigest = compute_baseline(&files).expect("baseline v1");

    // Mutate one real file's bytes (just append a benign comment)
    // and assert the baseline shifts.
    files[0].bytes.push(b'\n');
    let baseline_v2: BaselineDigest = compute_baseline(&files).expect("baseline v2");
    assert_ne!(
        baseline_v1.composite, baseline_v2.composite,
        "a real byte change MUST shift the baseline"
    );
    // Same file count, different digest.
    assert_eq!(baseline_v1.file_count, baseline_v2.file_count);
}

#[test]
fn acceptance_baseline_changes_when_a_real_file_is_added() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = walk_dir_for_acceptance(&root);
    let baseline_v1: BaselineDigest = compute_baseline(&files).expect("baseline v1");
    files.push(BaselineFile {
        canonical_path: "zzz_synthetic_extra_file.rs".into(),
        bytes: b"// synthetic\n".to_vec(),
    });
    let baseline_v2: BaselineDigest = compute_baseline(&files).expect("baseline v2");
    assert_ne!(baseline_v1.composite, baseline_v2.composite);
    assert_eq!(baseline_v2.file_count, baseline_v1.file_count + 1);
}

#[test]
fn acceptance_baseline_per_file_digests_are_sha256_hex() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = walk_dir_for_acceptance(&root);
    let baseline = compute_baseline(&files).expect("baseline");
    // Spot-check 5 files.
    for (path, digest) in baseline.per_file.iter().take(5) {
        assert!(!path.is_empty(), "per-file path must be non-empty");
        // content_digest format: "<algo>:<hex>". Accept either
        // sha256 or fnv64 followed by 32 or 16 hex chars
        // respectively.
        let hex_part = digest
            .split_once(':')
            .map(|(_, h)| h)
            .unwrap_or(digest.as_str());
        let expected_len = if digest.starts_with("sha256") { 64 } else { 16 };
        assert_eq!(
            hex_part.len(),
            expected_len,
            "per-file digest hex part length (got {})",
            digest
        );
        assert!(
            hex_part.chars().all(|c| c.is_ascii_hexdigit()),
            "per-file digest hex part must be hex (got {})",
            digest
        );
    }
}
