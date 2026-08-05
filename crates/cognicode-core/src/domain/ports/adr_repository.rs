//! ADR (Architecture Decision Record) repository port.
//!
//! Provides read-only access to ADR metadata for Spotter UI, inspector
//! panels, and knowledge rail listings.

/// Status of an ADR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdrStatus {
    /// Proposed but not yet decided.
    Proposed,
    /// Accepted and currently in force.
    Accepted,
    /// Superseded by a newer ADR.
    Superseded,
    /// Explicitly rejected.
    Rejected,
}

/// One Architecture Decision Record summary, sufficient for Spotter hits
/// and inspector panels.
#[derive(Debug, Clone)]
pub struct AdrSummary {
    /// Stable id (e.g. `"ADR-001"`).
    pub id: String,
    /// Human-readable title from frontmatter.
    pub title: String,
    /// Current lifecycle status.
    pub status: AdrStatus,
    /// ISO-8601 date string (e.g. `"2026-07-22"`).
    pub date: String,
    /// Optional list of topics covered by the ADR.
    pub topics: Vec<String>,
}

/// Errors returned by [`AdrRepository`] methods.
#[derive(Debug, thiserror::Error)]
pub enum AdrError {
    #[error("ADR not found: {0}")]
    NotFound(String),
    #[error("Store error: {0}")]
    Store(String),
}

/// Read-only port for ADR discovery.
pub trait AdrRepository: Send + Sync {
    /// List all ADRs for a workspace, optionally filtered by status.
    ///
    /// Empty `workspace` returns the global ADR set. `None` status
    /// returns all ADRs regardless of lifecycle status.
    fn list_adrs(
        &self,
        workspace: &str,
        status: Option<AdrStatus>,
    ) -> Result<Vec<AdrSummary>, AdrError>;

    /// Full-text search across ADR titles and topics.
    fn search_adrs(
        &self,
        workspace: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<AdrSummary>, AdrError>;

    /// Get a single ADR by id, returning its raw markdown content.
    fn get_adr(&self, id: &str) -> Result<String, AdrError>;
}
