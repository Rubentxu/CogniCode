//! Pages module
pub mod dashboard;
pub mod issues;
pub mod metrics;
pub mod quality_gate;
pub mod configuration;
pub mod issue_detail;
pub mod not_found;
pub mod projects;

pub use dashboard::DashboardPage;
pub use issues::IssuesPage;
pub use metrics::MetricsPage;
pub use quality_gate::QualityGatePage;
pub use configuration::ConfigurationPage;
pub use issue_detail::IssueDetailPage;
pub use not_found::NotFoundPage;
pub use projects::ProjectsPage;