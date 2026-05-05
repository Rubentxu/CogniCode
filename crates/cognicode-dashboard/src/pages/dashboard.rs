//! Dashboard Page — Main overview with real API data

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, RatingCard, MetricCard, GateStatusBar, IssueTable, LoadingSpinner};

/// Dashboard page component
#[component]
pub fn DashboardPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

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
                {/* Header: show selected project or link to Projects */}
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Dashboard</h1>
                    {
                        let st = state.clone();
                        let ra = run_analysis.clone();
                        move || render_header_info(st.clone(), ra.clone())
                    }
                </header>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Analyzing project..." /> })
                        } else { None }
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
                        if st.analysis.get().is_none() && st.selected_project_name.get().is_some() && !st.loading.get() {
                            Some(view! {
                                <div class="card text-center py-12">
                                    <p class="text-h3 text-text-muted">"No analysis yet"</p>
                                    <p class="text-body text-text-secondary mt-2">"Click 'Run Analysis' above to analyze this project"</p>
                                </div>
                            })
                        } else { None }
                    }
                }
            </div>
        </Shell>
    }
}

fn render_header_info(state: ReactiveAppState, run_analysis: impl Fn() + Clone + Send + 'static) -> impl IntoView {
    move || {
        let name = state.selected_project_name.get();
        let path = state.project_path.get();
        let ra = run_analysis.clone();
        if let Some(ref n) = name {
            view! {
                <div class="flex items-center gap-4 mt-2">
                    <span class="text-body text-text-secondary">
                        "Project: " <strong class="text-text-primary">{n.clone()}</strong>
                    </span>
                    <span class="text-mono text-body-sm text-text-muted">{path}</span>
                    <button class="btn btn-secondary btn-sm" on:click=move |_| ra()>
                        Run Analysis
                    </button>
                </div>
            }.into_any()
        } else {
            view! {
                <p class="text-body text-text-secondary mt-2">
                    "No project selected. " 
                    <a href="/projects" class="text-brand font-medium hover:underline">
                        "Go to Projects →"
                    </a>
                    " to register or select one."
                </p>
            }.into_any()
        }
    }
}
