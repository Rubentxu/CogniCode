//! S5693 — File upload without size limit
//!
//! Detects file upload handlers that don't enforce size limits.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5693"
    name: "File uploads should enforce size limits"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Without file size limits, attackers can upload large files to exhaust server disk space or cause denial of service.",
    clean_code: Focused,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect file upload patterns without size checks
        let upload_re = regex::Regex::new(r"request\.files\s*\[|file\s*\.\s*save\s*\(|upload\s*").unwrap();
        let size_check_re = regex::Regex::new(r"(MAX_CONTENT_LENGTH|content_length|file\.content_length|\.tell\(\))\s*(<|>|<=|>=|==)").unwrap();
        
        // Simplified approach: just check for request.files without size validation nearby
        let lines: Vec<_> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if upload_re.is_match(line) {
                let start = idx.saturating_sub(5);
                let end = (idx + 3).min(lines.len());
                let context = &lines[start..end].join("\n");
                if !size_check_re.is_match(context) && !context.contains("MAX_CONTENT_LENGTH") {
                    issues.push(Issue::new(
                        "PY_S5693",
                        format!("File upload without size limit - denial of service risk"),
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Enforce MAX_CONTENT_LENGTH or validate file size before saving."
                    )));
                    break;
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
    fn test_s5693_registered() {
        let rule = PY_S5693Rule::new();
        assert_eq!(rule.id(), "PY_S5693");
    }

    #[test]
    fn test_s5693_detects_upload_without_limit() {
        let rule = PY_S5693Rule::new();
        let smelly = r#"
@app.route('/upload', methods=['POST'])
def upload():
    file = request.files['file']
    file.save('/uploads/' + file.filename)
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect upload without size limit");
        assert_eq!(issues[0].rule_id, "PY_S5693");
    }

    #[test]
    fn test_s5693_allows_upload_with_limit() {
        let rule = PY_S5693Rule::new();
        let clean = r#"
@app.route('/upload', methods=['POST'])
def upload():
    if request.content_length > MAX_CONTENT_LENGTH:
        abort(413)
    file = request.files['file']
    file.save('/uploads/' + file.filename)
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag upload with size check");
    }
}
