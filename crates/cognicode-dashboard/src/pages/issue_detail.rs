//! Issue detail page component

use leptos::prelude::*;
use crate::state::Severity;
use crate::components::Shell;

#[component]
pub fn IssueDetailPage(rule_id: String) -> impl IntoView {
    let (issue_data, _) = signal(Option::<IssueDetailData>::None);

    view! {
        <Shell>
            <div style="max-width: 1000px; margin: 0 auto;">
                <a href="/issues" style="display: inline-flex; align-items: center; gap: 8px; color: var(--color-text-secondary); text-decoration: none; margin-bottom: 24px;">
                    <svg style="width: 16px; height: 16px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
                    </svg>
                    Back to Issues
                </a>

                <div class="card">
                    <div style="display: flex; align-items: flex-start; justify-content: space-between; margin-bottom: 24px;">
                        <div>
                            <span style="display: inline-block; padding: 4px 12px; border-radius: 4px; font-size: 12px; font-weight: 600; text-transform: uppercase; background: #1e88e5; color: white;">
                                Minor
                            </span>
                            <h1 style="margin-top: 16px; font-size: 24px; font-weight: 600;">{rule_id.clone()}</h1>
                        </div>
                    </div>

                    <p style="font-size: 16px; line-height: 1.6; color: var(--color-text-primary); margin-bottom: 24px;">
                        Replace this generic exception declaration with a more specific one.
                    </p>

                    <div style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 24px; padding-top: 24px; border-top: 1px solid var(--color-border);">
                        <div>
                            <p style="font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; margin-bottom: 4px;">File</p>
                            <p style="font-family: monospace; font-size: 14px; color: var(--color-text-primary);">src/main/java/com/example/Service.java</p>
                        </div>
                        <div>
                            <p style="font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; margin-bottom: 4px;">Line</p>
                            <p style="font-family: monospace; font-size: 14px; color: var(--color-text-primary);">42</p>
                        </div>
                        <div>
                            <p style="font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; margin-bottom: 4px;">Category</p>
                            <p style="font-size: 14px; color: var(--color-text-primary);">Maintainability</p>
                        </div>
                        <div>
                            <p style="font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; margin-bottom: 4px;">Severity</p>
                            <p style="font-size: 14px; color: var(--color-text-primary);">Minor</p>
                        </div>
                    </div>

                    <div style="margin-top: 24px; padding: 16px; background: rgba(99, 102, 241, 0.1); border-radius: 8px; border-left: 4px solid var(--color-brand);">
                        <p style="font-size: 12px; font-weight: 600; color: var(--color-brand); text-transform: uppercase; margin-bottom: 8px;">Remediation</p>
                        <p style="font-size: 14px; color: var(--color-text-primary);">Consider using IllegalArgumentException or a custom exception</p>
                    </div>
                </div>
            </div>
        </Shell>
    }
}

#[derive(Clone)]
struct IssueDetailData {
    rule_id: String,
    message: String,
    severity: Severity,
    file: String,
    line: usize,
}