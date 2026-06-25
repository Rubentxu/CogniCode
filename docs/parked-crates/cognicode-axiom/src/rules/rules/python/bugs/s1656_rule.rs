//! S1656 — Self-assignment
//!
//! Detects self-assignment statements like x = x or self.x = self.x.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1656"
    name: "Self-assignment should not be used"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "Self-assignment like 'x = x' has no effect and indicates a bug or copy-paste error.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect x = x, self.x = self.x patterns
        // Simple assignment pattern
        let assign_re = regex::Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(.+)\s*$").unwrap();
        // self attribute assignment
        let self_attr_assign_re = regex::Regex::new(r"^\s*self\.([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*self\.([a-zA-Z_][a-zA-Z0-9_]*)\s*$").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            
            // Check for self.x = self.x
            if let Some(caps) = self_attr_assign_re.captures(trimmed) {
                if let (Some(attr1), Some(attr2)) = (caps.get(1), caps.get(2)) {
                    if attr1.as_str() == attr2.as_str() {
                        issues.push(Issue::new(
                            "PY_S1656",
                            "Self-assignment has no effect",
                            Severity::Minor,
                            Category::Bug,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Remove the self-assignment statement as it has no effect."
                        )));
                        continue;
                    }
                }
            }
            
            // Check for x = x
            if let Some(caps) = assign_re.captures(trimmed) {
                if let (Some(var), Some(rhs)) = (caps.get(1), caps.get(2)) {
                    let var_str = var.as_str();
                    let rhs_str = rhs.as_str().trim();
                    if var_str == rhs_str {
                        issues.push(Issue::new(
                            "PY_S1656",
                            "Self-assignment has no effect",
                            Severity::Minor,
                            Category::Bug,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Remove the self-assignment statement as it has no effect."
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
    fn test_s1656_registered() {
        let rule = PY_S1656Rule::new();
        assert_eq!(rule.id(), "PY_S1656");
    }

    #[test]
    fn test_s1656_detects_self_assignment() {
        let rule = PY_S1656Rule::new();
        let smelly = r#"
def foo():
    x = 1
    x = x
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect self-assignment");
        assert_eq!(issues[0].rule_id, "PY_S1656");
    }

    #[test]
    fn test_s1656_detects_self_attr_assignment() {
        let rule = PY_S1656Rule::new();
        let smelly = r#"
class Foo:
    def bar(self):
        self.x = self.x
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect self.x = self.x");
    }

    #[test]
    fn test_s1656_allows_normal_assignment() {
        let rule = PY_S1656Rule::new();
        let clean = r#"
def foo():
    x = 1
    y = x
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal assignment");
    }
}
