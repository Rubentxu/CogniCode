//! Issue table component

use leptos::prelude::*;
use crate::api_client::IssueDto;
use crate::state::Severity;

#[component]
pub fn IssueTable(issues: Vec<IssueDto>) -> impl IntoView {
    if issues.is_empty() {
        return view! {
            <div class="card px-6 py-12 text-center">
                <p class="text-h3 text-text-muted">"No issues found"</p>
            </div>
        }.into_any();
    }

    let issues_count = issues.len();

    // Build rows with clickable links
    let mut rows_html = String::from(
        r#"<div class="flex items-center gap-4 px-6 py-4 border-b border-border bg-surface text-caption font-bold text-text-muted uppercase tracking-wider">
            <span class="w-28">Severity</span>
            <span class="w-32">Rule</span>
            <span class="flex-1">Message</span>
            <span class="w-48">File</span>
            <span class="w-16 text-center">Line</span>
        </div>"#,
    );

    for (idx, issue) in issues.iter().enumerate() {
        let sev = Severity::from_str(&issue.severity);
        let badge_class = match sev {
            Severity::Blocker | Severity::Critical => "badge-critical",
            Severity::Major => "badge-major",
            Severity::Minor => "badge-minor",
            Severity::Info => "badge-info",
        };
        let label = sev.label();

        rows_html.push_str(&format!(
            r#"<a href="/issues/{}" class="flex items-center gap-4 px-6 py-4 hover:bg-surface transition-colors cursor-pointer block no-underline">
                <span class="w-28"><span class="badge {}">{}</span></span>
                <span class="w-32 text-mono text-body-sm text-brand">{}</span>
                <span class="flex-1 text-body text-text-primary truncate">{}</span>
                <span class="w-48 text-body-sm text-text-secondary truncate">{}</span>
                <span class="w-16 text-center text-body-sm text-text-muted">{}</span>
            </a>"#,
            idx, badge_class, label, issue.rule_id, issue.message, issue.file, issue.line
        ));
    }

    rows_html.push_str(&format!(
        r#"<div class="px-6 py-2 text-body-sm text-text-muted">Showing {} issues</div>"#,
        issues_count
    ));

    view! { <div class="card overflow-hidden p-0" inner_html=rows_html /> }.into_any()
}
