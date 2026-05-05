//! Issues API - functions for issue management

use crate::state::{Category, IssueResult, Severity};
use serde::{Deserialize, Serialize};

/// Issue filter parameters
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct IssueFilter {
    pub severity: Option<Severity>,
    pub category: Option<Category>,
    pub rule_id: Option<String>,
    pub file_path: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

/// Paginated issue list response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueListResponse {
    pub issues: Vec<IssueResult>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

/// Get paginated issues with optional filters
pub async fn get_issues(filter: IssueFilter) -> Result<IssueListResponse, String> {
    // TODO: Integrate with cognicode-quality for actual issue data
    // For now, return mock data

    let issues = vec![
        IssueResult {
            rule_id: "java:S1130".to_string(),
            message: "Replace this generic exception declaration with a more specific one.".to_string(),
            severity: Severity::Minor,
            category: Category::Maintainability,
            file: "src/main/java/com/example/Service.java".to_string(),
            line: 42,
            column: Some(13),
            end_line: Some(42),
            remediation_hint: Some("Consider using IllegalArgumentException or a custom exception".to_string()),
        },
        IssueResult {
            rule_id: "java:S1135".to_string(),
            message: "Complete the task implementation to avoid code smell.".to_string(),
            severity: Severity::Info,
            category: Category::Maintainability,
            file: "src/main/java/com/example/Controller.java".to_string(),
            line: 78,
            column: Some(5),
            end_line: Some(78),
            remediation_hint: None,
        },
        IssueResult {
            rule_id: "java:S3752".to_string(),
            message: "This URL should be parameterised to prevent SQL injection.".to_string(),
            severity: Severity::Major,
            category: Category::Security,
            file: "src/main/java/com/example/Repository.java".to_string(),
            line: 156,
            column: Some(20),
            end_line: Some(156),
            remediation_hint: Some("Use PreparedStatement or a framework that handles parameterisation".to_string()),
        },
    ];

    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20);
    let total_count = issues.len();
    let total_pages = (total_count + page_size - 1) / page_size;

    Ok(IssueListResponse {
        issues,
        total_count,
        page,
        page_size,
        total_pages,
    })
}

/// Get a single issue by rule_id and file location
pub async fn get_issue(rule_id: String, file: String, line: usize) -> Result<Option<IssueResult>, String> {
    // TODO: Integrate with cognicode-quality for actual issue lookup
    // For now, return mock issue

    if rule_id == "java:S1130" {
        Ok(Some(IssueResult {
            rule_id,
            message: "Replace this generic exception declaration with a more specific one.".to_string(),
            severity: Severity::Minor,
            category: Category::Maintainability,
            file,
            line,
            column: Some(13),
            end_line: Some(line),
            remediation_hint: Some("Consider using IllegalArgumentException or a custom exception".to_string()),
        }))
    } else {
        Ok(None)
    }
}

/// Get issue counts grouped by severity
pub async fn get_issue_counts() -> Result<std::collections::HashMap<String, usize>, String> {
    // TODO: Integrate with cognicode-quality for actual counts

    let mut counts = std::collections::HashMap::new();
    counts.insert("blocker".to_string(), 0);
    counts.insert("critical".to_string(), 2);
    counts.insert("major".to_string(), 15);
    counts.insert("minor".to_string(), 28);
    counts.insert("info".to_string(), 5);
    counts.insert("total".to_string(), 50);

    Ok(counts)
}
