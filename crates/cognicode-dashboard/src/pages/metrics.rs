//! Metrics page with trends and distributions

use leptos::prelude::*;
use crate::state::{Severity, Category};
use crate::components::{Shell, MetricCard, TrendChart, Trend};

fn mock_trend_issues() -> Vec<f64> {
    vec![47.0, 52.0, 45.0, 48.0, 41.0, 38.0, 35.0, 32.0, 28.0, 25.0, 22.0]
}

fn mock_trend_coverage() -> Vec<f64> {
    vec![65.0, 67.0, 68.5, 69.0, 70.2, 71.0, 71.8, 72.5, 73.0, 73.2, 74.0]
}

fn mock_trend_complexity() -> Vec<f64> {
    vec![12.0, 14.0, 13.5, 15.0, 14.2, 13.8, 12.5, 12.0, 11.5, 11.0, 10.5]
}

fn mock_category_distribution() -> Vec<(Category, f64)> {
    vec![
        (Category::Reliability, 35.0),
        (Category::Security, 15.0),
        (Category::Maintainability, 40.0),
        (Category::Coverage, 10.0),
    ]
}

fn mock_severity_distribution() -> Vec<(Severity, f64)> {
    vec![
        (Severity::Blocker, 2.0),
        (Severity::Critical, 5.0),
        (Severity::Major, 15.0),
        (Severity::Minor, 20.0),
        (Severity::Info, 8.0),
    ]
}

#[component]
pub fn MetricsPage() -> impl IntoView {
    let trend_issues = mock_trend_issues();
    let trend_coverage = mock_trend_coverage();
    let trend_complexity = mock_trend_complexity();

    let category_dist = mock_category_distribution();
    let severity_dist = mock_severity_distribution();

    let total_issues: f64 = severity_dist.iter().map(|(_, v)| v).sum();

    view! {
        <Shell>
            <div style="max-width: 1400px; margin: 0 auto;">
                <header style="margin-bottom: 48px;">
                    <h1 class="text-h1">Metrics</h1>
                    <p style="margin-top: 8px; color: var(--color-text-secondary);">
                        Code quality metrics and trends over time
                    </p>
                </header>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Overview</h2>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 24px;">
                        <MetricCard
                            label="Total Issues"
                            value="50".to_string()
                            trend={Some(Trend::down("23%"))}
                            icon={Some("M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2")}
                        />
                        <MetricCard
                            label="Code Coverage"
                            value="74.0%".to_string()
                            trend={Some(Trend::up("9%"))}
                            icon={Some("M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z")}
                        />
                        <MetricCard
                            label="Avg Complexity"
                            value="10.5".to_string()
                            trend={Some(Trend::down("12%"))}
                            icon={Some("M13 10V3L4 14h7v7l9-11h-7z")}
                        />
                        <MetricCard
                            label="Duplicates"
                            value="2.4%".to_string()
                            trend={Some(Trend::neutral("0%"))}
                            icon={Some("M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z")}
                        />
                    </div>
                </section>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Trends Over Time</h2>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(400px, 1fr)); gap: 24px;">
                        <div class="card">
                            <h3 class="text-h3" style="margin-bottom: 16px;">Issues Trend</h3>
                            <TrendChart
                                data={trend_issues.clone()}
                                width={350}
                                height={200}
                                color="#ef4444"
                            />
                            <p style="margin-top: 16px; font-size: 14px; color: var(--color-text-muted); text-align: center;">
                                47 to 10 issues (last 11 analyses)
                            </p>
                        </div>

                        <div class="card">
                            <h3 class="text-h3" style="margin-bottom: 16px;">Coverage Trend</h3>
                            <TrendChart
                                data={trend_coverage.clone()}
                                width={350}
                                height={200}
                                color="#22c55e"
                            />
                            <p style="margin-top: 16px; font-size: 14px; color: var(--color-text-muted); text-align: center;">
                                65% to 74% (last 11 analyses)
                            </p>
                        </div>

                        <div class="card">
                            <h3 class="text-h3" style="margin-bottom: 16px;">Complexity Trend</h3>
                            <TrendChart
                                data={trend_complexity.clone()}
                                width={350}
                                height={200}
                                color="#8b5cf6"
                            />
                            <p style="margin-top: 16px; font-size: 14px; color: var(--color-text-muted); text-align: center;">
                                12.0 to 10.5 (last 11 analyses)
                            </p>
                        </div>
                    </div>
                </section>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Distribution by Category</h2>
                    <div class="card">
                        <div style="display: flex; flex-direction: column; gap: 20px;">
                            {category_dist.iter().map(|(category, count)| {
                                let percentage = (*count / 100.0) * 100.0;
                                let color = match category {
                                    Category::Reliability => "#3b82f6",
                                    Category::Security => "#ef4444",
                                    Category::Maintainability => "#f59e0b",
                                    Category::Coverage => "#22c55e",
                                    Category::Duplicate => "#8b5cf6",
                                    Category::Complexity => "#06b6d4",
                                };
                                let cat_label = category.label();
                                view! {
                                    <div>
                                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                                            <span style="font-weight: 500;">{cat_label}</span>
                                            <span style="color: var(--color-text-muted);">{count.to_string()}%</span>
                                        </div>
                                        <div style="height: 12px; background: var(--color-surface-raised); border-radius: 6px; overflow: hidden;">
                                            <div style={format!("height: 100%; width: {}%; background: {}; border-radius: 6px; transition: width 0.3s ease;", percentage, color)}></div>
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                </section>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Distribution by Severity</h2>
                    <div class="card">
                        <div style="display: flex; flex-direction: column; gap: 20px;">
                            {severity_dist.iter().map(|(severity, count)| {
                                let percentage = (*count / total_issues) * 100.0;
                                let (color, _bg) = match severity {
                                    Severity::Blocker => ("#c62828", "rgba(198, 40, 40, 0.15)"),
                                    Severity::Critical => ("#e53935", "rgba(229, 57, 53, 0.15)"),
                                    Severity::Major => ("#fb8c00", "rgba(251, 140, 0, 0.15)"),
                                    Severity::Minor => ("#1e88e5", "rgba(30, 136, 229, 0.15)"),
                                    Severity::Info => ("#757575", "rgba(117, 117, 117, 0.15)"),
                                };
                                let sev_label = severity.label();
                                let count_str = count.to_string();
                                let pct_str = format!("{:.0}%", percentage);
                                view! {
                                    <div>
                                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                                            <span style="display: inline-flex; align-items: center; gap: 8px;">
                                                <span style={format!("display: inline-block; width: 12px; height: 12px; border-radius: 2px; background: {};", color)}></span>
                                                <span style="font-weight: 500;">{sev_label}</span>
                                            </span>
                                            <span style="color: var(--color-text-muted);">{count_str} issues ({pct_str})</span>
                                        </div>
                                        <div style="height: 8px; background: var(--color-surface-raised); border-radius: 4px; overflow: hidden;">
                                            <div style={format!("height: 100%; width: {}%; background: {}; border-radius: 4px; transition: width 0.3s ease;", percentage, color)}></div>
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                </section>
            </div>
        </Shell>
    }
}
