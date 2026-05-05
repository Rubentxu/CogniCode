//! Issues page with filtering and pagination

use leptos::prelude::*;
use crate::state::IssueResult;
use crate::components::{Shell, FilterBar, IssueTable};

const PAGE_SIZE: usize = 10;

fn mock_issues() -> Vec<IssueResult> {
    vec![
        IssueResult {
            rule_id: "java:S1130".to_string(),
            message: "Replace this generic exception declaration with a more specific one.".to_string(),
            severity: crate::state::Severity::Minor,
            category: crate::state::Category::Maintainability,
            file: "src/main/java/com/example/Service.java".to_string(),
            line: 42,
            column: Some(13),
            end_line: Some(42),
            remediation_hint: Some("Consider using IllegalArgumentException".to_string()),
        },
        IssueResult {
            rule_id: "java:S3752".to_string(),
            message: "This URL should be parameterised to prevent SQL injection.".to_string(),
            severity: crate::state::Severity::Major,
            category: crate::state::Category::Security,
            file: "src/main/java/com/example/Repository.java".to_string(),
            line: 156,
            column: Some(20),
            end_line: Some(156),
            remediation_hint: Some("Use PreparedStatement".to_string()),
        },
    ]
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
                    <FilterBar on_severity_change={move |_| {}} on_category_change={move |_| {}} />
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