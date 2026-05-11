//! S1 — Hardcoded secrets
//!
//! Detects hardcoded passwords, API keys, and secrets in source code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2068"
    name: "Hardcoded passwords and secrets should not be present"
    severity: Critical
    category: Vulnerability
    language: "Go"
    params: {}

    explanation: "Hardcoded credentials can be extracted by anyone with access to the source code, leading to unauthorized access and data breaches.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Patterns for hardcoded credentials
        let patterns = [
            r#"(?i)password\s*:=?\s*["'][^"']+["']"#,
            r#"(?i)api[_-]?key\s*:=?\s*["'][^"']+["']"#,
            r#"(?i)secret\s*:=?\s*["'][^"']+["']"#,
            r#"(?i)token\s*:=?\s*["'][^"']+["']"#,
            r#"(?i)auth\s*:=?\s*["'][^"']+["']"#,
            r"aws[_-]?(access[_-]?key|secret)",
            r#"(?i)private[_-]?key\s*:=?\s*["'][^"']+["']"#,
        ];

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }
            for pattern in &patterns {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(line) {
                        issues.push(Issue::new(
                            "GO_S2068",
                            format!("Hardcoded credential detected: {}", trimmed),
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
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    fn with_go_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Go.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Go,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s1_registered() {
        let rule = GO_S2068Rule::new();
        assert_eq!(rule.id(), "GO_S2068");
    }

    #[test]
    fn test_s1_detects_password() {
        let rule = GO_S2068Rule::new();
        let smelly = r#"
password := "supersecret123"
"#;
        let issues = with_go_context(smelly, "config.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect hardcoded password");
        assert_eq!(issues[0].rule_id, "GO_S2068");
    }

    #[test]
    fn test_s1_detects_api_key() {
        let rule = GO_S2068Rule::new();
        let smelly = r#"
apiKey := "sk-abc123xyz789"
"#;
        let issues = with_go_context(smelly, "app.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect API key");
    }

    #[test]
    fn test_s1_allows_clean_code() {
        let rule = GO_S2068Rule::new();
        let clean = r#"
password := os.Getenv("PASSWORD")
apiKey := getSecret("api_key")
"#;
        let issues = with_go_context(clean, "app.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag environment variable usage");
    }
}
