//! S226 — Loop with size() in condition
//!
//! Detects loops calling size() in the condition which may be called repeatedly.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S226"
    name: "Loop with collection.size() in condition"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Calling collection.size() in a loop condition may be inefficient if size() is O(n). Cache the size before the loop.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find loops with .size() in condition
        let pattern = regex::Regex::new(r"(for|while)\s*\([^)]*\.size\s*\(\s*\)[^)]*\)").unwrap();

        for cap in pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "JAVA_S226",
                "Loop condition calls .size() which may be inefficient".to_string(),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Cache the collection size before the loop: int len = coll.size(); for (...)"
            )));
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
    fn test_s226_registered() {
        let rule = JAVA_S226Rule::new();
        assert_eq!(rule.id(), "JAVA_S226");
    }

    #[test]
    fn test_s226_detects_size_in_loop() {
        let rule = JAVA_S226Rule::new();
        let smelly = r#"
for (int i = 0; i < list.size(); i++) {
    System.out.println(list.get(i));
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect size() in loop condition");
        assert_eq!(issues[0].rule_id, "JAVA_S226");
    }

    #[test]
    fn test_s226_allows_cached_size() {
        let rule = JAVA_S226Rule::new();
        let clean = r#"
int len = list.size();
for (int i = 0; i < len; i++) {
    System.out.println(list.get(i));
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag cached size");
    }
}
