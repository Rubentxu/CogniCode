//! N7 — Too many function parameters (>6)
//!
//! Detects functions with too many parameters.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S107"
    name: "Function should not have too many parameters"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Functions with more than 6 parameters are hard to call and may indicate a design issue. Consider using a struct or splitting the function.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find function definitions and count parameters
        // Match: func name(a, b, c, ...) {
        let func_pattern = regex::Regex::new(r"func\s+(\w+)\s*\(([^)]*)\)").unwrap();

        for cap in func_pattern.captures_iter(source) {
            if let (Some(func_name), Some(params_str)) = (cap.get(1), cap.get(2)) {
                let func_name_str = func_name.as_str();
                let params = params_str.as_str().trim();

                // Skip if no params or just the receiver
                if params.is_empty() || params.starts_with("()") {
                    continue;
                }

                // Count parameters by commas (simplified - doesn't handle nested parens)
                let param_count = if params.is_empty() {
                    0
                } else {
                    params.split(',').count()
                };

                if param_count > 6 {
                    let line_num = source[..func_name.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "GO_S107",
                        format!("Function '{}' has {} parameters (max 6)", func_name_str, param_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Consider using a struct to group parameters or splitting the function"
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
    fn test_n7_registered() {
        let rule = GO_S107Rule::new();
        assert_eq!(rule.id(), "GO_S107");
    }

    #[test]
    fn test_n7_detects_too_many_params() {
        let rule = GO_S107Rule::new();
        let smelly = r#"
func myFunction(a, b, c, d, e, f, g, h int) {
    return
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect too many parameters");
        assert_eq!(issues[0].rule_id, "GO_S107");
    }

    #[test]
    fn test_n7_allows_normal_params() {
        let rule = GO_S107Rule::new();
        let clean = r#"
func myFunction(a, b, c int) {
    return
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag functions with <= 6 parameters");
    }
}
