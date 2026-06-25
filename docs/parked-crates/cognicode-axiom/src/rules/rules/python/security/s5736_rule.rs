//! S5736 — X-Content-Type-Options missing
//!
//! Detects missing X-Content-Type-Options header which prevents MIME-type sniffing.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5736"
    name: "X-Content-Type-Options header should be set to 'nosniff'"
    severity: Major
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Without the X-Content-Type-Options: nosniff header, browsers may MIME-sniff the response, potentially executing malicious content disguised as something else.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let nosniff_header = regex::Regex::new(r"X-Content-Type-Options").unwrap();
        let response_pattern = regex::Regex::new(r"(HttpResponse|JsonResponse)\s*\(").unwrap();
        
        let has_nosniff = ctx.source.lines().any(|line| nosniff_header.is_match(line));
        
        if !has_nosniff {
            for (line_num, line) in ctx.source.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    continue;
                }
                if response_pattern.is_match(trimmed) {
                    issues.push(Issue::new(
                        "PY_S5736",
                        "Response without X-Content-Type-Options header",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Add X-Content-Type-Options: nosniff header to prevent MIME-type sniffing attacks."
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
    fn test_s5736_registered() {
        let rule = PY_S5736Rule::new();
        assert_eq!(rule.id(), "PY_S5736");
    }

    #[test]
    fn test_s5736_detects_response_without_nosniff() {
        let rule = PY_S5736Rule::new();
        let smelly = r#"
def my_view(request):
    return HttpResponse("Hello")
"#;
        let issues = with_python_context(smelly, "views.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect response without X-Content-Type-Options");
        assert_eq!(issues[0].rule_id, "PY_S5736");
    }

    #[test]
    fn test_s5736_allows_response_with_nosniff() {
        let rule = PY_S5736Rule::new();
        let clean = r#"
def my_view(request):
    response = HttpResponse("Hello")
    response['X-Content-Type-Options'] = 'nosniff'
    return response
"#;
        let issues = with_python_context(clean, "views.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag response with nosniff");
    }
}
