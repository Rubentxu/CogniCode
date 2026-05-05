//! Not Found Page — 404

use leptos::prelude::*;
use crate::components::Shell;

/// Not found page component
#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <Shell>
            <div class="flex flex-col items-center justify-center min-h-[60vh] text-center">
                <h1 class="text-display font-bold text-text-muted">404</h1>
                <p class="text-h3 text-text-secondary mt-6">Page not found</p>
                <p class="text-body text-text-muted mt-2">"The page you're looking for doesn't exist."</p>
                <a href="/" class="btn btn-primary mt-6">Go to Dashboard</a>
            </div>
        </Shell>
    }
}
