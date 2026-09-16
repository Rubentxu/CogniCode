//! Self-hosting baseline (e76 WU1).
//!
//! "CogniCode evaluating CogniCode" starts with a deterministic baseline
//! over the project's own source. The baseline must:
//!
//! 1. **Composed**: re-use existing canonical ingestion (tree-sitter
//!    extraction + content digests) — no parallel digest scheme.
//! 2. **Deterministic**: identical bytes in → identical digest out,
//!    regardless of host OS, file enumeration order, or mtime.
//! 3. **Pure**: no async, no I/O on the FS beyond the caller-supplied
//!    path enumeration. The baseline is a value function over a list
//!    of `(canonical_path, bytes)` pairs.
//!
//! This module does NOT crawl the FS. The caller (e76 WU2 UAT, or a
//! future `cognicode self-host` CLI) enumerates files using the
//! canonical workspace model (`WorkspaceId`, `SnapshotId`) and hands
//! the bytes in. Keeping the baseline pure makes it trivially
//! testable and platform-equivalent (Linux/macOS/Windows give the
//! same digest for the same bytes, modulo CRLF normalisation which
//! the underlying `content_digest` already handles).
//!
//! Design constraint: this module MUST be platform-neutral. A static
//! test enforces it (see `baseline_module_does_not_leak_platform_specific_identifiers`).

use crate::application::portable_execution::content_digest;

/// A single file's contribution to the baseline.
///
/// The path is canonicalised by the caller (see
/// `application::portable_execution::CanonicalPath`); bytes are the
/// raw file contents as ingested by the canonical production
/// ingestion seam (`ingest_rust_facts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineFile {
    /// Canonical path string (POSIX-normalised; case-sensitive).
    pub canonical_path: String,
    /// Raw file bytes (CRLF normalisation happens inside
    /// `content_digest`).
    pub bytes: Vec<u8>,
}

/// The deterministic baseline digest over the whole project.
///
/// Combines per-file SHA-256 digests into a single baseline digest
/// using FNV-1a 64-bit accumulation in canonical path order. The
/// accumulation is content-only (path strings do not feed the
/// accumulation), so re-locating a file does not change the baseline
/// — only the bytes do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineDigest {
    /// FNV-1a 64-bit hash of the concatenation of per-file SHA-256
    /// digests, in canonical path order. Hex-encoded lowercase.
    pub composite: String,
    /// Number of files that contributed.
    pub file_count: usize,
    /// SHA-256 digests per file (in canonical path order). Useful
    /// for diffing: when the baseline shifts, the file-level diff
    /// shows which files moved.
    pub per_file: Vec<(String, String)>,
}

/// Errors raised by the baseline computation. Currently unreachable
/// in practice (`content_digest` is total) but kept for future
/// extension (e.g. file size limits).
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    /// Input file list was empty; baselines require at least one file.
    #[error("baseline requires at least one file; got an empty list")]
    Empty,
}

/// Compute the deterministic baseline over the given files.
///
/// Files are sorted canonically by `canonical_path` before hashing,
/// so the digest is invariant to enumeration order.
///
/// Algorithm:
/// 1. Reject empty input (a baseline of zero files would not be a
///    meaningful reference).
/// 2. Sort by canonical path (lexicographic, byte-wise).
/// 3. For each file in order:
///    a. SHA-256 digest the bytes via `content_digest`.
///    b. Record `(path, sha256)`.
///    c. Accumulate into a running FNV-1a 64-bit hash over the SHA-256 hex string.
/// 4. Hex-encode the final FNV-1a state as `composite`.
pub fn compute_baseline(files: &[BaselineFile]) -> Result<BaselineDigest, BaselineError> {
    if files.is_empty() {
        return Err(BaselineError::Empty);
    }
    let mut sorted: Vec<&BaselineFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.canonical_path.cmp(&b.canonical_path));

    let mut per_file: Vec<(String, String)> = Vec::with_capacity(sorted.len());
    let mut state: u64 = 0xcbf29ce484222325; // FNV-1a 64-bit offset basis.
    for f in &sorted {
        let digest = content_digest(&f.bytes);
        let hex = digest.as_str().to_string();
        for byte in hex.as_bytes() {
            state ^= *byte as u64;
            state = state.wrapping_mul(0x100000001b3); // FNV-1a 64-bit prime.
        }
        per_file.push((f.canonical_path.clone(), hex));
    }
    Ok(BaselineDigest {
        composite: format!("{:016x}", state),
        file_count: sorted.len(),
        per_file,
    })
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, bytes: &[u8]) -> BaselineFile {
        BaselineFile {
            canonical_path: path.into(),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn baseline_is_deterministic_for_identical_input() {
        let files = vec![
            file("src/lib.rs", b"pub fn x() {}"),
            file("src/main.rs", b"fn main() {}"),
            file("README.md", b"# Title\n"),
        ];
        let a = compute_baseline(&files).expect("baseline");
        let b = compute_baseline(&files).expect("baseline");
        assert_eq!(a, b);
    }

    #[test]
    fn baseline_is_invariant_to_input_order() {
        let f1 = file("src/a.rs", b"a");
        let f2 = file("src/b.rs", b"b");
        let f3 = file("src/c.rs", b"c");
        let baseline_fwd = compute_baseline(&[f1.clone(), f2.clone(), f3.clone()]).unwrap();
        let baseline_rev = compute_baseline(&[f3, f2, f1]).unwrap();
        // The composite is identical because the algorithm sorts.
        assert_eq!(baseline_fwd.composite, baseline_rev.composite);
        // The per-file ordering is also canonical (sorted).
        assert_eq!(baseline_fwd.per_file, baseline_rev.per_file);
    }

    #[test]
    fn baseline_changes_when_any_file_changes() {
        let baseline_v1 = compute_baseline(&[file("a.rs", b"v1")]).unwrap();
        let baseline_v2 = compute_baseline(&[file("a.rs", b"v2")]).unwrap();
        assert_ne!(
            baseline_v1.composite, baseline_v2.composite,
            "byte change must shift baseline"
        );
    }

    #[test]
    fn baseline_changes_when_a_file_is_added() {
        let one = compute_baseline(&[file("a.rs", b"a")]).unwrap();
        let two = compute_baseline(&[file("a.rs", b"a"), file("b.rs", b"b")]).unwrap();
        assert_ne!(one.composite, two.composite);
        assert_eq!(one.file_count, 1);
        assert_eq!(two.file_count, 2);
    }

    #[test]
    fn baseline_rejects_empty_input() {
        let err = compute_baseline(&[]).unwrap_err();
        assert!(matches!(err, BaselineError::Empty));
    }

    #[test]
    fn baseline_normalises_crlf_via_content_digest() {
        // `content_digest` (the underlying primitive) normalises
        // CRLF to LF. Two files that differ only in line endings
        // MUST produce the same baseline.
        let lf = compute_baseline(&[file("a.rs", b"line1\nline2\n")]).unwrap();
        let crlf = compute_baseline(&[file("a.rs", b"line1\r\nline2\r\n")]).unwrap();
        assert_eq!(lf.composite, crlf.composite);
    }

    #[test]
    fn baseline_module_does_not_leak_platform_specific_identifiers() {
        // The module is pure and platform-neutral. The seam-level
        // invariant: no platform-specific runtime identifier leaks
        // into the seam. We strip doc comments and the test
        // module itself (it necessarily enumerates the forbidden
        // identifiers as data for the check).
        let full = include_str!("baseline.rs");
        let stripped = crate::application::portable_execution::strip_doc_comments_and_tests(full);
        let lower = stripped.to_lowercase();
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !lower.contains(forbidden),
                "self_hosting::baseline code (non-test, non-doc) must not leak platform-specific symbol: {forbidden}"
            );
        }
    }
}
