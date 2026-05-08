//! Agent Stats Page — Read-only agent tool usage statistics

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, LoadingSpinner};

/// Agent Stats page component
#[component]
pub fn AgentStatsPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Load on mount
    {
        let st = state.clone();
        spawn_local(async move {
            st.load_agent_stats(None).await;
        });
    }

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Agent Stats</h1>
                    <p class="text-body text-text-secondary mt-1">Browse agent tool usage statistics</p>
                </header>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Loading agent stats..." /> })
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
                        let stats = st.agent_stats.get();
                        let loading = st.loading.get();
                        let error = st.error.get();
                        if !loading && error.is_none() && stats.is_empty() {
                            Some(view! {
                                <div class="card p-8 text-center">
                                    <p class="text-body text-text-muted">"No agent stats recorded"</p>
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
                        let total = st.agent_stats_count.get();
                        let current = st.agent_stats.get().len();
                        if total > 0 {
                            Some(view! {
                                <p class="text-body-sm text-text-muted mb-4">
                                    "Showing " {current} " of " {total} " agent stats"
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
                        let stats = st.agent_stats.get();
                        if stats.is_empty() {
                            None
                        } else {
                            Some(view! {
                                <div class="overflow-x-auto">
                                    <table class="w-full text-left">
                                        <thead>
                                            <tr class="border-b border-border">
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Tool Name</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Count</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Avg Duration (ms)</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Success</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Error</th>
                                                <th class="pb-3 text-body-sm font-semibold text-text-secondary">Other</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {stats.iter().map(|stat| {
                                                let avg_duration_display = format!("{:.1}", stat.avg_duration_ms);
                                                view! {
                                                    <tr class="border-b border-border hover:bg-surface-raised transition-colors">
                                                        <td class="py-3 pr-4 text-body text-text-primary font-mono text-sm">
                                                            {stat.tool_name.clone()}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary">
                                                            {stat.count}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary">
                                                            {avg_duration_display}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary">
                                                            {stat.result_status_breakdown.success}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary">
                                                            {stat.result_status_breakdown.error}
                                                        </td>
                                                        <td class="py-3 text-body text-text-primary">
                                                            {stat.result_status_breakdown.other}
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
