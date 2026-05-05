//! Issues API - client-side functions
//!
//! These functions provide issue data. Currently using mock data.

use crate::state::{IssueFilter, IssueListResponse, IssueResult, Severity, Category};

/// Get paginated issues with optional filters
pub async fn get_issues(filter: IssueFilter) -> Result<IssueListResponse, String> {
    let mut issues = create_mock_issues();

    // Apply filters
    if let Some(ref severity) = filter.severity {
        let sev = Severity::from_str(severity);
        issues.retain(|i| i.severity == sev);
    }
    if let Some(ref category) = filter.category {
        let cat = Category::from_str(category);
        issues.retain(|i| i.category == cat);
    }
    if let Some(ref rule_id) = filter.rule_id {
        issues.retain(|i| i.rule_id == *rule_id);
    }

    let total_count = issues.len();
    let page = filter.page.unwrap_or(1).max(1);
    let page_size = filter.page_size.unwrap_or(20).max(1);
    let total_pages = (total_count + page_size - 1) / page_size;

    let start = (page - 1) * page_size;
    let end = (start + page_size).min(total_count);

    let paginated_issues = if start < total_count {
        issues[start..end].to_vec()
    } else {
        vec![]
    };

    Ok(IssueListResponse {
        issues: paginated_issues,
        total_count,
        page,
        page_size,
        total_pages,
    })
}

/// Get a single issue by rule_id and file location
pub async fn get_issue(rule_id: String, _file: String, line: usize) -> Result<Option<IssueResult>, String> {
    let issues = create_mock_issues();
    Ok(issues.into_iter().find(|i| i.rule_id == rule_id && i.line == line))
}

/// Get issue counts grouped by severity
pub async fn get_issue_counts() -> Result<std::collections::HashMap<String, usize>, String> {
    let mut counts = std::collections::HashMap::new();
    counts.insert("blocker".to_string(), 0);
    counts.insert("critical".to_string(), 2);
    counts.insert("major".to_string(), 15);
    counts.insert("minor".to_string(), 28);
    counts.insert("info".to_string(), 5);
    counts.insert("total".to_string(), 50);
    Ok(counts)
}

fn create_mock_issues() -> Vec<IssueResult> {
    vec![
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
        IssueResult {
            rule_id: "java:S2229".to_string(),
            message: "This class should be made 'final' or have a private constructor.".to_string(),
            severity: Severity::Minor,
            category: Category::Security,
            file: "src/main/java/com/example/AuthProvider.java".to_string(),
            line: 23,
            column: Some(14),
            end_line: Some(23),
            remediation_hint: None,
        },
        IssueResult {
            rule_id: "java:S1114".to_string(),
            message: "Remove this redundant null check, 'obj' is already guaranteed to be non-null at this point.".to_string(),
            severity: Severity::Major,
            category: Category::Reliability,
            file: "src/main/java/com/example/Processor.java".to_string(),
            line: 89,
            column: Some(9),
            end_line: Some(92),
            remediation_hint: None,
        },
        IssueResult {
            rule_id: "java:S1854".to_string(),
            message: "Remove this useless assignment to variable 'result'.".to_string(),
            severity: Severity::Major,
            category: Category::Maintainability,
            file: "src/main/java/com/example/Handler.java".to_string(),
            line: 112,
            column: Some(15),
            end_line: Some(112),
            remediation_hint: Some("The variable is assigned but its value is never used.".to_string()),
        },
    ]
}