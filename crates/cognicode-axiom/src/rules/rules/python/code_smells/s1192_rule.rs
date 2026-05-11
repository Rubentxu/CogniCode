//! S1192 — String literal duplicates
//!
//! Detects repeated string literals that should be constants.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;
use std::collections::HashMap;

declare_rule! {
    id: "PY_S1192"
    name: "String literals should not be duplicated"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Duplicate string literals make maintenance harder. Define them as constants instead.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let threshold = 3;

        let mut string_counts: HashMap<String, Vec<usize>> = HashMap::new();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Extract string literals (simple regex heuristic)
            let single_string = regex::Regex::new(r#""([^"\\]|\\.)*""#).unwrap();
            let single_string_sq = regex::Regex::new(r#"'([^'\\]|\\.)*'"#).unwrap();

            for cap in single_string.find_iter(line) {
                let content = cap.as_str().trim_matches('"');
                if content.len() >= 3 && !content.starts_with('\\') {
                    string_counts.entry(content.to_string()).or_default().push(line_num + 1);
                }
            }
            for cap in single_string_sq.find_iter(line) {
                let content = cap.as_str().trim_matches('\'');
                if content.len() >= 3 && !content.starts_with('\\') {
                    string_counts.entry(content.to_string()).or_default().push(line_num + 1);
                }
            }
        }

        for (string, locations) in string_counts {
            if locations.len() >= threshold {
                issues.push(Issue::new(
                    "PY_S1192",
                    format!("String literal '{}' appears {} times - consider using a constant", string, locations.len()),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    locations[0],
                ).with_remediation(Remediation::quick(
                    "Define this string as a module-level constant."
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
    fn test_s1192_registered() {
        let rule = PY_S1192Rule::new();
        assert_eq!(rule.id(), "PY_S1192");
    }

    #[test]
    fn test_s1192_detects_duplicates() {
        let rule = PY_S1192Rule::new();
        let smelly = r#"
print("error")
print("error")
print("error")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect duplicate string literals");
        assert_eq!(issues[0].rule_id, "PY_S1192");
    }

    #[test]
    fn test_s1192_allows_unique_strings() {
        let rule = PY_S1192Rule::new();
        let clean = r#"
print("one")
print("two")
print("three")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag unique string literals");
    }
}
