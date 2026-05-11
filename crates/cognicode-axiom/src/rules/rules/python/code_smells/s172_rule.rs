//! S172 — print() in library code
//!
//! Detects print() statements in library/module code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S172"
    name: "print() should not be used in library code"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "print() statements should not be in library code. Use proper logging or return values instead.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let print_pattern = regex::Regex::new(r"\bprint\s*\(").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if print_pattern.is_match(line) {
                issues.push(Issue::new(
                    "PY_S172",
                    format!("print() statement found at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use proper logging instead of print() in library code."
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

    fn with_python_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Python.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Python,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s172_registered() {
        let rule = PY_S172Rule::new();
        assert_eq!(rule.id(), "PY_S172");
    }

    #[test]
    fn test_s172_detects_print() {
        let rule = PY_S172Rule::new();
        let smelly = r#"
def process():
    print("Processing...")
    return True
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect print statement");
        assert_eq!(issues[0].rule_id, "PY_S172");
    }

    #[test]
    fn test_s172_allows_no_print() {
        let rule = PY_S172Rule::new();
        let clean = r#"
def process():
    import logging
    logging.info("Processing...")
    return True
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag logging");
    }
}
