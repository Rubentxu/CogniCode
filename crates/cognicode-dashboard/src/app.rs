//! Application Router
//!
//! Sets up the Leptos router with all pages.

use leptos::prelude::*;
use crate::pages::*;
use crate::state::AppState;

/// Not found page component
#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 60vh; text-align: center;">
            <h1 style="font-size: 96px; font-weight: 700; color: var(--color-text-muted); margin: 0;">404</h1>
            <p style="font-size: 24px; color: var(--color-text-secondary); margin: 24px 0;">Page not found</p>
            <a href="/" class="btn btn-primary">Go to Dashboard</a>
        </div>
    }
}

/// Main application component - renders dashboard for now
#[component]
pub fn App() -> impl IntoView {
    // Provide global app state
    let app_state = AppState::new();
    provide_context(app_state);

    // For CSR, render the dashboard directly
    // Full routing would be added with proper router setup
    view! {
        <DashboardPage />
    }
}