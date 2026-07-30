//! Domain port for documentation discovery.
//!
//! Separated from [`super::search_repository::FuzzySymbolSearch`] per
//! ISP: documentation has its own identity, retrieval, and discovery
//! model that doesn't fit symbol-level search.
//!
//! Docs in CogniCode represent markdown files (ADRs, design docs,
//! onboarding guides, etc.) tracked by the graph as `NodeKind::Doc`.
//! They become first-class knowledge objects through this port.
//!
//! # Implementation
//!
//! Production adapter reads from the graph's Doc nodes (PostgreSQL).
//! In-memory adapter returns a small fixture for tests and previews.

use crate::error::ExplorerResult;
use serde::{Deserialize, Serialize};

/// One documentation summary, sufficient for Spotter hits, inspector
/// panes, and knowledge rail listings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocSummary {
    /// Stable id (e.g. `"doc:guide.md"`).
    pub id: String,
    /// Human-readable title from frontmatter or filename.
    pub title: String,
    /// Optional document section (e.g. "Introduction", "Architecture").
    #[serde(default)]
    pub section: Option<String>,
    /// File path or external URL where the doc lives.
    pub source_path: String,
    /// Optional one-line summary from frontmatter.
    #[serde(default)]
    pub excerpt: Option<String>,
}

/// Read-only port for documentation discovery.
pub trait DocRepository: Send + Sync {
    /// List docs for a workspace, optionally filtered by section.
    ///
    /// Empty `workspace_id` returns the global doc set.
    fn list_docs(
        &self,
        workspace_id: &str,
        section: Option<&str>,
    ) -> ExplorerResult<Vec<DocSummary>>;

    /// Full-text search across doc titles, sections, and excerpts.
    fn search_docs(
        &self,
        workspace_id: &str,
        query: &str,
        limit: usize,
    ) -> ExplorerResult<Vec<DocSummary>>;
}

/// In-memory adapter backed by a Vec — useful for tests and previews.
#[derive(Debug, Default, Clone)]
pub struct InMemoryDocRepository {
    docs: Vec<DocSummary>,
}

impl InMemoryDocRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_docs(mut self, docs: Vec<DocSummary>) -> Self {
        self.docs = docs;
        self
    }
}

impl DocRepository for InMemoryDocRepository {
    fn list_docs(
        &self,
        _workspace_id: &str,
        section: Option<&str>,
    ) -> ExplorerResult<Vec<DocSummary>> {
        Ok(self
            .docs
            .iter()
            .filter(|d| {
                section
                    .map(|s| d.section.as_deref() == Some(s))
                    .unwrap_or(true)
            })
            .cloned()
            .collect())
    }

    fn search_docs(
        &self,
        _workspace_id: &str,
        query: &str,
        limit: usize,
    ) -> ExplorerResult<Vec<DocSummary>> {
        let q = query.to_lowercase();
        Ok(self
            .docs
            .iter()
            .filter(|d| {
                d.title.to_lowercase().contains(&q)
                    || d.section
                        .as_deref()
                        .map(|s| s.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || d.excerpt
                        .as_deref()
                        .map(|e| e.to_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .take(limit)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> InMemoryDocRepository {
        InMemoryDocRepository::new().with_docs(vec![
            DocSummary {
                id: "doc:guide.md".into(),
                title: "Getting Started Guide".into(),
                section: Some("Introduction".into()),
                source_path: "/docs/guide.md".into(),
                excerpt: Some("A gentle introduction to CogniCode Explorer.".into()),
            },
            DocSummary {
                id: "doc:architecture.md".into(),
                title: "Architecture Overview".into(),
                section: Some("Architecture".into()),
                source_path: "/docs/architecture.md".into(),
                excerpt: Some("How the workbench shell is structured.".into()),
            },
            DocSummary {
                id: "doc:adr-001.md".into(),
                title: "ADR-001: Knowledge layer ports".into(),
                section: None,
                source_path: "/docs/adr/ADR-001.md".into(),
                excerpt: Some("Ports for docs, ADRs, and evidence.".into()),
            },
        ])
    }

    #[test]
    fn list_docs_returns_all_when_no_section_filter() {
        let r = fixture();
        let all = r.list_docs("ws-1", None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn list_docs_filters_by_section() {
        let r = fixture();
        let arch = r.list_docs("ws-1", Some("Architecture")).unwrap();
        assert_eq!(arch.len(), 1);
        assert_eq!(arch[0].id, "doc:architecture.md");
    }

    #[test]
    fn search_docs_matches_title_case_insensitively() {
        let r = fixture();
        let hits = r.search_docs("ws-1", "ARCHITECTURE", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "doc:architecture.md");
    }

    #[test]
    fn search_docs_matches_excerpt() {
        let r = fixture();
        let hits = r.search_docs("ws-1", "introduction", 10).unwrap();
        // "Introduction" matches both section and excerpt
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "doc:guide.md");
    }

    #[test]
    fn search_docs_respects_limit() {
        let r = fixture();
        let hits = r.search_docs("ws-1", "doc", 1).unwrap();
        assert_eq!(hits.len(), 1);
    }
}
