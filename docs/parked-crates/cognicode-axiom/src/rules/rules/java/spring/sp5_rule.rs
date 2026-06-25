//! SP5 — Transactional on private method
//!
//! Detects @Transactional on private methods (will not work).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP5"
    name: "@Transactional on private method will not work"
    severity: Major
    category: Bug
    language: "Java"
    params: {}

    explanation: "@Transactional uses Spring's AOP proxy which only intercepts public method calls through the proxy. Private methods are invoked directly on the target bean, bypassing the transaction proxy.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @Transactional followed by private/protected method declaration
        // This handles both same-line and multi-line cases
        let transactional_re = regex::Regex::new(r"@Transactional").unwrap();
        let method_re = regex::Regex::new(r"(private|protected)\s+\w+\s+(\w+)\s*\(").unwrap();

        for m in transactional_re.find_iter(source) {
            // Check the next few lines for a private/protected method
            let start = m.end();
            let search_region = &source[start..start + 200.min(source.len() - start)];

            if let Some(cap) = method_re.captures(search_region) {
                if let Some(method_name) = cap.get(2) {
                    let visibility = cap.get(1).map(|v| v.as_str()).unwrap_or("");
                    let line_num = source[..m.start()].lines().count() + 1;

                    issues.push(Issue::new(
                        "JAVA_SP5",
                        format!("@Transactional on {} method '{}' will not work", visibility, method_name.as_str()),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Move the @Transactional annotation to a public method or make this method public"
                    )));
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
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

    fn with_java_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Java.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Java,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_sp5_registered() {
        let rule = JAVA_SP5Rule::new();
        assert_eq!(rule.id(), "JAVA_SP5");
    }

    #[test]
    fn test_sp5_detects_private_transactional() {
        let rule = JAVA_SP5Rule::new();
        let smelly = r#"
@Service
public class MyService {
    @Transactional
    private void doSomething() { }
}
"#;
        let issues = with_java_context(smelly, "MyService.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect @Transactional on private method");
        assert_eq!(issues[0].rule_id, "JAVA_SP5");
    }

    #[test]
    fn test_sp5_allows_public_transactional() {
        let rule = JAVA_SP5Rule::new();
        let clean = r#"
@Service
public class MyService {
    @Transactional
    public void doSomething() { }
}
"#;
        let issues = with_java_context(clean, "MyService.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag @Transactional on public method");
    }
}
