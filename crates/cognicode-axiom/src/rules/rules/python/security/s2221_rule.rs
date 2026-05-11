//! S2221 — Catching BaseException
//!
//! Detects catching BaseException or bare except:, which catches all exceptions including SystemExit and KeyboardInterrupt.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2221"
    name: "BaseException should not be caught"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Catching BaseException catches all exceptions including SystemExit and KeyboardInterrupt, which are used for program termination. This can prevent proper cleanup and shutdown.",
    clean_code: Clear,
    impacts: [Security: Info, Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect except BaseException: or bare except:
        let catch_base = regex::Regex::new(r"except\s+BaseException\s*:\s*(?:#.*)?$").unwrap();
        let bare_except = regex::Regex::new(r"except\s*:\s*(?:#.*)?$").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if catch_base.is_match(trimmed) || bare_except.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S2221",
                    "Catching BaseException or bare except: catches system exits",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Catch Exception instead of BaseException to allow proper program termination via SystemExit and KeyboardInterrupt."
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
    fn test_s2221_registered() {
        let rule = PY_S2221Rule::new();
        assert_eq!(rule.id(), "PY_S2221");
    }

    #[test]
    fn test_s2221_detects_except_base_exception() {
        let rule = PY_S2221Rule::new();
        let smelly = r#"
try:
    operation()
except BaseException:
    cleanup()
"#;
        let issues = with_python_context(smelly, "handler.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect except BaseException:");
        assert_eq!(issues[0].rule_id, "PY_S2221");
    }

    #[test]
    fn test_s2221_detects_bare_except() {
        let rule = PY_S2221Rule::new();
        let smelly = r#"
try:
    operation()
except:
    cleanup()
"#;
        let issues = with_python_context(smelly, "handler.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect bare except:");
    }

    #[test]
    fn test_s2221_allows_except_exception() {
        let rule = PY_S2221Rule::new();
        let clean = r#"
try:
    operation()
except Exception as e:
    handle_error(e)
"#;
        let issues = with_python_context(clean, "handler.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag except Exception:");
    }
}
