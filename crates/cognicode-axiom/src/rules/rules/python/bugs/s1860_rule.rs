//! S1860 — Deadlock potential
//!
//! Detects nested Lock.acquire() calls which can cause deadlock.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1860"
    name: "Nested Lock.acquire() calls can cause deadlock"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "Calling Lock.acquire() inside a critical section that already holds the same lock causes deadlock. Use 'with lock:' statement instead.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect lock.with_lock or lock.acquire patterns
        let acquire_re = regex::Regex::new(r"\.acquire\s*\(").unwrap();
        let with_lock_re = regex::Regex::new(r"with\s+[a-zA-Z_][a-zA-Z0-9_.]*:\s*$").unwrap();
        let import_threading = regex::Regex::new(r"import\s+threading|from\s+threading\s+import").unwrap();
        
        let lines: Vec<&str> = ctx.source.lines().collect();
        
        // Check if threading is imported
        let has_threading = lines.iter().any(|l| import_threading.is_match(l.trim()));
        
        if !has_threading {
            return issues;
        }
        
        let mut in_with_lock = false;
        let mut lock_indent = 0;
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            
            let current_indent = line.len() - line.trim_start().len();
            
            // Check for with lock: pattern
            if with_lock_re.is_match(trimmed) && (trimmed.contains("lock") || trimmed.contains("Lock")) {
                in_with_lock = true;
                lock_indent = current_indent;
                continue;
            }
            
            // If we're in a with block and the indent decreases, we're out
            if in_with_lock && current_indent <= lock_indent && !trimmed.is_empty() {
                in_with_lock = false;
            }
            
            // Check for acquire inside with lock
            if in_with_lock && acquire_re.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S1860",
                    "Nested Lock.acquire() inside 'with lock' block can cause deadlock",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Remove the nested acquire() call or restructure the locking pattern."
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
    fn test_s1860_registered() {
        let rule = PY_S1860Rule::new();
        assert_eq!(rule.id(), "PY_S1860");
    }

    #[test]
    fn test_s1860_detects_nested_acquire() {
        let rule = PY_S1860Rule::new();
        let smelly = r#"
import threading

lock = threading.Lock()

with lock:
    lock.acquire()
    do_something()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect nested acquire");
        assert_eq!(issues[0].rule_id, "PY_S1860");
    }

    #[test]
    fn test_s1860_allows_simple_lock() {
        let rule = PY_S1860Rule::new();
        let clean = r#"
import threading

lock = threading.Lock()

with lock:
    do_something()
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag simple lock usage");
    }
}
