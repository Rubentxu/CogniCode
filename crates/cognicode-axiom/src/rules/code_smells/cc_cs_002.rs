//! CC_CS_002: Empty Nested Blocks Should Be Removed
//!
//! Detects empty blocks in if/else statements, loops, or closures.
//!
//! # Problem
//! Empty blocks add no logic but reduce code readability and can
//! indicate forgotten implementation.
//!
//! # Fix
//! Remove empty blocks entirely, or add the intended logic inside.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_002 Rule: Empty Nested Block Detection
pub struct EmptyNestedBlockRule;

impl Default for EmptyNestedBlockRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for EmptyNestedBlockRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_002")
    }

    fn name(&self) -> &'static str {
        "Empty Nested Blocks Should Be Removed"
    }

    fn description(&self) -> &'static str {
        "Empty blocks in if/else statements, loops, or closures add no logic but reduce code readability."
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

        // Match block nodes that are children of control flow statements
        // We use a simple query and filter in code
        let query_str = r#"(block) @block"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let block_node = cap.node;

                // Get the parent node
                if let Some(parent) = block_node.parent() {
                    let parent_kind = parent.kind();

                    // Skip function bodies, impl/trait blocks, struct bodies
                    let excluded_kinds = [
                        "function_item",
                        "impl_item",
                        "trait_item",
                        "associated_item",
                        "struct_item",
                        "enum_item",
                        "type_item",
                        "macro_invocation",
                        "macro_definition",
                    ];

                    if excluded_kinds.contains(&parent_kind) {
                        continue;
                    }

                    // Check if block is empty - it has no statement children
                    // A block like { } has tokens { and } but no actual statements
                    let has_statements = (0..block_node.child_count())
                        .filter_map(|i| block_node.child(i))
                        .any(|child| {
                            let kind = child.kind();
                            // Skip tokens (punctuation, identifiers that are part of syntax)
                            kind != "{" && kind != "}" && kind != ";" && kind != "empty_statement"
                        });

                    if !has_statements {
                        let pos = block_node.start_position();
                        issues.push(Issue::new(
                            "CC_CS_002",
                            "Empty Block",
                            Severity::Minor,
                            Category::Style,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            "Empty block detected. Remove it or add intended logic.",
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["empty", "block", "nested", "if", "loop"])
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
        let rule = EmptyNestedBlockRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_empty_if_block() {
        let code = r#"
fn example(x: Option<i32>) {
    if x.is_some() {
    }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect empty if block");
        assert_eq!(issues[0].rule_id, "CC_CS_002");
    }

    #[test]
    fn test_detects_empty_else_block() {
        let code = r#"
fn example(x: bool) {
    if x {
        println!("true");
    } else {
    }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect empty else block");
    }

    #[test]
    fn test_no_false_positive_function_body() {
        // Function bodies can be empty in trait declarations
        let code = r#"
trait MyTrait {
    fn empty_method();
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag trait method signatures");
    }

    #[test]
    fn test_no_false_positive_impl_block() {
        let code = r#"
struct MyStruct;
impl MyStruct {
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag empty impl blocks");
    }

    #[test]
    fn test_detects_empty_loop_block() {
        let code = r#"
fn example() {
    loop {
    }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect empty loop block");
    }
}