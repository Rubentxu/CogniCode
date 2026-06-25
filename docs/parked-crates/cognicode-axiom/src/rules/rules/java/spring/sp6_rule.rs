//! SP6 — Async without thread pool config
//!
//! Detects @Async methods without a configured thread pool.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP6"
    name: "@Async without thread pool configuration"
    severity: Major
    category: Bug
    language: "Java"
    params: {}

    explanation: "@Async methods use a default SimpleAsyncTaskExecutor which creates a new thread for each task. Configure a ThreadPoolTaskExecutor for better performance.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @Async without explicit executor
        let async_pattern = regex::Regex::new(r"@Async\s*(?:\(\s*\))?\s*(?:public\s+)?(\w+)").unwrap();

        for cap in async_pattern.captures_iter(source) {
            let full_match = cap.get(0).unwrap().as_str();
            // Check if @Async has a value argument (custom executor)
            let has_executor = full_match.contains("(");

            if !has_executor {
                let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
                let method_name = cap.get(1).map(|m| m.as_str()).unwrap_or("unknown");

                issues.push(Issue::new(
                    "JAVA_SP6",
                    format!("@Async method '{}' should configure a thread pool executor", method_name),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Configure a ThreadPoolTaskExecutor bean and reference it: @Async(\"taskExecutor\")"
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
    fn test_sp6_registered() {
        let rule = JAVA_SP6Rule::new();
        assert_eq!(rule.id(), "JAVA_SP6");
    }

    #[test]
    fn test_sp6_detects_async_without_executor() {
        let rule = JAVA_SP6Rule::new();
        let smelly = r#"
@Service
public class MyService {
    @Async
    public void doAsync() { }
}
"#;
        let issues = with_java_context(smelly, "MyService.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect @Async without executor config");
        assert_eq!(issues[0].rule_id, "JAVA_SP6");
    }

    #[test]
    fn test_sp6_allows_async_with_executor() {
        let rule = JAVA_SP6Rule::new();
        let clean = r#"
@Service
public class MyService {
    @Async("taskExecutor")
    public void doAsync() { }
}
"#;
        let issues = with_java_context(clean, "MyService.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag @Async with executor");
    }
}
