//! Application Router
//!
//! Sets up the Leptos router with all pages.

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use crate::state::ReactiveAppState;
use crate::pages::{
    DashboardPage, IssuesPage, IssueDetailPage,
    MetricsPage, QualityGatePage, ConfigurationPage,
    NotFoundPage, ProjectsPage,
};

/// Main application component with full routing
#[component]
pub fn App() -> impl IntoView {
    // Provide global app state
    let app_state = ReactiveAppState::new();
    provide_context(app_state);

    view! {
        <Router>
            <Routes fallback=NotFoundPage>
                <Route path=path!("/") view=DashboardPage />
                <Route path=path!("/projects") view=ProjectsPage />
                <Route path=path!("/issues") view=IssuesPage />
                <Route path=path!("/issues/:id") view=IssueDetailPage />
                <Route path=path!("/metrics") view=MetricsPage />
                <Route path=path!("/quality-gate") view=QualityGatePage />
                <Route path=path!("/configuration") view=ConfigurationPage />
                <Route path=path!("/*any") view=NotFoundPage />
            </Routes>
        </Router>
    }
}
