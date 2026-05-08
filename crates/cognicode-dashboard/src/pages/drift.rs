//! Drift Page — Read-only drift event browser

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, LoadingSpinner};

/// Drift page component
#[component]
pub fn DriftPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Load on mount
    {
        let st = state.clone();
        spawn_local(async move {
            st.load_drift(None, None, None, None, 0, 50).await;
        });
    }

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Drift</h1>
                    <p class="text-body text-text-secondary mt-1">Browse detected architectural drift events</p>
                </header>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Loading drift events..." /> })
                        } else {
                            None
                        }
                    }
                }

                {/* Error */}
                {
                    let st = state.clone();
                    move || {
                        st.error.get().map(|msg| {
                            view! {
                                <div class="card bg-accent-sunset mb-6">
                                    <p class="text-body text-severity-critical">{msg}</p>
                                </div>
                            }
                        })
                    }
                }

                {/* Empty state */}
                {
                    let st = state.clone();
                    move || {
                        let events = st.drift_events.get();
                        let loading = st.loading.get();
                        let error = st.error.get();
                        if !loading && error.is_none() && events.is_empty() {
                            Some(view! {
                                <div class="card p-8 text-center">
                                    <p class="text-body text-text-muted">"No drift events detected"</p>
                                </div>
                            })
                        } else {
                            None
                        }
                    }
                }

                {/* Count */}
                {
                    let st = state.clone();
                    move || {
                        let total = st.drift_total_count.get();
                        let current = st.drift_events.get().len();
                        if total > 0 {
                            Some(view! {
                                <p class="text-body-sm text-text-muted mb-4">
                                    "Showing " {current} " of " {total} " drift events"
                                </p>
                            })
                        } else {
                            None
                        }
                    }
                }

                {/* Table */}
                {
                    let st = state.clone();
                    move || {
                        let events = st.drift_events.get();
                        if events.is_empty() {
                            None
                        } else {
                            Some(view! {
                                <div class="overflow-x-auto">
                                    <table class="w-full text-left">
                                        <thead>
                                            <tr class="border-b border-border">
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Severity</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">File</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Function</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Score</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Intent</th>
                                                <th class="pb-3 text-body-sm font-semibold text-text-secondary">Timestamp</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {events.iter().map(|event| {
                                                let severity_class = match event.severity.to_lowercase().as_str() {
                                                    "blocker" => "badge severity-blocker",
                                                    "critical" => "badge severity-critical",
                                                    "major" => "badge severity-major",
                                                    "minor" => "badge severity-minor",
                                                    _ => "badge severity-info",
                                                };
                                                let intent_display = event.intent.clone().unwrap_or_else(|| "—".to_string());
                                                let drift_score = format!("{:.2}", event.drift_score);
                                                view! {
                                                    <tr class="border-b border-border hover:bg-surface-raised transition-colors">
                                                        <td class="py-3 pr-4">
                                                            <span class={severity_class}>{event.severity.to_uppercase()}</span>
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary font-mono text-sm">
                                                            {event.file_path.clone()}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary font-mono text-sm">
                                                            {event.function_name.clone()}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary">
                                                            {drift_score}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-secondary">
                                                            {intent_display}
                                                        </td>
                                                        <td class="py-3 text-body text-text-muted text-sm">
                                                            {event.timestamp.clone()}
                                                        </td>
                                                    </tr>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </tbody>
                                    </table>
                                </div>
                            })
                        }
                    }
                }
            </div>
        </Shell>
    }
}