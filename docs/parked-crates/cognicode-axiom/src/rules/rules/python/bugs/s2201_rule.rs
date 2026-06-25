//! S2201 — Return value ignored
//!
//! Detects function calls where the return value is assigned but never used.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2201"
    name: "Return value should be used or explicitly ignored"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "A function with an important return value (like sorted(), strip(), etc.) is called but the return value is ignored.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        // Functions whose return values should not be ignored
        let important_returns = [
            "sorted", "reversed", "strip", "lstrip", "rstrip",
            "lower", "upper", "title", "capitalize", "replace",
            "split", "join", "copy", "deepcopy", "get", "pop"
        ];
        
        // Pattern: standalone func(...) on its own line - return value completely ignored
        // We only flag standalone calls since assignments may capture the value for later use
        let standalone_call_re = regex::Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*$").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            
            // Check for standalone call: func(...) - return value completely ignored
            if let Some(caps) = standalone_call_re.captures(trimmed) {
                let func_name = caps.get(1).map(|m| m.as_str()).unwrap();
                // Skip common void functions and control flow
                if important_returns.contains(&func_name) {
                    issues.push(Issue::new(
                        "PY_S2201",
                        &format!("Return value of '{}()' is ignored", func_name),
                        Severity::Minor,
                        Category::Bug,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        &format!("Use the return value of '{}()' or explicitly ignore it with '_'.", func_name)
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
    fn test_s2201_registered() {
        let rule = PY_S2201Rule::new();
        assert_eq!(rule.id(), "PY_S2201");
    }

    #[test]
    fn test_s2201_detects_ignored_return() {
        let rule = PY_S2201Rule::new();
        let smelly = r#"
def process(data):
    sorted(data)
    return data
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect ignored return value");
        assert_eq!(issues[0].rule_id, "PY_S2201");
    }

    #[test]
    fn test_s2201_allows_proper_usage() {
        let rule = PY_S2201Rule::new();
        let clean = r#"
def process(data):
    sorted_data = sorted(data)
    return sorted_data
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag when return value is used");
    }
}
