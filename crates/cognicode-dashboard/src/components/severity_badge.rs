//! Severity badge component

use leptos::prelude::*;
use crate::state::Severity;

#[component]
pub fn SeverityBadge(severity: Severity) -> impl IntoView {
    let label = severity.label();
    let color_class = severity.color_class();

    view! {
        <span class={format!("badge {}", color_class)}>{label}</span>
    }
}
