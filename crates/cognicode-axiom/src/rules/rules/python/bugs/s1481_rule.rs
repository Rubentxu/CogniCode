//! S1481 — Unused variable
//!
//! Detects variables that are assigned but never read.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1481"
    name: "Variables should not be assigned and never used"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "A variable is assigned a value but the value is never read. This indicates dead code or forgotten logic.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Track variable assignments and uses
        let assign_re = regex::Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=").unwrap();
        
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut assigned_vars: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut used_vars: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        // First pass: collect all assignments
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            
            if let Some(caps) = assign_re.captures(trimmed) {
                if let Some(var) = caps.get(1) {
                    let var_str = var.as_str();
                    // Skip dunder variables and keywords
                    if !var_str.starts_with("__") && !var_str.contains("self.") {
                        assigned_vars.insert(var_str.to_string(), line_num);
                    }
                }
            }
        }
        
        // Second pass: collect usages (identifiers that appear in code but are not the assigned variable name on assignment lines)
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            
            // Check if this line is an assignment line
            let is_assign_line = assign_re.is_match(trimmed);
            
            // Find all identifiers in this line
            for cap in regex::Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap().find_iter(trimmed) {
                let id = cap.as_str();
                // Skip if this is the variable being assigned on its assignment line
                if is_assign_line {
                    if let Some(caps) = assign_re.captures(trimmed) {
                        if let Some(lhs) = caps.get(1) {
                            if lhs.as_str() == id {
                                continue;
                            }
                        }
                    }
                }
                used_vars.insert(id.to_string());
            }
        }
        
        // Report unused assignments
        for (var, line_num) in &assigned_vars {
            if !used_vars.contains(var) {
                issues.push(Issue::new(
                    "PY_S1481",
                    &format!("Unused variable: '{}' assigned but never used", var),
                    Severity::Minor,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    &format!("Remove the unused variable '{}' or use its value.", var)
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
    fn test_s1481_registered() {
        let rule = PY_S1481Rule::new();
        assert_eq!(rule.id(), "PY_S1481");
    }

    #[test]
    fn test_s1481_detects_unused_variable() {
        let rule = PY_S1481Rule::new();
        let smelly = r#"
def foo():
    x = 10
    return 42
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unused variable");
        assert_eq!(issues[0].rule_id, "PY_S1481");
    }

    #[test]
    fn test_s1481_allows_used_variable() {
        let rule = PY_S1481Rule::new();
        let clean = r#"
def foo():
    x = 10
    return x
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag used variable");
    }
}
