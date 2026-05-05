//! Trend chart component

use leptos::prelude::*;

#[component]
pub fn TrendChart(data: Vec<f64>, width: u32, height: u32, color: &'static str) -> impl IntoView {
    let _ = (width, height, color);
    view! {
        <div>
            <p class="text-caption">Chart with {data.len()} data points</p>
        </div>
    }
}
