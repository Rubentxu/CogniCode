//! SM2 — High complexity (>15)
//!
//! Detects functions with high cyclomatic complexity.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S3776"
    name: "Function should not have high cyclomatic complexity (>15)"
    severity: Major
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "High cyclomatic complexity indicates functions with many decision points. Consider refactoring to reduce complexity.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find function definitions
        let func_pattern = regex::Regex::new(r"func\s+(\w+)\s*\([^)]*\)\s*\{").unwrap();

        for cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = cap.get(1) {
                let func_start = cap.get(0).unwrap().start();

                // Find the closing brace
                let remaining = &source[func_start..];
                let mut brace_count = 0;
                let mut func_end = 0;

                for (i, c) in remaining.char_indices() {
                    match c {
                        '{' => {
                            brace_count += 1;
                            if brace_count == 1 {
                                func_end = i;
                            }
                        },
                        '}' => {
                            brace_count -= 1;
                            if brace_count == 0 {
                                func_end = i + 1;
                                break;
                            }
                        },
                        _ => {}
                    }
                }

                let func_body = &remaining[..func_end];

                // Count decision points: if, for, switch, case, select
                let if_count = regex::Regex::new(r"\bif\b").unwrap().find_iter(func_body).count();
                let for_count = regex::Regex::new(r"\bfor\b").unwrap().find_iter(func_body).count();
                let switch_count = regex::Regex::new(r"\bswitch\b").unwrap().find_iter(func_body).count();
                let case_count = regex::Regex::new(r"\bcase\b").unwrap().find_iter(func_body).count();
                let select_count = regex::Regex::new(r"\bselect\b").unwrap().find_iter(func_body).count();

                let complexity = if_count + for_count + switch_count + case_count + select_count;

                if complexity > 15 {
                    let line_num = source[..func_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "GO_S3776",
                        format!("Function '{}' has complexity {} (max 15)", func_name.as_str(), complexity),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Reduce complexity by extracting functions or simplifying logic"
                    )));
                }
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
    fn test_sm2_registered() {
        let rule = GO_S3776Rule::new();
        assert_eq!(rule.id(), "GO_S3776");
    }

    #[test]
    fn test_sm2_detects_high_complexity() {
        let rule = GO_S3776Rule::new();
        let smelly = r#"
func ComplexFunction() {
    if a { }
    if b { }
    if c { }
    if d { }
    if e { }
    if f { }
    if g { }
    if h { }
    if i { }
    if j { }
    if k { }
    if l { }
    if m { }
    if n { }
    if o { }
    if p { }
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect high complexity");
        assert_eq!(issues[0].rule_id, "GO_S3776");
    }

    #[test]
    fn test_sm2_allows_low_complexity() {
        let rule = GO_S3776Rule::new();
        let clean = r#"
func SimpleFunction() {
    if a { }
    if b { }
    if c { }
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag low complexity");
    }
}
