//! Issue row component

use leptos::prelude::*;
use crate::state::IssueResult;
use crate::components::SeverityBadge;

#[component]
pub fn IssueRow(issue: IssueResult) -> impl IntoView {
    view! {
        <a
            href=format!("/issues/{}", issue.rule_id)
            class="issue-row"
            style="display: flex; align-items: center; padding: 16px 24px; border-bottom: 1px solid var(--color-border); text-decoration: none; color: inherit; transition: background-color 0.15s ease;"
        >
            <div style="display: flex; align-items: center; gap: 16px; width: 100%;">
                <SeverityBadge severity=issue.severity.clone() />
                <span class="text-mono" style="font-size: 14px; font-weight: 500; color: var(--color-text-link); min-width: 120px;">
                    {issue.rule_id.clone()}
                </span>
                <span class="truncate" style="flex: 1; font-size: 14px; color: var(--color-text-primary);">
                    {issue.message.clone()}
                </span>
                <span class="text-mono" style="font-size: 12px; color: var(--color-text-muted); min-width: 200px; text-align: right;">
                    {issue.file.clone()}
                    <span style="color: var(--color-text-muted);">:{issue.line}</span>
                </span>
            </div>
        </a>
    }
}