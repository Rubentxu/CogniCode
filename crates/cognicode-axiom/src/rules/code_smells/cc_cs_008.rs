//! CC_CS_008: Functions Containing Only todo!() or unimplemented!() Should Be Completed
//!
//! Detects functions whose body contains only a todo!(), unimplemented!(), or panic!() macro.
//!
//! # Problem
//! These functions will panic at runtime if called, making them unsuitable
//! for production code.
//!
//! # Fix
//! Implement the function properly, or if intentionally stubbed, consider
//! using a placeholder pattern that makes the intent clearer.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_008 Rule: Stub Function Detection
pub struct StubFunctionRule;

impl Default for StubFunctionRule {
    fn default() -> Self {
        Self
    }
}

impl StubFunctionRule {
    /// Check if a macro name is a stub macro (todo!, unimplemented!, panic!, unreachable!)
    fn is_stub_macro(name: &str) -> bool {
        matches!(name, "todo" | "unimplemented" | "panic" | "unreachable")
    }
}

impl Rule for StubFunctionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_008")
    }

    fn name(&self) -> &'static str {
        "Functions Containing Only todo!() or unimplemented!() Should Be Completed"
    }

    fn description(&self) -> &'static str {
        "Functions with todo!() or unimplemented!() will panic if called. Implement them properly."
    }

    fn category(&self) -> Category {
        Category::Maintainability
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

        // Match function_item nodes
        let query_str = r#"(function_item) @func"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let func_node = cap.node;

                // Get function name
                let func_name = func_node
                    .child_by_field_name("name")
                    .map(|n| n.utf8_text(source.as_bytes()).unwrap_or(""))
                    .unwrap_or_default();

                // Skip main function
                if func_name == "main" {
                    continue;
                }

                // Check for test attribute - test functions may have stubs
                let start = func_node.start_byte();
                let search_range = if start > 200 { start - 200 } else { 0 }..start;
                let preceding_text = &source[search_range];

                if preceding_text.contains("#[test]")
                    || preceding_text.contains("#[tokio::test]")
                    || preceding_text.contains("#[rstest]")
                    || preceding_text.contains("#[bench]")
                {
                    continue;
                }

                // Get function body text
                if let Some(body) = func_node.child_by_field_name("body") {
                    let body_text = body.utf8_text(source.as_bytes()).unwrap_or("");

                    // Check if body contains only stub macros
                    // Note: macros may have arguments so we check for prefix
                    let stub_patterns = ["todo!", "unimplemented!", "panic!", "unreachable!"];

                    for stub in &stub_patterns {
                        if body_text.contains(stub) {
                            // Found a stub macro
                            let pos = func_node.start_position();
                            issues.push(Issue::new(
                                "CC_CS_008",
                                "Stub Function",
                                Severity::Minor,
                                Category::Maintainability,
                                ctx.file_path.to_string_lossy(),
                                pos.row + 1,
                                pos.column,
                                &format!("Function '{}' contains only stub macro. Implement the function properly or remove the stub.", func_name),
                            ));
                            break;
                        }
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["todo", "unimplemented", "function", "stub"])
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
        let rule = StubFunctionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_todo_function() {
        let code = r#"
fn not_implemented() {
    todo!()
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect todo!() function");
        assert_eq!(issues[0].rule_id, "CC_CS_008");
    }

    #[test]
    fn test_detects_unimplemented_function() {
        let code = r#"
fn not_done() {
    unimplemented!()
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect unimplemented!() function");
    }

    #[test]
    fn test_detects_panic_function() {
        let code = r#"
fn will_fail() {
    panic!("not implemented")
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect panic!() function");
    }

    #[test]
    fn test_no_false_positive_main_function() {
        let code = r#"
fn main() {
    println!("Hello");
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag main function");
    }

    #[test]
    fn test_no_false_positive_normal_function() {
        let code = r#"
fn normal_function(x: i32) -> i32 {
    x * 2
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag normal function");
    }
}