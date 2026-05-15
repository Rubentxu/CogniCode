//! CC_CS_007: Function Names Should Follow snake_case Convention
//!
//! Detects functions that don't follow Rust's snake_case naming convention.
//!
//! # Problem
//! Rust convention is snake_case for function and method names. Using
//! camelCase, PascalCase, or SCREAMING_SNAKE_CASE violates Rust API guidelines.
//!
//! # Fix
//! Rename the function to use snake_case: myFunction -> my_function.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_007 Rule: Function Naming Convention Detection
pub struct FunctionNamingConventionRule;

impl Default for FunctionNamingConventionRule {
    fn default() -> Self {
        Self
    }
}

impl FunctionNamingConventionRule {
    /// Check if a name follows snake_case convention
    fn is_snake_case(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // snake_case: lowercase, may contain underscores, no uppercase
        // Must start with lowercase letter
        let mut chars = name.chars();
        match chars.next() {
            Some(c) if c.is_ascii_lowercase() => {}
            _ => return false,
        }

        // Rest must be lowercase, digits, or underscores (no consecutive underscores)
        let mut prev_underscore = false;
        for c in chars {
            if c.is_ascii_uppercase() {
                return false;
            }
            if c == '_' {
                if prev_underscore {
                    return false; // No consecutive underscores
                }
                prev_underscore = true;
            } else {
                prev_underscore = false;
            }
        }

        // Should not end with underscore
        !name.ends_with('_')
    }

    /// Check if this is an allowed exception
    fn is_allowed_exception(name: &str, source: &str, node: tree_sitter::Node) -> bool {
        // main function is allowed
        if name == "main" {
            return true;
        }

        // Check for test attribute - test functions often use camelCase
        let node_start = node.start_byte();
        let search_range = std::cmp::max(0, node_start as i64 - 200) as usize..node_start;
        let preceding_text = &source[search_range];

        if preceding_text.contains("#[test]")
            || preceding_text.contains("#[tokio::test]")
            || preceding_text.contains("#[rstest]")
        {
            return true;
        }

        // Check for extern declaration
        if preceding_text.contains("extern") {
            return true;
        }

        // Check for generated code markers
        let search_range_full = std::cmp::max(0, node_start as i64 - 500) as usize..node_start;
        let broader_text = &source[search_range_full];
        if broader_text.contains("GENERATED")
            || broader_text.contains("DO NOT EDIT")
            || broader_text.contains("rustfmt")
        {
            return true;
        }

        false
    }
}

impl Rule for FunctionNamingConventionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_007")
    }

    fn name(&self) -> &'static str {
        "Function Names Should Follow snake_case Convention"
    }

    fn description(&self) -> &'static str {
        "Rust convention is snake_case for function and method names."
    }

    fn category(&self) -> Category {
        Category::Style
    }

    fn severity(&self) -> Severity {
        Severity::Minor
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Match function_item and method_declaration nodes
        let query_str = r#"
            (function_item
                name: (identifier) @func_name) @func
        "#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            let mut func_name_node = None;
            let mut func_node = None;

            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                match *name {
                    "func_name" => func_name_node = Some(cap.node),
                    "func" => func_node = Some(cap.node),
                    _ => {}
                }
            }

            if let Some(name_node) = func_name_node {
                let func_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");

                if !Self::is_snake_case(func_name) {
                    let func_node = func_node.unwrap_or(name_node);

                    // Check if this is an allowed exception
                    if !Self::is_allowed_exception(func_name, source, func_node) {
                        let pos = name_node.start_position();
                        issues.push(Issue::new(
                            "CC_CS_007",
                            "Non-snake_case Function Name",
                            Severity::Minor,
                            Category::Style,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            &format!("Function '{}' does not follow snake_case convention. Consider renaming to snake_case.", func_name),
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["naming", "function", "snake_case", "convention"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_code(code: &str) -> (tree_sitter::Tree, String) {
        let lang = SrcLanguage::Rust.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        (tree, code.to_string())
    }

    fn check_rule(code: &str) -> Vec<Issue> {
        let (tree, source) = parse_code(code);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.rs"),
            &SrcLanguage::Rust,
            &metrics,
        );
        let rule = FunctionNamingConventionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_camel_case_function() {
        let code = r#"
fn myFunction() {
    println!("hello");
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect camelCase function");
        assert_eq!(issues[0].rule_id, "CC_CS_007");
    }

    #[test]
    fn test_no_false_positive_snake_case() {
        let code = r#"
fn my_function() {
    println!("hello");
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag snake_case function");
    }

    #[test]
    fn test_no_false_positive_main_function() {
        let code = r#"
fn main() {
    println!("hello");
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag main function");
    }

    #[test]
    fn test_detects_uppercase_function() {
        let code = r#"
fn MY_FUNCTION() {
    println!("hello");
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect SCREAMING_SNAKE_CASE function");
    }

    #[test]
    fn test_no_false_positive_test_function() {
        let code = r#"
#[test]
fn testMyFunction() {
    assert_eq!(1, 1);
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag test functions with camelCase");
    }
}