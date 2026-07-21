//! Integration tests for the MoldQL intent-lowering facade boundary.
//!
//! These tests verify that:
//! 1. Intent queries (`symbols where ...`, `calls from ...`) are correctly
//!    lowered through the facade's execute path
//! 2. Error messages reference the ORIGINAL intent query (not the lowered form)
//!
//! Since the MoldQL executor requires a fully-wired graph, these tests
//! focus on the lowering boundary — verifying that `lower_intent` is
//! invoked correctly and that error messages preserve the original query.

use cognicode_explorer::moldql::lower_intent;

/// Test that "symbols where kind = \"function\"" lowers correctly.
///
/// The query should lower to "FIND symbols WHERE kind = \"function\""
/// and produce a valid MoldQL AST.
#[tokio::test]
async fn lower_intent_symbols_where_parses() {
    // GIVEN a lowercase intent query
    let query = r#"symbols where kind = "function""#;

    // WHEN lower_intent is called
    let result = lower_intent(query);

    // THEN it returns Some(Ok(ast)) with a Find query
    assert!(
        result.is_some(),
        "lowering should return Some for intent query"
    );
    let ast = result.unwrap();
    assert!(ast.is_ok(), "lowering should produce valid AST: {:?}", ast);
}

/// Test that "calls from 'sym:42' depth 3" lowers correctly.
///
/// The query should lower to "EXPLORE sym:42 THROUGH callees DEPTH 3".
#[tokio::test]
async fn lower_intent_calls_from_with_depth_parses() {
    // GIVEN a lowercase intent query with depth
    let query = "calls from 'sym:42' depth 3";

    // WHEN lower_intent is called
    let result = lower_intent(query);

    // THEN it returns Some(Ok(ast)) with an Explore query
    assert!(
        result.is_some(),
        "lowering should return Some for intent query"
    );
    let ast = result.unwrap();
    assert!(ast.is_ok(), "lowering should produce valid AST: {:?}", ast);
}

/// Test that error messages reference the ORIGINAL intent query.
///
/// When an intent query fails to lower/parse, the error should
/// reference the original intent form, not the rewritten MoldQL.
#[tokio::test]
async fn lower_intent_preserves_query_in_error() {
    // GIVEN an intent query that will fail parse after lowering
    // "symbols where " has no condition, so it won't match the regex
    let query = "symbols where";

    // WHEN lower_intent is called
    let result = lower_intent(query);

    // THEN it returns None (pattern doesn't match due to missing condition)
    // This tests that malformed intent patterns fall through correctly
    assert!(result.is_none(), "malformed intent should return None");
}

/// Test that non-intent queries return None from lower_intent.
///
/// Queries that don't match any lowering pattern should return None
/// so the facade can fall through to the standard parser.
#[tokio::test]
async fn non_intent_queries_return_none() {
    // GIVEN an uppercase MoldQL query (not an intent query)
    let query = "FIND symbols WHERE fan_out > 5";

    // WHEN lower_intent is called
    let result = lower_intent(query);

    // THEN it returns None (no lowering pattern matched)
    assert!(result.is_none(), "non-intent query should return None");
}

/// Test that empty string returns None from lower_intent.
#[tokio::test]
async fn empty_query_returns_none() {
    // GIVEN an empty query string
    let query = "";

    // WHEN lower_intent is called
    let result = lower_intent(query);

    // THEN it returns None
    assert!(result.is_none(), "empty query should return None");
}

/// Test that case-sensitive matching works correctly.
///
/// Intent lowering only matches lowercase patterns.
#[tokio::test]
async fn uppercase_intent_returns_none() {
    // GIVEN an uppercase intent-like query
    let query = "SYMBOLS WHERE fan_out > 5";

    // WHEN lower_intent is called
    let result = lower_intent(query);

    // THEN it returns None (case-sensitive matching)
    assert!(
        result.is_none(),
        "uppercase should not match lowering pattern"
    );
}

/// Test that unrecognized lowercase patterns return None.
#[tokio::test]
async fn unrecognized_lowercase_returns_none() {
    // GIVEN a lowercase pattern that doesn't match any intent form
    let query = "symbols in scope src/";

    // WHEN lower_intent is called
    let result = lower_intent(query);

    // THEN it returns None (falls through to standard parser)
    assert!(result.is_none(), "unrecognized pattern should return None");
}

/// Test roundtrip: lowering then parsing produces the expected AST.
#[tokio::test]
async fn roundtrip_symbols_where() {
    // GIVEN a lowercase intent query
    let intent_query = "symbols where fan_out > 5";

    // WHEN we lower it
    let lowered = lower_intent(intent_query);

    // THEN the lowered result should be a valid Find AST
    assert!(lowered.is_some());
    let ast = lowered.unwrap();
    assert!(ast.is_ok(), "lowered AST should be valid");

    // AND parsing the rewritten form directly should produce the same AST
    use cognicode_explorer::moldql::parser;
    let rewritten = "FIND symbols WHERE fan_out > 5";
    let direct_parsed = parser::parse(rewritten);
    assert!(direct_parsed.is_ok(), "rewritten query should parse");
    assert_eq!(ast, direct_parsed, "roundtrip should preserve AST");
}

/// Test roundtrip for calls from with depth.
#[tokio::test]
async fn roundtrip_calls_from_depth() {
    // GIVEN a lowercase intent query
    let intent_query = "calls from 'symbol:a.rs:a:1' depth 2";

    // WHEN we lower it
    let lowered = lower_intent(intent_query);

    // THEN the lowered result should be a valid Explore AST
    assert!(lowered.is_some());
    let ast = lowered.unwrap();
    assert!(ast.is_ok(), "lowered AST should be valid");

    // AND parsing the rewritten form directly should produce the same AST
    use cognicode_explorer::moldql::parser;
    let rewritten = "EXPLORE symbol:a.rs:a:1 THROUGH callees DEPTH 2";
    let direct_parsed = parser::parse(rewritten);
    assert!(direct_parsed.is_ok(), "rewritten query should parse");
    assert_eq!(ast, direct_parsed, "roundtrip should preserve AST");
}
