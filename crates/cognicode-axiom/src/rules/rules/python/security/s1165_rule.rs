//! S1165 — Exception swallowed without logging
//!
//! Detects except blocks that swallow exceptions without any logging or re-raising.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1165"
    name: "Exceptions should not be swallowed without logging"
    severity: Major
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Swallowing exceptions with 'pass' makes debugging difficult and can hide failures. Exceptions should be logged or explicitly handled.",
    clean_code: Clear,
    impacts: [Security: Info, Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect except: pass or except Exception: pass patterns
        let except_pass = regex::Regex::new(r"except\s*(Exception)?\s*:\s*(?:#.*)?$").unwrap();
        
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if except_pass.is_match(line) || line.starts_with("except :") || line.starts_with("except Exception:") {
                // Check if next non-empty line is just 'pass'
                if i + 1 < lines.len() {
                    let next_line = lines[i + 1].trim();
                    if next_line == "pass" || next_line == "pass  #" || next_line.starts_with("pass #") {
                        // Also check it's not logging or re-raising
                        let has_logging = (i + 2 < lines.len()) && (
                            lines[i + 2].contains("log") || 
                            lines[i + 2].contains("logger") ||
                            lines[i + 2].contains("raise")
                        );
                        if !has_logging {
                            issues.push(Issue::new(
                                "PY_S1165",
                                "Exception swallowed without logging or re-raising",
                                Severity::Major,
                                Category::Vulnerability,
                                ctx.file_path,
                                i + 1,
                            ).with_remediation(Remediation::moderate(
                                "Log the exception using logger.exception() or logging.error() with exc_info=True, or re-raise the exception."
                            )));
                            i += 2; // Skip the pass line too
                            continue;
                        }
                    }
                }
            }
            i += 1;
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
    fn test_s1165_registered() {
        let rule = PY_S1165Rule::new();
        assert_eq!(rule.id(), "PY_S1165");
    }

    #[test]
    fn test_s1165_detects_swallowed_exception() {
        let rule = PY_S1165Rule::new();
        let smelly = r#"
try:
    risky_operation()
except Exception:
    pass
"#;
        let issues = with_python_context(smelly, "handler.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect swallowed exception");
        assert_eq!(issues[0].rule_id, "PY_S1165");
    }

    #[test]
    fn test_s1165_detects_bare_except_pass() {
        let rule = PY_S1165Rule::new();
        let smelly = r#"
try:
    risky_operation()
except:
    pass
"#;
        let issues = with_python_context(smelly, "handler.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect bare except: pass");
    }

    #[test]
    fn test_s1165_allows_logged_exception() {
        let rule = PY_S1165Rule::new();
        let clean = r#"
try:
    risky_operation()
except Exception:
    logger.exception("Operation failed")
"#;
        let issues = with_python_context(clean, "handler.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag logged exception");
    }
}
