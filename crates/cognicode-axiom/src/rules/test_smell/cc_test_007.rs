//! CC_TEST_007: Duplicated Test Method
//!
//! Detects when two test functions have nearly identical bodies.
//!
//! # Problem
//! Duplicated tests waste maintenance effort and can indicate
//! that tests should be parameterized or use shared setup.
//!
//! # Fix
//! - Convert to parameterized tests
//! - Extract common setup into shared fixtures
//! - Use table-driven test patterns

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{SrcLanguage, Rule, RuleId};
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;

/// Normalize source text for comparison (remove whitespace variations)
fn normalize_for_comparison(source: &str) -> String {
    source
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Calculate similarity between two normalized texts (0.0 to 1.0)
fn calculate_similarity(text1: &str, text2: &str) -> f64 {
    let norm1 = normalize_for_comparison(text1);
    let norm2 = normalize_for_comparison(text2);

    if norm1.is_empty() && norm2.is_empty() {
        return 1.0;
    }
    if norm1.is_empty() || norm2.is_empty() {
        return 0.0;
    }

    let len1 = norm1.len();
    let len2 = norm2.len();

    // Simple Levenshtein distance
    let mut matrix = vec![vec![0usize; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    let chars1: Vec<char> = norm1.chars().collect();
    let chars2: Vec<char> = norm2.chars().collect();

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    let distance = matrix[len1][len2];
    let max_len = len1.max(len2);

    if max_len == 0 {
        1.0
    } else {
        1.0 - (distance as f64 / max_len as f64)
    }
}

/// CC_TEST_007 Rule: Duplicated Test Method
pub struct DuplicatedTestRule {
    /// Similarity threshold (0.0 to 1.0)
    threshold: f64,
}

impl Default for DuplicatedTestRule {
    fn default() -> Self {
        Self { threshold: 0.9 }
    }
}

impl DuplicatedTestRule {
    #[allow(dead_code)]
    pub fn new(threshold: f64) -> Self {
        Self { threshold }
    }
}

impl Rule for DuplicatedTestRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_007")
    }

    fn name(&self) -> &'static str {
        "Duplicated Test Method"
    }

    fn description(&self) -> &'static str {
        "Detects when two test functions have nearly identical bodies"
    }

    fn category(&self) -> Category {
        Category::TestSmell
    }

    fn severity(&self) -> Severity {
        Severity::Minor
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Python, SrcLanguage::JavaScript]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Build query based on language
        let test_query = match ctx.language {
            SrcLanguage::Python => {
                r#"(function_definition
                    name: (identifier) @test_name
                    body: (block) @test_body
                    (#match? @test_name "^test_"))"#
            }
            SrcLanguage::JavaScript => {
                r#"(call_expression
                    function: (identifier) @test_func
                    arguments: (arguments (arrow_function
                        body: (block) @test_body))
                    (#match? @test_func "^test$|^it$"))"#
            }
            _ => return issues,
        };

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, test_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Collect test functions with their bodies
        let mut test_functions: HashMap<String, (usize, String)> = HashMap::new();

        while let Some(m) = matches.next() {
            let mut name_node = None;
            let mut body_node = None;

            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                let field_str: &str = field_name;
                match field_str {
                    "test_name" | "test_func" => name_node = Some(cap.node),
                    "test_body" => body_node = Some(cap.node),
                    _ => {}
                }
            }

            if let (Some(name), Some(body)) = (name_node, body_node) {
                let name_text = name.utf8_text(source.as_bytes()).unwrap_or("");
                let body_text = body.utf8_text(source.as_bytes()).unwrap_or("");
                let start = body.start_position();

                test_functions.insert(
                    name_text.to_string(),
                    (start.row + 1, body_text.to_string()),
                );
            }
        }

        // Compare pairs of test functions for similarity
        let names: Vec<_> = test_functions.keys().collect();
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                let name1 = names[i];
                let name2 = names[j];

                // Skip if names are too similar (likely intentional variants)
                let similarity = calculate_similarity(name1, name2);
                if similarity > 0.8 {
                    continue; // Likely intentional naming like test_user_1, test_user_2
                }

                let (line1, body1) = &test_functions[name1];
                let (_, body2) = &test_functions[name2];

                let body_similarity = calculate_similarity(body1, body2);

                if body_similarity >= self.threshold {
                    issues.push(Issue::new(
                        "CC_TEST_007",
                        "Duplicated Test Method",
                        Severity::Minor,
                        Category::TestSmell,
                        ctx.file_path.to_string_lossy(),
                        *line1,
                        0,
                        format!(
                            "Test functions '{}' and '{}' have {:.0}% similar bodies. \
                             Consider using parameterized tests or extracting common setup.",
                            name1, name2, body_similarity * 100.0
                        ),
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["def test_", "function test_", "it(", "test("])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_code(code: &str, language: SrcLanguage) -> (tree_sitter::Tree, String) {
        let lang = language.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        (tree, code.to_string())
    }

    fn check_rule(code: &str, language: SrcLanguage) -> Vec<Issue> {
        let (tree, source) = parse_code(code, language);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.py"),
            &language,
            &metrics,
        );
        let rule = DuplicatedTestRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_duplicated_tests_python() {
        let code = r#"
def test_login():
    user = authenticate('user', 'pass')
    assertEqual(user.token, 'valid')

def test_login_copy():
    user = authenticate('user', 'pass')
    assertEqual(user.token, 'valid')
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect duplicated test methods");
        assert_eq!(issues[0].rule_id, "CC_TEST_007");
    }

    #[test]
    fn test_no_false_positive_on_different_assertions() {
        let code = r#"
def test_login_success():
    assertEqual(login('user', 'pass'), 'ok')

def test_login_failure():
    assertEqual(login('user', 'wrong'), 'denied')
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag tests with different assertions");
    }

    #[test]
    fn test_similarity_calculation() {
        // Identical texts
        let text1 = "assertEqual(x, 1) assertEqual(y, 2)";
        let text2 = "assertEqual(x, 1) assertEqual(y, 2)";

        // Test identical
        let sim_identical = calculate_similarity(text1, text2);
        assert!(sim_identical > 0.99, "Identical texts should have >99% similarity, got {}", sim_identical);

        // Test different - using very different strings
        let text3 = "function testFoo() { return 42; }";
        let text4 = "class MyClass { constructor(x) { this.x = x; } }";
        let sim_different = calculate_similarity(text3, text4);
        assert!(sim_different < 0.3, "Completely different texts should have <30% similarity, got {}", sim_different);
    }
}
