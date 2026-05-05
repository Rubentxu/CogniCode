//! Rating card component

use leptos::prelude::*;

#[component]
pub fn RatingCard(
    rating: char,
    label: &'static str,
) -> impl IntoView {
    let bg_class = match rating {
        'A' => "bg-accent-pale",
        'B' => "bg-accent-ocean",
        'C' => "bg-accent-sky",
        'D' | 'E' => "bg-accent-sunset",
        _ => "bg-surface",
    };

    view! {
        <div class={format!("rounded-xl p-6 text-center {}", bg_class)}>
            <span class="text-display font-bold">{rating}</span>
            <p class="text-caption text-text-muted mt-2">{label}</p>
        </div>
    }
}
