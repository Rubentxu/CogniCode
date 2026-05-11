//! T15 — network call in unit test
//!
//! Detects network calls in unit tests, which make tests slow and flaky.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T15"
    name: "Network call in unit test"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "Making network calls in unit tests makes them slow, dependent on network availability, and prone to flakiness. Use mocks instead.",
    clean_code: Clear,
    impacts: [Maintainability: High, Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find test methods
        let test_method_pattern = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();

        for cap in test_method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name_str = method_name.as_str();

                // Find the method body
                let remaining = &source[method_start..];
                let body_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let method_body = &remaining[2..body_end];

                // Check for network calls
                let has_network = method_body.contains("requests.")
                    || method_body.contains("urllib")
                    || method_body.contains("http.client")
                    || method_body.contains("httpx")
                    || method_body.contains("aiohttp")
                    || method_body.contains("urllib3")
                    || method_body.contains("boto3")
                    || method_body.contains("redis.")
                    || method_body.contains("pymongo")
                    || method_body.contains("mysql.connector")
                    || method_body.contains("psycopg2")
                    || method_body.contains("socket.");

                if has_network {
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T15",
                        format!("Test '{}' makes network calls", method_name_str),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Mock network calls using unittest.mock or responses library."
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
    use cognicode_core::infrastructure::parser::Language;

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
    fn test_t15_registered() {
        let rule = PY_T15Rule::new();
        assert_eq!(rule.id(), "PY_T15");
    }

    #[test]
    fn test_t15_detects_requests_call() {
        let rule = PY_T15Rule::new();
        let smelly = r#"
def test_api():
    response = requests.get('https://api.example.com/data')
    assert response.status_code == 200
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect requests call");
        assert_eq!(issues[0].rule_id, "PY_T15");
    }

    #[test]
    fn test_t15_detects_urllib_call() {
        let rule = PY_T15Rule::new();
        let smelly = r#"
def test_url():
    with urllib.request.urlopen('http://example.com') as response:
        data = response.read()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect urllib call");
        assert_eq!(issues[0].rule_id, "PY_T15");
    }

    #[test]
    fn test_t15_detects_mocked_network() {
        let rule = PY_T15Rule::new();
        let smelly = r#"
@patch('requests.get')
def test_api(mock_get):
    mock_get.return_value.status_code = 200
    response = requests.get('https://api.example.com/data')
    assert response.status_code == 200
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect network call even with mock");
        assert_eq!(issues[0].rule_id, "PY_T15");
    }
}
