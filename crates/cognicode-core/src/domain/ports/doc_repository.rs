//! Documentation repository port.
//!
//! Provides read-only access to documentation metadata for Spotter UI,
//! inspector panels, and knowledge rail listings.

/// One documentation summary, sufficient for Spotter hits and inspector panes.
#[derive(Debug, Clone)]
pub struct DocSummary {
    /// Stable id (e.g. `"doc:guide.md"`).
    pub id: String,
    /// Human-readable title from frontmatter or filename.
    pub title: String,
    /// Optional document section (e.g. `"Introduction"`, `"Architecture"`).
    pub section: String,
    /// File path or external URL where the doc lives.
    pub source_path: String,
    /// One-line summary from frontmatter.
    pub excerpt: String,
}

/// Errors returned by [`DocRepository`] methods.
#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("Document not found: {0}")]
    NotFound(String),
    #[error("Store error: {0}")]
    Store(String),
}

/// Read-only port for documentation discovery.
pub trait DocRepository: Send + Sync {
    /// List docs for a workspace, optionally filtered by section.
    fn list_docs(&self, workspace: &str, section: Option<&str>) -> Result<Vec<DocSummary>, DocError>;

    /// Full-text search across doc titles, sections, and excerpts.
    fn search_docs(&self, workspace: &str, query: &str, limit: usize) -> Result<Vec<DocSummary>, DocError>;
}
