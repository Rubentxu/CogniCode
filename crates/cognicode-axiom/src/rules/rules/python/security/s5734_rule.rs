//! S5734 — HSTS missing
//!
//! Detects missing Strict-Transport-Security header which forces HTTPS connections.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5734"
    name: "Strict-Transport-Security header should be set"
    severity: Major
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Without the Strict-Transport-Security (HSTS) header, browsers may allow unencrypted HTTP connections, exposing users to man-in-the-middle attacks.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let hsts_header = regex::Regex::new(r"Strict-Transport-Security").unwrap();
        let response_pattern = regex::Regex::new(r"(HttpResponse|JsonResponse)\s*\(").unwrap();
        
        let has_hsts = ctx.source.lines().any(|line| hsts_header.is_match(line));
        
        if !has_hsts {
            for (line_num, line) in ctx.source.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    continue;
                }
                if response_pattern.is_match(trimmed) {
                    issues.push(Issue::new(
                        "PY_S5734",
                        "Response without Strict-Transport-Security header",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Add Strict-Transport-Security header (e.g., 'max-age=31536000; includeSubDomains') to enforce HTTPS."
                    )));
                    break;
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
    fn test_s5734_registered() {
        let rule = PY_S5734Rule::new();
        assert_eq!(rule.id(), "PY_S5734");
    }

    #[test]
    fn test_s5734_detects_response_without_hsts() {
        let rule = PY_S5734Rule::new();
        let smelly = r#"
def my_view(request):
    return HttpResponse("Hello")
"#;
        let issues = with_python_context(smelly, "views.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect response without HSTS");
        assert_eq!(issues[0].rule_id, "PY_S5734");
    }

    #[test]
    fn test_s5734_allows_response_with_hsts() {
        let rule = PY_S5734Rule::new();
        let clean = r#"
def my_view(request):
    response = HttpResponse("Hello")
    response['Strict-Transport-Security'] = 'max-age=31536000'
    return response
"#;
        let issues = with_python_context(clean, "views.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag response with HSTS");
    }
}
