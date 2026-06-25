//! S4784 — ReDoS (Regular Expression Denial of Service)
//!
//! Detects potentially vulnerable regex patterns with nested quantifiers.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S4784"
    name: "Regular expressions with nested quantifiers should be avoided"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Regex patterns with nested quantifiers like (a+)+ can cause catastrophic backtracking, leading to denial of service when processing malicious input.",
    clean_code: Focused,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect nested quantifiers: (X+)+, (X*)+, (X+)*, (X*)*, etc.
        let nested_quantifiers = [
            r"\(\w*\+\)\+",   // (a+)+
            r"\(\w*\+\)\*",   // (a+)*
            r"\(\w*\*\)\+",   // (a*)*
            r"\(\w*\*\)\*",   // (a*)*
            r"\(\w+\+\)\+",   // (a+)+
            r"\(\w+\+\)\*",   // (a+)*
            r"\(\w+\*\)\+",   // (a*)*
            r"\(\w+\*\)\*",   // (a*)*
        ];
        let re = nested_quantifiers.iter()
            .map(|p| regex::Regex::new(p).unwrap())
            .collect::<Vec<_>>();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            // Only check lines with regex-related content
            if line.contains("re.compile") || line.contains("re.match") || 
               line.contains("re.search") || line.contains("re.findall") ||
               line.contains("regex") || line.contains("Pattern") {
                for regex in &re {
                    if regex.is_match(line) {
                        issues.push(Issue::new(
                            "PY_S4784",
                            format!("Potentially vulnerable regex: nested quantifiers detected"),
                            Severity::Critical,
                            Category::Vulnerability,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::moderate(
                            "Restructure the regex to avoid nested quantifiers, or use atomic groups or possessive quantifiers."
                        )));
                        break;
                    }
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
    fn test_s4784_registered() {
        let rule = PY_S4784Rule::new();
        assert_eq!(rule.id(), "PY_S4784");
    }

    #[test]
    fn test_s4784_detects_nested_quantifiers() {
        let rule = PY_S4784Rule::new();
        let smelly = r#"
pattern = re.compile(r'(a+)+$')
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect nested quantifiers");
        assert_eq!(issues[0].rule_id, "PY_S4784");
    }

    #[test]
    fn test_s4784_allows_safe_regex() {
        let rule = PY_S4784Rule::new();
        let clean = r#"
pattern = re.compile(r'[a-z]+$')
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag safe regex patterns");
    }
}
