//! Filter bar component

use leptos::prelude::*;

#[component]
pub fn FilterBar<F1, F2>(on_severity_change: F1, on_category_change: F2) -> impl IntoView
where
    F1: Fn(Option<String>) + 'static,
    F2: Fn(Option<String>) + 'static,
{
    let severity_options = vec![
        ("All Severities", "all"),
        ("Blocker", "blocker"),
        ("Critical", "critical"),
        ("Major", "major"),
        ("Minor", "minor"),
        ("Info", "info"),
    ];

    let category_options = vec![
        ("All Categories", "all"),
        ("Reliability", "reliability"),
        ("Security", "security"),
        ("Maintainability", "maintainability"),
        ("Coverage", "coverage"),
        ("Duplicates", "duplicate"),
        ("Complexity", "complexity"),
    ];

    view! {
        <div style="display: flex; align-items: center; gap: 16px; padding: 24px; background: var(--color-surface-raised); border-radius: var(--radius-lg); box-shadow: var(--shadow-card); flex-wrap: wrap;">
            <select
                class="input select"
                style="min-width: 160px;"
                on:change={move |e| {
                    let value = event_target_value(&e);
                    let filtered = if value == "all" { None } else { Some(value) };
                    on_severity_change(filtered);
                }}
            >
                {severity_options.iter().map(|(label, value)| {
                    view! {
                        <option value={*value}>
                            {*label}
                        </option>
                    }
                }).collect::<Vec<_>>()}
            </select>

            <select
                class="input select"
                style="min-width: 160px;"
                on:change={move |e| {
                    let value = event_target_value(&e);
                    let filtered = if value == "all" { None } else { Some(value) };
                    on_category_change(filtered);
                }}
            >
                {category_options.iter().map(|(label, value)| {
                    view! {
                        <option value={*value}>
                            {*label}
                        </option>
                    }
                }).collect::<Vec<_>>()}
            </select>

            <input
                type="text"
                class="input"
                placeholder="Search issues..."
                style="flex: 1; min-width: 200px;"
            />

            <div style="display: flex; gap: 8px;">
                <button class="btn btn-primary btn-sm">
                    Apply Filters
                </button>
                <button class="btn btn-secondary btn-sm">
                    Clear
                </button>
            </div>
        </div>
    }
}