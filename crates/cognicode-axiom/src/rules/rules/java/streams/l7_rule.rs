//! L22 — Optional.flatMap(Function.identity()) simplification
//!
//! Detects `flatMap(Function.identity())` which should be `map()`.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L22"
    name: "Optional.flatMap(Function.identity()) should be map()"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using flatMap(Function.identity()) on an Optional is redundant. Use map() instead.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .flatMap(Function.identity())
        let pattern = regex::Regex::new(r"\.flatMap\s*\(\s*Function\s*\.\s*identity\s*\(\s*\)\s*\)").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L22",
                    "Optional.flatMap(Function.identity()) should be map()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Replace flatMap(Function.identity()) with map(Function.identity())."
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
    use cognicode_core::infrastructure::parser::Language;

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
    fn test_l22_registered() {
        let rule = JAVA_L22Rule::new();
        assert_eq!(rule.id(), "JAVA_L22");
    }

    #[test]
    fn test_l22_detects_flatmap_identity() {
        let rule = JAVA_L22Rule::new();
        let smelly = r#"
Optional<Optional<String>> result = maybeOpt.flatMap(Function.identity());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(
            !issues.is_empty(),
            "Should detect flatMap(Function.identity())"
        );
        assert_eq!(issues[0].rule_id, "JAVA_L22");
    }

    #[test]
    fn test_l22_allows_proper_flatmap() {
        let rule = JAVA_L22Rule::new();
        let clean = r#"
Optional<String> result = maybeOpt.flatMap(s -> compute(s));
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag proper flatMap");
    }
}
