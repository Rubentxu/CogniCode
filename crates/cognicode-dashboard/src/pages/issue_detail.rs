//! Issue detail page component

use leptos::prelude::*;
use crate::state::{IssueResult, Severity, Category};
use crate::components::Shell;

/// Mock function to get an issue by rule_id
pub fn get_issue_by_rule_id(rule_id: &str) -> Option<IssueResult> {
    let issues = mock_issues_detail();
    issues.into_iter().find(|i| i.rule_id == rule_id)
}

fn mock_issues_detail() -> Vec<IssueResult> {
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
    ]
}

#[component]
pub fn IssueDetailPage(rule_id: String) -> impl IntoView {
    let issue = get_issue_by_rule_id(&rule_id);

    view! {
        <Shell>
            <div style="max-width: 1000px; margin: 0 auto;">
                <a href="/issues">Back to Issues</a>
                <h1 style="margin-top: 24px;">{rule_id}</h1>
                <p style="color: var(--color-text-secondary);">
                    {if issue.is_some() { "Issue details" } else { "Issue not found" }}
                </p>
            </div>
        </Shell>
    }
}
