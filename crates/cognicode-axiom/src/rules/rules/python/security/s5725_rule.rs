//! S5725 — CSP missing
//!
//! Detects missing Content-Security-Policy header in HTTP responses.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5725"
    name: "Content-Security-Policy header should be set"
    severity: Major
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Without a Content-Security-Policy header, browsers are more susceptible to XSS and data injection attacks. CSP helps prevent unauthorized code execution.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Look for response creation patterns - skip if CSP header is present anywhere in file
        let csp_header = regex::Regex::new(r"Content-Security-Policy").unwrap();
        let response_pattern = regex::Regex::new(r"(HttpResponse|JsonResponse)\s*\(").unwrap();
        
        let has_csp = ctx.source.lines().any(|line| csp_header.is_match(line));
        
        if !has_csp {
            for (line_num, line) in ctx.source.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    continue;
                }
                if response_pattern.is_match(trimmed) {
                    issues.push(Issue::new(
                        "PY_S5725",
                        "Response without Content-Security-Policy header",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Add Content-Security-Policy header to HTTP responses to prevent XSS attacks."
                    )));
                    break; // Only flag once per file
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
    fn test_s5725_registered() {
        let rule = PY_S5725Rule::new();
        assert_eq!(rule.id(), "PY_S5725");
    }

    #[test]
    fn test_s5725_detects_response_without_csp() {
        let rule = PY_S5725Rule::new();
        let smelly = r#"
def my_view(request):
    return HttpResponse("Hello")
"#;
        let issues = with_python_context(smelly, "views.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect response without CSP");
        assert_eq!(issues[0].rule_id, "PY_S5725");
    }

    #[test]
    fn test_s5725_allows_response_with_csp() {
        let rule = PY_S5725Rule::new();
        let clean = r#"
def my_view(request):
    response = HttpResponse("Hello")
    response['Content-Security-Policy'] = "default-src 'self'"
    return response
"#;
        let issues = with_python_context(clean, "views.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag response with CSP");
    }
}
