//! Quality Gate Page — Full condition details and editable thresholds

use leptos::prelude::*;
use crate::state::ReactiveAppState;
use crate::api_client::QualityGateResultDto;
use crate::components::{Shell, GateStatusBar, LoadingSpinner};

/// Quality Gate page with condition editor
#[component]
pub fn QualityGatePage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();
    let (edit_mode, set_edit_mode) = signal(false);

    let toggle_edit = move |_| {
        set_edit_mode.update(|v| *v = !*v);
    };

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8 flex items-center justify-between">
                    <div>
                        <h1 class="text-h1 text-text-primary">Quality Gate</h1>
                        <p class="text-body text-text-secondary mt-1">Gate evaluation and configuration</p>
                    </div>
                    <button class="btn btn-secondary"
                        on:click=toggle_edit>
                        {move || if edit_mode.get() { "Done" } else { "Edit Conditions" }}
                    </button>
                </header>

                {render_loading(state.clone())}
                {render_gate_content(state.clone(), edit_mode)}
                {render_empty_state(state.clone())}
            </div>
        </Shell>
    }
}

fn render_loading(state: ReactiveAppState) -> impl IntoView {
    move || {
        if state.loading.get() {
            Some(view! { <LoadingSpinner message="Loading..." /> })
        } else {
            None
        }
    }
}

fn render_gate_content(
    state: ReactiveAppState,
    edit_mode: ReadSignal<bool>,
) -> impl IntoView {
    move || {
        state.analysis.get().map(|summary| {
            let gate = summary.quality_gate.clone();
            let is_edit = edit_mode.get();

            view! {
                <div>
                    <section class="mb-8">
                        <GateStatusBar gate={gate.clone()} />
                    </section>

                    <section class="mb-8">
                        <h2 class="text-h3 text-text-primary mb-4">"Conditions (" {gate.conditions.len()} ")"</h2>
                        <div class="card overflow-hidden p-0">
                            <table class="w-full">
                                <thead class="bg-surface">
                                    <tr>
                                        <th class="px-6 py-4 text-left text-caption text-text-muted uppercase">Status</th>
                                        <th class="px-6 py-4 text-left text-caption text-text-muted uppercase">Condition</th>
                                        <th class="px-6 py-4 text-left text-caption text-text-muted uppercase">Metric</th>
                                        <th class="px-6 py-4 text-left text-caption text-text-muted uppercase">Operator</th>
                                        <th class="px-6 py-4 text-left text-caption text-text-muted uppercase">Threshold</th>
                                        {is_edit.then(|| view! { <th class="px-6 py-4 text-left text-caption text-text-muted uppercase">Actions</th> })}
                                    </tr>
                                </thead>
                                <tbody class="divide-y divide-border">
                                    {render_conditions(&gate, is_edit, state.clone())}
                                </tbody>
                            </table>
                        </div>
                    </section>

                    {is_edit.then(|| render_add_condition_form(state.clone()))}

                    {render_gate_summary(&gate)}
                </div>
            }
        })
    }
}

fn render_conditions(gate: &QualityGateResultDto, is_edit: bool, state: ReactiveAppState) -> Vec<impl IntoView> {
    let conditions = gate.conditions.clone();
    conditions.into_iter().map(|cond| {
        let status_class = if cond.passed { "text-success" } else { "text-error" };
        let status_text = if cond.passed { "PASS" } else { "FAIL" };
        let op_label: String = match cond.operator.as_str() {
            "LT" => "<".to_string(),
            "LTE" => "<=".to_string(),
            "GT" => ">".to_string(),
            "GTE" => ">=".to_string(),
            "EQ" => "=".to_string(),
            "NEQ" => "!=".to_string(),
            other => other.to_string(),
        };
        let cond_name = cond.name.clone();
        let cond_metric = cond.metric.clone();
        let st = state.clone();

        view! {
            <tr class="hover:bg-surface">
                <td class="px-6 py-4">
                    <span class={format!("badge {}", status_class)}>{status_text}</span>
                </td>
                <td class="px-6 py-4 text-body text-text-primary">{cond_name}</td>
                <td class="px-6 py-4">
                    <span class="text-mono text-body-sm text-text-secondary">{cond_metric}</span>
                </td>
                <td class="px-6 py-4">
                    <span class="badge badge-info">{op_label}</span>
                </td>
                <td class="px-6 py-4 text-mono text-body text-text-primary">
                    {cond.threshold}
                </td>
                {is_edit.then(|| view! {
                    <td class="px-6 py-4">
                        <button class="btn btn-secondary btn-sm text-error"
                            on:click=move |_| {
                                st.error.set(Some("Condition removal requires server-side API".to_string()));
                            }>
                            Remove
                        </button>
                    </td>
                })}
            </tr>
        }
    }).collect()
}

fn render_add_condition_form(state: ReactiveAppState) -> impl IntoView {
    view! {
        <section class="mb-8">
            <h2 class="text-h3 text-text-primary mb-4">Add Condition</h2>
            <div class="card p-6">
                <p class="text-body text-text-muted mb-4">
                    "Add new conditions to the quality gate. (Server-side API pending)"
                </p>
                <div class="flex items-center gap-4">
                    <select class="input select w-48">
                        <option value="code_smells">Code Smells</option>
                        <option value="bugs">Bugs</option>
                        <option value="vulnerabilities">Vulnerabilities</option>
                        <option value="coverage">Coverage</option>
                        <option value="duplications">Duplications</option>
                    </select>
                    <select class="input select w-24">
                        <option value="LT">{"<"}</option>
                        <option value="GT">{">"}</option>
                        <option value="LTE">{"<="}</option>
                        <option value="GTE">{">="}</option>
                        <option value="EQ">{"="}</option>
                    </select>
                    <input type="number" class="input w-24" placeholder="Value" />
                    <button class="btn btn-primary"
                        on:click=move |_| {
                            state.error.set(Some("Add condition requires server-side API".to_string()));
                        }>
                        Add
                    </button>
                </div>
            </div>
        </section>
    }
}

fn render_gate_summary(gate: &QualityGateResultDto) -> impl IntoView {
    let passing = gate.conditions.iter().filter(|c| c.passed).count();
    view! {
        <section>
            <div class="card">
                <h3 class="text-h3 text-text-primary mb-2">Gate Summary</h3>
                <div class="grid grid-cols-3 gap-6">
                    <div>
                        <p class="text-caption text-text-muted">Status</p>
                        <p class={format!("text-h2 font-bold mt-1 {}", if gate.status == "PASSED" { "text-success" } else { "text-error" })}>
                            {gate.status.clone()}
                        </p>
                    </div>
                    <div>
                        <p class="text-caption text-text-muted">Total Conditions</p>
                        <p class="text-h2 font-bold mt-1">{gate.conditions.len()}</p>
                    </div>
                    <div>
                        <p class="text-caption text-text-muted">Passing</p>
                        <p class="text-h2 font-bold mt-1 text-success">{passing}</p>
                    </div>
                </div>
            </div>
        </section>
    }
}

fn render_empty_state(state: ReactiveAppState) -> impl IntoView {
    move || {
        if state.analysis.get().is_none() && !state.loading.get() {
            Some(view! {
                <div class="card text-center py-12">
                    <p class="text-h3 text-text-muted">"No analysis data"</p>
                    <p class="text-body text-text-secondary mt-2">"Run an analysis first from the Dashboard"</p>
                </div>
            })
        } else {
            None
        }
    }
}
