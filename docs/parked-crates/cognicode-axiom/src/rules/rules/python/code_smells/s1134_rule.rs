//! S1134 — Deprecated API usage
//!
//! Detects usage of deprecated Python APIs.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1134"
    name: "Deprecated API should not be used"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using deprecated APIs may cause compatibility issues in future versions. Use the recommended alternative.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        // Common deprecated patterns
        let deprecated_patterns = [
            (r"\bmd5\s*\(", "md5 (use hashlib.md5)"),
            (r"\bsha\s*\(", "sha (use hashlib.sha256)"),
            (r"\bapply\s*\(", "apply (removed in Python 3)"),
            (r"\breload\s*\(", "reload (use importlib.reload)"),
            (r"\bexecfile\s*\(", "execfile (removed in Python 3)"),
            (r"\bfile\s*\(", "file (use open)"),
            (r"\braw_input\s*\(", "raw_input (use input)"),
            (r"\bintern\s*\(", "intern (use sys.intern in Python 3)"),
            (r"\bunicode\s*\(", "unicode (use str)"),
            (r"\bxrange\s*\(", "xrange (use range)"),
        ];

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Skip if using hashlib.md5 or hashlib.sha etc
            if trimmed.contains("hashlib.md5") || trimmed.contains("hashlib.sha") {
                continue;
            }
            for (pattern, description) in &deprecated_patterns {
                let re = regex::Regex::new(pattern).unwrap();
                if re.is_match(trimmed) {
                    issues.push(Issue::new(
                        "PY_S1134",
                        format!("Deprecated API used: {} - {}", description, pattern),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Use the non-deprecated alternative."
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
    fn test_s1134_registered() {
        let rule = PY_S1134Rule::new();
        assert_eq!(rule.id(), "PY_S1134");
    }

    #[test]
    fn test_s1134_detects_deprecated() {
        let rule = PY_S1134Rule::new();
        let smelly = r#"
import hashlib
result = md5(b"data")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect deprecated API usage");
        assert_eq!(issues[0].rule_id, "PY_S1134");
    }

    #[test]
    fn test_s1134_allows_modern_api() {
        let rule = PY_S1134Rule::new();
        let clean = r#"
import hashlib
result = hashlib.md5(b"data")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag modern API usage");
    }
}
