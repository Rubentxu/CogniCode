//! Configuration Page

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, LoadingSpinner};

#[component]
pub fn ConfigurationPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Configuration</h1>
                    <p class="text-body text-text-secondary mt-1">Project settings</p>
                </header>

                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Loading..." /> })
                        } else {
                            None
                        }
                    }
                }

                <div class="max-w-2xl space-y-6">
                    <div class="card">
                        <h3 class="text-h3 text-text-primary mb-4">Project Path</h3>
                        {
                            let st = state.clone();
                            view! {
                                <input type="text" class="input"
                                    prop:value={move || st.project_path.get()}
                                    on:change=move |ev| {
                                        state.project_path.set(event_target_value(&ev));
                                    }
                                />
                            }
                        }
                        <button class="btn btn-primary mt-4"
                            on:click=move |_| {
                                let st = state.clone();
                                spawn_local(async move {
                                    st.run_analysis().await;
                                });
                            }
                        >
                            Run Analysis
                        </button>
                    </div>
                </div>
            </div>
        </Shell>
    }
}
