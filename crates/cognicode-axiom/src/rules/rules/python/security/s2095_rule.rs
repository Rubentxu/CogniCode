//! S2095 — Resource leak (file not closed)
//!
//! Detects open() calls without corresponding with statement or close() call.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2095"
    name: "Resources should be closed"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "Files opened without using a 'with' statement or explicit close() may lead to resource leaks, file descriptor exhaustion, and data not being flushed to disk.",
    clean_code: Clear,
    impacts: [Reliability: Medium, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Pattern to detect open() without 'with'
        let open_re = regex::Regex::new(r"\bopen\s*\([^)]+\)").unwrap();
        let with_re = regex::Regex::new(r"\bwith\s+.*\bopen\s*\(").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            // If line has open() but not with statement
            if open_re.is_match(line) && !with_re.is_match(line) {
                // Check if there's a close() on the same line or nearby
                let has_close = line.contains(".close()");
                if !has_close {
                    issues.push(Issue::new(
                        "PY_S2095",
                        format!("File opened without 'with' statement - resource may not be properly closed"),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Use 'with open(...) as f:' to ensure proper resource cleanup."
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
    fn test_s2095_registered() {
        let rule = PY_S2095Rule::new();
        assert_eq!(rule.id(), "PY_S2095");
    }

    #[test]
    fn test_s2095_detects_unclosed_file() {
        let rule = PY_S2095Rule::new();
        let smelly = r#"
f = open("data.txt")
content = f.read()
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unclosed file");
        assert_eq!(issues[0].rule_id, "PY_S2095");
    }

    #[test]
    fn test_s2095_allows_with_statement() {
        let rule = PY_S2095Rule::new();
        let clean = r#"
with open("data.txt") as f:
    content = f.read()
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag 'with' statement");
    }
}
