//! Loading spinner component

use leptos::prelude::*;

#[component]
pub fn LoadingSpinner(message: &'static str) -> impl IntoView {
    view! {
        <div class="flex flex-col items-center justify-center gap-4 py-12">
            <div class="w-12 h-12 border-4 border-border border-t-brand rounded-full animate-spin"></div>
            <p class="text-body text-text-muted">{message}</p>
        </div>
    }
}
