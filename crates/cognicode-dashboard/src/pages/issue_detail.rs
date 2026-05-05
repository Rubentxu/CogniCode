//! Issue Detail Page — Real data from state

use leptos::prelude::*;
use crate::state::{ReactiveAppState, Severity};
use crate::components::{Shell, SeverityBadge, LoadingSpinner};

/// Issue detail page component
#[component]
pub fn IssueDetailPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Get the issue ID from the query parameter or path
    // For now, we'll use a simple approach: find by index from state
    let (issue_idx, set_issue_idx) = signal(0usize);

    view! {
        <Shell>
            <div class="p-8 max-w-4xl mx-auto">
                {/* Back button */}
                <a href="/issues" class="inline-flex items-center gap-2 text-body text-text-secondary hover:text-text-primary transition-colors mb-6">
                    <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
                    </svg>
                    Back to Issues
                </a>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Loading issue details..." /> })
                        } else {
                            None
                        }
                    }
                }

                {/* Issue detail from state */}
                {
                    let st = state.clone();
                    move || {
                        let issues = st.issues.get();
                        let idx = issue_idx.get();

                        if issues.is_empty() {
                            return Some(view! {
                                <div class="card text-center py-12">
                                    <p class="text-h3 text-text-muted">"No issues loaded"</p>
                                    <p class="text-body text-text-secondary mt-2">"Go to the Issues page and select an issue"</p>
                                </div>
                            }.into_any());
                        }

                        if idx >= issues.len() {
                            return Some(view! {
                                <div class="card text-center py-12">
                                    <p class="text-h3 text-text-muted">"Issue not found"</p>
                                    <p class="text-body text-text-secondary mt-2">"The issue may have been removed"</p>
                                </div>
                            }.into_any());
                        }

                        let issue = &issues[idx];
                        let sev = Severity::from_str(&issue.severity);

                        Some(view! {
                            <div class="card">
                                {/* Header */}
                                <div class="flex items-start justify-between mb-6">
                                    <div>
                                        <SeverityBadge severity={sev} />
                                        <h1 class="text-h1 text-text-primary mt-4">{issue.rule_id.clone()}</h1>
                                    </div>
                                    <div class="flex gap-2">
                                        {if idx > 0 {
                                            let prev_idx = idx - 1;
                                            Some(view! {
                                                <button class="btn btn-secondary btn-sm"
                                                    on:click=move |_| set_issue_idx.set(prev_idx)>
                                                    "← Previous"
                                                </button>
                                            }.into_any())
                                        } else {
                                            None
                                        }}
                                        {if idx < issues.len() - 1 {
                                            let next_idx = idx + 1;
                                            Some(view! {
                                                <button class="btn btn-secondary btn-sm"
                                                    on:click=move |_| set_issue_idx.set(next_idx)>
                                                    "Next →"
                                                </button>
                                            }.into_any())
                                        } else {
                                            None
                                        }}
                                    </div>
                                </div>

                                {/* Message */}
                                <p class="text-body text-text-primary mb-6">{issue.message.clone()}</p>

                                {/* Metadata Grid */}
                                <div class="grid grid-cols-2 gap-6 pt-6 border-t border-border">
                                    <div>
                                        <p class="text-caption text-text-muted uppercase font-semibold mb-1">File</p>
                                        <p class="text-mono text-body-sm text-text-primary">{issue.file.clone()}</p>
                                    </div>
                                    <div>
                                        <p class="text-caption text-text-muted uppercase font-semibold mb-1">Line</p>
                                        <p class="text-mono text-body-sm text-text-primary">{issue.line}</p>
                                    </div>
                                    <div>
                                        <p class="text-caption text-text-muted uppercase font-semibold mb-1">Category</p>
                                        <p class="text-body text-text-primary">{issue.category.clone()}</p>
                                    </div>
                                    <div>
                                        <p class="text-caption text-text-muted uppercase font-semibold mb-1">Severity</p>
                                        <p class="text-body text-text-primary">{issue.severity.clone()}</p>
                                    </div>
                                    {if let Some(col) = issue.column {
                                        Some(view! {
                                            <div>
                                                <p class="text-caption text-text-muted uppercase font-semibold mb-1">Column</p>
                                                <p class="text-mono text-body-sm text-text-primary">{col}</p>
                                            </div>
                                        }.into_any())
                                    } else {
                                        None
                                    }}
                                    {if let Some(end_line) = issue.end_line {
                                        Some(view! {
                                            <div>
                                                <p class="text-caption text-text-muted uppercase font-semibold mb-1">End Line</p>
                                                <p class="text-mono text-body-sm text-text-primary">{end_line}</p>
                                            </div>
                                        }.into_any())
                                    } else {
                                        None
                                    }}
                                </div>

                                {/* Remediation */}
                                {if let Some(remediation) = &issue.remediation_hint {
                                    Some(view! {
                                        <div class="mt-6 p-4 bg-brand/10 rounded-lg border-l-4 border-brand">
                                            <p class="text-caption text-brand uppercase font-semibold mb-2">Remediation</p>
                                            <p class="text-body text-text-primary">{remediation.clone()}</p>
                                        </div>
                                    }.into_any())
                                } else {
                                    None
                                }}
                            </div>
                        }.into_any())
                    }
                }
            </div>
        </Shell>
    }
}
