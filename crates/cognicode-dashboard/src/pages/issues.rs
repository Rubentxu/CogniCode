//! Issues Page — Real API data with filtering and pagination

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, IssueTable, LoadingSpinner};

/// Issues page component
#[component]
pub fn IssuesPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();
    let (severity, set_severity) = signal(None::<String>);
    let (category, set_category) = signal(None::<String>);
    let (file, set_file) = signal(None::<String>);
    let (page, set_page) = signal(1usize);

    // Load on mount with page 1
    {
        let st = state.clone();
        spawn_local(async move {
            st.load_issues(None, None, None, 1).await;
        });
    }

    let load_issues = {
        let st = state.clone();
        move || {
            let s = st.clone();
            let sev = severity.get();
            let cat = category.get();
            let f = file.get();
            let p = page.get();
            spawn_local(async move {
                s.load_issues(sev.as_deref(), cat.as_deref(), f.as_deref(), p).await;
            });
        }
    };

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Issues</h1>
                    <p class="text-body text-text-secondary mt-1">Browse and filter code quality issues</p>
                </header>

                {/* Filters */}
                <div class="card flex flex-wrap items-center gap-4 mb-6">
                    <select class="input select w-40"
                        on:change=move |e| {
                            set_severity.set(if event_target_value(&e) == "all" { None } else { Some(event_target_value(&e)) });
                        }
                    >
                        <option value="all">All</option>
                        <option value="Blocker">Blocker</option>
                        <option value="Critical">Critical</option>
                        <option value="Major">Major</option>
                        <option value="Minor">Minor</option>
                        <option value="Info">Info</option>
                    </select>

                    <select class="input select w-48"
                        on:change=move |e| {
                            set_category.set(if event_target_value(&e) == "all" { None } else { Some(event_target_value(&e)) });
                        }
                    >
                        <option value="all">All</option>
                        <option value="Reliability">Reliability</option>
                        <option value="Security">Security</option>
                        <option value="Maintainability">Maintainability</option>
                        <option value="Coverage">Coverage</option>
                    </select>

                    <input type="text" class="input flex-1" placeholder="Search by file..."
                        on:input=move |e| {
                            let v = event_target_value(&e);
                            set_file.set(if v.is_empty() { None } else { Some(v) });
                        }
                    />

                    <button class="btn btn-primary" on:click=move |_| { set_page.set(1); load_issues(); }>Apply</button>
                </div>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Loading issues..." /> })
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

                {/* Count */}
                {
                    let st = state.clone();
                    move || {
                        let total = st.total_issues_count.get();
                        let current = st.issues.get().len();
                        view! {
                            <p class="text-body-sm text-text-muted mb-4">
                                "Showing " {current} " of " {total} " issues"
                            </p>
                        }
                    }
                }

                {/* Table */}
                {
                    let st = state.clone();
                    move || view! { <IssueTable issues={st.issues.get()} /> }
                }

                {/* Pagination */}
                {
                    let st = state.clone();
                    move || {
                        let current = page.get();
                        let total = st.total_pages.get();
                        if total > 1 {
                            let on_prev = {
                                let s = state.clone();
                                let sev = severity;
                                let cat = category;
                                let f = file;
                                move || {
                                    let new_page = page.get().saturating_sub(1);
                                    if new_page >= 1 {
                                        set_page.set(new_page);
                                        let s = s.clone();
                                        let sev_v = sev.get();
                                        let cat_v = cat.get();
                                        let f_v = f.get();
                                        let p = new_page;
                                        spawn_local(async move {
                                            s.load_issues(sev_v.as_deref(), cat_v.as_deref(), f_v.as_deref(), p).await;
                                        });
                                    }
                                }
                            };
                            let on_next = {
                                let s = state.clone();
                                let sev = severity;
                                let cat = category;
                                let f = file;
                                let total_p = total;
                                move || {
                                    let new_page = page.get() + 1;
                                    if new_page <= total_p {
                                        set_page.set(new_page);
                                        let s = s.clone();
                                        let sev_v = sev.get();
                                        let cat_v = cat.get();
                                        let f_v = f.get();
                                        let p = new_page;
                                        spawn_local(async move {
                                            s.load_issues(sev_v.as_deref(), cat_v.as_deref(), f_v.as_deref(), p).await;
                                        });
                                    }
                                }
                            };
                            Some(view! {
                                <div class="flex items-center justify-center gap-2 mt-6">
                                    <button class="btn btn-secondary btn-sm"
                                        disabled=current == 1
                                        on:click=move |_| on_prev()>
                                        "← Prev"
                                    </button>
                                    <span class="text-body-sm text-text-muted px-4">
                                        "Page " {current} " of " {total}
                                    </span>
                                    <button class="btn btn-secondary btn-sm"
                                        disabled=current == total
                                        on:click=move |_| on_next()>
                                        "Next →"
                                    </button>
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
