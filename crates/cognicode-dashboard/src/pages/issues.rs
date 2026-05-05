//! Issues page with filtering and pagination

use leptos::prelude::*;
use crate::state::{Severity, Category, IssueResult};
use crate::components::{Shell, FilterBar, IssueTable};

const PAGE_SIZE: usize = 10;

/// Generate mock issues for demonstration
fn mock_issues() -> Vec<IssueResult> {
    let severities = vec![
        Severity::Blocker,
        Severity::Critical,
        Severity::Major,
        Severity::Major,
        Severity::Minor,
        Severity::Minor,
        Severity::Minor,
        Severity::Info,
        Severity::Info,
        Severity::Info,
    ];

    let categories = vec![
        Category::Reliability,
        Category::Security,
        Category::Maintainability,
        Category::Maintainability,
        Category::Reliability,
        Category::Security,
        Category::Coverage,
        Category::Complexity,
        Category::Duplicate,
        Category::Maintainability,
    ];

    let rule_ids = vec![
        ("java:S1130", "Replace this generic exception declaration with a more specific one."),
        ("java:S3752", "This URL should be parameterised to prevent SQL injection."),
        ("java:S2229", "This class should be made 'final' or have a private constructor."),
        ("java:S1114", "Remove this redundant null check."),
        ("java:S1197", "Array designators should be placed on the type, not the variable."),
        ("java:S1481", "Unused method parameters should be removed."),
        ("java:S1854", "Remove this useless assignment."),
        ("java:S1135", "Complete the task implementation to avoid code smell."),
        ("java:S2201", "The return value of a method must be used."),
        ("java:S2250", "The expression can be simplified."),
    ];

    let files = vec![
        "src/main/java/com/example/Service.java",
        "src/main/java/com/example/Repository.java",
        "src/main/java/com/example/AuthProvider.java",
        "src/main/java/com/example/Processor.java",
        "src/main/java/com/example/Controller.java",
        "src/main/java/com/example/Database.java",
        "src/main/java/com/example/Utils.java",
        "src/main/java/com/example/Handler.java",
        "src/main/java/com/example/ServiceImpl.java",
        "src/main/java/com/example/Validator.java",
    ];

    (0..50).map(|i| {
        let idx = i % rule_ids.len();
        IssueResult {
            rule_id: rule_ids[idx].0.to_string(),
            message: rule_ids[idx].1.to_string(),
            severity: severities[idx].clone(),
            category: categories[idx].clone(),
            file: files[idx].to_string(),
            line: 20 + (i * 7) % 200,
            column: Some(10 + (i * 3) % 40),
            end_line: Some(20 + (i * 7) % 200),
            remediation_hint: if i % 3 == 0 {
                Some("Consider refactoring this code for better quality.".to_string())
            } else {
                None
            },
        }
    }).collect()
}

#[component]
pub fn IssuesPage() -> impl IntoView {
    let all_issues = mock_issues();
    let (current_page, set_current_page) = signal(0isize);
    let total_issues = all_issues.len();

    let paginated_issues = move || {
        let page = current_page.get() as usize;
        let start = page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(total_issues);
        if start < total_issues {
            all_issues[start..end].to_vec()
        } else {
            vec![]
        }
    };

    let total_pages = ((total_issues + PAGE_SIZE - 1) / PAGE_SIZE) as isize;

    let prev_disabled = move || current_page.get() == 0;
    let next_disabled = move || current_page.get() >= total_pages - 1;

    view! {
        <Shell>
            <div style="max-width: 1400px; margin: 0 auto;">
                <header style="margin-bottom: 32px;">
                    <h1 class="text-h1">Issues</h1>
                    <p style="margin-top: 8px; color: var(--color-text-secondary);">
                        Browse and filter code quality issues
                    </p>
                </header>

                <section style="margin-bottom: 32px;">
                    <FilterBar />
                </section>

                <section style="margin-bottom: 24px; display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <span style="font-size: 14px; color: var(--color-text-secondary);">
                            Showing {current_page.get() as usize * PAGE_SIZE + 1}-{((current_page.get() as usize + 1) * PAGE_SIZE).min(total_issues)} of {total_issues} issues
                        </span>
                    </div>
                    <div style="display: flex; align-items: center; gap: 16px;">
                        <span style="font-size: 14px; color: var(--color-text-muted);">
                            Page {current_page.get() + 1} of {total_pages}
                        </span>
                    </div>
                </section>

                <section style="margin-bottom: 32px;">
                    <IssueTable issues={paginated_issues()} />
                </section>

                <section style="display: flex; justify-content: center; gap: 16px;">
                    <button
                        class="btn btn-secondary"
                        disabled={prev_disabled()}
                        on:click={move |_| {
                            if !prev_disabled() {
                                set_current_page.set(current_page.get() - 1);
                            }
                        }}
                        style={if prev_disabled() { "opacity: 0.5; cursor: not-allowed;" } else { "" }}
                    >
                        <span style="display: inline-flex; align-items: center; gap: 8px;">
                            <svg style="width: 16px; height: 16px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
                            </svg>
                            Previous
                        </span>
                    </button>
                    <button
                        class="btn btn-primary"
                        disabled={next_disabled()}
                        on:click={move |_| {
                            if !next_disabled() {
                                set_current_page.set(current_page.get() + 1);
                            }
                        }}
                        style={if next_disabled() { "opacity: 0.5; cursor: not-allowed;" } else { "" }}
                    >
                        <span style="display: inline-flex; align-items: center; gap: 8px;">
                            Next
                            <svg style="width: 16px; height: 16px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"/>
                            </svg>
                        </span>
                    </button>
                </section>
            </div>
        </Shell>
    }
}
