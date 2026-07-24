//! Domain port for ADR (Architecture Decision Record) discovery.
//!
//! Separated from [`super::search_repository::SearchRepository`] per
//! ISP: "find me an ADR by identity / status / workspace" is a distinct
//! concern from full-text symbol search.
//!
//! ADRs are first-class knowledge objects that the user navigates from
//! the Spotter UI. This port enables the "adr" family in `SpotterResult`
//! without coupling to any concrete storage backend.
//!
//! # Implementation
//!
//! Production adapter reads from `docs/adr/*.md` files (frontmatter +
//! prose). In-memory adapter returns a small fixture for tests and
//! previews.

use crate::error::ExplorerResult;
use serde::{Deserialize, Serialize};

/// Status of an ADR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdrSummary {
    /// Stable id (e.g. `"ADR-001"` or filesystem-relative path slug).
    pub id: String,
    /// Human-readable title from frontmatter (e.g. "Knowledge layer ports").
    pub title: String,
    /// Current lifecycle status.
    pub status: AdrStatus,
    /// ISO-8601 date string from frontmatter (e.g. "2026-07-22").
    pub date: String,
    /// Optional list of topics covered by the ADR.
    #[serde(default)]
    pub topics: Vec<String>,
}

/// Read-only port for ADR discovery.
pub trait AdrRepository: Send + Sync {
    /// List all ADRs for a workspace, optionally filtered by status.
    ///
    /// Empty `workspace_id` returns the global ADR set.
    fn list_adrs(
        &self,
        workspace_id: &str,
        status: Option<AdrStatus>,
    ) -> ExplorerResult<Vec<AdrSummary>>;

    /// Full-text search across ADR titles and topics.
    fn search_adrs(
        &self,
        workspace_id: &str,
        query: &str,
        limit: usize,
    ) -> ExplorerResult<Vec<AdrSummary>>;
}

/// In-memory adapter backed by a Vec — useful for tests and previews.
#[derive(Debug, Default, Clone)]
pub struct InMemoryAdrRepository {
    adrs: Vec<AdrSummary>,
}

impl InMemoryAdrRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_adrs(mut self, adrs: Vec<AdrSummary>) -> Self {
        self.adrs = adrs;
        self
    }
}

impl AdrRepository for InMemoryAdrRepository {
    fn list_adrs(
        &self,
        _workspace_id: &str,
        status: Option<AdrStatus>,
    ) -> ExplorerResult<Vec<AdrSummary>> {
        Ok(self
            .adrs
            .iter()
            .filter(|a| status.map(|s| a.status == s).unwrap_or(true))
            .cloned()
            .collect())
    }

    fn search_adrs(
        &self,
        _workspace_id: &str,
        query: &str,
        limit: usize,
    ) -> ExplorerResult<Vec<AdrSummary>> {
        let q = query.to_lowercase();
        Ok(self
            .adrs
            .iter()
            .filter(|a| {
                a.title.to_lowercase().contains(&q)
                    || a.topics.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .take(limit)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> InMemoryAdrRepository {
        InMemoryAdrRepository::new().with_adrs(vec![
            AdrSummary {
                id: "ADR-001".into(),
                title: "Knowledge layer ports".into(),
                status: AdrStatus::Accepted,
                date: "2026-07-22".into(),
                topics: vec!["knowledge".into(), "ports".into()],
            },
            AdrSummary {
                id: "ADR-002".into(),
                title: "Diagram artifacts".into(),
                status: AdrStatus::Accepted,
                date: "2026-07-15".into(),
                topics: vec!["diagrams".into()],
            },
            AdrSummary {
                id: "ADR-003".into(),
                title: "Superseded design".into(),
                status: AdrStatus::Superseded,
                date: "2026-06-01".into(),
                topics: vec!["legacy".into()],
            },
        ])
    }

    #[test]
    fn list_adrs_returns_all_when_no_status_filter() {
        let r = fixture();
        let all = r.list_adrs("ws-1", None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn list_adrs_filters_by_status() {
        let r = fixture();
        let sup = r.list_adrs("ws-1", Some(AdrStatus::Superseded)).unwrap();
        assert_eq!(sup.len(), 1);
        assert_eq!(sup[0].id, "ADR-003");
    }

    #[test]
    fn search_adrs_matches_title_case_insensitively() {
        let r = fixture();
        let hits = r.search_adrs("ws-1", "KNOWLEDGE", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "ADR-001");
    }

    #[test]
    fn search_adrs_matches_topics() {
        let r = fixture();
        let hits = r.search_adrs("ws-1", "diagrams", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "ADR-002");
    }

    #[test]
    fn search_adrs_respects_limit() {
        let r = fixture();
        let hits = r.search_adrs("ws-1", "a", 1).unwrap();
        assert_eq!(hits.len(), 1);
    }
}