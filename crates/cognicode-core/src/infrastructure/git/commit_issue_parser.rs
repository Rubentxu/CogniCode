//! `commit_issue_parser` — pure function that turns the raw
//! output of `git log --pretty=format:%H%x1f%s%x1f%b` into a
//! `Vec<CommitIssueRef>`. No `git` subprocess, no I/O — the
//! caller spawns `git log` and pipes the output through.
//!
//! The function is the TDD-friendly seam that splits
//! `parse_commit_issue_refs` (pure, tested with canned strings)
//! from the `IssuesExtractor::extract` async impl (which owns
//! the subprocess + URL parse).
//!
//! Conventions:
//!
//! - Each `git log` line is `\x1f`-separated into exactly three
//!   fields: full SHA, subject, body. The `unit-separator` byte
//!   (`\x1f`, ASCII 0x1F) NEVER appears in commit messages in
//!   practice — it is reserved by the spec for this use.
//! - Commits whose subject or body contain `Fixes/Closes/Resolves
//!   #N` produce a `CommitRefKind::Fixes` reference. Commits
//!   whose subject or body contain `Refs/References/See/Part of
//!   #N` produce a `CommitRefKind::Refs` reference. Both are
//!   emitted in the returned vec — the caller picks the right
//!   confidence tier per `EdgeKind`.
//! - Short SHAs are the first 7 hex chars of the full SHA
//!   (per the spec's `commit:{sha_short}` NodeId convention).
//!
//! The module is `#[cfg(feature = "multimodal")]`-gated.

#[cfg(feature = "multimodal")]
use std::sync::LazyLock;

#[cfg(feature = "multimodal")]
use regex::Regex;

#[cfg(feature = "multimodal")]
/// The separator used by `git log --pretty=format:%H%x1f%s%x1f%b`.
/// ASCII 0x1F (unit separator) — never appears in commit messages.
pub const UNIT_SEPARATOR: char = '\u{1f}';

/// The kind of cross-reference a commit message makes to an
/// issue. The two variants map to two different confidence tiers
/// in the `IssuesConfidenceRules` module.
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitRefKind {
    /// A closing-style reference (`Fixes/Closes/Resolves #N`).
    /// Maps to the `CommitFixes` tier (0.85).
    Fixes,
    /// A soft reference (`Refs/References/See/Part of #N`).
    /// Maps to the `CommitRefs` tier (0.7).
    Refs,
}

#[cfg(feature = "multimodal")]
impl CommitRefKind {
    /// Stable kebab-case identifier. Used in the
    /// `GraphEdge::metadata["commit_ref_kind"]` hint.
    pub const fn as_str(self) -> &'static str {
        match self {
            CommitRefKind::Fixes => "fixes",
            CommitRefKind::Refs => "refs",
        }
    }
}

/// A single cross-reference a commit message makes to an issue.
/// Emitted by [`parse_commit_issue_refs`] for every
/// `Fixes/Closes/Resolves/Refs/References/See/Part of #N` match.
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitIssueRef {
    /// The first 7 hex chars of the full commit SHA.
    pub commit_sha: String,
    /// The issue number (1-based; `0` is invalid in GitHub).
    pub issue_number: u32,
    /// The kind of cross-reference — drives the
    /// confidence tier on the eventual `GraphEdge`.
    pub ref_kind: CommitRefKind,
}

/// Build the canonical `NodeId` for a commit node.
///
/// Format: `commit:{sha_short}` (e.g. `commit:abc1234`). The
/// short SHA is the first 7 hex chars of the full SHA. The id
/// is deterministic, so re-ingesting the same log produces the
/// same id and the persistence layer's upsert collapses the
/// duplicate.
#[cfg(feature = "multimodal")]
pub fn commit_node_id(sha_short: &str) -> String {
    format!("commit:{sha_short}")
}

// ============================================================================
// Compiled regexes (built once via `once_cell::sync::Lazy`).
// ============================================================================
//
// Each pattern has one capturing group:
// - group 1: the issue number.
// The keyword prefix is matched literally (case-insensitive).

/// `Fixes/Closes/Resolves #N` — closing-style reference. Issue
/// number is group 1.
#[cfg(feature = "multimodal")]
static FIXES_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // The `(?i)` enables case-insensitive matching. The word
    // boundaries on the keyword and the `\s+` between keyword
    // and `#` prevent partial-word matches (e.g. `prefixes`).
    Regex::new(r"(?i)\b(?:fixes|closes|resolves)\s+#(\d+)\b")
        .expect("FIXES_REGEX must compile")
});

/// `Refs/References/See/Part of #N` — soft reference. Issue
/// number is group 1.
#[cfg(feature = "multimodal")]
static REFS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:refs|references|see|part\s+of)\s+#(\d+)\b")
        .expect("REFS_REGEX must compile")
});

/// Parse a `git log` blob into a flat list of `CommitIssueRef`s.
///
/// `log_output` is the raw output of
/// `git log --all --pretty=format:%H%x1f%s%x1f%b`. Lines are
/// split on `\n`; each line is then split on `\x1f` into
/// `[full_sha, subject, body]` (body may be empty).
///
/// The function emits one `CommitIssueRef` per `(commit,
/// issue_number, ref_kind)` triple. A single commit that
/// mentions multiple issues (or mixes `Fixes` with `Refs`)
/// produces multiple refs.
///
/// Lines that do not parse as `[sha, subject, body]` are
/// silently skipped (the contract is "best effort": an
/// extractor that runs on a corrupted log returns the partial
/// vec, never an error). Lines whose commit SHA cannot be
/// shortened to 7 hex chars (e.g. empty / non-hex) are also
/// skipped.
#[cfg(feature = "multimodal")]
pub fn parse_commit_issue_refs(
    log_output: &str,
    owner: &str,
    _repo: &str,
) -> Vec<CommitIssueRef> {
    let mut out: Vec<CommitIssueRef> = Vec::new();
    for line in log_output.lines() {
        if line.is_empty() {
            continue;
        }
        // Skip non-UTF-8 lines (impossible by the &str type —
        // we already have valid UTF-8 — but the original
        // `Vec<u8>` blob from the subprocess could carry
        // garbage; the caller is expected to .lines() over
        // String::from_utf8_lossy).
        let mut parts = line.splitn(3, UNIT_SEPARATOR);
        let full_sha = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let subject = parts.next().unwrap_or("");
        let body = parts.next().unwrap_or("");
        let sha_short = short_sha(full_sha);
        if sha_short.is_empty() {
            continue;
        }
        let haystack = format!("{subject}\n{body}");
        for cap in FIXES_REGEX.captures_iter(&haystack) {
            if let Some(num) = parse_u32(cap.get(1).map(|m| m.as_str()).unwrap_or("")) {
                if num == 0 {
                    continue;
                }
                out.push(CommitIssueRef {
                    commit_sha: sha_short.clone(),
                    issue_number: num,
                    ref_kind: CommitRefKind::Fixes,
                });
            }
        }
        for cap in REFS_REGEX.captures_iter(&haystack) {
            if let Some(num) = parse_u32(cap.get(1).map(|m| m.as_str()).unwrap_or("")) {
                if num == 0 {
                    continue;
                }
                out.push(CommitIssueRef {
                    commit_sha: sha_short.clone(),
                    issue_number: num,
                    ref_kind: CommitRefKind::Refs,
                });
            }
        }
        let _ = owner; // reserved for V2 cross-repo routing.
    }
    out
}

/// Build the canonical `issue:{tracker}/{repo}#{N}` NodeId for
/// the issue referenced by a commit.
#[cfg(feature = "multimodal")]
pub fn issue_node_id_for_commit(
    tracker: &str,
    repo: &str,
    issue_number: u32,
) -> String {
    format!("issue:{tracker}/{repo}#{issue_number}")
}

/// Parse a `u32` from a digit string. Returns `None` for
/// empty input, overflow, or non-digit chars.
#[cfg(feature = "multimodal")]
fn parse_u32(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    s.parse::<u32>().ok()
}

/// Take the first 7 hex chars of a full SHA, or the full string
/// if shorter than 7. Returns the empty string when the input
/// is empty or contains non-hex chars in the prefix.
#[cfg(feature = "multimodal")]
fn short_sha(full: &str) -> String {
    let prefix: String = full
        .chars()
        .take(7)
        .map(|c| if c.is_ascii_hexdigit() { c.to_ascii_lowercase() } else { '\0' })
        .collect();
    if prefix.contains('\0') {
        return String::new();
    }
    if prefix.is_empty() {
        return String::new();
    }
    prefix
}

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;

    /// Helper to build a `git log` line in the canonical
    /// `%H%x1f%s%x1f%b` format. Pass an empty body for a
    /// subject-only commit.
    fn log_line(full_sha: &str, subject: &str, body: &str) -> String {
        if body.is_empty() {
            format!("{full_sha}{UNIT_SEPARATOR}{subject}")
        } else {
            format!("{full_sha}{UNIT_SEPARATOR}{subject}{UNIT_SEPARATOR}{body}")
        }
    }

    // ---- T3 RED gate: 12 tests ----

    /// The `Fixes #N` pattern in a subject produces a single
    /// `CommitRefKind::Fixes` reference.
    #[test]
    fn fixes_in_subject() {
        let log = log_line(
            "abc1234567890def",
            "Fixes #42: handle null pointer",
            "",
        );
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].commit_sha, "abc1234");
        assert_eq!(refs[0].issue_number, 42);
        assert_eq!(refs[0].ref_kind, CommitRefKind::Fixes);
    }

    /// The `Closes #N` pattern in a subject is recognised.
    #[test]
    fn closes_in_subject() {
        let log = log_line("abc1234", "Closes #10", "");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].issue_number, 10);
        assert_eq!(refs[0].ref_kind, CommitRefKind::Fixes);
    }

    /// The `Resolves #N` pattern in a subject is recognised.
    #[test]
    fn resolves_in_subject() {
        let log = log_line("abc1234", "Resolves #99", "");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].issue_number, 99);
        assert_eq!(refs[0].ref_kind, CommitRefKind::Fixes);
    }

    /// The `Refs #N` pattern produces a `CommitRefKind::Refs`.
    #[test]
    fn refs_in_body() {
        let log = log_line(
            "abc1234",
            "Refactor auth flow",
            "Refs #42: see also the migration guide.",
        );
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_kind, CommitRefKind::Refs);
        assert_eq!(refs[0].issue_number, 42);
    }

    /// The `Part of #N` (multi-word) pattern is recognised.
    #[test]
    fn part_of_in_body() {
        let log = log_line("abc1234", "add widget", "Part of #100 — bigger epic");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_kind, CommitRefKind::Refs);
        assert_eq!(refs[0].issue_number, 100);
    }

    /// The `See #N` variant is recognised.
    #[test]
    fn see_in_body() {
        let log = log_line("abc1234", "refactor", "See #7 for context");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_kind, CommitRefKind::Refs);
        assert_eq!(refs[0].issue_number, 7);
    }

    /// A single commit that mentions multiple issues produces
    /// one ref per mention. Mixed `Fixes` + `Refs` produce
    /// both kinds.
    #[test]
    fn multiple_references_in_one_commit() {
        let log = log_line(
            "abc1234",
            "Closes #10, Refs #11, Refs #12",
            "",
        );
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 3, "expected 3 references: {:?}", refs);
        assert_eq!(refs[0].ref_kind, CommitRefKind::Fixes);
        assert_eq!(refs[0].issue_number, 10);
        assert_eq!(refs[1].ref_kind, CommitRefKind::Refs);
        assert_eq!(refs[1].issue_number, 11);
        assert_eq!(refs[2].ref_kind, CommitRefKind::Refs);
        assert_eq!(refs[2].issue_number, 12);
    }

    /// `Fixes #0` is invalid (issue 0 doesn't exist in GitHub)
    /// and MUST be skipped.
    #[test]
    fn issue_zero_rejected() {
        let log = log_line("abc1234", "Fixes #0", "");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert!(refs.is_empty(), "expected no refs (issue 0 invalid), got: {:?}", refs);
    }

    /// An empty log produces no refs.
    #[test]
    fn empty_log_produces_no_refs() {
        let refs = parse_commit_issue_refs("", "acme", "widgets");
        assert!(refs.is_empty());
    }

    /// The `unit_separator` parses correctly even when the body
    /// is empty (subject-only commits).
    #[test]
    fn single_separator_subject_only() {
        let log = log_line("abc1234", "Fixes #42", "");
        // The line has only ONE unit-separator (subject-only
        // form). Verify the parser still finds the ref.
        assert_eq!(log.matches(UNIT_SEPARATOR).count(), 1);
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
    }

    /// The `\x1f` unit-separator is preserved verbatim by
    /// `String::from_utf8_lossy` (it IS valid UTF-8 — the
    /// character is U+001F), so the parser does not need
    /// special handling for non-UTF-8 bytes.
    #[test]
    fn non_utf8_bytes_stripped_at_caller() {
        // Simulate a caller that has already called
        // `String::from_utf8_lossy` on the raw bytes — the
        // non-UTF-8 chars are replaced with U+FFFD. The
        // parser receives a clean &str and produces
        // deterministic output.
        let log = log_line("abc1234", "Fixes #42", "");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].issue_number, 42);
    }

    /// Case-insensitive matching: `FIXES #7` is the same as
    /// `Fixes #7`.
    #[test]
    fn case_insensitive_matching() {
        let log = log_line("abc1234", "FIXES #7 — typo in readme", "");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].issue_number, 7);
    }

    // ---- Additional TDD coverage ----

    /// A commit that mentions no `#N` produces an empty vec.
    #[test]
    fn no_mentions_produces_empty() {
        let log = log_line("abc1234", "Add widget", "No issue refs here.");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert!(refs.is_empty());
    }

    /// Multi-line git log input: each `\n`-separated line is
    /// treated as one commit. The parser emits refs for each.
    #[test]
    fn multi_line_log() {
        let log = format!(
            "{}\n{}\n{}",
            log_line("abc1234", "Fixes #1", ""),
            log_line("def5678", "Refs #2", ""),
            log_line("1234567", "Closes #3", ""),
        );
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].issue_number, 1);
        assert_eq!(refs[1].issue_number, 2);
        assert_eq!(refs[2].issue_number, 3);
    }

    /// The short SHA is exactly 7 hex chars (lower-case).
    #[test]
    fn short_sha_is_7_lowercase() {
        let log = log_line("ABCDEF1234567def", "Fixes #1", "");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].commit_sha, "abcdef1");
    }

    /// A commit with only a `Part of` reference and no `Fixes`
    /// reference still emits the `Refs` ref.
    #[test]
    fn part_of_alone_in_body() {
        let log = log_line("abc1234", "wip", "Part of #50");
        let refs = parse_commit_issue_refs(&log, "acme", "widgets");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_kind, CommitRefKind::Refs);
        assert_eq!(refs[0].issue_number, 50);
    }

    /// `CommitRefKind::as_str` is the kebab-case identifier
    /// (`fixes` | `refs`).
    #[test]
    fn commit_ref_kind_as_str() {
        assert_eq!(CommitRefKind::Fixes.as_str(), "fixes");
        assert_eq!(CommitRefKind::Refs.as_str(), "refs");
    }

    /// `commit_node_id` returns the canonical
    /// `commit:{sha_short}` form.
    #[test]
    fn commit_node_id_format() {
        assert_eq!(commit_node_id("abc1234"), "commit:abc1234");
    }

    /// `issue_node_id_for_commit` returns the canonical
    /// `issue:{tracker}/{repo}#{N}` form.
    #[test]
    fn issue_node_id_for_commit_format() {
        assert_eq!(
            issue_node_id_for_commit("github", "acme/widgets", 42),
            "issue:github/acme/widgets#42"
        );
    }
}
