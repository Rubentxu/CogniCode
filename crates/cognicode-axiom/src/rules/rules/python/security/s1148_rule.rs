//! S1148 — traceback.print_exc() instead of logging
//!
//! Detects use of traceback.print_exc() instead of proper logging.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1148"
    name: "traceback.print_exc() should not be used"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using traceback.print_exc() writes to stdout instead of proper logging, making it harder to monitor and analyze errors in production systems.",
    clean_code: Clear,
    impacts: [Security: Info, Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let print_exc = regex::Regex::new(r"traceback\.print_exc\s*\(").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if print_exc.is_match(line) {
                issues.push(Issue::new(
                    "PY_S1148",
                    "traceback.print_exc() used instead of proper logging",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use logging.exception() or logging.error() with exc_info=True instead of traceback.print_exc()."
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
    fn test_s1148_registered() {
        let rule = PY_S1148Rule::new();
        assert_eq!(rule.id(), "PY_S1148");
    }

    #[test]
    fn test_s1148_detects_print_exc() {
        let rule = PY_S1148Rule::new();
        let smelly = r#"
try:
    risky_operation()
except Exception:
    traceback.print_exc()
"#;
        let issues = with_python_context(smelly, "handler.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect traceback.print_exc()");
        assert_eq!(issues[0].rule_id, "PY_S1148");
    }

    #[test]
    fn test_s1148_allows_logging_exception() {
        let rule = PY_S1148Rule::new();
        let clean = r#"
try:
    risky_operation()
except Exception:
    logger.exception("Operation failed")
"#;
        let issues = with_python_context(clean, "handler.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag logging.exception()");
    }
}
