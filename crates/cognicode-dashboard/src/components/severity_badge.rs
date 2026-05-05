//! Severity badge component with proper styling

use leptos::prelude::*;
use crate::state::Severity;

#[component]
pub fn SeverityBadge(severity: Severity) -> impl IntoView {
    let (bg, color, border) = match severity {
        Severity::Blocker => ("rgba(198, 40, 40, 0.15)", "#c62828", "rgba(198, 40, 40, 0.3)"),
        Severity::Critical => ("rgba(229, 57, 53, 0.15)", "#e53935", "rgba(229, 57, 53, 0.3)"),
        Severity::Major => ("rgba(251, 140, 0, 0.15)", "#fb8c00", "rgba(251, 140, 0, 0.3)"),
        Severity::Minor => ("rgba(30, 136, 229, 0.15)", "#1e88e5", "rgba(30, 136, 229, 0.3)"),
        Severity::Info => ("rgba(117, 117, 117, 0.15)", "#757575", "rgba(117, 117, 117, 0.3)"),
    };

    let label = severity.label();

    view! {
        <span style={format!("display: inline-flex; align-items: center; padding: 4px 12px; border-radius: 9999px; font-size: 11px; font-weight: 700; letter-spacing: 0.05em; text-transform: uppercase; white-space: nowrap; background: {}; color: {}; border: 1px solid {};", bg, color, border)}>
            {label}
        </span>
    }
}