//! SP8 — Scheduled without fixed delay
//!
//! Detects @Scheduled without fixedDelay or fixedRate.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP8"
    name: "@Scheduled should specify fixed delay or rate"
    severity: Major
    category: Bug
    language: "Java"
    params: {}

    explanation: "@Scheduled methods without fixedDelay or fixedRate may cause issues. Without explicit timing, the behavior depends on the task execution policy.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @Scheduled annotations and check if they have fixedDelay or fixedRate
        let scheduled_re = regex::Regex::new(r"@Scheduled(?:\([^)]*\))?").unwrap();

        for cap in scheduled_re.find_iter(source) {
            let annotation = cap.as_str();
            // If annotation has parentheses, check if they contain fixedDelay or fixedRate
            if annotation.contains('(') {
                if !annotation.contains("fixedDelay") && !annotation.contains("fixedRate") {
                    let line_num = source[..cap.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JAVA_SP8",
                        "@Scheduled without fixedDelay or fixedRate".to_string(),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Add fixedDelay or fixedRate: @Scheduled(fixedDelay = 5000)"
                    )));
                }
            } else {
                // @Scheduled without parentheses
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_SP8",
                    "@Scheduled without fixedDelay or fixedRate".to_string(),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Add fixedDelay or fixedRate: @Scheduled(fixedDelay = 5000)"
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
    fn test_sp8_registered() {
        let rule = JAVA_SP8Rule::new();
        assert_eq!(rule.id(), "JAVA_SP8");
    }

    #[test]
    fn test_sp8_detects_scheduled_without_delay() {
        let rule = JAVA_SP8Rule::new();
        let smelly = r#"
@Component
public class MyService {
    @Scheduled
    public void doSomething() { }
}
"#;
        let issues = with_java_context(smelly, "MyService.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect @Scheduled without fixedDelay");
        assert_eq!(issues[0].rule_id, "JAVA_SP8");
    }

    #[test]
    fn test_sp8_allows_scheduled_with_delay() {
        let rule = JAVA_SP8Rule::new();
        let clean = r#"
@Component
public class MyService {
    @Scheduled(fixedDelay = 5000)
    public void doSomething() { }
}
"#;
        let issues = with_java_context(clean, "MyService.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag @Scheduled with fixedDelay");
    }
}
