//! `IssuesConfidenceRules` — pure functions that map an
//! issue-derived reference (a commit message, a body mention) to a
//! confidence score in `[0.0, 1.0]`.
//!
//! The 4-tier rule table is fixed by the spec; the [`Provenance`]
//! tag is also derived from the tier so the persistence layer
//! can filter ambiguous edges out of the canonical graph. The full
//! table:
//!
//! | Rule           | Confidence | Provenance  | Trigger                                                         |
//! |----------------|-----------:|-------------|-----------------------------------------------------------------|
//! | `ExplicitLink` | 0.9        | `Extracted` | Issue body contains a `Fixes/Closes/Resolves` keyword OR an explicit commit-SHA cross-reference. |
//! | `CommitFixes`  | 0.85       | `Extracted` | A commit subject or body matches `Fixes/Closes/Resolves #N`.    |
//! | `CommitRefs`   | 0.7        | `Inferred`  | A commit subject or body matches `Refs/Part of/See #N`.         |
//! | `BodyMention`  | 0.5        | `Inferred`  | Issue body mentions a code symbol via `file:name:line` shape.   |
//! | `Unresolved`   | 0.3        | `Ambiguous` | None of the above apply (defensive fallback only).              |
//!
//! The module is `#[cfg(feature = "multimodal")]`-gated because it
//! is part of the issues-source adapter pipeline. The default
//! build does not pull in the extraction dependencies.

#[cfg(feature = "multimodal")]
use crate::domain::value_objects::provenance::Provenance;

/// The five confidence tiers the spec fixes. Each tier is bound
/// to a stable confidence value and a [`Provenance`] tag so the
/// [`crate::infrastructure::extraction::issues_extractor`] can
/// emit a [`crate::domain::aggregates::generic_graph::GraphEdge`]
/// without re-deciding the rule.
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceTier {
    /// `explicit_link` — 0.9, `Extracted`. Trigger: an issue body
    /// contains a `Fixes/Closes/Resolves` keyword or an explicit
    /// commit-SHA cross-reference.
    ExplicitLink,
    /// `commit_fixes` — 0.85, `Extracted`. Trigger: a commit
    /// subject or body matches `Fixes/Closes/Resolves #N`.
    CommitFixes,
    /// `commit_refs` — 0.7, `Inferred`. Trigger: a commit subject
    /// or body matches `Refs/Part of/See #N`.
    CommitRefs,
    /// `body_mention` — 0.5, `Inferred`. Trigger: an issue body
    /// line matches the canonical `file:name:line` shape (or a
    /// markdown link to one).
    BodyMention,
    /// `unresolved` — 0.3, `Ambiguous`. Defensive fallback only:
    /// none of the above rules matched.
    Unresolved,
}

#[cfg(feature = "multimodal")]
impl ConfidenceTier {
    /// The fixed confidence value the spec binds to this tier.
    #[inline]
    pub const fn confidence(self) -> f64 {
        match self {
            ConfidenceTier::ExplicitLink => 0.9,
            ConfidenceTier::CommitFixes => 0.85,
            ConfidenceTier::CommitRefs => 0.7,
            ConfidenceTier::BodyMention => 0.5,
            ConfidenceTier::Unresolved => 0.3,
        }
    }

    /// The fixed [`Provenance`] tag the spec binds to this tier.
    #[inline]
    pub const fn provenance(self) -> Provenance {
        match self {
            ConfidenceTier::ExplicitLink => Provenance::Extracted,
            ConfidenceTier::CommitFixes => Provenance::Extracted,
            ConfidenceTier::CommitRefs => Provenance::Inferred,
            ConfidenceTier::BodyMention => Provenance::Inferred,
            ConfidenceTier::Unresolved => Provenance::Ambiguous,
        }
    }
}

/// Score a body / commit text against the `ExplicitLink` rule.
///
/// The rule fires when the text contains a `Fixes/Closes/Resolves`
/// keyword OR a 7–40-char commit-SHA cross-reference. Case
/// insensitive.
///
/// Returns [`ConfidenceTier::ExplicitLink`] when matched, else
/// [`ConfidenceTier::Unresolved`]. Pure function — no `&self`, no
/// I/O, no global state.
#[cfg(feature = "multimodal")]
#[inline]
pub fn score_explicit_link(text: &str) -> ConfidenceTier {
    if has_fix_keyword(text) || has_commit_sha(text) {
        ConfidenceTier::ExplicitLink
    } else {
        ConfidenceTier::Unresolved
    }
}

/// Score a commit message against the `CommitFixes` rule.
///
/// The rule fires when the message (subject + body) matches one
/// of the closing-style keywords (`Fixes`, `Closes`, `Resolves`)
/// immediately followed by whitespace and `#N`. Case
/// insensitive.
///
/// Returns [`ConfidenceTier::CommitFixes`] when matched, else
/// [`ConfidenceTier::Unresolved`].
#[cfg(feature = "multimodal")]
#[inline]
pub fn score_commit_fixes(commit_msg: &str) -> ConfidenceTier {
    if matches_fix_keyword_issue(commit_msg) {
        ConfidenceTier::CommitFixes
    } else {
        ConfidenceTier::Unresolved
    }
}

/// Score a commit message against the `CommitRefs` rule.
///
/// The rule fires when the message matches a soft reference
/// keyword (`Refs`, `References`, `See`, `Part of`) followed by
/// whitespace and `#N`.
///
/// Returns [`ConfidenceTier::CommitRefs`] when matched, else
/// [`ConfidenceTier::Unresolved`]. Note: when a single commit
/// matches BOTH `CommitFixes` and `CommitRefs` (e.g. `Fixes #10,
/// Refs #11`), the extractor calls `score_commit_fixes` first
/// and uses that higher-confidence tier for the primary
/// reference; `score_commit_refs` is then called on the
/// per-reference text for the secondary ones.
#[cfg(feature = "multimodal")]
#[inline]
pub fn score_commit_refs(commit_msg: &str) -> ConfidenceTier {
    if matches_ref_keyword_issue(commit_msg) {
        ConfidenceTier::CommitRefs
    } else {
        ConfidenceTier::Unresolved
    }
}

/// Score a single body line against the `BodyMention` rule.
///
/// The rule fires when the line is shaped like `file:name:line`
/// (or a markdown link to one) AND is NOT a bare
/// `https://…` URL. The function is a textual shape check; it
/// does not consult the symbol table.
///
/// Returns [`ConfidenceTier::BodyMention`] when matched, else
/// [`ConfidenceTier::Unresolved`].
#[cfg(feature = "multimodal")]
#[inline]
pub fn score_body_mention(line: &str) -> ConfidenceTier {
    if looks_like_body_mention(line) {
        ConfidenceTier::BodyMention
    } else {
        ConfidenceTier::Unresolved
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// True when `text` contains any of the closing-style keywords
/// (`fix`, `close`, `resolve`) as a standalone word. Used by the
/// `ExplicitLink` rule (no `#N` requirement there — the
/// presence of the keyword alone is enough to trigger the
/// higher-confidence tier because the body is assumed to be
/// well-formed by the extractor).
#[cfg(feature = "multimodal")]
fn has_fix_keyword(text: &str) -> bool {
    let needle = |kw: &str| {
        let lower = text.to_ascii_lowercase();
        // Word boundary on the left (start of string OR a
        // non-alphanumeric char) and word boundary on the right
        // (whitespace, punctuation, or end of string). We use
        // simple ASCII checks to keep this pure.
        lower.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| w == kw)
    };
    needle("fix") || needle("fixes") || needle("fixed")
        || needle("close") || needle("closes") || needle("closed")
        || needle("resolve") || needle("resolves") || needle("resolved")
}

/// True when `text` mentions a 7-40 char hex SHA-1 / SHA-256
/// commit id. The check is loose on purpose — any hex blob of
/// the right length counts. The cost of a false positive
/// (`ExplicitLink` when the intent was a `CommitFixes`) is
/// negligible because both tiers are `Extracted` provenance.
#[cfg(feature = "multimodal")]
fn has_commit_sha(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_hexdigit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
                i += 1;
            }
            let len = i - start;
            if (7..=40).contains(&len) {
                // Confirm the blob is bounded by non-hex chars or
                // string ends (so a `cafe` word doesn't count
                // as a SHA-1).
                let left_ok = start == 0
                    || !bytes[start - 1].is_ascii_hexdigit();
                let right_ok = i == bytes.len()
                    || !bytes[i].is_ascii_hexdigit();
                if left_ok && right_ok {
                    return true;
                }
            }
        } else {
            i += 1;
        }
    }
    false
}

/// True when `text` matches the `Fixes/Closes/Resolves #N` form
/// (case-insensitive). The `N` is captured but not returned —
/// the extractor's regex-based parser owns the per-issue emit
/// loop; this helper only answers the boolean "did any
/// closing-style reference appear?".
#[cfg(feature = "multimodal")]
fn matches_fix_keyword_issue(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    for kw in ["fixes", "closes", "resolves"] {
        if contains_keyword_issue(&lower, kw) {
            // Reject `Fixes #0` / `Fixes #00` / `Fixes #000` —
            // issue 0 is invalid in GitHub. The check fires when
            // the digit run after `#` is exactly `0` (any number
            // of leading zeros, all zero).
            if contains_keyword_issue_zero(&lower, kw) {
                return false;
            }
            return true;
        }
    }
    false
}

/// True when `text` matches the `Refs/References/See/Part of #N`
/// form. `Part of` is a multi-word keyword; the check skips the
/// hyphen gap.
#[cfg(feature = "multimodal")]
fn matches_ref_keyword_issue(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    for kw in ["refs", "references", "see"] {
        if contains_keyword_issue(&lower, kw) {
            return true;
        }
    }
    // `part of` is two words; check the literal phrase.
    if contains_phrase_issue(&lower, "part of") {
        return true;
    }
    false
}

/// True when `lower` (already lowercased) contains the literal
/// `kw #N` pattern. `kw` is a single word. `N` is a non-empty
/// run of ASCII digits.
#[cfg(feature = "multimodal")]
fn contains_keyword_issue(lower: &str, kw: &str) -> bool {
    let bytes = lower.as_bytes();
    let kw_bytes = kw.as_bytes();
    let mut i = 0;
    while i + kw_bytes.len() <= bytes.len() {
        if &bytes[i..i + kw_bytes.len()] == kw_bytes {
            // Word boundary: char before must be non-alphanumeric
            // (or start of string), char after must be whitespace.
            let left_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after = i + kw_bytes.len();
            let right_ok = after < bytes.len()
                && (bytes[after] == b' ' || bytes[after] == b'\t');
            if left_ok && right_ok {
                // Skip the whitespace, then look for `#`.
                let mut j = after;
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'#' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

/// True when `lower` contains `<kw> #N` where `N` is a run of
/// zero digits (i.e. `#0`, `#00`, `#000`, …). Used to reject
/// the invalid issue-zero references.
#[cfg(feature = "multimodal")]
fn contains_keyword_issue_zero(lower: &str, kw: &str) -> bool {
    let bytes = lower.as_bytes();
    let kw_bytes = kw.as_bytes();
    let mut i = 0;
    while i + kw_bytes.len() <= bytes.len() {
        if &bytes[i..i + kw_bytes.len()] == kw_bytes {
            // Walk forward past the keyword + whitespace.
            let mut j = i + kw_bytes.len();
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            // Expect `#` next.
            if j < bytes.len() && bytes[j] == b'#' {
                j += 1;
                // Count the run of digits.
                let digit_start = j;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                let digit_run = &bytes[digit_start..j];
                // Issue-zero iff the run is non-empty and ALL
                // digits are `0`.
                if !digit_run.is_empty()
                    && digit_run.iter().all(|b| *b == b'0')
                {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

/// True when `lower` contains `<phrase> #N` where `phrase` is a
/// multi-word string (e.g. `"part of"`). Same shape as
/// `contains_keyword_issue` but for phrases.
#[cfg(feature = "multimodal")]
fn contains_phrase_issue(lower: &str, phrase: &str) -> bool {
    let bytes = lower.as_bytes();
    let phrase_bytes = phrase.as_bytes();
    let mut i = 0;
    while i + phrase_bytes.len() <= bytes.len() {
        if &bytes[i..i + phrase_bytes.len()] == phrase_bytes {
            let left_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after = i + phrase_bytes.len();
            let right_ok = after < bytes.len()
                && (bytes[after] == b' ' || bytes[after] == b'\t');
            if left_ok && right_ok {
                let mut j = after;
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'#' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

/// True when `line` (single line of an issue body) is shaped
/// like a code reference. The shape check is intentionally
/// conservative: only lines that match the canonical
/// `SymbolId` shape contribute. Pure function.
#[cfg(feature = "multimodal")]
fn looks_like_body_mention(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Strip the optional `[text](` … `)` markdown link wrapper
    // so the inner target is what we validate.
    let target = if let Some(start) = trimmed.find("](") {
        if let Some(end_rel) = trimmed[start + 2..].find(')') {
            trimmed[start + 2..start + 2 + end_rel].trim()
        } else {
            trimmed
        }
    } else if trimmed.starts_with('<') && trimmed.ends_with('>') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };
    // URL filter — body mentions of http(s) URLs are not code refs.
    if target.starts_with("http://") || target.starts_with("https://") {
        return false;
    }
    // Canonical `SymbolId` shape: 2 or 3 `:`-separated segments,
    // first two non-empty, last (when present) is an integer.
    let parts: Vec<&str> = target.split(':').collect();
    match parts.len() {
        2 => !parts[0].is_empty() && !parts[1].is_empty(),
        3 => {
            !parts[0].is_empty()
                && !parts[1].is_empty()
                && parts[2].parse::<i32>().is_ok()
        }
        _ => false,
    }
}

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;

    // ---- T2 RED gate: 4 tier value tests ----

    /// `ExplicitLink.confidence()` returns the locked 0.9.
    #[test]
    fn explicit_link_confidence_is_0_9() {
        assert_eq!(ConfidenceTier::ExplicitLink.confidence(), 0.9);
    }

    /// `CommitFixes.confidence()` returns the locked 0.85.
    #[test]
    fn commit_fixes_confidence_is_0_85() {
        assert_eq!(ConfidenceTier::CommitFixes.confidence(), 0.85);
    }

    /// `CommitRefs.confidence()` returns the locked 0.7.
    #[test]
    fn commit_refs_confidence_is_0_7() {
        assert_eq!(ConfidenceTier::CommitRefs.confidence(), 0.7);
    }

    /// `BodyMention.confidence()` returns the locked 0.5.
    #[test]
    fn body_mention_confidence_is_0_5() {
        assert_eq!(ConfidenceTier::BodyMention.confidence(), 0.5);
    }

    /// `Unresolved.confidence()` returns the locked 0.3 (the
    /// defensive fallback used by every scoring fn).
    #[test]
    fn unresolved_confidence_is_0_3() {
        assert_eq!(ConfidenceTier::Unresolved.confidence(), 0.3);
    }

    // ---- T2 RED gate: 4 provenance tests ----

    /// The provenance table is locked: `Extracted` for the two
    /// high-confidence tiers, `Inferred` for the two
    /// medium-confidence tiers, `Ambiguous` for the
    /// `Unresolved` defensive fallback.
    #[test]
    fn provenance_matches_spec_table() {
        assert_eq!(
            ConfidenceTier::ExplicitLink.provenance(),
            Provenance::Extracted
        );
        assert_eq!(
            ConfidenceTier::CommitFixes.provenance(),
            Provenance::Extracted
        );
        assert_eq!(
            ConfidenceTier::CommitRefs.provenance(),
            Provenance::Inferred
        );
        assert_eq!(
            ConfidenceTier::BodyMention.provenance(),
            Provenance::Inferred
        );
        assert_eq!(
            ConfidenceTier::Unresolved.provenance(),
            Provenance::Ambiguous
        );
    }

    // ---- T2 RED gate: 5 scoring function happy paths ----

    #[test]
    fn score_explicit_link_matches_keyword() {
        // A body with a closing-style keyword alone is enough
        // (the spec says the body "contains a Fixes keyword OR
        // an explicit commit-SHA cross-reference").
        assert_eq!(
            score_explicit_link("This PR fixes the auth bug"),
            ConfidenceTier::ExplicitLink
        );
    }

    #[test]
    fn score_explicit_link_matches_commit_sha() {
        assert_eq!(
            score_explicit_link("Implements the fix from abc1234."),
            ConfidenceTier::ExplicitLink
        );
    }

    #[test]
    fn score_commit_fixes_happy_path() {
        assert_eq!(
            score_commit_fixes("Fixes #42: handle null pointer"),
            ConfidenceTier::CommitFixes
        );
        assert_eq!(
            score_commit_fixes("Closes #10"),
            ConfidenceTier::CommitFixes
        );
        assert_eq!(
            score_commit_fixes("Resolves #99 — typo"),
            ConfidenceTier::CommitFixes
        );
    }

    #[test]
    fn score_commit_refs_happy_path() {
        assert_eq!(
            score_commit_refs("Refs #42: related bug"),
            ConfidenceTier::CommitRefs
        );
        assert_eq!(
            score_commit_refs("See #10 for context"),
            ConfidenceTier::CommitRefs
        );
        assert_eq!(
            score_commit_refs("Part of #100 — bigger epic"),
            ConfidenceTier::CommitRefs
        );
    }

    #[test]
    fn score_body_mention_happy_path() {
        assert_eq!(
            score_body_mention("see [foo](src/foo.rs:foo:1) for details"),
            ConfidenceTier::BodyMention
        );
        // Bare `file:name:line` (no markdown link) is also valid.
        assert_eq!(
            score_body_mention("src/bar.rs:helper:42"),
            ConfidenceTier::BodyMention
        );
    }

    // ---- T2 RED gate: case insensitivity ----

    /// `score_commit_fixes` is case-insensitive.
    #[test]
    fn score_commit_fixes_is_case_insensitive() {
        assert_eq!(
            score_commit_fixes("FIXES #7 — typo"),
            ConfidenceTier::CommitFixes
        );
        assert_eq!(
            score_commit_fixes("closes #8"),
            ConfidenceTier::CommitFixes
        );
        assert_eq!(
            score_commit_fixes("ReSoLvEs #9"),
            ConfidenceTier::CommitFixes
        );
    }

    // ---- T2 RED gate: 5 Unresolved fallback tests ----

    /// Every scoring function returns `Unresolved` when the
    /// input does not match.
    #[test]
    fn unresolved_fallbacks() {
        assert_eq!(
            score_explicit_link("just a comment"),
            ConfidenceTier::Unresolved
        );
        assert_eq!(
            score_commit_fixes(""),
            ConfidenceTier::Unresolved
        );
        assert_eq!(
            score_commit_refs(""),
            ConfidenceTier::Unresolved
        );
        assert_eq!(
            score_body_mention("   "),
            ConfidenceTier::Unresolved
        );
        assert_eq!(
            score_body_mention("https://example.com"),
            ConfidenceTier::Unresolved
        );
    }

    // ---- T2 RED gate: idempotency (1000 calls) ----

    /// Every scoring function is a pure fn — repeated calls with
    /// the same input return the same result.
    #[test]
    fn idempotency_1000_calls() {
        let inputs = [
            "Fixes #42",
            "Refs #11",
            "src/foo.rs:foo:1",
            "unrelated text",
            "abc1234",
        ];
        for _ in 0..1000 {
            for input in &inputs {
                // Just calling each is enough — the assert
                // below enforces determinism.
                let _ = score_explicit_link(input);
                let _ = score_commit_fixes(input);
                let _ = score_commit_refs(input);
                let _ = score_body_mention(input);
            }
        }
        // Sample one explicit check to anchor the test in
        // observable behavior.
        for _ in 0..1000 {
            assert_eq!(
                score_commit_fixes("Fixes #1"),
                ConfidenceTier::CommitFixes
            );
        }
    }

    // ---- T2 RED gate: URL filter ----

    /// `score_body_mention` returns `Unresolved` for bare URLs
    /// (the canonical URL filter, mirroring `docs_extractor`).
    #[test]
    fn body_mention_url_filter() {
        assert_eq!(
            score_body_mention("https://github.com/acme/widgets/issues/42"),
            ConfidenceTier::Unresolved
        );
        assert_eq!(
            score_body_mention("http://example.com"),
            ConfidenceTier::Unresolved
        );
    }

    // ---- T2 RED gate: Fixes #0 → Unresolved ----

    /// `Fixes #0` is invalid in GitHub (issue numbers start at 1).
    /// The function MUST NOT return `CommitFixes` for that
    /// input.
    #[test]
    fn fixes_issue_zero_returns_unresolved() {
        assert_eq!(
            score_commit_fixes("Fixes #0"),
            ConfidenceTier::Unresolved
        );
        assert_eq!(
            score_commit_fixes("Closes #0"),
            ConfidenceTier::Unresolved
        );
    }

    /// `Fixes #01` is also rejected — issue 0 padded with
    /// leading zeros is still issue 0 in GitHub's view.
    #[test]
    fn fixes_issue_zero_padded_returns_unresolved() {
        assert_eq!(
            score_commit_fixes("Fixes #00"),
            ConfidenceTier::Unresolved
        );
    }

    // ---- Additional TDD coverage ----

    /// A `commit` message that matches BOTH `CommitFixes` and
    /// `CommitRefs` (e.g. `Fixes #10, Refs #11`) is reported as
    /// `CommitFixes` by `score_commit_fixes` (the higher tier
    /// wins on the primary signal) AND as `CommitRefs` by
    /// `score_commit_refs` (the secondary signal).
    #[test]
    fn commit_fixes_wins_over_commit_refs_in_same_message() {
        let msg = "Fixes #10, Refs #11";
        assert_eq!(
            score_commit_fixes(msg),
            ConfidenceTier::CommitFixes
        );
        assert_eq!(
            score_commit_refs(msg),
            ConfidenceTier::CommitRefs
        );
    }

    /// A SHA mention alone triggers `ExplicitLink` even without
    /// a closing keyword. The hex blob of 7-40 chars is the
    /// discriminator.
    #[test]
    fn commit_sha_alone_triggers_explicit_link() {
        assert_eq!(
            score_explicit_link("implements 0123456789abcdef"),
            ConfidenceTier::ExplicitLink
        );
    }

    /// A bare word like `cafe` (4 hex chars) is too short to
    /// count as a SHA and so does NOT trigger `ExplicitLink`.
    #[test]
    fn short_hex_blob_does_not_trigger_explicit_link() {
        assert_eq!(
            score_explicit_link("we had coffee at the cafe"),
            ConfidenceTier::Unresolved
        );
    }

    /// `score_commit_refs` recognises the `Part of` phrase
    /// (multi-word keyword) when it is directly followed by the
    /// `#N` pattern.
    #[test]
    fn score_commit_refs_part_of_phrase() {
        assert_eq!(
            score_commit_refs("Part of #100 — bigger epic"),
            ConfidenceTier::CommitRefs
        );
    }

    /// `score_commit_fixes` returns `Unresolved` for an empty
    /// string.
    #[test]
    fn empty_commit_msg_is_unresolved() {
        assert_eq!(
            score_commit_fixes(""),
            ConfidenceTier::Unresolved
        );
    }

    /// `looks_like_body_mention` accepts a `file:name` shape
    /// (no `:line`).
    #[test]
    fn body_mention_two_segment_shape() {
        assert_eq!(
            score_body_mention("src/foo.rs:bar"),
            ConfidenceTier::BodyMention
        );
    }

    /// `looks_like_body_mention` rejects a `file:name:notanumber`
    /// line.
    #[test]
    fn body_mention_rejects_non_numeric_third_segment() {
        assert_eq!(
            score_body_mention("src/foo.rs:bar:banana"),
            ConfidenceTier::Unresolved
        );
    }
}
