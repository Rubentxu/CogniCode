//! P6 — Nil pointer dereference
//!
//! Detects potential nil pointer dereferences.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2259"
    name: "Potential nil pointer dereference"
    severity: Major
    category: Bug
    language: "Go"
    params: {}

    explanation: "Dereferencing a pointer without checking for nil can cause panics.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find pointer dereferences *ptr
        let deref_pattern = regex::Regex::new(r"\*\s*([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();

        for cap in deref_pattern.captures_iter(source) {
            if let Some(ptr_name) = cap.get(1) {
                let ptr_str = ptr_name.as_str();
                let ptr_start = ptr_name.start();

                // Find the line containing this dereference by searching backwards for newline
                let line_start = source[..ptr_start].rfind('\n').map(|p| p + 1).unwrap_or(0);
                let line_end = source[ptr_start..].find('\n').map(|p| ptr_start + p).unwrap_or(source.len());
                let line_content = &source[line_start..line_end];
                let line_num = source[..line_start].lines().count() + 1;

                // Look for nil check in preceding lines
                let nil_check_str = format!("if {} != nil", ptr_str);
                let nil_check_str2 = format!("if {} == nil", ptr_str);

                let mut has_nil_check = false;
                let lines: Vec<&str> = source.lines().collect();
                let deref_line_idx = source[..line_start].lines().count();

                // Search in the 5 lines before the dereference
                let search_start = if deref_line_idx >= 5 { deref_line_idx - 5 } else { 0 };
                for line in &lines[search_start..deref_line_idx] {
                    if line.contains(&nil_check_str) || line.contains(&nil_check_str2) {
                        has_nil_check = true;
                        break;
                    }
                }

                if !has_nil_check {
                    // Skip if it's a type declaration (var or explicit type annotation)
                    let is_type_decl = line_content.trim_start().starts_with("var ")
                        || line_content.trim_start().starts_with("const ")
                        || line_content.trim_start().starts_with("type ")
                        || (line_content.contains(":") && line_content.contains("*"))
                        || (line_content.contains(" *") && !line_content.contains("fmt."))
                        || (line_content.contains("*") && line_content.trim_start().starts_with("func"));

                    if !is_type_decl {
                        issues.push(Issue::new(
                            "GO_S2259",
                            format!("Potential nil pointer dereference of '{}'", ptr_str),
                            Severity::Major,
                            Category::Bug,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Add a nil check before dereferencing the pointer"
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

    fn with_go_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Go.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Go,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_p6_registered() {
        let rule = GO_S2259Rule::new();
        assert_eq!(rule.id(), "GO_S2259");
    }

    #[test]
    fn test_p6_detects_potential_nil_deref() {
        let rule = GO_S2259Rule::new();
        let smelly = r#"
func main() {
    var p *int
    fmt.Println(*p)
}
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect potential nil dereference");
        assert_eq!(issues[0].rule_id, "GO_S2259");
    }

    #[test]
    fn test_p6_allows_nil_checked() {
        let rule = GO_S2259Rule::new();
        let clean = r#"
func main() {
    var p *int
    if p != nil {
        fmt.Println(*p)
    }
}
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag nil-checked dereference");
    }
}
