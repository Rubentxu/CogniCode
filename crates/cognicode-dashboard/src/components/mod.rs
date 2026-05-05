//! UI Components module
pub mod shell;
pub mod rating_card;
pub mod metric_card;
pub mod severity_badge;
pub mod gate_status_bar;
pub mod issue_table;
pub mod trend_chart;
pub mod issue_row;
pub mod filter_bar;
pub mod loading_spinner;

pub use shell::Shell;
pub use rating_card::RatingCard;
pub use metric_card::{MetricCard, Trend, TrendDirection};
pub use severity_badge::SeverityBadge;
pub use gate_status_bar::GateStatusBar;
pub use issue_table::IssueTable;
pub use trend_chart::TrendChart;
pub use issue_row::IssueRow;
pub use filter_bar::FilterBar;
pub use loading_spinner::LoadingSpinner;