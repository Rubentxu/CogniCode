//! Filter bar component

use leptos::prelude::*;

#[component]
pub fn FilterBar<F>(
    _severity: Signal<Option<String>>,
    _category: Signal<Option<String>>,
    _file_filter: Signal<Option<String>>,
    on_apply: F,
) -> impl IntoView
where
    F: Fn() + 'static,
{
    view! {
        <div class="card flex flex-wrap items-center gap-4">
            {/* Severity Filter */}
            <div class="flex flex-col gap-1">
                <label class="text-caption text-text-muted uppercase tracking-wider">Severity</label>
                <select class="input select w-40"
                    on:change=move |_e| {
                        // Filter logic handled by parent
                    }
                >
                    <option value="all">All</option>
                    <option value="Blocker">Blocker</option>
                    <option value="Critical">Critical</option>
                    <option value="Major">Major</option>
                    <option value="Minor">Minor</option>
                    <option value="Info">Info</option>
                </select>
            </div>

            {/* Category Filter */}
            <div class="flex flex-col gap-1">
                <label class="text-caption text-text-muted uppercase tracking-wider">Category</label>
                <select class="input select w-48"
                    on:change=move |_e| {
                        // Filter logic handled by parent
                    }
                >
                    <option value="all">All</option>
                    <option value="Reliability">Reliability</option>
                    <option value="Security">Security</option>
                    <option value="Maintainability">Maintainability</option>
                    <option value="Coverage">Coverage</option>
                    <option value="Duplicate">Duplicates</option>
                    <option value="Complexity">Complexity</option>
                </select>
            </div>

            {/* Search */}
            <div class="flex flex-col gap-1 flex-1 min-w-64">
                <label class="text-caption text-text-muted uppercase tracking-wider">Search</label>
                <input
                    type="text"
                    class="input"
                    placeholder="Search by file..."
                />
            </div>

            {/* Apply Button */}
            <div class="flex items-end">
                <button class="btn btn-primary" on:click=move |_| on_apply()>
                    Apply Filters
                </button>
            </div>
        </div>
    }
}
