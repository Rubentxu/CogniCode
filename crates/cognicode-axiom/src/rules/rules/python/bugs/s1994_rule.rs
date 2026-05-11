//! S1994 — Loop counter modified inside loop
//!
//! Detects for loops where the loop counter is modified inside the loop body.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1994"
    name: "Loop counter should not be modified inside the loop"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "Modifying the loop counter inside the loop body makes the code hard to understand and can lead to infinite loops or unexpected behavior.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect for loops with range() and counter modification
        let for_range_re = regex::Regex::new(r"for\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+in\s+range\s*\(").unwrap();
        // Capture the variable being assigned to: var = ... or var += ... etc
        let counter_mod_re = regex::Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*[+\-*\/]?=\s*").unwrap();
        
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_for_loop = false;
        let mut loop_var: Option<String> = None;
        let mut loop_indent = 0;
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            
            // Check for for loop with range
            if let Some(caps) = for_range_re.captures(trimmed) {
                if let Some(var) = caps.get(1) {
                    loop_var = Some(var.as_str().to_string());
                    loop_indent = line.len() - line.trim_start().len();
                    in_for_loop = true;
                    continue;
                }
            }
            
            // If we're in a for loop and encounter another for/def/class, reset
            if in_for_loop && (trimmed.starts_with("for ") || trimmed.starts_with("def ") || trimmed.starts_with("class ")) {
                in_for_loop = false;
                loop_var = None;
            }
            
            // Check for modification of loop variable
            if in_for_loop {
                if let Some(ref lv) = loop_var {
                    // Check if this line modifies the loop variable
                    if let Some(caps) = counter_mod_re.captures(trimmed) {
                        if let Some(assign_var) = caps.get(1) {
                            if assign_var.as_str() == lv.as_str() {
                                issues.push(Issue::new(
                                    "PY_S1994",
                                    &format!("Loop counter '{}' should not be modified inside the loop", lv),
                                    Severity::Minor,
                                    Category::Bug,
                                    ctx.file_path,
                                    line_num + 1,
                                ).with_remediation(Remediation::quick(
                                    "Use a different variable or reconsider the loop logic."
                                )));
                            }
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
    fn test_s1994_registered() {
        let rule = PY_S1994Rule::new();
        assert_eq!(rule.id(), "PY_S1994");
    }

    #[test]
    fn test_s1994_detects_counter_modification() {
        let rule = PY_S1994Rule::new();
        let smelly = r#"
for i in range(n):
    i = i + 1
    print(i)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect loop counter modification");
        assert_eq!(issues[0].rule_id, "PY_S1994");
    }

    #[test]
    fn test_s1994_allows_normal_loop() {
        let rule = PY_S1994Rule::new();
        let clean = r#"
for i in range(n):
    print(i)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal loop");
    }
}
