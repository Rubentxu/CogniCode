//! S1523 — eval()/exec() usage
//!
//! Detects the use of eval() and exec() functions which can execute arbitrary code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1523"
    name: "eval() and exec() functions should not be used"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Using eval() or exec() with user input can allow arbitrary code execution, leading to severe security vulnerabilities including remote code execution attacks.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\b(eval|exec)\s*\(").unwrap();
        for (line_num, line) in ctx.source.lines().enumerate() {
            // Skip comments
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if re.is_match(line) {
                issues.push(Issue::new(
                    "PY_S1523",
                    format!("Use of {} detected - avoid eval/exec to prevent code injection", 
                        if line.contains("eval(") { "eval()" } else { "exec()" }),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::substantial(
                    "Avoid using eval() or exec(). Use safer alternatives like ast.literal_eval() for parsing, or refactor to avoid dynamic code execution."
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
    fn test_s1523_registered() {
        let rule = PY_S1523Rule::new();
        assert_eq!(rule.id(), "PY_S1523");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_s1523_detects_eval() {
        let rule = PY_S1523Rule::new();
        let smelly = r#"
user_input = request.args.get('data')
result = eval(user_input)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect eval() usage");
        assert_eq!(issues[0].rule_id, "PY_S1523");
    }

    #[test]
    fn test_s1523_detects_exec() {
        let rule = PY_S1523Rule::new();
        let smelly = r#"
code = "print('hello')"
exec(code)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect exec() usage");
    }

    #[test]
    fn test_s1523_allows_safe_code() {
        let rule = PY_S1523Rule::new();
        let clean = r#"
# This is a comment
x = 10
y = 20
result = x + y
def safe_function():
    return "This is safe"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag clean code");
    }
}
