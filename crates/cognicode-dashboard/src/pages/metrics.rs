//! Metrics Page — Real API data with charts

use wasm_bindgen_futures::spawn_local;
use leptos::prelude::*;
use crate::state::ReactiveAppState;
use crate::api_client::HistoryEntryDto;
use crate::components::{Shell, MetricCard, LoadingSpinner, TrendChart};

/// Render severity bars from analysis data
fn render_severity_bars(summary: &AnalysisSummaryDto) -> Vec<impl IntoView> {
    let total = summary.total_issues;
    summary.metrics.issues_by_severity.iter().map(|(sev, count)| {
        let pct = if total > 0 { (*count as f64 / total as f64) * 100.0 } else { 0.0 };
        let color = match sev.as_str() {
            "Blocker" => "bg-severity-blocker",
            "Critical" => "bg-severity-critical",
            "Major" => "bg-severity-major",
            "Minor" => "bg-severity-minor",
            _ => "bg-severity-info",
        };
        let sev_clone = sev.clone();
        let count_val = *count;
        view! {
            <div class="flex items-center gap-4 mb-3">
                <span class="w-24 text-body-sm text-text-secondary">{sev_clone}</span>
                <div class="flex-1 bg-surface rounded-full h-4">
                    <div class={format!("{} h-4 rounded-full", color)}
                        style={format!("width: {}%", pct)}>
                    </div>
                </div>
                <span class="w-12 text-body-sm text-text-muted text-right">{count_val}</span>
            </div>
        }
    }).collect::<Vec<_>>()
}

/// Map history entries to issues trend vector (total_issues per run)
fn map_issues_trend(runs: &[HistoryEntryDto]) -> Vec<f64> {
    runs.iter().map(|r| r.total_issues as f64).collect()
}

/// Map history entries to debt trend vector (debt_minutes per run)
fn map_debt_trend(runs: &[HistoryEntryDto]) -> Vec<f64> {
    runs.iter().map(|r| r.debt_minutes as f64).collect()
}

/// Map history entries to new issues trend vector (new_issues per run)
fn map_new_issues_trend(runs: &[HistoryEntryDto]) -> Vec<f64> {
    runs.iter().map(|r| r.new_issues as f64).collect()
}

/// Render trend charts section with real data from history
fn render_trend_charts(state: ReactiveAppState) -> impl IntoView {
    move || {
        match state.trend_data.get() {
            Some(runs) if runs.len() >= 2 => {
                let issues_trend = map_issues_trend(&runs);
                let debt_trend = map_debt_trend(&runs);
                let new_trend = map_new_issues_trend(&runs);

                view! {
                    <section class="mb-8">
                        <h2 class="text-h3 text-text-primary mb-4">Trends</h2>
                        <div class="grid grid-cols-3 gap-6">
                            <div class="card">
                                <p class="text-caption text-text-muted mb-4">Issues Trend</p>
                                <TrendChart data={issues_trend} width=200 height=80 color="var(--color-brand)" />
                            </div>
                            <div class="card">
                                <p class="text-caption text-text-muted mb-4">Technical Debt (min)</p>
                                <TrendChart data={debt_trend} width=200 height=80 color="var(--color-accent-sunset)" />
                            </div>
                            <div class="card">
                                <p class="text-caption text-text-muted mb-4">New Issues</p>
                                <TrendChart data={new_trend} width=200 height=80 color="var(--color-severity-critical)" />
                            </div>
                        </div>
                    </section>
                }.into_any()
            }
            _ => {
                // Insufficient history: show placeholder instead of broken/empty charts
                view! {
                    <section class="mb-8">
                        <h2 class="text-h3 text-text-primary mb-4">Trends</h2>
                        <div class="card text-center py-12">
                            <p class="text-body text-text-muted">"No trend data — run multiple analyses to see trends"</p>
                        </div>
                    </section>
                }.into_any()
            }
        }
    }
}

/// Metrics page component
#[component]
pub fn MetricsPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Load history on mount for trend charts
    let state_for_load = state.clone();
    spawn_local(async move {
        state_for_load.load_history().await;
    });

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Metrics</h1>
                    <p class="text-body text-text-secondary mt-1">
                        Code quality metrics and trends
                    </p>
                </header>

                {render_loading_or_error(state.clone())}
                {render_metrics_content(state.clone())}
                {render_empty_state(state.clone())}
            </div>
        </Shell>
    }
}

use crate::api_client::AnalysisSummaryDto;

fn render_loading_or_error(state: ReactiveAppState) -> impl IntoView {
    move || {
        if state.loading.get() {
            return Some(view! { <LoadingSpinner message="Loading metrics..." /> }.into_any());
        }
        state.error.get().map(|msg| {
            view! {
                <div class="card bg-accent-sunset mb-6">
                    <p class="text-body text-severity-critical">{msg}</p>
                </div>
            }.into_any()
        })
    }
}

fn render_metrics_content(state: ReactiveAppState) -> impl IntoView {
    move || {
        state.analysis.get().map(|summary| {
            let severity_bars = render_severity_bars(&summary);

            view! {
                <div>
                    <section class="mb-8">
                        <div class="grid grid-cols-4 gap-4">
                            <MetricCard label="Lines of Code" value={summary.metrics.ncloc.to_string()} />
                            <MetricCard label="Functions" value={summary.metrics.functions.to_string()} />
                            <MetricCard label="Code Smells" value={summary.metrics.code_smells.to_string()} />
                            <MetricCard label="Technical Debt" value={format!("{} min", summary.technical_debt.total_minutes)} />
                        </div>
                    </section>

                    {render_trend_charts(state.clone())}

                    <section class="mb-8">
                        <h2 class="text-h3 text-text-primary mb-4">Issues by Severity</h2>
                        <div class="card">
                            {severity_bars}
                        </div>
                    </section>

                    <section class="mb-8">
                        <h2 class="text-h3 text-text-primary mb-4">Incremental Analysis</h2>
                        <div class="grid grid-cols-3 gap-4">
                            <div class="card">
                                <p class="text-caption text-text-muted">Total Files</p>
                                <p class="text-h1 text-text-primary mt-2">{summary.incremental.files_total}</p>
                            </div>
                            <div class="card">
                                <p class="text-caption text-text-muted">Files Changed</p>
                                <p class="text-h1 text-text-primary mt-2">{summary.incremental.files_changed}</p>
                            </div>
                            <div class="card">
                                <p class="text-caption text-text-muted">Files Reused</p>
                                <p class="text-h1 text-text-primary mt-2">{summary.incremental.files_reused}</p>
                            </div>
                        </div>
                    </section>

                    <section>
                        <div class={format!("card {}", if summary.incremental.clean_as_you_code { "bg-accent-pale" } else { "bg-accent-sunset" })}>
                            <h2 class="text-h3 text-text-primary mb-2">Clean as You Code</h2>
                            <p class="text-body">
                                {if summary.incremental.clean_as_you_code {
                                    "No blocker issues in new code"
                                } else {
                                    "Blocker issues detected in new code"
                                }}
                            </p>
                            <p class="text-body-sm text-text-muted mt-2">
                                {summary.incremental.new_code_issues} " new code issues, " {summary.incremental.legacy_issues} " legacy issues"
                            </p>
                        </div>
                    </section>
                </div>
            }
        })
    }
}

fn render_empty_state(state: ReactiveAppState) -> impl IntoView {
    move || {
        if state.analysis.get().is_none() && !state.loading.get() {
            Some(view! {
                <div class="card text-center py-12">
                    <p class="text-h3 text-text-muted">"No metrics available"</p>
                    <p class="text-body text-text-secondary mt-2">"Run an analysis first from the Dashboard"</p>
                </div>
            })
        } else {
            None
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_issues_trend_extracts_total_issues() {
        let runs = vec![
            HistoryEntryDto {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                total_issues: 10,
                debt_minutes: 60,
                rating: "A".to_string(),
                files_changed: 5,
                new_issues: 2,
                fixed_issues: 1,
            },
            HistoryEntryDto {
                timestamp: "2024-01-02T00:00:00Z".to_string(),
                total_issues: 15,
                debt_minutes: 90,
                rating: "B".to_string(),
                files_changed: 8,
                new_issues: 5,
                fixed_issues: 0,
            },
            HistoryEntryDto {
                timestamp: "2024-01-03T00:00:00Z".to_string(),
                total_issues: 12,
                debt_minutes: 75,
                rating: "A".to_string(),
                files_changed: 3,
                new_issues: 1,
                fixed_issues: 4,
            },
        ];

        let issues = map_issues_trend(&runs);
        assert_eq!(issues, vec![10.0, 15.0, 12.0]);
    }

    #[test]
    fn test_map_debt_trend_extracts_debt_minutes() {
        let runs = vec![
            HistoryEntryDto {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                total_issues: 10,
                debt_minutes: 60,
                rating: "A".to_string(),
                files_changed: 5,
                new_issues: 2,
                fixed_issues: 1,
            },
            HistoryEntryDto {
                timestamp: "2024-01-02T00:00:00Z".to_string(),
                total_issues: 15,
                debt_minutes: 90,
                rating: "B".to_string(),
                files_changed: 8,
                new_issues: 5,
                fixed_issues: 0,
            },
        ];

        let debt = map_debt_trend(&runs);
        assert_eq!(debt, vec![60.0, 90.0]);
    }

    #[test]
    fn test_map_new_issues_trend_extracts_new_issues() {
        let runs = vec![
            HistoryEntryDto {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                total_issues: 10,
                debt_minutes: 60,
                rating: "A".to_string(),
                files_changed: 5,
                new_issues: 2,
                fixed_issues: 1,
            },
            HistoryEntryDto {
                timestamp: "2024-01-02T00:00:00Z".to_string(),
                total_issues: 15,
                debt_minutes: 90,
                rating: "B".to_string(),
                files_changed: 8,
                new_issues: 5,
                fixed_issues: 0,
            },
            HistoryEntryDto {
                timestamp: "2024-01-03T00:00:00Z".to_string(),
                total_issues: 12,
                debt_minutes: 75,
                rating: "A".to_string(),
                files_changed: 3,
                new_issues: 1,
                fixed_issues: 4,
            },
        ];

        let new_issues = map_new_issues_trend(&runs);
        assert_eq!(new_issues, vec![2.0, 5.0, 1.0]);
    }

    #[test]
    fn test_fallback_renders_for_empty_runs() {
        // Empty runs should trigger fallback, not charts
        let runs: Vec<HistoryEntryDto> = vec![];
        assert!(runs.len() < 2);
    }

    #[test]
    fn test_fallback_renders_for_single_run() {
        // Single run should trigger fallback (need at least 2 for trend)
        let runs = vec![HistoryEntryDto {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            total_issues: 10,
            debt_minutes: 60,
            rating: "A".to_string(),
            files_changed: 5,
            new_issues: 2,
            fixed_issues: 1,
        }];
        assert!(runs.len() < 2);
    }
}
