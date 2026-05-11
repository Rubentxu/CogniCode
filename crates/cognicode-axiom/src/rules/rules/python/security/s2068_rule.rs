//! S2068 — Hardcoded credentials
//!
//! Detects hardcoded passwords, API keys, and secrets in source code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2068"
    name: "Hardcoded passwords and secrets should not be present"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Hardcoded credentials can be extracted by anyone with access to the source code, leading to unauthorized access and data breaches.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Patterns for hardcoded credentials
        let patterns = [
            r#"(?i)password\s*=\s*["'][^"']+["']"#,
            r#"(?i)api[_-]?key\s*=\s*["'][^"']+["']"#,
            r#"(?i)secret\s*=\s*["'][^"']+["']"#,
            r#"(?i)token\s*=\s*["'][^"']+["']"#,
            r#"(?i)auth\s*=\s*["'][^"']+["']"#,
            r"aws[_-]?(access[_-]?key|secret)", 
            r#"(?i)private[_-]?key\s*=\s*["'][^"']+["']"#,
        ];
        let re = patterns.iter()
            .map(|p| regex::Regex::new(p).unwrap())
            .collect::<Vec<_>>();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Skip comments and docstrings
            if trimmed.starts_with('#') || trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                continue;
            }
            for regex in &re {
                if regex.is_match(line) {
                    issues.push(Issue::new(
                        "PY_S2068",
                        format!("Hardcoded credential detected: {}", trimmed.lines().next().unwrap_or(trimmed)),
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Use environment variables or a secrets manager to store credentials."
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
    fn test_s2068_registered() {
        let rule = PY_S2068Rule::new();
        assert_eq!(rule.id(), "PY_S2068");
    }

    #[test]
    fn test_s2068_detects_password() {
        let rule = PY_S2068Rule::new();
        let smelly = r#"
db_password = "supersecret123"
"#;
        let issues = with_python_context(smelly, "config.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect hardcoded password");
        assert_eq!(issues[0].rule_id, "PY_S2068");
    }

    #[test]
    fn test_s2068_detects_api_key() {
        let rule = PY_S2068Rule::new();
        let smelly = r#"
api_key = "sk-abc123xyz789"
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect API key");
    }

    #[test]
    fn test_s2068_allows_clean_code() {
        let rule = PY_S2068Rule::new();
        let clean = r#"
# This is a comment
password = os.environ.get("PASSWORD")
api_key = get_secret("api_key")
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag environment variable usage");
    }
}
