//! Trends Page — Time-series charts for issues, debt, and ratings

use wasm_bindgen_futures::spawn_local;
use leptos::prelude::*;
use crate::state::ReactiveAppState;
use crate::api_client::{TrendsResponseDto, TrendEntryDto};
use crate::components::{Shell, LoadingSpinner, TrendChart};

/// Date range filter options
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DateRange {
    SevenDays,
    ThirtyDays,
    NinetyDays,
    All,
}

impl DateRange {
    pub fn limit(&self) -> Option<usize> {
        match self {
            DateRange::SevenDays => Some(7),
            DateRange::ThirtyDays => Some(30),
            DateRange::NinetyDays => Some(90),
            DateRange::All => None,
        }
    }
}

/// Map trend entries to total issues series
fn map_issues_series(trends: &[TrendEntryDto]) -> Vec<f64> {
    trends.iter().map(|t| t.total_issues as f64).collect()
}

/// Map trend entries to debt series
fn map_debt_series(trends: &[TrendEntryDto]) -> Vec<f64> {
    trends.iter().map(|t| t.debt_minutes as f64).collect()
}

/// Map trend entries to rating series (convert A-E to numeric)
fn map_rating_series(trends: &[TrendEntryDto]) -> Vec<f64> {
    trends.iter().map(|t| {
        match t.rating.chars().next().unwrap_or('A') {
            'A' => 5.0,
            'B' => 4.0,
            'C' => 3.0,
            'D' => 2.0,
            'E' => 1.0,
            _ => 3.0,
        }
    }).collect()
}

/// Compute change between first and last value
fn compute_change(values: &[f64]) -> Option<(f64, f64)> {
    if values.len() >= 2 {
        Some((values[values.len() - 1], values[values.len() - 1] - values[0]))
    } else {
        None
    }
}

/// Trends page component
#[component]
pub fn TrendsPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Local signals
    let (trends_data, set_trends_data) = signal(Option::<TrendsResponseDto>::None);
    let (date_range, set_date_range) = signal(DateRange::ThirtyDays);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    // Load trends data
    let load_trends = {
        let state = state.clone();
        move || {
            let range = date_range.get();
            let project_path = state.project_path.get();

            if project_path.is_empty() {
                set_error.set(Some("No project selected".to_string()));
                return;
            }

            set_loading.set(true);
            set_error.set(None);

            let api = state.api.clone();
            spawn_local(async move {
                match api.get_trends(&project_path, range.limit()).await {
                    Ok(data) => {
                        set_trends_data.set(Some(data));
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                    }
                }
                set_loading.set(false);
            });
        }
    };

    // Initial load
    {
        load_trends();
    }

    // Date range buttons
    let date_range_buttons: Vec<_> = {
        let ranges = [
            (DateRange::SevenDays, "7D"),
            (DateRange::ThirtyDays, "30D"),
            (DateRange::NinetyDays, "90D"),
            (DateRange::All, "All"),
        ];

        ranges.iter().map(|(range, label)| {
            let r = *range;
            let is_active = date_range.get() == r;
            let lbl = *label;
            view! {
                <button
                    class={format!(
                        "px-4 py-2 rounded-lg text-body-sm font-medium transition-colors {}",
                        if is_active { "bg-brand text-white" } else { "bg-surface-raised text-text-secondary hover:bg-border" }
                    )}
                    on:click={
                        let lt = load_trends.clone();
                        move |_| {
                            set_date_range.set(r);
                            lt();
                        }
                    }
                >
                    {lbl}
                </button>
            }
        }).collect()
    };

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8 flex items-center justify-between">
                    <div>
                        <h1 class="text-h1 text-text-primary">Trends</h1>
                        <p class="text-body text-text-secondary mt-1">
                            Historical analysis data and trends over time
                        </p>
                    </div>

                    {/* Date range selector */}
                    <div class="flex gap-2">
                        {date_range_buttons}
                    </div>
                </header>

                {/* Loading */}
                {
                    let loading = loading.get();
                    move || {
                        if loading {
                            Some(view! { <LoadingSpinner message="Loading trends..." /> })
                        } else {
                            None
                        }
                    }
                }

                {/* Error */}
                {
                    move || {
                        error.get().map(|msg| {
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
                    let trends = trends_data.get();
                    let loading = loading.get();
                    let error = error.get();
                    move || {
                        if !loading && error.is_none() && trends.is_none() {
                            Some(view! {
                                <div class="card text-center py-12">
                                    <p class="text-body text-text-muted">"No trend data available"</p>
                                    <p class="text-body-sm text-text-secondary mt-2">
                                        "Run multiple analyses to see trends over time"
                                    </p>
                                </div>
                            })
                        } else {
                            None
                        }
                    }
                }

                {/* Trends content */}
                {
                    move || {
                        let data = trends_data.get();
                        let loading = loading.get();
                        if let Some(data) = data {
                            if !loading {
                                let issues_series = map_issues_series(&data.trends);
                                let debt_series = map_debt_series(&data.trends);
                                let rating_series = map_rating_series(&data.trends);

                                let (current_issues, issues_delta) = compute_change(&issues_series).unwrap_or((0.0, 0.0));
                                let (current_debt, debt_delta) = compute_change(&debt_series).unwrap_or((0.0, 0.0));

                                let baseline = data.baseline.clone();
                                let trends_clone = data.trends.clone();

                                Some(view! {
                                    <div>
                                        {/* Summary cards */}
                                        <div class="grid grid-cols-4 gap-4 mb-8">
                                            <div class="card p-4">
                                                <p class="text-body-sm text-text-muted mb-1">Total Issues</p>
                                                <p class="text-h2 text-text-primary">{format!("{:.0}", current_issues)}</p>
                                                <p class={format!("text-body-sm mt-1 {}", if issues_delta <= 0.0 { "text-accent-ocean" } else { "text-severity-critical" })}>
                                                    {format!("{} {:.0}", if issues_delta < 0.0 { "↓" } else if issues_delta > 0.0 { "↑" } else { "→" }, issues_delta.abs())}
                                                </p>
                                            </div>

                                            <div class="card p-4">
                                                <p class="text-body-sm text-text-muted mb-1">Tech Debt</p>
                                                <p class="text-h2 text-text-primary">{format!("{:.0} min", current_debt)}</p>
                                                <p class={format!("text-body-sm mt-1 {}", if debt_delta <= 0.0 { "text-accent-ocean" } else { "text-severity-critical" })}>
                                                    {format!("{} {:.0} min", if debt_delta < 0.0 { "↓" } else if debt_delta > 0.0 { "↑" } else { "→" }, debt_delta.abs())}
                                                </p>
                                            </div>

                                            <div class="card p-4">
                                                <p class="text-body-sm text-text-muted mb-1">Current Rating</p>
                                                <p class="text-h2 text-text-primary">
                                                    {data.trends.last().map(|t| t.rating.clone()).unwrap_or_else(|| "N/A".to_string())}
                                                </p>
                                            </div>

                                            <div class="card p-4">
                                                <p class="text-body-sm text-text-muted mb-1">Data Points</p>
                                                <p class="text-h2 text-text-primary">{data.trends.len()}</p>
                                            </div>
                                        </div>

                                        {/* Baseline comparison */}
                                        {
                                            let b = baseline.clone();
                                            if let Some(baseline_data) = &b {
                                                Some(view! {
                                                    <div class="card bg-accent-pale mb-8 p-4">
                                                        <h3 class="text-h4 text-text-primary mb-2">Baseline Comparison</h3>
                                                        <div class="grid grid-cols-2 gap-4">
                                                            <div>
                                                                <p class="text-body-sm text-text-muted">Issues Change</p>
                                                                <p class={format!("text-h3 {}", if baseline_data.issues_delta < 0 { "text-accent-ocean" } else { "text-severity-critical" })}>
                                                                    {if baseline_data.issues_delta < 0 { format!("-{}", baseline_data.issues_delta.abs()) } else { format!("+{}", baseline_data.issues_delta) }}
                                                                </p>
                                                            </div>
                                                            <div>
                                                                <p class="text-body-sm text-text-muted">Debt Change</p>
                                                                <p class={format!("text-h3 {}", if baseline_data.debt_delta < 0 { "text-accent-ocean" } else { "text-severity-critical" })}>
                                                                    {if baseline_data.debt_delta < 0 { format!("-{} min", baseline_data.debt_delta.abs()) } else { format!("+{} min", baseline_data.debt_delta) }}
                                                                </p>
                                                            </div>
                                                        </div>
                                                    </div>
                                                })
                                            } else {
                                                None
                                            }
                                        }

                                        {/* Charts */}
                                        <div class="card p-6 mb-6">
                                            <h3 class="text-h4 text-text-primary mb-4">Issues Over Time</h3>
                                            <TrendChart data={issues_series} width=700 height=150 color="var(--color-brand)" />
                                        </div>

                                        <div class="card p-6 mb-6">
                                            <h3 class="text-h4 text-text-primary mb-4">Technical Debt (minutes)</h3>
                                            <TrendChart data={debt_series} width=700 height=150 color="var(--color-accent-sunset)" />
                                        </div>

                                        <div class="card p-6 mb-6">
                                            <h3 class="text-h4 text-text-primary mb-4">Quality Rating Over Time (A-E)</h3>
                                            <TrendChart data={rating_series} width=700 height=150 color="var(--color-accent-sky)" />
                                        </div>

                                        {/* Historical table */}
                                        <div class="card">
                                            <h3 class="text-h4 text-text-primary mb-4 p-6 border-b border-border">Historical Data</h3>
                                            <div class="overflow-x-auto">
                                                <table class="w-full">
                                                    <thead class="bg-surface text-text-muted text-body-sm">
                                                        <tr>
                                                            <th class="text-left p-4">Date</th>
                                                            <th class="text-right p-4">Issues</th>
                                                            <th class="text-right p-4">Debt (min)</th>
                                                            <th class="text-center p-4">Rating</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {trends_clone.iter().rev().map(|t| {
                                                            let date = t.date.clone();
                                                            let issues = t.total_issues;
                                                            let debt = t.debt_minutes;
                                                            let rating = t.rating.clone();
                                                            let rating_class = match t.rating.chars().next().unwrap_or('C') {
                                                                'A' => "bg-green-100 text-green-800",
                                                                'B' => "bg-blue-100 text-blue-800",
                                                                'C' => "bg-yellow-100 text-yellow-800",
                                                                'D' => "bg-orange-100 text-orange-800",
                                                                _ => "bg-red-100 text-red-800",
                                                            };
                                                            view! {
                                                                <tr class="border-t border-border hover:bg-surface transition-colors">
                                                                    <td class="p-4 text-text-secondary">{date}</td>
                                                                    <td class="p-4 text-right text-text-primary">{issues}</td>
                                                                    <td class="p-4 text-right text-text-primary">{debt}</td>
                                                                    <td class="p-4 text-center">
                                                                        <span class={format!("badge {}", rating_class)}>
                                                                            {rating}
                                                                        </span>
                                                                    </td>
                                                                </tr>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </tbody>
                                                </table>
                                            </div>
                                        </div>
                                    </div>
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                }
            </div>
        </Shell>
    }
}
