use crate::fixture::TestCase;
use cognicode_axiom::rules::types::Issue;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TestReport {
    pub rule_id: String,
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<CaseResult>,
}

#[derive(Debug, Serialize)]
pub struct CaseResult {
    pub name: String,
    pub passed: bool,
    pub expected_count: String,
    pub actual_count: usize,
    pub issues_found: Vec<IssueSummary>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct IssueSummary {
    pub rule_id: String,
    pub severity: String,
    pub line: usize,
    pub message: String,
}

impl TestReport {
    /// Create a new test report for a rule
    pub fn new(rule_id: &str) -> Self {
        Self {
            rule_id: rule_id.to_string(),
            total_cases: 0,
            passed: 0,
            failed: 0,
            results: Vec::new(),
        }
    }

    /// Add a result for a test case
    pub fn add_result(&mut self, case: &TestCase, issues: &[Issue]) {
        let mut errors = Vec::new();
        let count = issues.len();

        // Check issue count
        if count < case.expected_min_issues {
            errors.push(format!(
                "Expected at least {} issues, got {}",
                case.expected_min_issues, count
            ));
        }
        if count > case.expected_max_issues {
            errors.push(format!(
                "Expected at most {} issues, got {}",
                case.expected_max_issues, count
            ));
        }

        // Check rule IDs if specified
        if !case.expected_rule_ids.is_empty() {
            let actual_ids: Vec<String> = issues.iter().map(|i| i.rule_id.clone()).collect();
            for expected_id in &case.expected_rule_ids {
                if !actual_ids.contains(expected_id) {
                    errors.push(format!("Expected rule_id '{}' not found", expected_id));
                }
            }
        }

        // Check severities if specified
        if !case.expected_severities.is_empty() {
            let actual_severities: Vec<String> = issues
                .iter()
                .map(|i| format!("{:?}", i.severity))
                .collect();
            for expected_severity in &case.expected_severities {
                if !actual_severities.contains(expected_severity) {
                    errors.push(format!(
                        "Expected severity '{}' not found in {:?}",
                        expected_severity, actual_severities
                    ));
                }
            }
        }

        let passed = errors.is_empty();
        let expected_count = if case.expected_min_issues == case.expected_max_issues {
            format!("{}", case.expected_min_issues)
        } else {
            format!("{}-{}", case.expected_min_issues, case.expected_max_issues)
        };

        let summaries: Vec<IssueSummary> = issues
            .iter()
            .map(|i| IssueSummary {
                rule_id: i.rule_id.clone(),
                severity: format!("{:?}", i.severity),
                line: i.line,
                message: i.message.clone(),
            })
            .collect();

        self.total_cases += 1;
        if passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }

        self.results.push(CaseResult {
            name: case.name.clone(),
            passed,
            expected_count,
            actual_count: count,
            issues_found: summaries,
            errors,
        });
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Get a summary string
    pub fn summary(&self) -> String {
        format!("{}: {}/{} passed", self.rule_id, self.passed, self.total_cases)
    }
}
