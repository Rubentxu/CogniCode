//! Rating card component with SonarQube-style colors

use leptos::prelude::*;

#[component]
pub fn RatingCard(
    rating: char,
    label: &'static str,
) -> impl IntoView {
    let (bg_color, text_color) = match rating {
        'A' => ("#22c55e", "white"),
        'B' => ("#84cc16", "white"),
        'C' => ("#eab308", "white"),
        'D' => ("#f97316", "white"),
        'E' => ("#ef4444", "white"),
        _ => ("#f8f9fc", "#9ca3af"),
    };

    let desc = match rating {
        'A' => "Excellent",
        'B' => "Good",
        'C' => "Acceptable",
        'D' => "Warning",
        'E' => "Critical",
        _ => "Unknown",
    };

    view! {
        <div style="display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 32px; background: var(--color-surface-raised); border-radius: 24px; box-shadow: var(--shadow-card); border: 2px solid transparent; transition: transform 0.2s ease, box-shadow 0.2s ease;">
            <div style={format!("width: 80px; height: 80px; border-radius: 16px; display: flex; align-items: center; justify-content: center; font-size: 2.5rem; font-weight: 800; letter-spacing: -0.02em; background: {}; color: {};", bg_color, text_color)}>
                {rating}
            </div>
            <div style="text-align: center;">
                <p style="font-size: 16px; font-weight: 600; color: var(--color-text-primary); margin: 0;">{label}</p>
                <p style="font-size: 12px; color: var(--color-text-muted); margin: 4px 0 0 0; text-transform: uppercase; letter-spacing: 0.05em;">{desc}</p>
            </div>
        </div>
    }
}