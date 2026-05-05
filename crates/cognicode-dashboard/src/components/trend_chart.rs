//! Trend chart component with real SVG path generation

use leptos::prelude::*;

#[component]
pub fn TrendChart(
    data: Vec<f64>,
    width: u32,
    height: u32,
    color: &'static str,
) -> impl IntoView {
    // Calculate SVG path from data
    let (line_path, area_path) = if data.len() >= 2 {
        let min_val = data.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = if (max_val - min_val).abs() < 0.001 { 1.0 } else { max_val - min_val };

        let points: Vec<String> = data.iter().enumerate().map(|(i, v)| {
            let x = (i as f64) / ((data.len() - 1) as f64) * (width as f64);
            let y = height as f64 - ((v - min_val) / range * (height as f64 * 0.8) + (height as f64 * 0.1));
            format!("{:.1},{:.1}", x, y)
        }).collect();

        let lp = format!("M {}", points.join(" L "));

        let first_x = 0.0;
        let last_x = width as f64;
        let bottom_y = height as f64;

        let ap = format!("M {:.1},{:.1} L {} L {:.1},{:.1} Z", first_x, bottom_y, points.join(" L "), last_x, bottom_y);

        (lp, ap)
    } else {
        (String::new(), String::new())
    };

    // Always render with empty or actual values
    view! {
        <div style={format!("position: relative; width: {}px; height: {}px;", width, height)}>
            <svg
                width={width}
                height={height}
                viewBox={format!("0 0 {} {}", width, height)}
                style="overflow: visible;"
            >
                <defs>
                    <linearGradient id="gradientFill" x1="0%" y1="0%" x2="0%" y2="100%">
                        <stop offset="0%" style={format!("stop-color: {}; stop-opacity: 0.3;", color)}/>
                        <stop offset="100%" style={format!("stop-color: {}; stop-opacity: 0.0;", color)}/>
                    </linearGradient>
                </defs>

                <path d={area_path} fill="url(#gradientFill)" stroke="none"/>
                <path d={line_path} fill="none" stroke={color} stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>

            {/* Data point count label */}
            <div style="position: absolute; bottom: -24px; left: 0; right: 0; text-align: center; font-size: 12px; color: var(--color-text-muted);">
                {data.len()} data points
            </div>
        </div>
    }
}
