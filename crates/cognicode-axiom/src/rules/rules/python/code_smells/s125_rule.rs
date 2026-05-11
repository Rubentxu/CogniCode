//! S125 — Commented-out code
//!
//! Detects commented-out code that should be removed.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S125"
    name: "Commented-out code should be removed"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Commented-out code makes the codebase harder to understand and maintain. Remove it or use version control.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("#") {
                let content_after_hash = trimmed.trim_start_matches('#').trim();
                // Skip shebang, docstring markers, and type: ignore
                if content_after_hash.is_empty() || content_after_hash.starts_with("!") || content_after_hash.starts_with("type:") || content_after_hash.starts_with(" noqa") {
                    continue;
                }
                // Check if commented line looks like executable code
                let looks_like_code = content_after_hash.starts_with("def ") ||
                    content_after_hash.starts_with("class ") ||
                    content_after_hash.starts_with("if ") ||
                    content_after_hash.starts_with("for ") ||
                    content_after_hash.starts_with("while ") ||
                    content_after_hash.starts_with("return ") ||
                    content_after_hash.starts_with("import ") ||
                    content_after_hash.starts_with("from ") ||
                    content_after_hash.starts_with("=") ||
                    content_after_hash.starts_with("print(") ||
                    content_after_hash.starts_with("self.") ||
                    regex::Regex::new(r"^\w+\s*\(").unwrap().is_match(content_after_hash);
                if looks_like_code && content_after_hash.len() > 5 {
                    issues.push(Issue::new(
                        "PY_S125",
                        format!("Commented-out code found at line {}", line_num + 1),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Remove commented-out code. Use version control to preserve history."
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
    fn test_s125_registered() {
        let rule = PY_S125Rule::new();
        assert_eq!(rule.id(), "PY_S125");
    }

    #[test]
    fn test_s125_detects_commented_code() {
        let rule = PY_S125Rule::new();
        let smelly = r#"
def old_function():
    # return process_data()
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect commented-out code");
        assert_eq!(issues[0].rule_id, "PY_S125");
    }

    #[test]
    fn test_s125_allows_normal_comments() {
        let rule = PY_S125Rule::new();
        let clean = r#"
def process():
    # This is a normal comment explaining the logic
    return True
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal comments");
    }
}
