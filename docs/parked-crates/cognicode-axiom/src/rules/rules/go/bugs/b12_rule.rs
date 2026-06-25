//! B12 — Error returned but not checked
//!
//! Detects function calls where error return is not checked.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1160"
    name: "Error return value is not checked"
    severity: Major
    category: Bug
    language: "Go"
    params: {}

    explanation: "Functions that return errors should have their error values checked.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Common functions that return (value, error)
        let error_returning_funcs = [
            "os.Open", "ioutil.ReadFile", "ioutil.WriteFile", "ioutil.ReadAll",
            "json.Marshal", "json.Unmarshal", "xml.Marshal", "xml.Unmarshal",
            "io.ReadFull", "io.Copy", "http.Get", "http.Post",
            "strconv.Atoi", "strconv.ParseInt", "strconv.ParseFloat",
        ];

        for func_name in &error_returning_funcs {
            let escaped = regex::escape(func_name);
            // Only flag: func() alone on a line OR _, err := func() OR data, _ := func()
            // NOT: data, err := func() (error IS captured)
            let line_pattern = format!(r"(?m){}\s*\([^)]*\)", escaped);
            if let Ok(re) = regex::Regex::new(&line_pattern) {
                for cap in re.find_iter(source) {
                    let line_num = source[..cap.start()].lines().count() + 1;
                    let line_start = source[..cap.start()].rfind('\n').map(|p| p + 1).unwrap_or(0);
                    let full_line = &source[line_start..];

                    // Skip if error is properly captured with 'err' variable
                    // Pattern: data, err := func() or err := func()
                    let err_capture = regex::Regex::new(r",\s*err\s*:?=").unwrap();
                    if err_capture.is_match(full_line) {
                        continue;
                    }

                    // Skip if it's inside an if-check already (e.g., if err != nil)
                    if full_line.trim_start().starts_with("if") {
                        continue;
                    }

                    issues.push(Issue::new(
                        "GO_S1160",
                        format!("Error return value of '{}' is not checked", func_name),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Check the error return value"
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
    fn test_b12_registered() {
        let rule = GO_S1160Rule::new();
        assert_eq!(rule.id(), "GO_S1160");
    }

    #[test]
    fn test_b12_detects_unchecked_error() {
        let rule = GO_S1160Rule::new();
        let smelly = r#"
func main() {
    data, _ := json.Marshal(x)
    fmt.Println(data)
}
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unchecked error");
        assert_eq!(issues[0].rule_id, "GO_S1160");
    }

    #[test]
    fn test_b12_allows_checked_error() {
        let rule = GO_S1160Rule::new();
        let clean = r#"
func main() {
    data, err := json.Marshal(x)
    if err != nil {
        return
    }
    fmt.Println(data)
}
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag checked error");
    }
}
