//! N8 — Deep nesting (>3 levels)
//!
//! Detects code blocks nested more than 3 levels deep.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S134"
    name: "Code should not be nested more than 3 levels deep"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Deeply nested code (>3 levels) is hard to read and maintain. Consider refactoring to reduce nesting.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        let mut max_nesting = 0;
        let mut max_nesting_line = 1;
        let mut current_nesting = 0;
        let mut in_string = false;
        let mut in_single_comment = false;
        let mut in_multiline_comment = false;
        let mut chars = source.char_indices().peekable();

        while let Some((idx, c)) = chars.next() {
            // Handle string literals
            if c == '"' && !in_single_comment && !in_multiline_comment {
                // Check if it's a raw string (backtick) or escaped
                if source[idx..].starts_with("`") {
                    // Raw string - find closing backtick
                    let rest = &source[idx+1..];
                    if let Some(close_idx) = rest.find('`') {
                        for _ in 0..close_idx {
                            chars.next();
                        }
                        chars.next(); // skip closing `
                        continue;
                    }
                } else if !in_string {
                    in_string = true;
                    continue;
                } else {
                    in_string = false;
                    continue;
                }
            }

            if in_string {
                continue;
            }

            // Handle comments
            if c == '/' {
                if let Some(&(_, next_c)) = chars.peek() {
                    if next_c == '/' && !in_multiline_comment {
                        in_single_comment = true;
                        continue;
                    }
                    if next_c == '*' && !in_single_comment {
                        in_multiline_comment = true;
                        chars.next();
                        continue;
                    }
                }
            }

            if in_single_comment {
                if c == '\n' {
                    in_single_comment = false;
                }
                continue;
            }

            if in_multiline_comment {
                if c == '*' {
                    if let Some(&(_, next_c)) = chars.peek() {
                        if next_c == '/' {
                            in_multiline_comment = false;
                            chars.next();
                        }
                    }
                }
                continue;
            }

            // Track nesting level
            if c == '{' {
                current_nesting += 1;
                if current_nesting > max_nesting {
                    max_nesting = current_nesting;
                    max_nesting_line = source[..idx].lines().count() + 1;
                }
            } else if c == '}' {
                if current_nesting > 0 {
                    current_nesting -= 1;
                }
            }
        }

        if max_nesting > 3 {
            issues.push(Issue::new(
                "GO_S134",
                format!("Code is nested {} levels deep (max 3)", max_nesting),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                max_nesting_line,
            ).with_remediation(Remediation::quick(
                "Refactor to reduce nesting: extract functions, early return, or restructure"
            )));
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    fn with_go_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Go.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Go,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_n8_registered() {
        let rule = GO_S134Rule::new();
        assert_eq!(rule.id(), "GO_S134");
    }

    #[test]
    fn test_n8_detects_deep_nesting() {
        let rule = GO_S134Rule::new();
        let smelly = r#"
if a {
    if b {
        if c {
            if d {
                x = 1
            }
        }
    }
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect deep nesting");
        assert_eq!(issues[0].rule_id, "GO_S134");
    }

    #[test]
    fn test_n8_allows_normal_nesting() {
        let rule = GO_S134Rule::new();
        let clean = r#"
if a {
    if b {
        if c {
            x = 1
        }
    }
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag nesting <= 3 levels");
    }
}
