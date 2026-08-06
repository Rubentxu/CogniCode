//! Intent lowering layer — translates lowercase query strings to MoldQL AST.
//!
//! This module provides the `lower_intent` function which acts as a
//! preprocessor at the `execute_query` facade boundary. It translates
//! natural-language-style lowercase query strings into the canonical
//! `MoldQLQuery` AST before they reach the uppercase parser.
//!
//! ## Supported patterns
//!
//! - `symbols where <condition>` → `FIND symbols WHERE <condition>`
//! - `calls from '<id>' [depth N]` → `EXPLORE <id> THROUGH callees DEPTH N`
//! - `match ... return ...` (lowercase Pattern Profile fragment) → `MoldQLQuery::Pattern(...)`
//!
//! Any input not matching these patterns falls through to the canonical
//! `parse()` function unchanged.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(unused_imports)]

use std::sync::OnceLock;

use regex::Regex;

use crate::moldql::parser;
use crate::moldql::parser_pattern_profile;
use crate::moldql::{MoldQLQuery, ParseError};

/// Regex for pattern 1: `symbols where <condition>`
static RE_SYMBOLS_WHERE: OnceLock<Regex> = OnceLock::new();

/// Regex for pattern 2: `calls from '<id>' [depth N]`
static RE_CALLS_FROM: OnceLock<Regex> = OnceLock::new();

/// Regex for pattern 3: lowercase `match ...` or `shortest match ...`
static RE_PATTERN_PROFILE: OnceLock<Regex> = OnceLock::new();

fn re_symbols_where() -> &'static Regex {
    RE_SYMBOLS_WHERE
        .get_or_init(|| Regex::new(r"^symbols\s+where\s+(.+)$").expect("regex is valid"))
}

fn re_calls_from() -> &'static Regex {
    RE_CALLS_FROM.get_or_init(|| {
        // Matches: calls from 'id' depth N  or  calls from "id" depth N  or  calls from id depth N
        Regex::new("^calls\\s+from\\s+['\"]?([^'\"\\s]+)['\"]?(?:\\s+depth\\s+(\\d+))?$")
            .expect("regex is valid")
    })
}

fn re_pattern_profile() -> &'static Regex {
    RE_PATTERN_PROFILE.get_or_init(|| {
        // GATE: only lowercase "match " / "shortest match " at start.
        // Conservative: rejects "MATCH" (uppercase), rejects other leading tokens.
        Regex::new(r"^(?:shortest\s+)?match\s+").expect("regex is valid")
    })
}

/// Mutating keywords that, if present anywhere in the query, should cause
/// the pattern to fall through to the canonical parser (which will surface
/// the proper UnsupportedConstruct error). This is a belt-and-suspenders
/// check — the parser also rejects mutations, but falling through at the
/// intent layer keeps the canonical parser as the single source of truth
/// for mutation diagnostics.
const MUTATION_KEYWORDS: &[&str] = &["create ", "delete ", "set ", "merge ", "detach ", "remove "];

/// Translates a lowercase intent query into a MoldQL AST.
///
/// Returns `Some(Ok(ast))` for supported lowercase patterns,
/// `Some(Err(parse_err))` if the rewritten form fails to parse,
/// or `None` if the input doesn't match any lowering pattern.
pub fn lower_intent(query: &str) -> Option<Result<MoldQLQuery, ParseError>> {
    // Pattern 1: symbols where <condition>
    if re_symbols_where().is_match(query) {
        let cap = re_symbols_where().captures(query).unwrap();
        let condition = &cap[1];
        let rewritten = format!("FIND symbols WHERE {condition}");
        return Some(parser::parse(&rewritten));
    }

    // Pattern 2: calls from '<id>' [depth N]
    if re_calls_from().is_match(query) {
        let cap = re_calls_from().captures(query).unwrap();
        let id = &cap[1];
        let depth = cap.get(2).map(|m| m.as_str()).unwrap_or("1");
        let depth: u32 = depth.parse().unwrap_or(1);
        let rewritten = format!("EXPLORE {id} THROUGH callees DEPTH {depth}");
        return Some(parser::parse(&rewritten));
    }

    // Pattern 3: lowercase Pattern Profile fragment (match ... return ...)
    // Only triggers when the query starts with lowercase "match " or "shortest match ".
    if re_pattern_profile().is_match(query) {
        // Belt-and-suspenders: reject mutating fragments early so they fall
        // through to the canonical parser which surfaces the proper diagnostic.
        let lower = query.to_ascii_lowercase();
        if MUTATION_KEYWORDS.iter().any(|k| lower.contains(k)) {
            return None;
        }
        // Canonical parser is NOT invoked — we lower directly via the pattern parser.
        let result = parser_pattern_profile::parse_pattern_query_from_str(query);
        return Some(result);
    }

    // No pattern matched — fall through to parse()
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Pattern 1: symbols where <condition>
    // =========================================================================

    #[test]
    fn test_symbols_where_lowercase_happy_path() {
        // GIVEN a query string `symbols where kind = "function"`
        // WHEN lower_intent is called
        // THEN it returns a MoldQLQuery::Find AST
        // AND the target is TargetType::Symbols
        // AND the conditions contain the single predicate `kind = "function"`
        let result = lower_intent("symbols where kind = \"function\"");
        assert!(
            matches!(result, Some(Ok(MoldQLQuery::Find(_)))),
            "expected Some(Ok(Find)), got {result:?}"
        );
    }

    #[test]
    fn test_symbols_where_condition_lowering() {
        // Verify the condition is properly preserved
        let result = lower_intent("symbols where fan_out > 5");
        assert!(result.is_some(), "expected Some, got None");
        let Ok(MoldQLQuery::Find(find)) = result.unwrap() else {
            panic!("expected Find variant");
        };
        assert_eq!(find.conditions.len(), 1, "expected 1 condition");
    }

    #[test]
    fn test_symbols_where_missing_condition_returns_none() {
        // GIVEN a query string `symbols where` (missing condition)
        // WHEN lower_intent is called
        // THEN it returns None
        let result = lower_intent("symbols where");
        assert!(
            result.is_none(),
            "expected None for malformed pattern, got {result:?}"
        );
    }

    // =========================================================================
    // Pattern 2: calls from '<id>' [depth N]
    // =========================================================================

    #[test]
    fn test_calls_from_with_explicit_depth() {
        // GIVEN a query string `calls from 'sym:42' depth 3`
        // WHEN lower_intent is called
        // THEN it returns a MoldQLQuery::Explore AST
        // AND object_ref is `sym:42`
        // AND direction is Direction::Callees
        // AND depth is 3
        let Some(Ok(MoldQLQuery::Explore(explore))) = lower_intent("calls from 'sym:42' depth 3")
        else {
            panic!(
                "expected Some(Ok(Explore)), got {:?}",
                lower_intent("calls from 'sym:42' depth 3")
            );
        };
        assert_eq!(explore.object_ref, "sym:42");
        assert_eq!(explore.depth, 3);
    }

    #[test]
    fn test_calls_from_without_depth_defaults_to_one() {
        // GIVEN a query string `calls from 'sym:42'`
        // WHEN lower_intent is called
        // THEN it returns a MoldQLQuery::Explore AST
        // AND depth is 1
        let Some(Ok(MoldQLQuery::Explore(explore))) = lower_intent("calls from 'sym:42'") else {
            panic!(
                "expected Some(Ok(Explore)), got {:?}",
                lower_intent("calls from 'sym:42'")
            );
        };
        assert_eq!(explore.depth, 1, "depth should default to 1");
    }

    #[test]
    fn test_calls_from_quoted_id() {
        // Verify quoted ID is handled
        let Some(Ok(MoldQLQuery::Explore(explore))) =
            lower_intent("calls from 'symbol:src/a.rs:a:1' depth 2")
        else {
            panic!(
                "expected Some(Ok(Explore)), got {:?}",
                lower_intent("calls from 'symbol:src/a.rs:a:1' depth 2")
            );
        };
        assert_eq!(explore.object_ref, "symbol:src/a.rs:a:1");
    }

    // =========================================================================
    // Fall-through patterns
    // =========================================================================

    #[test]
    fn test_uppercase_find_falls_through() {
        // GIVEN a query string `FIND symbols WHERE fan_out > 5`
        // WHEN lower_intent is called
        // THEN it returns None
        // AND the canonical parse() function handles this input
        let result = lower_intent("FIND symbols WHERE fan_out > 5");
        assert!(
            result.is_none(),
            "uppercase should return None, got {result:?}"
        );
    }

    #[test]
    fn test_unrecognized_lowercase_returns_none() {
        // Any lowercase pattern not matching the two supported forms should
        // return None so the facade falls through to parse()
        let result = lower_intent("symbols in scope src/");
        assert!(
            result.is_none(),
            "unrecognized pattern should return None, got {result:?}"
        );
    }

    // =========================================================================
    // Edge cases
    // =========================================================================

    #[test]
    fn test_empty_string_returns_none() {
        let result = lower_intent("");
        assert!(result.is_none(), "empty string should return None");
    }

    #[test]
    fn test_whitespace_only_returns_none() {
        let result = lower_intent("   ");
        assert!(result.is_none(), "whitespace should return None");
    }

    #[test]
    fn test_uppercase_symbols_where_returns_none() {
        // Case-sensitive: uppercase SYMBOLS WHERE should not match
        let result = lower_intent("SYMBOLS WHERE fan_out > 5");
        assert!(result.is_none(), "uppercase SYMBOLS should return None");
    }

    #[test]
    fn test_mixed_case_find_does_not_match() {
        // Mixed-case `find` should NOT match v1 (case-sensitive lowercase prefix)
        let result = lower_intent("find symbols where fan_out > 5");
        assert!(result.is_none(), "mixed-case find should return None");
    }

    // =========================================================================
    // Roundtrip tests — lower_intent output must parse through parser::parse()
    // =========================================================================

    #[test]
    fn test_roundtrip_symbols_where() {
        // lower_intent("symbols where fan_out > 5") must equal
        // Some(Ok(parser::parse("FIND symbols WHERE fan_out > 5")))
        use crate::moldql::parser;
        let lowered = lower_intent("symbols where fan_out > 5").unwrap();
        let parsed = parser::parse("FIND symbols WHERE fan_out > 5").unwrap();
        assert_eq!(lowered, Ok(parsed), "roundtrip mismatch for symbols where");
    }

    #[test]
    fn test_roundtrip_calls_from_depth() {
        // lower_intent("calls from 'symbol:a.rs:a:1' depth 2") must equal
        // Some(Ok(parser::parse("EXPLORE symbol:a.rs:a:1 THROUGH callees DEPTH 2")))
        use crate::moldql::parser;
        let lowered = lower_intent("calls from 'symbol:a.rs:a:1' depth 2").unwrap();
        let parsed = parser::parse("EXPLORE symbol:a.rs:a:1 THROUGH callees DEPTH 2").unwrap();
        assert_eq!(lowered, Ok(parsed), "roundtrip mismatch for calls from");
    }

    // =========================================================================
    // Pattern 3: lowercase match ... return ... (T4 — Pattern Profile lowering)
    // =========================================================================

    #[test]
    fn test_lowercase_pattern_lowers_directly() {
        // Scenario: Lowercase pattern lowers directly
        // GIVEN `match (r:Route)-[:Calls*1..3]->(f:Function) return path(r,f)`
        // WHEN lower_intent is called
        // THEN it returns Some(Ok(MoldQLQuery::Pattern(...)))
        // AND the canonical parser is NOT called.
        use crate::moldql::ast::PatternProjection;
        let result = lower_intent("match (r:Route)-[:Calls*1..3]->(f:Function) return path(r,f)");
        assert!(
            matches!(result, Some(Ok(MoldQLQuery::Pattern(_)))),
            "expected Some(Ok(Pattern)), got {result:?}"
        );
        let Some(Ok(MoldQLQuery::Pattern(pq))) = result else {
            return;
        };
        // Verify bindings: r and f (PATH only includes start/end nodes, not edge binding)
        assert_eq!(pq.bindings.len(), 2, "expected 2 bindings in PATH(r,f)");
        assert_eq!(pq.edges.len(), 1, "expected 1 edge");
        // Verify quantifier bound: *1..3
        let edge = &pq.edges[0];
        assert_eq!(edge.quantifier.max_hops, Some(3), "max_hops should be 3");
        assert_eq!(edge.quantifier.min_hops, 1, "min_hops should be 1");
        // Verify projection is Path with 2 bindings
        assert!(
            matches!(&pq.projection, PatternProjection::Path { bindings } if bindings.len() == 2),
            "expected Path projection with 2 bindings, got {:?}",
            pq.projection
        );
    }

    #[test]
    fn test_lowercase_mixed_unsupported_fragment_falls_through() {
        // Scenario: Mixed unsupported fragment falls through
        // GIVEN `match (f:Function) detach delete f`
        // WHEN lower_intent is called
        // THEN it returns None
        // AND the canonical parser surfaces the unsupported mutation diagnostic.
        let result = lower_intent("match (f:Function) detach delete f");
        assert!(
            result.is_none(),
            "mutation fragment should return None, got {result:?}"
        );
    }

    #[test]
    fn test_lowercase_aggregate_and_limit() {
        // Scenario: Lower aggregate and limit
        // Uses uppercase COUNT (lowercase count is a parser edge case).
        // Tests: + maps to 1..8, ordering and limit are preserved.
        use crate::moldql::ast::{OrderDirection, PatternProjection};
        let result = lower_intent(
            "match (f:Function)-[c:Calls+]->(g:Function) return COUNT(c) AS calls ORDER BY calls DESC LIMIT 5",
        );
        assert!(
            matches!(result, Some(Ok(MoldQLQuery::Pattern(_)))),
            "expected Some(Ok(Pattern)), got {result:?}"
        );
        let Some(Ok(MoldQLQuery::Pattern(pq))) = result else {
            return;
        };
        // Verify + maps to 1..8 (profile max hops default = 8)
        assert_eq!(pq.edges.len(), 1);
        let edge = &pq.edges[0];
        assert_eq!(edge.quantifier.min_hops, 1, "+ should map to min_hops=1");
        assert_eq!(
            edge.quantifier.max_hops,
            Some(8),
            "+ should map to max_hops=8"
        );
        // Verify projection has ordering and limit
        if let PatternProjection::Row {
            ordering, limit, ..
        } = &pq.projection
        {
            assert!(ordering.is_some(), "expected ordering");
            let ord = ordering.as_ref().unwrap();
            assert!(
                matches!(ord.direction, OrderDirection::Desc),
                "expected DESC"
            );
            assert_eq!(ord.by, "calls", "order by calls");
            assert_eq!(*limit, Some(5), "limit should be 5");
        } else {
            panic!("expected Row projection, got {:?}", pq.projection);
        }
    }

    #[test]
    fn test_lowercase_optional_relationship() {
        // Scenario: Lower optional relationship
        // GIVEN `match (f:Function)-[:Calls?]->(x:Function) return node(x)`
        // WHEN lowered
        // THEN its relationship bound is 0..1.
        use crate::moldql::ast::PatternProjection;
        let result = lower_intent("match (f:Function)-[:Calls?]->(x:Function) return node(x)");
        assert!(
            matches!(result, Some(Ok(MoldQLQuery::Pattern(_)))),
            "expected Some(Ok(Pattern)), got {result:?}"
        );
        let Some(Ok(MoldQLQuery::Pattern(pq))) = result else {
            return;
        };
        // Verify ? maps to 0..1
        assert_eq!(pq.edges.len(), 1);
        let edge = &pq.edges[0];
        assert_eq!(edge.quantifier.min_hops, 0, "? should map to min_hops=0");
        assert_eq!(
            edge.quantifier.max_hops,
            Some(1),
            "? should map to max_hops=1"
        );
        // Verify node(x) projection
        assert!(
            matches!(&pq.projection, PatternProjection::Node { binding } if binding == "x"),
            "expected Node(x) projection, got {:?}",
            pq.projection
        );
    }

    #[test]
    fn test_lowercase_shortest_pattern() {
        // Scenario: SHORTEST keyword is preserved in lowering
        // Grammar: [SHORTEST] MATCH NodePattern — SHORTEST comes BEFORE MATCH.
        use crate::moldql::ast::PatternProjection;
        let result =
            lower_intent("shortest match (r:Route)-[:Calls*1..6]->(f:Function) return path(r,f)");
        assert!(
            matches!(result, Some(Ok(MoldQLQuery::Pattern(_)))),
            "expected Some(Ok(Pattern)), got {result:?}"
        );
        let Some(Ok(MoldQLQuery::Pattern(pq))) = result else {
            return;
        };
        assert!(pq.shortest, "SHORTEST keyword should be preserved");
    }

    #[test]
    fn test_existing_intent_lowering_unchanged() {
        // Scenario: Existing intent lowering is unchanged
        // GIVEN `symbols where kind = "function"` or `calls from 'sym:42' depth 3`
        // WHEN lower_intent is called
        // THEN each produces the same existing AST and defaults as before.
        use crate::moldql::parser;

        // symbols where — unchanged
        let symbols = lower_intent("symbols where kind = \"function\"").unwrap();
        let symbols_parsed = parser::parse("FIND symbols WHERE kind = \"function\"").unwrap();
        assert_eq!(
            symbols,
            Ok(symbols_parsed),
            "symbols where roundtrip broken"
        );

        // calls from — unchanged
        let calls = lower_intent("calls from 'sym:42' depth 3").unwrap();
        let calls_parsed = parser::parse("EXPLORE sym:42 THROUGH callees DEPTH 3").unwrap();
        assert_eq!(calls, Ok(calls_parsed), "calls from roundtrip broken");
    }

    #[test]
    fn test_uppercase_match_falls_through() {
        // Uppercase MATCH should NOT be handled by lower_intent — it goes
        // through the canonical parser.
        let result = lower_intent("MATCH (f:Function) return node(f)");
        assert!(
            result.is_none(),
            "uppercase MATCH should return None, got {result:?}"
        );
    }

    #[test]
    fn test_mixed_case_match_falls_through() {
        // Mixed-case `Match` should NOT be handled — only lowercase.
        let result = lower_intent("Match (f:Function) return node(f)");
        assert!(
            result.is_none(),
            "mixed-case Match should return None, got {result:?}"
        );
    }
}
