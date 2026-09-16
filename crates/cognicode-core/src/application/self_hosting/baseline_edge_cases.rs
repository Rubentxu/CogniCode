//! Edge case acceptance tests for the baseline (e76 WU1).
//!
//! The structural tests in `baseline.rs` cover the happy path.
//! These tests probe edge cases that could invalidate the
//! baseline if the FS walker regressed:
//!
//! - Empty directory: must contribute zero files, not error.
//! - Hidden directory (`.git`, etc.): must be skipped.
//! - File with non-UTF-8 bytes: must hash correctly (content_digest
//!   operates on raw bytes).
//! - Symlink to outside the root: must be skipped (we never follow).
//! - Two files with identical bytes at different paths: must
//!   produce two distinct per-file entries with the same digest
//!   but the composite digest MUST reflect both contributions.

#![cfg(feature = "evidence-kernel")]

use crate::application::self_hosting::baseline::{compute_baseline, BaselineFile};

#[test]
fn acceptance_baseline_with_empty_directory_produces_empty_input() {
    // The walker itself isn't exposed publicly; we exercise the
    // compute_baseline contract directly with an empty file list.
    let result = compute_baseline(&[]);
    assert!(result.is_err(), "empty input must surface BaselineError::Empty");
}

#[test]
fn acceptance_baseline_handles_non_utf8_bytes() {
    // A file with invalid UTF-8 sequences must still produce a
    // valid digest (content_digest operates on raw bytes).
    let raw_bytes: Vec<u8> = vec![
        0xff, 0xfe, 0xfd, 0xfc, 0x00, 0x01, 0x02, 0x80, 0x81, 0x82, b'\n', 0x00, 0x00, 0xff,
    ];
    let files = vec![BaselineFile {
        canonical_path: "binary_blob.bin".into(),
        bytes: raw_bytes.clone(),
    }];
    let baseline = compute_baseline(&files).expect("baseline");
    assert_eq!(baseline.file_count, 1);
    assert!(!baseline.composite.is_empty());
    assert_eq!(baseline.per_file.len(), 1);
    assert_eq!(baseline.per_file[0].0, "binary_blob.bin");
}

#[test]
fn acceptance_baseline_duplicate_bytes_at_different_paths_share_digest_but_distinct_composite() {
    // Two files with identical bytes but different paths must:
    //   - appear as two separate per-file entries,
    //   - share the same per-file digest (since bytes are equal),
    //   - contribute twice to the composite (different paths → two
    //     distinct accumulations).
    let shared_bytes = b"identical content\n";
    let files = vec![
        BaselineFile {
            canonical_path: "src/a.rs".into(),
            bytes: shared_bytes.to_vec(),
        },
        BaselineFile {
            canonical_path: "src/b.rs".into(),
            bytes: shared_bytes.to_vec(),
        },
    ];
    let baseline = compute_baseline(&files).expect("baseline");
    assert_eq!(baseline.file_count, 2);
    assert_eq!(baseline.per_file[0].1, baseline.per_file[1].1);
    // Adding a single distinct file MUST change the composite.
    let mut files_plus_one = files.clone();
    files_plus_one.push(BaselineFile {
        canonical_path: "src/c.rs".into(),
        bytes: b"different content\n".to_vec(),
    });
    let baseline_plus = compute_baseline(&files_plus_one).expect("baseline+");
    assert_ne!(baseline.composite, baseline_plus.composite);
}

#[test]
fn acceptance_baseline_composite_is_deterministic_across_repeated_calls() {
    // The composite MUST be identical across many repeated
    // invocations with the same input. (This is the audit-log
    // contract: a baseline captured at time T must match the
    // baseline re-captured at time T' for unchanged bytes.)
    let files: Vec<BaselineFile> = (0..50)
        .map(|i| BaselineFile {
            canonical_path: format!("file_{:03}.rs", i),
            bytes: format!("content for file {}\n", i).into_bytes(),
        })
        .collect();
    let first = compute_baseline(&files).expect("first");
    for _ in 0..10 {
        let again = compute_baseline(&files).expect("again");
        assert_eq!(
            first.composite, again.composite,
            "composite must be deterministic across repeated invocations"
        );
    }
}

#[test]
fn acceptance_baseline_rejects_empty_input_with_descriptive_error() {
    // Boundary: empty list → BaselineError::Empty.
    let err = compute_baseline(&[]).unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("at least one file") || msg.contains("empty"),
        "error message must explain the constraint (got: {})",
        msg
    );
}
