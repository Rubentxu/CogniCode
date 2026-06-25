//! S1160 — Public function raises generic exception
//!
//! Detects public functions that raise generic Exception.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1160"
    name: "Public function raises generic exception"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Public functions should raise specific exception types, not generic Exception. This helps callers handle errors appropriately.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;
        let lines: Vec<&str> = source.lines().collect();

        let mut in_public_func = false;
        let mut func_name = String::new();
        let mut func_line = 0;
        let mut func_col = 0;

        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect public function definition
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                if let Some(paren) = trimmed.find('(') {
                    let name = &trimmed[4..paren].trim();
                    if !name.starts_with('_') && !name.starts_with("__") {
                        in_public_func = true;
                        func_name = name.to_string();
                        func_col = line.len() - line.trim_start().len();
                        func_line = line_num + 1;
                    } else {
                        in_public_func = false;
                    }
                }
            } else if in_public_func && !trimmed.is_empty() {
                let col = line.len() - line.trim_start().len();
                if col <= func_col && !trimmed.starts_with('#') {
                    in_public_func = false;
                    continue;
                }
            }

            if in_public_func && trimmed.starts_with("raise Exception") {
                issues.push(Issue::new(
                    "PY_S1160",
                    format!("Public function '{}' raises generic Exception at line {}", func_name, line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Raise a specific exception type instead of generic Exception."
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
    fn test_s1160_registered() {
        let rule = PY_S1160Rule::new();
        assert_eq!(rule.id(), "PY_S1160");
    }

    #[test]
    fn test_s1160_detects_public_func_raising_generic() {
        let rule = PY_S1160Rule::new();
        let smelly = r#"
def process_data(data):
    raise Exception("Invalid data")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect public function raising generic Exception");
        assert_eq!(issues[0].rule_id, "PY_S1160");
    }

    #[test]
    fn test_s1160_allows_private_func() {
        let rule = PY_S1160Rule::new();
        let clean = r#"
def _internal_process(data):
    raise Exception("Invalid data")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag private functions");
    }

    #[test]
    fn test_s1160_allows_specific_exception() {
        let rule = PY_S1160Rule::new();
        let clean = r#"
def process_data(data):
    raise ValueError("Invalid data")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag specific exceptions");
    }
}
