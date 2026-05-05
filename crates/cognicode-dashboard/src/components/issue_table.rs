//! Issue table component

use leptos::prelude::*;
use crate::state::IssueResult;

#[component]
pub fn IssueTable(issues: Vec<IssueResult>) -> impl IntoView {
    let count = issues.len();
    let summary = format!("{} issues found", count);

    view! {
        <div class="card" style="padding: 24px;">
            <p style="font-size: 16px; font-weight: 500;">{summary}</p>
            <p style="font-size: 14px; color: var(--color-text-secondary); margin-top: 8px;">
                View individual issues for details
            </p>
        </div>
    }
}
