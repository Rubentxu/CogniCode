//! S5247 — XSS in templates
//!
//! Detects potentially unsafe template rendering that could enable XSS attacks.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5247"
    name: "Templates should not directly render unsanitized user input"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Rendering user input directly in templates without proper escaping enables cross-site scripting (XSS) attacks.",
    clean_code: Focused,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect |safe filter and mark_safe in Jinja2 templates
        let patterns = [
            r"\|\s*safe\b",           // |safe filter
            r"mark_safe\s*\(",        // mark_safe() call
            r"Markup\s*\(\s*",        // Markup() constructor
            r"autoescape\s*=\s*False", // autoescape disabled
        ];
        let re = patterns.iter()
            .map(|p| regex::Regex::new(p).unwrap())
            .collect::<Vec<_>>();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            for regex in &re {
                if regex.is_match(line) {
                    issues.push(Issue::new(
                        "PY_S5247",
                        format!("Potential XSS: template marked as safe without proper sanitization"),
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Ensure user input is properly escaped. Only use |safe or mark_safe() on trusted, sanitized content."
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
    fn test_s5247_registered() {
        let rule = PY_S5247Rule::new();
        assert_eq!(rule.id(), "PY_S5247");
    }

    #[test]
    fn test_s5247_detects_safe_filter() {
        let rule = PY_S5247Rule::new();
        let smelly = r#"
{{ user_input | safe }}
"#;
        let issues = with_python_context(smelly, "template.html", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect |safe filter");
        assert_eq!(issues[0].rule_id, "PY_S5247");
    }

    #[test]
    fn test_s5247_detects_mark_safe() {
        let rule = PY_S5247Rule::new();
        let smelly = r#"
from django.utils.safestring import mark_safe
html = mark_safe(user_content)
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect mark_safe");
    }

    #[test]
    fn test_s5247_allows_safe_code() {
        let rule = PY_S5247Rule::new();
        let clean = r#"
{{ user_input | escape }}
"#;
        let issues = with_python_context(clean, "template.html", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag properly escaped content");
    }
}
