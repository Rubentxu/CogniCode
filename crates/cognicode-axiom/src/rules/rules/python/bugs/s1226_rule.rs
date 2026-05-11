//! S1226 — Parameter reassigned
//!
//! Detects function parameters being reassigned within the function body.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1226"
    name: "Function parameters should not be reassigned"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "Reassigning a function parameter is confusing and can lead to bugs. Use a local variable instead.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        
        // Simple line-based detection: parameter reassignment pattern
        // This is a heuristic - in real implementation would need AST analysis
        let assign_re = regex::Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*").unwrap();
        let param_re = regex::Regex::new(r"def\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\(([^)]+)\)").unwrap();
        
        let lines: Vec<&str> = ctx.source.lines().collect();
        
        // Find function definitions and their parameters
        let mut params: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut in_function = false;
        let mut function_indent = 0;
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            
            // Check for function definition
            if let Some(caps) = param_re.captures(trimmed) {
                if let Some(params_match) = caps.get(1) {
                    let params_str = params_match.as_str();
                    // Extract parameter names
                    for param in params_str.split(',') {
                        let param_name = param.trim().split(':').next().unwrap_or(param.trim());
                        let param_name = param_name.trim().split('=').next().unwrap_or(param_name.trim());
                        if !param_name.is_empty() && !param_name.starts_with('*') && !param_name.starts_with("**") {
                            params.insert(param_name.to_string());
                        }
                    }
                    in_function = true;
                    function_indent = line.len() - line.trim_start().len();
                    continue;
                }
            }
            
            // If we're in a function and hit another def or class, reset
            if in_function && (trimmed.starts_with("def ") || trimmed.starts_with("class ") || trimmed.starts_with("@")) {
                params.clear();
                in_function = false;
            }
            
            // Check for parameter reassignment
            if in_function && !params.is_empty() {
                if let Some(caps) = assign_re.captures(trimmed) {
                    if let Some(var) = caps.get(1) {
                        let var_str = var.as_str();
                        if params.contains(var_str) {
                            issues.push(Issue::new(
                                "PY_S1226",
                                &format!("Parameter '{}' should not be reassigned", var_str),
                                Severity::Minor,
                                Category::Bug,
                                ctx.file_path,
                                line_num + 1,
                            ).with_remediation(Remediation::quick(
                                "Use a local variable instead of reassigning the parameter."
                            )));
                        }
                    }
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
    fn test_s1226_registered() {
        let rule = PY_S1226Rule::new();
        assert_eq!(rule.id(), "PY_S1226");
    }

    #[test]
    fn test_s1226_detects_param_reassignment() {
        let rule = PY_S1226Rule::new();
        let smelly = r#"
def foo(x):
    x = x + 1
    return x
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect parameter reassignment");
        assert_eq!(issues[0].rule_id, "PY_S1226");
    }

    #[test]
    fn test_s1226_allows_local_var() {
        let rule = PY_S1226Rule::new();
        let clean = r#"
def foo(x):
    y = x + 1
    return y
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag local variable reassignment");
    }
}
