//! S222 — Instanceof check without cast
//!
//! Detects instanceof followed by casting that could use pattern matching.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S222"
    name: "instanceof check without pattern matching"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "After instanceof check, consider using Java 16+ pattern matching for cleaner code.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find instanceof without pattern variable (old style): instanceof Type)
        // Pattern matching (Java 16+): instanceof Type variableName)
        let lines: Vec<&str> = source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            // Check if line has instanceof followed by type and closing paren (no variable name)
            // Pattern: instanceof String) - old style
            // NOT: instanceof String s) - pattern matching
            let re = regex::Regex::new(r"instanceof\s+(\w+)\s*\)").unwrap();
            if let Some(cap) = re.captures(line) {
                if let Some(type_name) = cap.get(1) {
                    let tn = type_name.as_str();
                    // Look for cast to same type in next few lines
                    let search_region = lines[idx..(idx + 5).min(lines.len())].join("\n");
                    let cast_re = regex::Regex::new(&format!(r"\(\s*{}\s*\)", regex::escape(tn))).unwrap();

                    if cast_re.is_match(&search_region) {
                        let line_num = idx + 1;
                        issues.push(Issue::new(
                            "JAVA_S222",
                            "instanceof check followed by cast could use pattern matching".to_string(),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Use Java 16+ pattern matching: if (obj instanceof Type t) { ... use t directly ... }"
                        )));
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
    fn test_s222_registered() {
        let rule = JAVA_S222Rule::new();
        assert_eq!(rule.id(), "JAVA_S222");
    }

    #[test]
    fn test_s222_detects_instanceof_with_cast() {
        let rule = JAVA_S222Rule::new();
        let smelly = r#"
if (obj instanceof String) {
    String s = (String) obj;
    System.out.println(s.length());
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect instanceof with cast");
        assert_eq!(issues[0].rule_id, "JAVA_S222");
    }

    #[test]
    fn test_s222_allows_pattern_matching() {
        let rule = JAVA_S222Rule::new();
        let clean = r#"
if (obj instanceof String s) {
    System.out.println(s.length());
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag pattern matching");
    }
}
