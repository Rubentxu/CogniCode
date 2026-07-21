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
//!
//! Any input not matching these patterns falls through to the canonical
//! `parse()` function unchanged.

use std::sync::OnceLock;

use regex::Regex;

use crate::moldql::parser;
use crate::moldql::{MoldQLQuery, ParseError};

/// Regex for pattern 1: `symbols where <condition>`
static RE_SYMBOLS_WHERE: OnceLock<Regex> = OnceLock::new();

/// Regex for pattern 2: `calls from '<id>' [depth N]`
static RE_CALLS_FROM: OnceLock<Regex> = OnceLock::new();

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

/// Translates a lowercase intent query into a MoldQL AST.
///
/// Returns `Some(Ok(ast))` for supported lowercase patterns,
/// `Some(Err(parse_err))` if the rewritten form fails to parse,
/// or `None` if the input doesn't match any lowering pattern.
pub fn lower_intent(query: &str) -> Option<Result<MoldQLQuery, ParseError>> {
    // Pattern 1: symbols where <condition>
    if let Some(captures) = re_symbols_where().captures(query) {
        let condition = &captures[1];
        let rewritten = format!("FIND symbols WHERE {condition}");
        return Some(parser::parse(&rewritten));
    }

    // Pattern 2: calls from '<id>' [depth N]
    if let Some(captures) = re_calls_from().captures(query) {
        let id = &captures[1];
        let depth = captures.get(2).map(|m| m.as_str()).unwrap_or("1");
        let depth: u32 = depth.parse().unwrap_or(1);
        let rewritten = format!("EXPLORE {id} THROUGH callees DEPTH {depth}");
        return Some(parser::parse(&rewritten));
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
}
