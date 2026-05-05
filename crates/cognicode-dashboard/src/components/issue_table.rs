//! Issue table component

use leptos::prelude::*;
use crate::state::IssueResult;

#[component]
pub fn IssueTable(issues: Vec<IssueResult>) -> impl IntoView {
    view! {
        <div class="card">
            <p class="text-body">{issues.len()} issues</p>
        </div>
    }
}
