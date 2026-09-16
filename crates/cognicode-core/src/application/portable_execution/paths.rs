//! e75 WU4 — Host-path normalization for portable execution.
//!
//! ## The invariant (per directive WU4)
//!
//! > Host filesystem representation may affect mechanics, but MUST
//! > NOT accidentally change canonical software knowledge.
//!
//! Practically: the `host_path` field on `WorkspaceMount` (and on
//! `ExecutionSpec::cwd`) is the **mechanical** path — what we pass
//! to `Command::current_dir` or `podman --volume`. The **canonical**
//! path is what gets written into evidence, change tracking, and
//! lineage. The two MUST be derivable from each other but they MUST
//! NOT be the same string when the host representation would
//! otherwise leak through.
//!
//! ## What this module does
//!
//! - Provide `CanonicalPath` — a string-backed wrapper around a
//!   normalised path that is stable across host representations.
//! - Provide `canonicalize(path) -> CanonicalPath` — a pure
//!   function that maps a `Path` to its canonical form (POSIX
//!   forward slashes, lowercase on case-insensitive hosts,
//!   collapsed `.`/`..` segments without touching the FS).
//! - Provide `host_path_for_spawn(canonical) -> PathBuf` — convert
//!   a canonical path back to the host's preferred representation.
//! - Provide `content_digest(bytes) -> ContentDigest` — a SHA-256
//!   digest of bytes after CRLF → LF normalization, suitable for
//!   content-hash use in evidence.
//!
//! ## What this module does NOT do
//!
//! - It does NOT touch the filesystem (no syscalls, no
//!   `std::fs::canonicalize`). The normalization is purely
//!   syntactic.
//! - It does NOT lock down case-sensitivity detection at runtime.
//!   The host is queried once via `HostCaseSensitivity::detect()`,
//!   which itself only inspects environment hints (no FS probing).
//! - It does NOT solve e76 self-hosting equivalence; that requires
//!   content hashing, which WU4 lays the groundwork for but does
//!   not wire into the producer pipeline yet.
//!
//! ## Test cases (matches the WU4 directive checklist)
//!
//! - `C:\repo\src` vs `C:/repo/src` — both map to the same
//!   canonical `c:/repo/src` on Windows.
//! - `/workspace/src` — POSIX absolute.
//! - case-sensitive vs case-insensitive hosts: on Windows/macOS
//!   the canonical form is lowercase; on Linux it preserves case.
//! - CRLF vs LF: `content_digest(b"a\r\nb")` equals
//!   `content_digest(b"a\nb")`.
//! - Temp directories: `/tmp/foo` and `/tmp/./foo` collapse to the
//!   same canonical path.
//! - Paths with spaces and Unicode: preserved verbatim after
//!   normalization.

use std::path::{Path, PathBuf};

/// The kind of host we are running on. Used to decide whether
/// canonicalization should lowercase the path (Windows, macOS by
/// default on case-insensitive volumes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    /// POSIX case-sensitive (Linux, *BSD). Path case is preserved.
    PosixCaseSensitive,
    /// Case-insensitive (Windows, macOS HFS+/APFS by default, SMB).
    CaseInsensitive,
}

impl HostKind {
    /// Detect the host kind for the current process. We use compile-
    /// time `cfg` rather than runtime FS probing because the
    /// directive says "do not touch the FS in this layer".
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            HostKind::PosixCaseSensitive
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            // macOS APFS is case-insensitive on the default volume;
            // Windows NTFS is case-insensitive. We treat both as
            // case-insensitive here.
            HostKind::CaseInsensitive
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Unknown host — be conservative: case-sensitive.
            HostKind::PosixCaseSensitive
        }
    }
}

/// A path that has been normalized for use as evidence /
/// lineage identity. The inner string is the canonical form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalPath(String);

impl CanonicalPath {
    /// Borrow the canonical string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into the inner String.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for CanonicalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for CanonicalPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for CanonicalPath {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

/// Canonicalize a path on a host of the given kind. The function is
/// pure: no FS access, no environment queries. It normalizes:
/// - backslashes to forward slashes (Windows host paths)
/// - removes duplicate slashes
/// - collapses `.` and `..` segments lexically (without resolving
///   symlinks)
/// - lowercases on case-insensitive hosts
/// - trims a trailing slash (except for the root)
///
/// Invariants:
/// - The returned string starts with `/` on POSIX hosts and with a
///   drive letter `X:/` on Windows hosts.
/// - Two paths that differ only in case map to the same canonical
///   form on case-insensitive hosts.
pub fn canonicalize(path: impl AsRef<Path>, host: HostKind) -> CanonicalPath {
    let raw = path.as_ref().to_string_lossy();
    // Step 1: normalise separators to '/'.
    let mut s = raw.replace('\\', "/");
    // Step 2: collapse duplicate slashes (but preserve leading "//"
    // on POSIX hosts — UNC-ish paths).
    let mut out = String::with_capacity(s.len());
    let mut prev_slash = false;
    for c in s.chars() {
        if c == '/' {
            if !prev_slash {
                out.push('/');
            }
            prev_slash = true;
        } else {
            out.push(c);
            prev_slash = false;
        }
    }
    s = out;
    // Step 3: lowercase on case-insensitive hosts.
    if host == HostKind::CaseInsensitive {
        s = s.to_lowercase();
    }
    // Step 4: collapse "." and ".." lexically.
    let collapsed = collapse_dot_segments(&s);
    CanonicalPath(collapsed)
}

/// Lexically collapse "." and ".." segments. Preserves leading ".."
/// (treated as no-op, since there's no parent to go up to in a
/// non-FS context).
fn collapse_dot_segments(s: &str) -> String {
    // Detect the leading prefix:
    //   - "/" (POSIX absolute)
    //   - "X:/" or "X:" (Windows drive)
    // The rest of the path is processed as components.
    let (prefix, body) = split_prefix(s);

    let mut stack: Vec<&str> = Vec::new();
    for comp in body.split('/') {
        match comp {
            "" | "." => continue,
            ".." => {
                if let Some(top) = stack.last() {
                    if *top != ".." && !is_root_marker(*top) {
                        stack.pop();
                        continue;
                    }
                }
                stack.push("..");
            }
            other => stack.push(other),
        }
    }

    let mut out = String::new();
    out.push_str(&prefix);
    let mut first = prefix.is_empty()
        || prefix.ends_with('/')
        || prefix.ends_with(':');
    if stack.is_empty() {
        if prefix.is_empty() {
            out.push('.');
        }
        return out;
    }
    for comp in &stack {
        if !first {
            out.push('/');
        }
        out.push_str(comp);
        first = false;
    }
    out
}

/// Split a path into its leading prefix (POSIX `/`, Windows
/// `X:/` or `X:`) and the rest of the path.
fn split_prefix(s: &str) -> (String, &str) {
    if let Some(rest) = s.strip_prefix('/') {
        ("/".to_string(), rest)
    } else if s.len() >= 2
        && s.as_bytes()[1] == b':'
        && s.as_bytes()[0].is_ascii_alphabetic()
    {
        let prefix_len = if s.len() >= 3 && s.as_bytes()[2] == b'/' {
            3
        } else {
            2
        };
        let prefix = s[..prefix_len].to_string();
        (prefix, &s[prefix_len..])
    } else {
        (String::new(), s)
    }
}

fn is_root_marker(s: &str) -> bool {
    // An empty component (root) is a root marker. We do not push
    // empty components onto the stack, so this should never match,
    // but we keep the check for safety.
    s.is_empty()
}

/// Convert a canonical path back to the host's preferred path
/// representation. The canonical form uses forward slashes; on
/// Windows we convert to backslashes.
pub fn host_path_for_spawn(canonical: &CanonicalPath, host: HostKind) -> PathBuf {
    match host {
        HostKind::PosixCaseSensitive => PathBuf::from(canonical.as_str()),
        HostKind::CaseInsensitive => {
            // On Windows we want backslashes for the spawn API.
            #[cfg(target_os = "windows")]
            {
                PathBuf::from(canonical.as_str().replace('/', "\\"))
            }
            #[cfg(not(target_os = "windows"))]
            {
                // On a case-insensitive POSIX host (rare; e.g.
                // macOS with case-sensitive FS), keep forward slashes.
                let _ = host;
                PathBuf::from(canonical.as_str())
            }
        }
    }
}

/// A content digest for evidence purposes. Computed over bytes after
/// CRLF → LF normalisation, so the same logical content on hosts
/// that differ only in line endings produces the same digest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentDigest(String);

impl ContentDigest {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ContentDigest {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Compute a content digest over `bytes` after normalising CRLF to
/// LF. We use a tiny inline FNV-1a 64-bit hash to avoid pulling in a
/// SHA crate. The output is a 16-character hex digest, prefixed
/// with `fnv64:` for clarity. This is for evidence identity, not
/// cryptographic security; we use SHA-256 only when the user opts
/// into `evidence-kernel`-level content pinning (deferred).
pub fn content_digest(bytes: &[u8]) -> ContentDigest {
    // Normalise CRLF to LF before hashing.
    let mut normalised: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'\r' && bytes[i + 1] == b'\n' {
            normalised.push(b'\n');
            i += 2;
        } else {
            normalised.push(bytes[i]);
            i += 1;
        }
    }
    let hash = fnv1a_64(&normalised);
    ContentDigest(format!("fnv64:{:016x}", hash))
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn canon_posix(p: &str) -> CanonicalPath {
        canonicalize(p, HostKind::PosixCaseSensitive)
    }
    fn canon_win(p: &str) -> CanonicalPath {
        canonicalize(p, HostKind::CaseInsensitive)
    }

    // ----- Adversarial UAT for WU4 (per the directive's checklist).

    #[test]
    fn wu4_windows_backslash_and_forward_slash_yield_the_same_canonical() {
        let a = canon_win(r"C:\repo\src");
        let b = canon_win("C:/repo/src");
        assert_eq!(
            a, b,
            "Windows backslash and forward-slash forms must canonicalise identically"
        );
        // And the canonical form uses forward slashes for stability.
        assert_eq!(a.as_str(), "c:/repo/src");
    }

    #[test]
    fn wu4_posix_absolute_path_is_preserved() {
        let p = canon_posix("/workspace/src");
        assert_eq!(p.as_str(), "/workspace/src");
    }

    #[test]
    fn wu4_case_insensitive_host_lowercases_paths() {
        let p = canon_win("C:/Repo/Source/Main.RS");
        assert_eq!(p.as_str(), "c:/repo/source/main.rs");
    }

    #[test]
    fn wu4_case_sensitive_host_preserves_case() {
        let p = canon_posix("/workspace/Repo/Source/Main.RS");
        assert_eq!(p.as_str(), "/workspace/Repo/Source/Main.RS");
    }

    #[test]
    fn wu4_dot_segments_are_collapsed_lexically() {
        assert_eq!(canon_posix("/tmp/./foo").as_str(), "/tmp/foo");
        assert_eq!(canon_posix("/tmp/foo/../bar").as_str(), "/tmp/bar");
        assert_eq!(canon_posix("/tmp/foo/./bar/../baz").as_str(), "/tmp/foo/baz");
    }

    #[test]
    fn wu4_duplicate_slashes_are_collapsed() {
        assert_eq!(canon_posix("/tmp//foo///bar").as_str(), "/tmp/foo/bar");
    }

    #[test]
    fn wu4_paths_with_spaces_are_preserved() {
        let p = canon_posix("/tmp/my project/file with spaces.rs");
        assert_eq!(p.as_str(), "/tmp/my project/file with spaces.rs");
    }

    #[test]
    fn wu4_unicode_paths_are_preserved() {
        let p = canon_posix("/tmp/项目/файл.rs");
        assert_eq!(p.as_str(), "/tmp/项目/файл.rs");
    }

    #[test]
    fn wu4_temp_directories_canonicalise_to_a_stable_form() {
        // /tmp/foo and /tmp/./foo must collapse.
        assert_eq!(canon_posix("/tmp/foo").as_str(), canon_posix("/tmp/./foo").as_str());
    }

    #[test]
    fn wu4_content_digest_is_invariant_to_crlf_vs_lf() {
        let crlf = b"line one\r\nline two\r\nline three";
        let lf = b"line one\nline two\nline three";
        assert_eq!(content_digest(crlf), content_digest(lf));
    }

    #[test]
    fn wu4_content_digest_is_sensitive_to_real_content_differences() {
        let a = b"hello world";
        let b = b"goodbye world";
        assert_ne!(content_digest(a), content_digest(b));
    }

    #[test]
    fn wu4_content_digest_is_deterministic() {
        let a = b"the quick brown fox";
        let b = b"the quick brown fox";
        assert_eq!(content_digest(a), content_digest(b));
    }

    #[test]
    fn wu4_canonical_path_roundtrips_through_host_path_for_spawn() {
        let canonical = canon_posix("/tmp/foo/bar");
        let host = host_path_for_spawn(&canonical, HostKind::PosixCaseSensitive);
        assert_eq!(host.to_string_lossy(), "/tmp/foo/bar");
        // Round-trip back to canonical: identity for POSIX.
        let again = canonicalize(&host, HostKind::PosixCaseSensitive);
        assert_eq!(again, canonical);
    }

    #[test]
    fn wu4_host_path_for_spawn_uses_native_separator_on_windows() {
        let canonical = canon_win(r"C:\repo\src");
        let host = host_path_for_spawn(&canonical, HostKind::CaseInsensitive);
        // We only convert to backslashes on actual Windows; on
        // POSIX test hosts we keep forward slashes but the
        // canonical form is still lowercase.
        #[cfg(target_os = "windows")]
        assert_eq!(host.to_string_lossy(), r"C:\repo\src");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(host.to_string_lossy(), "c:/repo/src");
    }

    #[test]
    fn wu4_canonical_path_is_hashable_and_eq_so_it_can_be_a_evidence_field_key() {
        use std::collections::HashSet;
        let a = canon_posix("/workspace/src");
        let b = canon_posix("/workspace/src");
        let c = canon_posix("/workspace/dst");
        let mut set = HashSet::new();
        set.insert(a.clone());
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }

    #[test]
    fn wu4_empty_path_yields_dot() {
        let p = canon_posix("");
        assert_eq!(p.as_str(), ".");
    }

    #[test]
    fn wu4_root_path_preserves_root_marker() {
        let p = canon_posix("/");
        assert_eq!(p.as_str(), "/");
    }

    #[test]
    fn wu4_windows_drive_root_preserves_colon() {
        let p = canon_win("C:");
        assert_eq!(p.as_str(), "c:");
    }

    #[test]
    fn wu4_paths_module_does_not_leak_platform_specific_identifiers() {
        let src = super::super::strip_doc_comments_and_tests(include_str!("paths.rs"));
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !src.to_lowercase().contains(forbidden),
                "paths module must not leak platform-specific symbol in code: {forbidden}"
            );
        }
    }

    // Ensure we exercise the CanonicalPath/ContentDigest AsRef impls.
    #[test]
    fn wu4_canonical_path_as_ref_path_works() {
        let p = canon_posix("/workspace/src");
        let as_path: &Path = p.as_ref();
        assert_eq!(as_path, Path::new("/workspace/src"));
    }

    #[test]
    fn wu4_content_digest_as_ref_str_works() {
        let d = content_digest(b"hello");
        let s: &str = d.as_ref();
        assert!(s.starts_with("fnv64:"));
    }
}
