//! Gate status bar component

use leptos::prelude::*;
use crate::api_client::QualityGateResultDto;

#[component]
pub fn GateStatusBar(gate: QualityGateResultDto) -> impl IntoView {
    let is_passed = gate.status == "PASSED";
    let bg_class = if is_passed { "bg-success" } else { "bg-error" };

    view! {
        <div class={format!("rounded-xl p-4 {} text-white", bg_class)}>
            <p class="font-semibold text-h3">{gate.name.clone()}</p>
            <p class="text-body-sm opacity-90">{gate.status.clone()}</p>
        </div>
    }
}
