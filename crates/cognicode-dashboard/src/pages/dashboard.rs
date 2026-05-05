//! Dashboard Page — Main overview with real API data

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, RatingCard, MetricCard, GateStatusBar, IssueTable, LoadingSpinner};

/// Dashboard page component
#[component]
pub fn DashboardPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Set default project path on mount
    {
        let st = state.clone();
        spawn_local(async move {
            if st.project_path.get().is_empty() {
                st.project_path.set(std::env::current_dir()
                    .unwrap_or_default()
                    .display()
                    .to_string());
            }
        });
    }

    let run_analysis = {
        let st = state.clone();
        move || {
            let s = st.clone();
            spawn_local(async move {
                s.run_analysis().await;
            });
        }
    };

    view! {
        <Shell>
            <div class="p-8">
                {/* Project Path Bar */}
                <div class="flex items-center gap-4 mb-8">
                    <input
                        type="text"
                        class="input flex-1"
                        placeholder="Enter project path..."
                        prop:value={move || state.project_path.get()}
                        on:change=move |ev| {
                            state.project_path.set(event_target_value(&ev));
                        }
                    />
                    <button class="btn btn-primary" on:click=move |_| run_analysis()>
                        Run Analysis
                    </button>
                </div>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Analyzing project..." /> })
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
                            let s = st.clone();
                            view! {
                                <div class="card bg-accent-sunset mb-6">
                                    <p class="text-body text-severity-critical">{msg}</p>
                                    <button class="btn btn-secondary btn-sm mt-2"
                                        on:click=move |_| s.clear_error()>
                                        Dismiss
                                    </button>
                                </div>
                            }
                        })
                    }
                }

                {/* Analysis Results */}
                {
                    let st = state.clone();
                    move || {
                        st.analysis.get().map(|summary| {
                            view! {
                                <div>
                                    <header class="mb-8">
                                        <h1 class="text-h1 text-text-primary">Dashboard</h1>
                                        <p class="text-body text-text-secondary mt-1">
                                            {summary.project_path.clone()} " - " {summary.total_files} " files - " {summary.total_issues} " issues"
                                        </p>
                                    </header>

                                    <section class="mb-8">
                                        <GateStatusBar gate={summary.quality_gate.clone()} />
                                    </section>

                                    <section class="mb-8">
                                        <h2 class="text-h3 text-text-primary mb-4">Project Ratings</h2>
                                        <div class="grid grid-cols-4 gap-4">
                                            <RatingCard rating={summary.ratings.reliability} label="Reliability" />
                                            <RatingCard rating={summary.ratings.security} label="Security" />
                                            <RatingCard rating={summary.ratings.maintainability} label="Maintainability" />
                                            <RatingCard rating={summary.ratings.coverage} label="Coverage" />
                                        </div>
                                    </section>

                                    <section class="mb-8">
                                        <h2 class="text-h3 text-text-primary mb-4">Metrics</h2>
                                        <div class="grid grid-cols-4 gap-4">
                                            <MetricCard label="Total Issues" value={summary.total_issues.to_string()} />
                                            <MetricCard label="Code Smells" value={summary.metrics.code_smells.to_string()} />
                                            <MetricCard label="Bugs" value={summary.metrics.bugs.to_string()} />
                                            <MetricCard label="Vulnerabilities" value={summary.metrics.vulnerabilities.to_string()} />
                                        </div>
                                    </section>

                                    <section class="mb-8">
                                        <div class="card">
                                            <h2 class="text-h3 text-text-primary mb-4">Technical Debt</h2>
                                            <div class="flex items-center gap-6">
                                                <span class="text-display font-bold">{summary.technical_debt.total_minutes} " min"</span>
                                                <span class="badge badge-info">{summary.technical_debt.label.clone()}</span>
                                            </div>
                                        </div>
                                    </section>

                                    <section>
                                        <div class="flex items-center justify-between mb-4">
                                            <h2 class="text-h3 text-text-primary">Recent Issues</h2>
                                            <a href="/issues" class="text-body-sm text-brand font-medium hover:underline">
                                                "View all ->"
                                            </a>
                                        </div>
                                        <IssueTable issues={st.issues.get()} />
                                    </section>
                                </div>
                            }
                        })
                    }
                }

                {/* Empty State */}
                {
                    let st = state.clone();
                    move || {
                        if st.analysis.get().is_none() && !st.loading.get() {
                            Some(view! {
                                <div class="card text-center py-12">
                                    <p class="text-h3 text-text-muted">"No analysis yet"</p>
                                    <p class="text-body text-text-secondary mt-2">"Enter a project path and click Run Analysis"</p>
                                </div>
                            })
                        } else {
                            None
                        }
                    }
                }
            </div>
        </Shell>
    }
}
