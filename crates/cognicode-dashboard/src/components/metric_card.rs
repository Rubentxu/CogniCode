//! Metric card component with icon and trend indicator

use leptos::prelude::*;

#[derive(Clone, Debug)]
pub enum TrendDirection {
    Up,
    Down,
    Neutral,
}

#[derive(Clone, Debug)]
pub struct Trend {
    pub direction: TrendDirection,
    pub value: String,
}

impl Trend {
    pub fn up(value: &str) -> Self {
        Self { direction: TrendDirection::Up, value: value.to_string() }
    }
    pub fn down(value: &str) -> Self {
        Self { direction: TrendDirection::Down, value: value.to_string() }
    }
    pub fn neutral(value: &str) -> Self {
        Self { direction: TrendDirection::Neutral, value: value.to_string() }
    }
}

#[component]
pub fn MetricCard(
    label: &'static str,
    value: String,
    trend: Option<Trend>,
    icon: Option<&'static str>,
) -> impl IntoView {
    let trend_html = trend.map(|t| {
        let (trend_class, arrow) = match t.direction {
            TrendDirection::Up => ("trend-up", "↑"),
            TrendDirection::Down => ("trend-down", "↓"),
            TrendDirection::Neutral => ("trend-neutral", "→"),
        };
        view! {
            <span class={format!("trend-indicator {}", trend_class)}>
                {format!("{} {}", arrow, t.value)}
            </span>
        }
    });

    let icon_html = icon.map(|ico| {
        view! {
            <div style="width: 48px; height: 48px; padding: 12px; background: var(--color-accent-violet); border-radius: 8px; color: var(--color-brand); flex-shrink: 0;">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width: 100%; height: 100%;">
                    <path stroke-linecap="round" stroke-linejoin="round" d={ico}/>
                </svg>
            </div>
        }
    });

    view! {
        <div style="display: flex; align-items: flex-start; gap: 24px; padding: 32px; background: var(--color-surface-raised); border-radius: 24px; box-shadow: var(--shadow-card); transition: transform 0.2s ease, box-shadow 0.2s ease;">
            {icon_html}
            <div style="flex: 1; min-width: 0;">
                <p style="font-size: 12px; font-weight: 500; color: var(--color-text-muted); margin: 0 0 4px 0; text-transform: uppercase; letter-spacing: 0.05em;">{label}</p>
                <div style="display: flex; align-items: baseline; gap: 16px;">
                    <p style="font-size: 24px; font-weight: 700; color: var(--color-text-primary); margin: 0;">{value}</p>
                    {trend_html}
                </div>
            </div>
        </div>
    }
}