//! N14 — Missing type hints on public functions
//!
//! Detects public functions without type hints on parameters.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N14"
    name: "Missing type hints on public function"
    severity: Info
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Public functions should have type hints on parameters for better code documentation and IDE support.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find public function definitions (not starting with _)
        let func_pattern = regex::Regex::new(r"def\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)\s*(?:->[^:]+)?:").unwrap();

        for cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = cap.get(1) {
                let func_name_str = func_name.as_str();

                // Skip private functions and dunder methods
                if func_name_str.starts_with('_') && !func_name_str.starts_with("__") {
                    continue;
                }

                let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");

                // Skip if no parameters
                if params.trim().is_empty() {
                    continue;
                }

                // Check if any parameter (excluding self, cls) has a type hint
                let param_pattern = regex::Regex::new(r"(\w+)\s*:").unwrap();
                let has_type_hint = param_pattern.captures_iter(params)
                    .any(|p| {
                        let name = p.get(1).map(|m| m.as_str()).unwrap_or("");
                        name != "self" && name != "cls"
                    });

                if !has_type_hint {
                    let line_num = source[..func_name.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_N14",
                        format!("Public function '{}' is missing type hints on parameters", func_name_str),
                        Severity::Info,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Add type hints to function parameters: def foo(x: int, y: str) -> None:"
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
    fn test_n14_registered() {
        let rule = PY_N14Rule::new();
        assert_eq!(rule.id(), "PY_N14");
    }

    #[test]
    fn test_n14_detects_missing_type_hints() {
        let rule = PY_N14Rule::new();
        let smelly = r#"
def public_func(x, y):
    return x + y
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect missing type hints");
        assert_eq!(issues[0].rule_id, "PY_N14");
    }

    #[test]
    fn test_n14_allows_with_type_hints() {
        let rule = PY_N14Rule::new();
        let clean = r#"
def public_func(x: int, y: int) -> int:
    return x + y
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag function with type hints");
    }

    #[test]
    fn test_n14_allows_private_function() {
        let rule = PY_N14Rule::new();
        let clean = r#"
def _private_func(x, y):
    return x + y
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag private functions");
    }

    #[test]
    fn test_n14_allows_no_params() {
        let rule = PY_N14Rule::new();
        let clean = r#"
def public_func():
    return 1
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag function with no parameters");
    }
}