//! Metric card component

use leptos::prelude::*;

#[component]
pub fn MetricCard(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="card">
            <p class="text-caption text-text-muted">{label}</p>
            <p class="text-h1 text-text-primary mt-2">{value}</p>
        </div>
    }
}
