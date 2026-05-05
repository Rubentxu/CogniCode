//! Metrics page with trends and distributions

use leptos::prelude::*;
use crate::components::{Shell, MetricCard};

#[component]
pub fn MetricsPage() -> impl IntoView {
    view! {
        <Shell>
            <div style="max-width: 1400px; margin: 0 auto;">
                <header style="margin-bottom: 48px;">
                    <h1 class="text-h1">Metrics</h1>
                    <p style="margin-top: 8px; color: var(--color-text-secondary);">
                        Code quality metrics from the latest analysis
                    </p>
                </header>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Overview</h2>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 24px;">
                        <MetricCard
                            label="Total Issues"
                            value="50".to_string()
                            trend={None}
                            icon={None}
                        />
                        <MetricCard
                            label="Code Coverage"
                            value="74.0%".to_string()
                            trend={None}
                            icon={None}
                        />
                        <MetricCard
                            label="Functions"
                            value="123".to_string()
                            trend={None}
                            icon={None}
                        />
                        <MetricCard
                            label="Code Smells"
                            value="35".to_string()
                            trend={None}
                            icon={None}
                        />
                    </div>
                </section>

                <section style="margin-bottom: 48px;">
                    <div style="text-align: center; padding: 48px; background: var(--color-surface-raised); border-radius: var(--radius-lg);">
                        <p style="color: var(--color-text-secondary); margin-bottom: 16px;">
                            Run an analysis from the Dashboard to see detailed metrics.
                        </p>
                        <a href="/" class="btn btn-primary">Go to Dashboard</a>
                    </div>
                </section>
            </div>
        </Shell>
    }
}