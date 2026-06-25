//! S4502 — CSRF disabled
//!
//! Detects CSRF protection being disabled via @csrf_exempt or middleware removal.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S4502"
    name: "CSRF protection should not be disabled"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Disabling CSRF protection makes the application vulnerable to Cross-Site Request Forgery attacks, allowing attackers to perform actions on behalf of authenticated users.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let csrf_exempt = regex::Regex::new(r"@csrf_exempt").unwrap();
        let csrf_disable = regex::Regex::new(r"csrf\.csrf_exempt\s*\(").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if csrf_exempt.is_match(line) || csrf_disable.is_match(line) {
                issues.push(Issue::new(
                    "PY_S4502",
                    "CSRF protection disabled - vulnerable to CSRF attacks",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::substantial(
                    "Remove @csrf_exempt decorator or ensure CsrfViewMiddleware is active. Use CSRF protection for all state-changing requests."
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
    fn test_s4502_registered() {
        let rule = PY_S4502Rule::new();
        assert_eq!(rule.id(), "PY_S4502");
    }

    #[test]
    fn test_s4502_detects_csrf_exempt() {
        let rule = PY_S4502Rule::new();
        let smelly = r#"
@csrf_exempt
def my_view(request):
    return HttpResponse("OK")
"#;
        let issues = with_python_context(smelly, "views.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect @csrf_exempt");
        assert_eq!(issues[0].rule_id, "PY_S4502");
    }

    #[test]
    fn test_s4502_allows_normal_view() {
        let rule = PY_S4502Rule::new();
        let clean = r#"
def my_view(request):
    return HttpResponse("OK")
"#;
        let issues = with_python_context(clean, "views.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal views");
    }
}
