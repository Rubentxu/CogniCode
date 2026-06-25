//! S1163 — Catch-all except Exception: pass
//!
//! Detects catch-all except blocks that swallow exceptions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1163"
    name: "Catch-all except blocks should not swallow exceptions"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using 'except:' or 'except Exception:' without specific exception types or proper handling can mask bugs and make debugging difficult.",
    clean_code: Clear,
    impacts: [Security: Info, Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect bare except: or except Exception: (with possible variable)
        let catch_all = regex::Regex::new(r"except\s*(\s*Exception\s*(,\s*\w+)?)?\s*:\s*(?:#.*)?$").unwrap();
        let bare_except = regex::Regex::new(r"except\s*:\s*(?:#.*)?$").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if bare_except.is_match(trimmed) || (trimmed.starts_with("except Exception") && catch_all.is_match(trimmed)) {
                issues.push(Issue::new(
                    "PY_S1163",
                    "Catch-all except block detected - be more specific",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Catch specific exception types instead of using bare 'except:' or 'except Exception:'. This makes error handling more precise and maintainable."
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
    fn test_s1163_registered() {
        let rule = PY_S1163Rule::new();
        assert_eq!(rule.id(), "PY_S1163");
    }

    #[test]
    fn test_s1163_detects_bare_except() {
        let rule = PY_S1163Rule::new();
        let smelly = r#"
try:
    risky_operation()
except:
    handle_error()
"#;
        let issues = with_python_context(smelly, "handler.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect bare except:");
        assert_eq!(issues[0].rule_id, "PY_S1163");
    }

    #[test]
    fn test_s1163_detects_except_exception() {
        let rule = PY_S1163Rule::new();
        let smelly = r#"
try:
    risky_operation()
except Exception:
    handle_error()
"#;
        let issues = with_python_context(smelly, "handler.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect except Exception:");
    }

    #[test]
    fn test_s1163_allows_specific_exception() {
        let rule = PY_S1163Rule::new();
        let clean = r#"
try:
    risky_operation()
except ValueError as e:
    handle_value_error(e)
except IOError as e:
    handle_io_error(e)
"#;
        let issues = with_python_context(clean, "handler.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag specific exception types");
    }
}
