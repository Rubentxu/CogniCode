//! CC_TEST_014: Act Wrapper Missing
//!
//! Detects state updates (fireEvent, userEvent, dispatch) not wrapped in act().
//!
//! # Problem
//! State updates not wrapped in act() can complete after the test
//! assertion, causing 'not wrapped in act' warnings and flaky assertions.
//!
//! # Fix
//! Wrap state updates in act():
//! - act(() => { fireEvent.click(...) })
//! - await act(async () => { ... }) for async
//! Or use waitFor from @testing-library which handles act internally.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_014 Rule: Act Wrapper Missing
pub struct ActWrapperMissingRule;

impl Default for ActWrapperMissingRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for ActWrapperMissingRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_014")
    }

    fn name(&self) -> &'static str {
        "Act Wrapper Missing"
    }

    fn description(&self) -> &'static str {
        "Detects state updates not wrapped in act()"
    }

    fn category(&self) -> Category {
        Category::TestSmell
    }

    fn severity(&self) -> Severity {
        Severity::Major
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::JavaScript]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // For JavaScript: match call_expression with member_expression functions like fireEvent.click()
        let query_str = match ctx.language {
            SrcLanguage::JavaScript => {
                r#"(call_expression
                    function: (member_expression
                        object: (identifier) @event_obj
                        property: (property_identifier) @event_method)
                    arguments: (arguments) @args)"#
            }
            _ => return issues,
        };

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Event handler patterns that should be wrapped in act
        let event_handlers = ["fireEvent", "userEvent", "dispatch"];

        while let Some(m) = matches.next() {
            let mut event_obj = String::new();
            let mut event_method = String::new();
            let mut pos = tree_sitter::Point::new(0, 0);

            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                match *field_name {
                    "event_obj" => {
                        event_obj = cap.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                        pos = cap.node.start_position();
                    }
                    "event_method" => {
                        event_method = cap.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    }
                    _ => {}
                }
            }

            // Check if this is an event handler that should be wrapped in act
            if !event_handlers.contains(&event_obj.as_str()) {
                continue;
            }

            // We need to find the call_expression node that contains this fireEvent.click
            // The capture gives us event_obj (identifier node), its parent is member_expression,
            // and member_expression's parent is the call_expression
            let event_obj_node = m.captures.iter()
                .find(|c| c.node.kind() == "identifier" && c.node.utf8_text(source.as_bytes()).unwrap_or("") == event_obj)
                .map(|c| c.node);
            let member_node = event_obj_node.and_then(|n| n.parent());
            let call_node = member_node.and_then(|m| m.parent());

            // Check if this is wrapped in act
            if let Some(call) = call_node {
                let mut is_wrapped = false;

                // Walk up the tree to find if we're inside an act() call
                let mut current = call;
                while let Some(parent) = current.parent() {
                    // Check if this is a call_expression with act as function
                    if parent.kind() == "call_expression" {
                        if let Some(func) = parent.child_by_field_name("function") {
                            let func_text = func.utf8_text(source.as_bytes()).unwrap_or("");
                            if func_text == "act" {
                                is_wrapped = true;
                                break;
                            }
                        }
                    }
                    // Stop at program level or expression_statement (top level test)
                    if parent.kind() == "program" {
                        break;
                    }
                    current = parent;
                }

                // fireEvent, userEvent, dispatch should be wrapped in act
                if !is_wrapped {
                    issues.push(Issue::new(
                        "CC_TEST_014",
                        "Act Wrapper Missing",
                        Severity::Major,
                        Category::TestSmell,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        pos.column,
                        format!(
                            "Event '{}' should be wrapped in act() to avoid React \
                             state update warnings. Wrap the call in: await act(async () => {{ ... }})",
                            format!("{}.{}", event_obj, event_method)
                        ),
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["act", "fireEvent", "userEvent", "setState", "useState", "dispatch"])
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
            std::path::Path::new("test.js"),
            &language,
            &metrics,
        );
        let rule = ActWrapperMissingRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_fire_event_without_act() {
        let code = r#"
test('clicks button', () => {
    fireEvent.click(button);
    expect(screen.getByText('Submitted')).toBeInTheDocument();
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect fireEvent without act");
        assert_eq!(issues[0].rule_id, "CC_TEST_014");
    }

    #[test]
    fn test_no_false_positive_with_act() {
        let code = r#"
test('clicks button', async () => {
    await act(async () => {
        fireEvent.click(button);
    });
    expect(screen.getByText('Submitted')).toBeInTheDocument();
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag fireEvent wrapped in act");
    }
}
