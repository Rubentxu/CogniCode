//! S218 — Switch with too few cases
//!
//! Detects switch statements with fewer than 3 case labels, suggesting if-else instead.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S218"
    name: "Switch with too few cases"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Switch statements with fewer than 3 cases are often clearer as if-else statements. Consider refactoring to if-else for better readability.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect switch statements and count case labels
        let switch_pattern = regex::Regex::new(r"switch\s*\([^)]+\)\s*\{").unwrap();

        for cap in switch_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let switch_start = matched.start();

                // Find the end of this switch block
                let after_switch = &source[switch_start..];
                let block_end = find_closing_brace(after_switch, 0);

                if let Some(end_idx) = block_end {
                    let switch_block = &after_switch[..end_idx + 1];

                    // Count case labels
                    let case_count = switch_block.matches("case ").count();

                    if case_count > 0 && case_count < 3 {
                        let line_num = source[..switch_start].lines().count() + 1;
                        issues.push(Issue::new(
                            "JAVA_S218",
                            format!("Switch with only {} cases - consider if-else", case_count),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Consider using if-else instead of switch for better readability."
                        )));
                    }
                }
            }
        }
        issues
    }
}

// Helper function to find matching closing brace
fn find_closing_brace(s: &str, start_idx: usize) -> Option<usize> {
    let mut depth = 0;
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if *c == '{' {
            depth += 1;
        } else if *c == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(start_idx + i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;

    fn with_java_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Java.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Java,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s218_registered() {
        let rule = JAVA_S218Rule::new();
        assert_eq!(rule.id(), "JAVA_S218");
    }

    #[test]
    fn test_s218_detects_few_cases() {
        let rule = JAVA_S218Rule::new();
        let smelly = r#"
switch (value) {
    case 1:
        return "one";
    case 2:
        return "two";
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect switch with few cases");
        assert_eq!(issues[0].rule_id, "JAVA_S218");
    }

    #[test]
    fn test_s218_allows_many_cases() {
        let rule = JAVA_S218Rule::new();
        let clean = r#"
switch (value) {
    case 1:
        return "one";
    case 2:
        return "two";
    case 3:
        return "three";
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag switch with 3+ cases");
    }
}
