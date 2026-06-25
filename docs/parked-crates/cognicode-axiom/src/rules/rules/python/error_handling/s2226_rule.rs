//! S2226 — Logging exception without traceback
//!
//! Detects logging.error() or logging.warning() in except block without exc_info=True.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2226"
    name: "Logging exception without traceback"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "When logging an exception, always use exc_info=True to include the full traceback. Without it, debugging becomes difficult.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;
        let lines: Vec<&str> = source.lines().collect();

        let mut in_except = false;
        let mut except_col = 0;

        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.starts_with("except") && trimmed.ends_with(':') {
                in_except = true;
                except_col = line.len() - line.trim_start().len();
                continue;
            }

            if in_except {
                let col = line.len() - line.trim_start().len();
                if col <= except_col && !trimmed.is_empty() {
                    in_except = false;
                    continue;
                }

                // Check for logging.error/warning without exc_info=True
                let logging_pattern = regex::Regex::new(r"logging\.(error|warning|info|critical)\s*\(").unwrap();
                if logging_pattern.is_match(trimmed) && !trimmed.contains("exc_info=True") {
                    issues.push(Issue::new(
                        "PY_S2226",
                        format!("Logging exception without exc_info=True at line {}", line_num + 1),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Add exc_info=True to the logging call to include the full traceback."
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
    fn test_s2226_registered() {
        let rule = PY_S2226Rule::new();
        assert_eq!(rule.id(), "PY_S2226");
    }

    #[test]
    fn test_s2226_detects_logging_without_exc_info() {
        let rule = PY_S2226Rule::new();
        let smelly = r#"
try:
    do_something()
except Exception:
    logging.error("Failed")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect logging without exc_info=True");
        assert_eq!(issues[0].rule_id, "PY_S2226");
    }

    #[test]
    fn test_s2226_allows_logging_with_exc_info() {
        let rule = PY_S2226Rule::new();
        let clean = r#"
try:
    do_something()
except Exception:
    logging.error("Failed", exc_info=True)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow logging with exc_info=True");
    }
}
