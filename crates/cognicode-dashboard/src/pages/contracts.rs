//! Contracts Page — Read-only contract browser

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, LoadingSpinner};

/// Contracts page component
#[component]
pub fn ContractsPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Load on mount
    {
        let st = state.clone();
        spawn_local(async move {
            st.load_contracts(50).await;
        });
    }

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Contracts</h1>
                    <p class="text-body text-text-secondary mt-1">Browse AVC contract summaries</p>
                </header>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Loading contracts..." /> })
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
                        let contracts = st.contracts.get();
                        let loading = st.loading.get();
                        let error = st.error.get();
                        if !loading && error.is_none() && contracts.is_empty() {
                            Some(view! {
                                <div class="card p-8 text-center">
                                    <p class="text-body text-text-muted">"No contracts found"</p>
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
                        let total = st.contracts_count.get();
                        let current = st.contracts.get().len();
                        if total > 0 {
                            Some(view! {
                                <p class="text-body-sm text-text-muted mb-4">
                                    "Showing " {current} " of " {total} " contracts"
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
                        let contracts = st.contracts.get();
                        if contracts.is_empty() {
                            None
                        } else {
                            Some(view! {
                                <div class="overflow-x-auto">
                                    <table class="w-full text-left">
                                        <thead>
                                            <tr class="border-b border-border">
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">ID</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Source File</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Function</th>
                                                <th class="pb-3 pr-4 text-body-sm font-semibold text-text-secondary">Compliance Score</th>
                                                <th class="pb-3 text-body-sm font-semibold text-text-secondary">Generated At</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {contracts.iter().map(|contract| {
                                                let compliance_display = format!("{:.1}%", contract.compliance_score * 100.0);
                                                view! {
                                                    <tr class="border-b border-border hover:bg-surface-raised transition-colors">
                                                        <td class="py-3 pr-4 text-body text-text-primary">
                                                            {contract.id}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary font-mono text-sm">
                                                            {contract.source_file.clone()}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary font-mono text-sm">
                                                            {contract.function_name.clone()}
                                                        </td>
                                                        <td class="py-3 pr-4 text-body text-text-primary">
                                                            {compliance_display}
                                                        </td>
                                                        <td class="py-3 text-body text-text-muted text-sm">
                                                            {contract.generated_at.clone()}
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
