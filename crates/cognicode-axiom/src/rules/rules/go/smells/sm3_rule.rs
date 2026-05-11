//! SM3 — Switch without default
//!
//! Detects switch statements without a default case.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S131"
    name: "Switch should have a default case"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Switch statements without a default case may miss unhandled cases. Consider adding a default case.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find switch statements
        let switch_pattern = regex::Regex::new(r"switch\s+\{" ).unwrap();
        let switch_expr_pattern = regex::Regex::new(r"switch\s+[^{]+\{").unwrap();
        let default_pattern = regex::Regex::new(r"\bdefault\s*:").unwrap();

        // Find all switch positions
        let switches: Vec<_> = switch_pattern.find_iter(source)
            .chain(switch_expr_pattern.find_iter(source))
            .collect();

        for sw in switches {
            let sw_start = sw.start();
            let sw_end = sw.start() + source[sw_start..].find('{').map(|p| p + 1).unwrap_or(0);

            // Find the end of this switch block
            let remaining = &source[sw_end..];
            let mut brace_count = 1;
            let mut block_end = 0;

            for (i, c) in remaining.char_indices() {
                match c {
                    '{' => brace_count += 1,
                    '}' => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            block_end = sw_end + i;
                            break;
                        }
                    },
                    _ => {}
                }
            }

            if block_end == 0 {
                continue;
            }

            let switch_body = &source[sw_end..block_end];

            // Check if default exists in this switch body
            if !default_pattern.is_match(switch_body) {
                let line_num = source[..sw_start].lines().count() + 1;
                issues.push(Issue::new(
                    "GO_S131",
                    format!("Switch statement has no default case"),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Add a default case to handle unhandled values"
                )));
            }
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
    fn test_sm3_registered() {
        let rule = GO_S131Rule::new();
        assert_eq!(rule.id(), "GO_S131");
    }

    #[test]
    fn test_sm3_detects_no_default() {
        let rule = GO_S131Rule::new();
        let smelly = r#"
switch x {
case 1:
    fmt.Println("one")
case 2:
    fmt.Println("two")
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect switch without default");
        assert_eq!(issues[0].rule_id, "GO_S131");
    }

    #[test]
    fn test_sm3_allows_with_default() {
        let rule = GO_S131Rule::new();
        let clean = r#"
switch x {
case 1:
    fmt.Println("one")
case 2:
    fmt.Println("two")
default:
    fmt.Println("other")
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag switch with default");
    }
}
