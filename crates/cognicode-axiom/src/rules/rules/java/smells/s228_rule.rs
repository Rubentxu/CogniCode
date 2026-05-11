//! S228 — Thread created but not started
//!
//! Detects Thread instances created but never started.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S228"
    name: "Thread created but never started"
    severity: Major
    category: Bug
    language: "Java"
    params: {}

    explanation: "Creating a Thread and not calling start() means the code in run() will never execute.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find Thread construction without start()
        let thread_pattern = regex::Regex::new(r"Thread\s+\w+\s*=\s*new\s+Thread\s*\(").unwrap();

        for cap in thread_pattern.find_iter(source) {
            let thread_pos = cap.start();
            let after_thread = &source[thread_pos..];

            // Check if start() is called on this thread instance within reasonable distance
            let has_start = after_thread.lines().take(20).any(|line| line.contains(".start()"));

            if !has_start {
                let line_num = source[..thread_pos].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_S228",
                    "Thread created but start() may never be called".to_string(),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Call start() on the thread or use ExecutorService instead"
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
    fn test_s228_registered() {
        let rule = JAVA_S228Rule::new();
        assert_eq!(rule.id(), "JAVA_S228");
    }

    #[test]
    fn test_s228_detects_unstarted_thread() {
        let rule = JAVA_S228Rule::new();
        let smelly = r#"
Thread t = new Thread(() -> System.out.println("hello"));
System.out.println("Thread created");
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unstarted thread");
        assert_eq!(issues[0].rule_id, "JAVA_S228");
    }

    #[test]
    fn test_s228_allows_started_thread() {
        let rule = JAVA_S228Rule::new();
        let clean = r#"
Thread t = new Thread(() -> System.out.println("hello"));
t.start();
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag started thread");
    }
}
