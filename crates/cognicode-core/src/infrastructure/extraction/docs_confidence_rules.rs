//! `DocsConfidenceRules` — pure functions that map a docs-derived
//! reference (a markdown link, a heading match) to a confidence
//! score in `[0.0, 1.0]`.
//!
//! The 4-tier rule table is fixed by the spec; the [`Provenance`]
//! tag in [`EdgeKind`] is also derived from the tier so the
//! persistence layer can filter ambiguous edges out of the
//! canonical graph. The full table:
//!
//! | Rule          | Confidence | Provenance  | Trigger                                  |
//! |---------------|-----------:|-------------|------------------------------------------|
//! | `link_exact`  | 0.9        | `Extracted` | `link_text` is a case-insensitive exact match of a target symbol's last path component. |
//! | `link_fuzzy`  | 0.6        | `Ambiguous` | `link_text` is a case-insensitive substring of a target symbol.  |
//! | `heading_match` | 0.7      | `Extracted` | `heading` is a case-insensitive substring of `context`.          |
//! | `unresolved`  | 0.3        | `Ambiguous` | None of the above apply.                  |
//!
//! The module is `#[cfg(feature = "multimodal")]`-gated because it
//! is part of the docs-source adapter pipeline. The default build
//! does not pull in the extraction dependencies.

#[cfg(feature = "multimodal")]
use crate::domain::aggregates::SymbolId;
#[cfg(feature = "multimodal")]
use crate::domain::value_objects::provenance::Provenance;

/// The four confidence tiers the spec fixes. Each tier is bound
/// to a stable confidence value and a [`Provenance`] tag so the
/// [`crate::infrastructure::extraction::docs_extractor`] can emit
/// a [`crate::domain::aggregates::generic_graph::GraphEdge`]
/// without re-deciding the rule.
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceTier {
    /// `link_exact` — 0.9, `Extracted`.
    LinkExact,
    /// `link_fuzzy` — 0.6, `Ambiguous`.
    LinkFuzzy,
    /// `heading_match` — 0.7, `Extracted`.
    HeadingMatch,
    /// `unresolved` — 0.3, `Ambiguous`.
    Unresolved,
}

#[cfg(feature = "multimodal")]
impl ConfidenceTier {
    /// The fixed confidence value the spec binds to this tier.
    pub const fn confidence(self) -> f64 {
        match self {
            ConfidenceTier::LinkExact => 0.9,
            ConfidenceTier::LinkFuzzy => 0.6,
            ConfidenceTier::HeadingMatch => 0.7,
            ConfidenceTier::Unresolved => 0.3,
        }
    }

    /// The fixed [`Provenance`] tag the spec binds to this tier.
    pub const fn provenance(self) -> Provenance {
        match self {
            ConfidenceTier::LinkExact => Provenance::Extracted,
            ConfidenceTier::LinkFuzzy => Provenance::Ambiguous,
            ConfidenceTier::HeadingMatch => Provenance::Extracted,
            ConfidenceTier::Unresolved => Provenance::Ambiguous,
        }
    }
}

/// Score a markdown link against a list of known code symbols.
///
/// Returns a `(confidence, provenance, matched_symbol_id)` triple
/// that the extractor can plug directly into a
/// [`crate::domain::aggregates::generic_graph::GraphEdge`]:
///
/// - `LinkExact` (0.9, `Extracted`) when `link_text` matches a
///   target symbol's last `:`-segment (the symbol's short name) on
///   a case-insensitive basis. The matched symbol id is returned.
/// - `LinkFuzzy` (0.6, `Ambiguous`) when `link_text` is a
///   case-insensitive substring of a target symbol's full id.
///   The first substring match wins.
/// - `Unresolved` (0.3, `Ambiguous`) when no target matches. The
///   `matched_symbol_id` is `None`.
///
/// The link text is trimmed of leading/trailing whitespace and
/// backticks (the markdown link target is the bare identifier
/// inside the `[text](id)` form, so a backtick-wrapped link like
/// `` [`foo`] `` still resolves to `foo`).
#[cfg(feature = "multimodal")]
pub fn score_link(
    link_text: &str,
    target_symbols: &[SymbolId],
) -> (ConfidenceTier, Option<SymbolId>) {
    let needle = normalise_link(link_text);
    if needle.is_empty() {
        return (ConfidenceTier::Unresolved, None);
    }
    // Tier 1: exact match against the symbol's short name (the
    // last `:`-segment of the canonical `file:name:line` id).
    for sym in target_symbols {
        let short = sym_short_name(sym);
        if short.eq_ignore_ascii_case(&needle) {
            return (ConfidenceTier::LinkExact, Some(sym.clone()));
        }
    }
    // Tier 2: substring match against the full id.
    for sym in target_symbols {
        if sym.as_str().to_ascii_lowercase().contains(&needle) {
            return (ConfidenceTier::LinkFuzzy, Some(sym.clone()));
        }
    }
    (ConfidenceTier::Unresolved, None)
}

/// Score a heading against the surrounding textual context.
///
/// Returns the [`ConfidenceTier`] the heading falls into:
///
/// - `HeadingMatch` (0.7, `Extracted`) when `heading` is a
///   case-insensitive substring of `context` (i.e. the heading
///   names an entity that the surrounding body also names).
/// - `Unresolved` (0.3, `Ambiguous`) otherwise.
///
/// The function does NOT consult the symbol table — it is a
/// textual affinity check. The docs-source adapter uses the
/// returned tier to set the edge's confidence + provenance; the
/// actual symbol resolution happens later (via the graph_search
/// FTS5 path or a future fuzzy matcher).
#[cfg(feature = "multimodal")]
pub fn score_heading(heading: &str, context: &str) -> ConfidenceTier {
    let needle = normalise_link(heading);
    if needle.is_empty() {
        return ConfidenceTier::Unresolved;
    }
    if context.to_ascii_lowercase().contains(&needle) {
        ConfidenceTier::HeadingMatch
    } else {
        ConfidenceTier::Unresolved
    }
}

/// Convenience: `score_link`'s confidence, returned as `f64`.
/// Equivalent to `score_link(..).0.confidence()`. Provided so the
/// call site can stay on the existing `f64` shape that
/// `GraphEdge::new` accepts.
#[cfg(feature = "multimodal")]
pub fn link_confidence(link_text: &str, target_symbols: &[SymbolId]) -> f64 {
    score_link(link_text, target_symbols).0.confidence()
}

/// Convenience: `score_heading`'s confidence, returned as `f64`.
#[cfg(feature = "multimodal")]
pub fn heading_confidence(heading: &str, context: &str) -> f64 {
    score_heading(heading, context).confidence()
}

/// Trim whitespace, ASCII backticks, and lower-case the link
/// text. Markdown link targets in our docs use the form
/// `[text](target)` where `target` is the bare identifier — but
/// autolinks and the legacy `` `[foo]` `` form may wrap the
/// identifier in backticks. We strip the wrapper uniformly.
#[cfg(feature = "multimodal")]
fn normalise_link(s: &str) -> String {
    s.trim()
        .trim_matches('`')
        .trim()
        .to_ascii_lowercase()
}

/// The "short name" of a `SymbolId` is the segment after the LAST
/// `:` (i.e. the symbol's bare name, not its file:line prefix).
/// For the canonical `file:name:line` shape, the last segment is
/// `line`; for the `file:name` shape, it's `name`. We pick the
/// penultimate segment so the exact-match tier hits the symbol's
/// actual name (`name`), not its line number.
#[cfg(feature = "multimodal")]
pub(crate) fn sym_short_name(s: &SymbolId) -> String {
    let raw = s.as_str();
    // Walk from the right; the last segment is the line number,
    // the penultimate is the symbol name. Split only on `:` so
    // Windows paths (containing `:`) stay intact in the head
    // (we only inspect the trailing two segments).
    if let Some(last_colon) = raw.rfind(':') {
        let head = &raw[..last_colon];
        if let Some(second_colon) = head.rfind(':') {
            head[second_colon + 1..].to_ascii_lowercase()
        } else {
            // No second colon — `head` IS the symbol name.
            head.to_ascii_lowercase()
        }
    } else {
        // No colon at all — the whole id is the name.
        raw.to_ascii_lowercase()
    }
}

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;

    // ---- T11 RED gate ----

    /// `score_link("foo", [SymbolId("src/a.rs:foo:1")])` must
    /// return the `LinkExact` tier (0.9) and the matched id.
    #[test]
    fn exact_link_scores_0_9() {
        let targets = vec![SymbolId::new("src/a.rs:foo:1")];
        let (tier, matched) = score_link("foo", &targets);
        assert_eq!(tier, ConfidenceTier::LinkExact);
        assert_eq!(tier.confidence(), 0.9);
        assert_eq!(matched.as_ref().map(SymbolId::as_str), Some("src/a.rs:foo:1"));
    }

    /// `score_link("foo", [SymbolId("src/other.rs:foobar:2")])`
    /// must return `LinkFuzzy` (0.6) because `foo` is a
    /// case-insensitive substring of `foobar`.
    #[test]
    fn fuzzy_link_scores_0_6() {
        let targets = vec![SymbolId::new("src/other.rs:foobar:2")];
        let (tier, matched) = score_link("foo", &targets);
        assert_eq!(tier, ConfidenceTier::LinkFuzzy);
        assert_eq!(tier.confidence(), 0.6);
        assert_eq!(
            matched.as_ref().map(SymbolId::as_str),
            Some("src/other.rs:foobar:2")
        );
    }

    /// `score_heading("Authentication", "...see Authentication
    /// for details...")` must return `HeadingMatch` (0.7).
    #[test]
    fn heading_match_scores_0_7() {
        let context = "see Authentication for details about the login flow.";
        let tier = score_heading("Authentication", context);
        assert_eq!(tier, ConfidenceTier::HeadingMatch);
        assert_eq!(tier.confidence(), 0.7);
    }

    /// `score_link("ghost", [])` (no targets) and
    /// `score_heading("Nonexistent", "totally unrelated")` must
    /// both return `Unresolved` (0.3).
    #[test]
    fn unresolved_scores_0_3() {
        // No targets at all -> unresolved.
        let (tier, matched) = score_link("ghost", &[]);
        assert_eq!(tier, ConfidenceTier::Unresolved);
        assert_eq!(tier.confidence(), 0.3);
        assert!(matched.is_none());

        // Targets present, but the link doesn't substring-match.
        let targets = vec![SymbolId::new("src/a.rs:foo:1")];
        let (tier, matched) = score_link("ghost", &targets);
        assert_eq!(tier, ConfidenceTier::Unresolved);
        assert_eq!(tier.confidence(), 0.3);
        assert!(matched.is_none());

        // Heading affinity misses too.
        let tier = score_heading("Nonexistent", "totally unrelated text");
        assert_eq!(tier, ConfidenceTier::Unresolved);
        assert_eq!(tier.confidence(), 0.3);
    }

    // ---- Additional TDD coverage ----

    /// Provenance tags must match the spec table.
    #[test]
    fn tier_provenance_matches_spec() {
        assert_eq!(ConfidenceTier::LinkExact.provenance(), Provenance::Extracted);
        assert_eq!(ConfidenceTier::LinkFuzzy.provenance(), Provenance::Ambiguous);
        assert_eq!(ConfidenceTier::HeadingMatch.provenance(), Provenance::Extracted);
        assert_eq!(
            ConfidenceTier::Unresolved.provenance(),
            Provenance::Ambiguous
        );
    }

    /// Case-insensitive matching for `score_link`.
    #[test]
    fn score_link_is_case_insensitive() {
        let targets = vec![SymbolId::new("src/a.rs:MyFunc:1")];
        let (tier, _) = score_link("MYFUNC", &targets);
        assert_eq!(tier, ConfidenceTier::LinkExact);

        let (tier, _) = score_link("myfunc", &targets);
        assert_eq!(tier, ConfidenceTier::LinkExact);

        let (tier, _) = score_link("MyFunc", &targets);
        assert_eq!(tier, ConfidenceTier::LinkExact);
    }

    /// Backtick-wrapped links (e.g. `` [`foo`] ``) must
    /// normalise to the bare identifier before matching.
    #[test]
    fn score_link_strips_backticks() {
        let targets = vec![SymbolId::new("src/a.rs:foo:1")];
        let (tier, _) = score_link("`foo`", &targets);
        assert_eq!(tier, ConfidenceTier::LinkExact);
    }

    /// Empty / whitespace-only link text must be `Unresolved`
    /// (don't accidentally match the empty needle against every
    /// target's id).
    #[test]
    fn score_link_empty_input_is_unresolved() {
        let targets = vec![SymbolId::new("src/a.rs:foo:1")];
        assert_eq!(score_link("", &targets).0, ConfidenceTier::Unresolved);
        assert_eq!(score_link("   ", &targets).0, ConfidenceTier::Unresolved);
        assert_eq!(score_link("``", &targets).0, ConfidenceTier::Unresolved);
    }

    /// When multiple targets match exactly, the first one wins.
    /// (The contract says "at most one" exact match per call —
    /// the extractor guarantees target_symbols is deduped upstream,
    /// so this is a defensive ordering assertion.)
    #[test]
    fn score_link_first_exact_match_wins() {
        let targets = vec![
            SymbolId::new("src/a.rs:foo:1"),
            SymbolId::new("src/b.rs:foo:2"),
        ];
        let (tier, matched) = score_link("foo", &targets);
        assert_eq!(tier, ConfidenceTier::LinkExact);
        assert_eq!(matched.as_ref().map(SymbolId::as_str), Some("src/a.rs:foo:1"));
    }

    /// Exact match is checked BEFORE fuzzy match. `score_link("foo",
    /// [SymbolId("src/x.rs:foo:1"), SymbolId("src/y.rs:foobar:2")])`
    /// must return `LinkExact` (matching the first symbol), not
    /// `LinkFuzzy` (which would also match the first).
    #[test]
    fn exact_takes_precedence_over_fuzzy() {
        let targets = vec![
            SymbolId::new("src/x.rs:foo:1"),
            SymbolId::new("src/y.rs:foobar:2"),
        ];
        let (tier, matched) = score_link("foo", &targets);
        assert_eq!(tier, ConfidenceTier::LinkExact);
        assert_eq!(matched.as_ref().map(SymbolId::as_str), Some("src/x.rs:foo:1"));
    }

    /// Convenience helpers return the same confidence as the
    /// tier's `confidence()` method.
    #[test]
    fn convenience_helpers_match_tier_confidence() {
        let targets = vec![SymbolId::new("src/a.rs:foo:1")];
        assert_eq!(link_confidence("foo", &targets), 0.9);
        assert_eq!(link_confidence("ghost", &targets), 0.3);
        assert_eq!(heading_confidence("Auth", "see Auth for details"), 0.7);
        assert_eq!(heading_confidence("Auth", "totally unrelated"), 0.3);
    }
}
